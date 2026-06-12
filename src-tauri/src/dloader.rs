//! Firmware download integration: drives the 32-bit `r26-cli` sidecar.
//!
//! FactoryTool stays 64-bit and never links the Unisoc DLLs. It spawns the
//! self-contained 32-bit `r26-cli.exe` sidecar, decoding its JSON event
//! output into Tauri `firmware-event`s for the UI.

use std::sync::{Arc, Mutex, OnceLock};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_dialog::DialogExt;

// ── Sidecar version gate ─────────────────────────────────────────────────────

/// Version the vendored sidecar must report via `r26-cli --version`.
/// Update together with binaries/r26-cli.version.txt when re-vendoring
/// (scripts/update-dloader-cli.ps1 writes the stamp file).
const SIDECAR_EXPECTED_VERSION: &str = "0.1.0";

/// Cached once per process — the sidecar binary can't change while we run.
static SIDECAR_VERSION_CHECK: OnceLock<Result<(), String>> = OnceLock::new();

/// Verify the vendored sidecar reports the expected version. Turns a silently
/// incompatible binary swap into an explicit error instead of mysterious
/// parse failures mid-flash. Runs the sidecar at most once per process.
async fn ensure_sidecar_version(app: &AppHandle) -> Result<(), String> {
    if let Some(cached) = SIDECAR_VERSION_CHECK.get() {
        return cached.clone();
    }
    let result = async {
        let output = app
            .shell()
            .sidecar("r26-cli")
            .map_err(|e| format!("无法定位刷机组件: {e}"))?
            .args(["--version"])
            .output()
            .await
            .map_err(|e| format!("启动刷机组件失败: {e}"))?;
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if version.ends_with(SIDECAR_EXPECTED_VERSION) {
            Ok(())
        } else {
            Err(format!(
                "刷机组件版本不匹配：期望 {SIDECAR_EXPECTED_VERSION}，实际 \"{version}\"。\
                 请运行 scripts/update-dloader-cli.ps1 重新同步"
            ))
        }
    }
    .await;
    // Concurrent first calls may each probe once; set() losing the race is
    // harmless because both computed the same answer.
    let _ = SIDECAR_VERSION_CHECK.set(result.clone());
    result
}

// ── DTOs mirroring r26-core's serialized safety types ───────────────────────

/// Risk level of one PAC file entry — local mirror of r26-core's `RiskLevel`.
/// Custom serde keeps wire compatibility (plain variant-name strings) while
/// preserving unrecognized values as `Unknown` instead of silently
/// misclassifying them, so contract drift stays visible end to end.
#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Safe,
    NvWrite,
    RfCalibration,
    Erase,
    EraseAll,
    PhaseCheck,
    /// A variant this build doesn't know (raw string preserved round-trip).
    Unknown(String),
}

impl RiskLevel {
    fn as_str(&self) -> &str {
        match self {
            Self::Safe => "Safe",
            Self::NvWrite => "NvWrite",
            Self::RfCalibration => "RfCalibration",
            Self::Erase => "Erase",
            Self::EraseAll => "EraseAll",
            Self::PhaseCheck => "PhaseCheck",
            Self::Unknown(s) => s,
        }
    }
}

impl Serialize for RiskLevel {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RiskLevel {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(match s.as_str() {
            "Safe" => Self::Safe,
            "NvWrite" => Self::NvWrite,
            "RfCalibration" => Self::RfCalibration,
            "Erase" => Self::Erase,
            "EraseAll" => Self::EraseAll,
            "PhaseCheck" => Self::PhaseCheck,
            _ => Self::Unknown(s),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRiskDto {
    pub file_id: String,
    pub file_type: String,
    pub risk_level: RiskLevel,
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

/// The decision for how to flash a given PAC. As an enum, "blocked plans
/// carry no allow flags" is unrepresentable as invalid — this guards a
/// hardware-flash safety gate, so the invariant lives in the type.
#[derive(Debug, Clone, PartialEq)]
pub enum FlashDecision {
    /// Do NOT flash; show this reason to the operator.
    Blocked { reason: String },
    /// Proceed, passing these `--allow-*` flags to `r26-cli download`.
    Proceed { allow_flags: Vec<&'static str> },
}

/// Factory-safe flashing policy.
///
/// erase + NV writes are auto-allowed (a full firmware flash needs them, and
/// DLFrame.dll preserves the device's real calibration via Research-mode NV
/// backup). RF calibration and PhaseCheck are NEVER allowed — such PACs are
/// blocked outright so the sidecar is never even spawned for them.
pub fn plan_flash(report: &SafetyReportDto) -> FlashDecision {
    // `touches_rf_calibration` and `rf_cali_files` both come from r26-cli and
    // are checked disjunctively on purpose: if either signal fires, block.
    if report.touches_rf_calibration || !report.rf_cali_files.is_empty() {
        return FlashDecision::Blocked {
            reason: "此 PAC 含射频校准分区，出于保护已禁止刷写".to_string(),
        };
    }
    if !report.phasecheck_files.is_empty() {
        return FlashDecision::Blocked {
            reason: "此 PAC 含 PhaseCheck 生产数据分区，已禁止刷写".to_string(),
        };
    }
    let mut allow_flags = Vec::new();
    if !report.erase_files.is_empty() {
        allow_flags.push("--allow-erase");
    }
    if !report.nv_files.is_empty() {
        allow_flags.push("--allow-nv-write");
    }
    FlashDecision::Proceed { allow_flags }
}

// ── Cross-process event contract (typed) ────────────────────────────────────

/// Local mirror of r26-cli's `EngineEvent` (externally-tagged serde JSON),
/// plus the locally synthesized `Terminated` variant. Typed deserialization
/// at the forwarding boundary means an upstream field rename surfaces here
/// as a visible "contract drift" log instead of the frontend silently
/// reading `undefined`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FirmwareEvent {
    Log { level: String, message: String },
    Progress { port: u32, percent: f32, file_id: String },
    /// `EngineState` is all unit variants, so serde carries them as strings.
    StateChange { from: String, to: String },
    Error { code: u32, message: String },
    Completed { port: u32, result: DownloadResultDto },
    SafetyReportReady { report: serde_json::Value },
    SafetyViolation { violation: serde_json::Value },
    SafetyConfirmed { category: String },
    PacLoadProgress { percent: u32 },
    /// Synthesized locally: the sidecar process exited.
    Terminated { code: Option<i32> },
}

/// Mirror of r26-core's `DownloadResult`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResultDto {
    pub port: u32,
    pub success: bool,
    pub chip_uid: Option<String>,
    pub flash_uid: Option<String>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Run `r26-cli pac-info <path> --json` and parse the last JSON object line
/// from stdout (tolerates any preamble lines before the report).
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
    ensure_sidecar_version(&app).await?;
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
    ensure_sidecar_version(&app).await?;

    // 1. Re-analyze + policy gate. Deliberately does NOT trust the frontend's
    //    cached report: re-running pac-info here closes the TOCTOU window
    //    where the file could be swapped after the operator confirmed it.
    //    RF / PhaseCheck PACs never reach the spawn below.
    let report = run_pac_info(&app, &path).await?;
    let allow_flags = match plan_flash(&report) {
        FlashDecision::Blocked { reason } => return Err(reason),
        FlashDecision::Proceed { allow_flags } => allow_flags,
    };

    // 2. Build args: download <path> --port 0 --json [--allow-*]
    let mut args: Vec<String> = vec![
        "download".into(), path, "--port".into(), "0".into(), "--json".into(),
    ];
    for f in &allow_flags {
        args.push((*f).to_string());
    }

    // 3. Spawn the sidecar and register the child atomically: hold the state
    //    lock across the re-entry check, the spawn, and the store so two
    //    concurrent invocations can't both start a download. spawn() is
    //    synchronous, so no .await happens while the std Mutex is held.
    let state = app.state::<DloaderState>();
    let child_slot = state.child.clone();
    let mut rx = {
        let mut guard = state.child.lock().unwrap();
        if guard.is_some() {
            return Err("下载正在进行中".to_string());
        }
        let (rx, child) = app
            .shell()
            .sidecar("r26-cli")
            .map_err(|e| format!("无法定位刷机组件: {e}"))?
            .args(args)
            .spawn()
            .map_err(|e| format!("启动刷机失败: {e}"))?;
        *guard = Some(child);
        rx
    };

    // 4. Forward stdout lines as typed `firmware-event`s.
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
                    let fw_event = match serde_json::from_str::<FirmwareEvent>(trimmed) {
                        Ok(ev) => ev,
                        // A JSON object that doesn't match our contract is
                        // almost certainly upstream drift — surface it loudly.
                        Err(e) if trimmed.starts_with('{') => FirmwareEvent::Log {
                            level: "warn".to_string(),
                            message: format!("无法识别的事件（契约漂移？{e}）: {trimmed}"),
                        },
                        // Plain text line → informational log.
                        Err(_) => FirmwareEvent::Log {
                            level: "info".to_string(),
                            message: trimmed.to_string(),
                        },
                    };
                    let _ = app_for_task.emit("firmware-event", fw_event);
                }
                CommandEvent::Stderr(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let _ = app_for_task.emit(
                            "firmware-event",
                            FirmwareEvent::Log {
                                level: "stderr".to_string(),
                                message: trimmed.to_string(),
                            },
                        );
                    }
                }
                CommandEvent::Error(err) => {
                    let _ = app_for_task.emit(
                        "firmware-event",
                        FirmwareEvent::Error { code: 0, message: err },
                    );
                }
                CommandEvent::Terminated(payload) => {
                    *child_slot.lock().unwrap() = None;
                    let _ = app_for_task.emit(
                        "firmware-event",
                        FirmwareEvent::Terminated { code: payload.code },
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
        FileRiskDto {
            file_id: id.into(),
            file_type: "".into(),
            risk_level: RiskLevel::Safe,
            reason: "".into(),
        }
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
        assert_eq!(
            plan_flash(&report()),
            FlashDecision::Proceed { allow_flags: vec![] }
        );
    }

    #[test]
    fn erase_and_nv_auto_allowed() {
        let mut r = report();
        r.erase_files = vec![risk("EraseFlash")];
        r.nv_files = vec![risk("NV_NORFLASH")];
        assert_eq!(
            plan_flash(&r),
            FlashDecision::Proceed { allow_flags: vec!["--allow-erase", "--allow-nv-write"] }
        );
    }

    #[test]
    fn rf_calibration_is_blocked() {
        let mut r = report();
        r.touches_rf_calibration = true;
        r.rf_cali_files = vec![risk("LTE_CALI")];
        assert!(matches!(plan_flash(&r), FlashDecision::Blocked { .. }));
    }

    #[test]
    fn phasecheck_is_blocked() {
        let mut r = report();
        r.phasecheck_files = vec![risk("PhaseCheck")];
        assert!(matches!(plan_flash(&r), FlashDecision::Blocked { .. }));
    }

    #[test]
    fn unknown_risk_level_round_trips() {
        let lvl: RiskLevel = serde_json::from_str("\"FutureRisk\"").unwrap();
        assert_eq!(lvl, RiskLevel::Unknown("FutureRisk".into()));
        assert_eq!(serde_json::to_string(&lvl).unwrap(), "\"FutureRisk\"");
        let known: RiskLevel = serde_json::from_str("\"NvWrite\"").unwrap();
        assert_eq!(known, RiskLevel::NvWrite);
    }

    #[test]
    fn firmware_event_parses_engine_event_json() {
        let line = r#"{"Progress":{"port":3,"percent":52.5,"file_id":"FDL2"}}"#;
        match serde_json::from_str::<FirmwareEvent>(line).unwrap() {
            FirmwareEvent::Progress { port, percent, file_id } => {
                assert_eq!(port, 3);
                assert_eq!(file_id, "FDL2");
                assert!((percent - 52.5).abs() < f32::EPSILON);
            }
            other => panic!("wrong variant: {other:?}"),
        }
        let done = r#"{"Completed":{"port":3,"result":{"port":3,"success":true,"chip_uid":null,"flash_uid":null,"duration_ms":61000,"error":null}}}"#;
        assert!(matches!(
            serde_json::from_str::<FirmwareEvent>(done).unwrap(),
            FirmwareEvent::Completed { .. }
        ));
        // A renamed upstream field must FAIL deserialization (drift stays visible)
        let drifted = r#"{"Progress":{"port":3,"percent":52.5,"file_name":"FDL2"}}"#;
        assert!(serde_json::from_str::<FirmwareEvent>(drifted).is_err());
    }
}
