//! Firmware download integration: drives the 32-bit `r26-cli` sidecar.
//!
//! FactoryTool stays 64-bit and never links the Unisoc DLLs. It spawns the
//! self-contained 32-bit `r26-cli.exe` sidecar, decoding its line-delimited
//! JSON into Tauri `firmware-event`s for the UI.

use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_dialog::DialogExt;

// ── DTOs mirroring r26-core's serialized safety types ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRiskDto {
    pub file_id: String,
    pub file_type: String,
    /// Serialized RiskLevel variant name, e.g. "NvWrite", "Erase", "RfCalibration".
    pub risk_level: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyReportDto {
    pub total_files: usize,
    pub safe_files: Vec<String>,
    pub nv_files: Vec<FileRiskDto>,
    pub rf_cali_files: Vec<FileRiskDto>,
    pub erase_files: Vec<FileRiskDto>,
    pub phasecheck_files: Vec<FileRiskDto>,
    pub has_risks: bool,
    pub touches_rf_calibration: bool,
    pub summary: String,
}

/// The decision for how to flash a given PAC.
#[derive(Debug, Clone, PartialEq)]
pub struct FlashPlan {
    /// Some(reason) => do NOT flash; show this message. None => proceed.
    pub blocked_reason: Option<String>,
    /// `--allow-*` flags to pass to `r26-cli download`.
    pub allow_flags: Vec<&'static str>,
}

/// Factory-safe flashing policy.
///
/// erase + NV writes are auto-allowed (a full firmware flash needs them, and
/// DLFrame.dll preserves the device's real calibration via Research-mode NV
/// backup). RF calibration and PhaseCheck are NEVER allowed — such PACs are
/// blocked outright so the sidecar is never even spawned for them.
pub fn plan_flash(report: &SafetyReportDto) -> FlashPlan {
    if report.touches_rf_calibration || !report.rf_cali_files.is_empty() {
        return FlashPlan {
            blocked_reason: Some("此 PAC 含射频校准分区，出于保护已禁止刷写".to_string()),
            allow_flags: vec![],
        };
    }
    if !report.phasecheck_files.is_empty() {
        return FlashPlan {
            blocked_reason: Some("此 PAC 含 PhaseCheck 生产数据分区，已禁止刷写".to_string()),
            allow_flags: vec![],
        };
    }
    let mut allow_flags = Vec::new();
    if !report.erase_files.is_empty() {
        allow_flags.push("--allow-erase");
    }
    if !report.nv_files.is_empty() {
        allow_flags.push("--allow-nv-write");
    }
    FlashPlan { blocked_reason: None, allow_flags }
}

/// Run `r26-cli pac-info <path> --json` and parse the single JSON report line.
async fn run_pac_info(app: &AppHandle, path: &str) -> Result<SafetyReportDto, String> {
    let output = app
        .shell()
        .sidecar("r26-cli")
        .map_err(|e| format!("无法定位刷机组件: {e}"))?
        .args(["pac-info", path, "--json"])
        .output()
        .await
        .map_err(|e| format!("启动刷机组件失败: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "PAC 分析失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or("刷机组件未返回安全报告")?;
    serde_json::from_str::<SafetyReportDto>(line.trim())
        .map_err(|e| format!("解析安全报告失败: {e}"))
}

/// Open a native file picker for a `.pac` file. Returns the chosen path, or
/// None if the user cancelled. Driven from Rust so the frontend needs no
/// dialog-plugin JS.
#[tauri::command]
pub async fn pick_pac_file(app: AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("PAC 固件包", &["pac"])
        .pick_file(move |f| {
            let _ = tx.send(f);
        });
    let picked = rx.await.map_err(|e| e.to_string())?;
    Ok(picked.map(|p| p.to_string()))
}

/// Analyze a PAC file and return its safety report to the frontend.
#[tauri::command]
pub async fn pac_info(app: AppHandle, path: String) -> Result<SafetyReportDto, String> {
    run_pac_info(&app, &path).await
}

/// Holds the currently-running download sidecar so `stop` can kill it.
#[derive(Default)]
pub struct DloaderState {
    pub child: Arc<Mutex<Option<CommandChild>>>,
}

/// Analyze the PAC, enforce the factory-safe policy, then spawn the sidecar
/// download and forward its JSON events to the frontend as `firmware-event`.
#[tauri::command]
pub async fn start_firmware_download(
    app: AppHandle,
    path: String,
) -> Result<(), String> {
    // One download at a time. The webview disables the button during a run,
    // but the IPC endpoint must reject re-entry itself, or a second call would
    // orphan the first sidecar (its process keeps running with no Terminated).
    if app.state::<DloaderState>().child.lock().unwrap().is_some() {
        return Err("下载正在进行中".to_string());
    }

    // 1. Analyze + policy gate (RF / PhaseCheck PACs never reach the sidecar).
    let report = run_pac_info(&app, &path).await?;
    let plan = plan_flash(&report);
    if let Some(reason) = plan.blocked_reason {
        return Err(reason);
    }

    // 2. Build args: download <path> --port 0 --json [--allow-*]
    let mut args: Vec<String> = vec![
        "download".into(), path, "--port".into(), "0".into(), "--json".into(),
    ];
    for f in &plan.allow_flags {
        args.push((*f).to_string());
    }

    // 3. Spawn the sidecar (Rust-side => no webview permission needed; the
    //    shell plugin runs it without a console window on Windows).
    let (mut rx, child) = app
        .shell()
        .sidecar("r26-cli")
        .map_err(|e| format!("无法定位刷机组件: {e}"))?
        .args(args)
        .spawn()
        .map_err(|e| format!("启动刷机失败: {e}"))?;

    let state = app.state::<DloaderState>();
    *state.child.lock().unwrap() = Some(child);
    let child_slot = state.child.clone();

    // 4. Forward stdout JSON lines verbatim as `firmware-event`.
    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<serde_json::Value>(trimmed) {
                        Ok(v) => {
                            let _ = app_for_task.emit("firmware-event", v);
                        }
                        Err(_) => {
                            let _ = app_for_task.emit(
                                "firmware-event",
                                serde_json::json!({ "Log": { "level": "info", "message": trimmed } }),
                            );
                        }
                    }
                }
                CommandEvent::Stderr(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let _ = app_for_task.emit(
                            "firmware-event",
                            serde_json::json!({ "Log": { "level": "stderr", "message": trimmed } }),
                        );
                    }
                }
                CommandEvent::Error(err) => {
                    let _ = app_for_task.emit(
                        "firmware-event",
                        serde_json::json!({ "Error": { "code": 0, "message": err } }),
                    );
                }
                CommandEvent::Terminated(payload) => {
                    *child_slot.lock().unwrap() = None;
                    let _ = app_for_task.emit(
                        "firmware-event",
                        serde_json::json!({ "Terminated": { "code": payload.code } }),
                    );
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(())
}

/// Kill the running download sidecar, if any.
#[tauri::command]
pub fn stop_firmware_download(state: State<'_, DloaderState>) -> Result<(), String> {
    if let Some(child) = state.child.lock().unwrap().take() {
        child.kill().map_err(|e| format!("停止失败: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn risk(id: &str) -> FileRiskDto {
        FileRiskDto { file_id: id.into(), file_type: "".into(), risk_level: "".into(), reason: "".into() }
    }
    fn report() -> SafetyReportDto {
        SafetyReportDto {
            total_files: 0, safe_files: vec![], nv_files: vec![], rf_cali_files: vec![],
            erase_files: vec![], phasecheck_files: vec![], has_risks: false,
            touches_rf_calibration: false, summary: String::new(),
        }
    }

    #[test]
    fn safe_pac_proceeds_with_no_flags() {
        let plan = plan_flash(&report());
        assert_eq!(plan.blocked_reason, None);
        assert!(plan.allow_flags.is_empty());
    }

    #[test]
    fn erase_and_nv_auto_allowed() {
        let mut r = report();
        r.erase_files = vec![risk("EraseFlash")];
        r.nv_files = vec![risk("NV_NORFLASH")];
        let plan = plan_flash(&r);
        assert_eq!(plan.blocked_reason, None);
        assert_eq!(plan.allow_flags, vec!["--allow-erase", "--allow-nv-write"]);
    }

    #[test]
    fn rf_calibration_is_blocked() {
        let mut r = report();
        r.touches_rf_calibration = true;
        r.rf_cali_files = vec![risk("LTE_CALI")];
        let plan = plan_flash(&r);
        assert!(plan.blocked_reason.is_some());
        assert!(plan.allow_flags.is_empty());
    }

    #[test]
    fn phasecheck_is_blocked() {
        let mut r = report();
        r.phasecheck_files = vec![risk("PhaseCheck")];
        let plan = plan_flash(&r);
        assert!(plan.blocked_reason.is_some());
    }
}
