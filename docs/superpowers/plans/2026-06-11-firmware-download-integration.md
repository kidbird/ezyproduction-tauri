# Firmware Download Integration (方案 A · Sidecar) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a standalone "固件下载" (firmware download) page to FactoryTool that flashes Unisoc PAC files by driving the prebuilt 32-bit `r26-cli.exe` as a hidden Tauri sidecar — all inside the existing single FactoryTool window.

**Architecture:** FactoryTool stays 64-bit. The 32-bit `r26-cli.exe` (self-contained, embeds the Unisoc DLLs) ships as a Tauri `externalBin` sidecar. A new Rust module `dloader.rs` spawns the sidecar from the backend (so it bypasses the webview capability system), reads its line-delimited JSON output, and re-emits each line to the frontend as a `firmware-event`. The frontend firmware page mirrors r26-dloader's `DownloadPage` layout/visual style in vanilla HTML/CSS/JS. Safety policy is factory-safe: erase + NV writes are auto-allowed (DLFrame.dll preserves real calibration via Research-mode NV backup), while **RF calibration and PhaseCheck are never allowed** — such PACs are blocked before the sidecar is even spawned.

**Tech Stack:** Tauri 2, Rust, `tauri-plugin-shell` (sidecar), `tauri-plugin-dialog` (file picker), vanilla HTML/CSS/JS frontend, `r26-cli` (clap + serde_json) in the sibling `r26-dloader` workspace.

**Repo boundaries:** Phase 1 edits live in the **sibling repo** `D:/code/r26-dloader` and commit there. Phases 2–4 edit the **current FactoryTool worktree** and commit here. Each phase notes its repo.

---

## File Structure

**Sibling repo `D:/code/r26-dloader` (Phase 1):**
- Modify: `crates/r26-cli/Cargo.toml` — add `serde_json` dependency.
- Modify: `crates/r26-cli/src/main.rs` — add global `--json` flag; emit JSON from `pac-info` and `download`.

**FactoryTool worktree (Phases 2–4):**
- Create: `src-tauri/binaries/r26-cli-x86_64-pc-windows-msvc.exe` — the prebuilt 32-bit sidecar (named with the host triple, per Tauri convention).
- Create: `scripts/update-dloader-cli.ps1` — rebuild + copy the sidecar from the sibling repo.
- Create: `src-tauri/capabilities/default.json` — grant the webview `core:default` (includes `core:event` for `listen`).
- Create: `src-tauri/src/dloader.rs` — firmware DTOs, safety planning, and the 4 Tauri commands.
- Modify: `src-tauri/Cargo.toml` — add `tauri-plugin-shell`, `tauri-plugin-dialog`.
- Modify: `src-tauri/tauri.conf.json` — add `bundle.externalBin`.
- Modify: `src-tauri/src/lib.rs` — register plugins, manage `DloaderState`, register the 4 commands.
- Modify: `src/index.html` — add nav item, firmware page markup, CSS for chips/progress/alert/log, and page JS.

---

## Phase 1 — r26-dloader: add `--json` to r26-cli

> All Phase 1 edits and commits happen in `D:/code/r26-dloader`.

### Task 1: Add `--json` output mode to r26-cli

**Files:**
- Modify: `D:/code/r26-dloader/crates/r26-cli/Cargo.toml`
- Modify: `D:/code/r26-dloader/crates/r26-cli/src/main.rs`

- [ ] **Step 1: Add the `serde_json` dependency**

In `crates/r26-cli/Cargo.toml`, under `[dependencies]`, add:

```toml
serde_json = { workspace = true }
```

(The workspace root already declares `serde_json = "1"`, so this just pulls it in.)

- [ ] **Step 2: Add a global `--json` flag to the CLI struct**

In `crates/r26-cli/src/main.rs`, add this field to `struct Cli` (after the `dll_dir` field):

```rust
    /// Emit machine-readable JSON instead of human text.
    /// pac-info prints one PacSafetyReport object; download prints one
    /// EngineEvent object per line. Used by the FactoryTool sidecar bridge.
    #[arg(long, global = true)]
    json: bool,
```

`global = true` makes `cli.json` readable regardless of subcommand.

- [ ] **Step 3: Thread `json` into the dispatch in `main`**

Replace the `match cli.command` arms that call `show_pac_info` / `run_download` so they pass `cli.json`:

```rust
    match cli.command {
        Some(Commands::Devices) => {
            list_devices()?;
        }
        Some(Commands::PacInfo { ref path }) => {
            show_pac_info(path, cli.json)?;
        }
        Some(Commands::Download { ref pac, port, allow_erase, allow_nv_write, allow_rf_calibration, allow_phasecheck }) => {
            let pac = pac.clone();
            run_download(&pac, port, allow_erase, allow_nv_write, allow_rf_calibration, allow_phasecheck, cli.json).await?;
        }
        None => {
            if let Some(pac) = cli.pac.clone() {
                run_download(&pac, cli.port, cli.allow_erase, cli.allow_nv_write, cli.allow_rf_calibration, cli.allow_phasecheck, cli.json).await?;
            } else {
                eprintln!("No command specified. Use --help for usage.");
            }
        }
    }
```

- [ ] **Step 4: Make `show_pac_info` emit JSON when requested**

Replace the whole `fn show_pac_info(path: &str)` with this `json`-aware version:

```rust
fn show_pac_info(path: &str, json: bool) -> anyhow::Result<()> {
    let pac = match r26_core::pac::PacFile::parse(path) {
        Ok(pac) => pac,
        Err(e) => {
            if json {
                println!("{}", serde_json::json!({ "error": format!("{e}") }));
            } else {
                eprintln!("Failed to parse PAC file: {}", e);
            }
            return Ok(());
        }
    };
    let report = r26_core::safety::PacSafetyReport::analyze(&pac.entries);

    if json {
        // Single-line JSON object: the FactoryTool bridge parses this.
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }

    println!("PAC file: {}", path);
    println!("Product Name:    {}", pac.header.product_name);
    println!("Product Version: {}", pac.header.product_version);
    println!("File Count:      {}", pac.header.file_count);
    println!("Flash Type:      {}", pac.header.flash_type);
    println!("Encrypted:       {}", pac.header.encrypted);
    println!("\nFile Entries:");
    for (idx, entry) in pac.entries.iter().enumerate() {
        let risk = risk_tag(&report, &entry.file_id);
        println!(
            "  [{:02}] ID: {:<20} | Name: {:<30} | Size: {:10} bytes | {}",
            idx + 1, entry.file_id, entry.file_name, entry.file_size, risk,
        );
    }
    println!("\n安全分析: {}", report.summary());
    Ok(())
}
```

- [ ] **Step 5: Make `run_download` emit JSON per event when requested**

Change the signature and body of the `#[cfg(feature = "ffi-mode")]` `run_download`. Add a trailing `json: bool` param, gate every human `println!`/`eprintln!` behind `!json`, and in the event loop print each event as a JSON line when `json`. Replace the function with:

```rust
#[cfg(feature = "ffi-mode")]
async fn run_download(
    pac_path: &str,
    port: u32,
    allow_erase: bool,
    allow_nv_write: bool,
    allow_rf_calibration: bool,
    allow_phasecheck: bool,
    json: bool,
) -> anyhow::Result<()> {
    use r26_core::engine::{DownloadEngine, EngineEvent};

    macro_rules! say { ($($a:tt)*) => { if !json { println!($($a)*); } } }

    let dll_dir = resolve_dll_dir(None)?;
    say!("DLL 目录: {}", dll_dir.display());

    let mut engine = DownloadEngine::new_ffi(Some(&dll_dir))
        .map_err(|e| anyhow::anyhow!("加载 DLFrame.dll 失败: {e}"))?;
    let mut events = engine.subscribe();

    say!("加载 PAC: {pac_path}");
    engine
        .load_pac(pac_path)
        .map_err(|e| anyhow::anyhow!("加载 PAC 失败: {e}"))?;

    if let Some(report) = &engine.safety_report {
        say!("{}", report.summary());
    }

    if allow_erase { say!("⚠ --allow-erase"); let _ = engine.confirm_dangerous_operation("erase"); }
    if allow_nv_write { say!("⚠ --allow-nv-write"); let _ = engine.confirm_dangerous_operation("nv_write"); }
    if allow_rf_calibration { say!("⚠⚠ --allow-rf-calibration"); let _ = engine.confirm_dangerous_operation("rf_calibration"); }
    if allow_phasecheck { say!("⚠ --allow-phasecheck"); let _ = engine.confirm_dangerous_operation("phasecheck"); }

    if let Err(v) = engine.check_safety().and_then(|_| engine.check_environment()) {
        if json {
            let ev = EngineEvent::Error { code: 2, message: format!("{v}") };
            println!("{}", serde_json::to_string(&ev).unwrap_or_default());
        } else {
            eprintln!("安全策略拦截:\n{v}");
        }
        std::process::exit(2);
    }

    say!("开始下载 (端口: {})", if port == 0 { "自动检测".into() } else { format!("COM{port}") });
    engine
        .start_download(port)
        .map_err(|e| anyhow::anyhow!("启动下载失败: {e}"))?;

    loop {
        match events.recv().await {
            Ok(event) => {
                if json {
                    println!("{}", serde_json::to_string(&event).unwrap_or_default());
                }
                match event {
                    EngineEvent::Log { level, message } => { say!("[{level}] {message}"); }
                    EngineEvent::Progress { port, percent, file_id } => { say!("COM{port}: {file_id} - {percent:.1}%"); }
                    EngineEvent::StateChange { from, to } => { say!("[状态] {:?} → {:?}", from, to); }
                    EngineEvent::PacLoadProgress { percent } => { say!("[PAC 加载] {percent}%"); }
                    EngineEvent::Completed { port: _, result } => {
                        if result.success {
                            say!("✅ 下载完成！耗时 {} 秒", result.duration_ms / 1000);
                            break;
                        } else {
                            say!("❌ 下载失败: {}", result.error.as_deref().unwrap_or("未知错误"));
                            std::process::exit(1);
                        }
                    }
                    EngineEvent::Error { code, message } => {
                        say!("❌ 错误 [{code}]: {message}");
                        std::process::exit(1);
                    }
                    _ => { say!("[事件] (其它)"); }
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => { say!("[警告] 跳过 {n} 个事件"); }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => { break; }
        }
    }

    Ok(())
}
```

Also update the `#[cfg(not(feature = "ffi-mode"))]` stub signature so it still compiles:

```rust
#[cfg(not(feature = "ffi-mode"))]
async fn run_download(
    _pac_path: &str, _port: u32, _allow_erase: bool, _allow_nv_write: bool,
    _allow_rf_calibration: bool, _allow_phasecheck: bool, _json: bool,
) -> anyhow::Result<()> {
    anyhow::bail!("此构建未启用 ffi-mode，无法执行下载。");
}
```

- [ ] **Step 6: Verify it compiles (host target check is fine for syntax)**

Run: `cd /d D:\code\r26-dloader && cargo check -p r26-cli`
Expected: `Finished` with no errors. (Builds for the workspace's i686 target per `.cargo/config.toml`.)

- [ ] **Step 7: Verify JSON output on a real PAC**

Run (substitute any `.pac` you have, e.g. the one named in DOWNLOAD_MODE_GUIDE.md):
`cargo run -p r26-cli -- pac-info "D:/M1200_RG200U_CNV1_R03_015_005.pac" --json`
Expected: a single line starting with `{"total_files":` and containing `"rf_cali_files"`, `"erase_files"`, `"touches_rf_calibration"`. If you have no PAC file, skip the run and rely on Step 6.

- [ ] **Step 8: Commit (in the r26-dloader repo)**

```bash
cd /d D:\code\r26-dloader
git add crates/r26-cli/Cargo.toml crates/r26-cli/src/main.rs
git commit -m "feat(cli): add --json output for pac-info and download"
```

---

### Task 2: Build the 32-bit sidecar and place it in FactoryTool

**Files:**
- Create: `D:/code/ezyproduction-tauri/.claude/worktrees/jovial-clarke-d128ad/src-tauri/binaries/r26-cli-x86_64-pc-windows-msvc.exe`
- Create: `D:/code/ezyproduction-tauri/.claude/worktrees/jovial-clarke-d128ad/scripts/update-dloader-cli.ps1`

- [ ] **Step 1: Build the release CLI (32-bit)**

Run: `cd /d D:\code\r26-dloader && cargo build --release -p r26-cli`
Expected: produces `target/i686-pc-windows-msvc/release/r26-cli.exe` (the `.cargo/config.toml` forces i686). `Finished release`.

- [ ] **Step 2: Create the refresh script**

Create `scripts/update-dloader-cli.ps1` in the FactoryTool worktree:

```powershell
# Rebuilds the 32-bit r26-cli and copies it into FactoryTool as the Tauri
# sidecar. The sidecar MUST be named with the HOST triple (x86_64) even though
# the binary itself is 32-bit — Tauri selects the sidecar by host triple, and a
# 32-bit child runs fine under a 64-bit parent on Windows (WOW64).
$ErrorActionPreference = "Stop"
$dloader = "D:\code\r26-dloader"
$dest = Join-Path $PSScriptRoot "..\src-tauri\binaries\r26-cli-x86_64-pc-windows-msvc.exe"

Push-Location $dloader
cargo build --release -p r26-cli
Pop-Location

$src = Join-Path $dloader "target\i686-pc-windows-msvc\release\r26-cli.exe"
New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
Copy-Item $src $dest -Force
Write-Host "Copied $src -> $dest"
```

- [ ] **Step 3: Run the script to place the sidecar**

Run: `pwsh -File scripts/update-dloader-cli.ps1`
Expected: `Copied ... -> ...\src-tauri\binaries\r26-cli-x86_64-pc-windows-msvc.exe`

- [ ] **Step 4: Confirm the sidecar exists and is the expected size**

Run: `Get-Item src-tauri/binaries/r26-cli-x86_64-pc-windows-msvc.exe | Select-Object Name,Length`
Expected: Name = `r26-cli-x86_64-pc-windows-msvc.exe`, Length roughly 32–34 MB.

- [ ] **Step 5: Commit (in the FactoryTool repo)**

```bash
git add scripts/update-dloader-cli.ps1 src-tauri/binaries/r26-cli-x86_64-pc-windows-msvc.exe
git commit -m "build: vendor 32-bit r26-cli sidecar + refresh script"
```

---

## Phase 2 — FactoryTool backend

> All Phase 2 edits and commits happen in the current FactoryTool worktree.

### Task 3: Wire plugins, capability, and externalBin

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json:25-34` (the `bundle` block)
- Create: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/lib.rs:322-326` (the builder plugin chain)

- [ ] **Step 1: Add the two plugin dependencies**

In `src-tauri/Cargo.toml`, under `[dependencies]` (after the `tauri-plugin-single-instance` line), add:

```toml
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
```

- [ ] **Step 2: Register `externalBin` in tauri.conf.json**

In `src-tauri/tauri.conf.json`, change the `bundle` block to include `externalBin`:

```json
  "bundle": {
    "active": true,
    "targets": "all",
    "externalBin": ["binaries/r26-cli"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/256x256.png",
      "icons/icon.ico"
    ]
  }
```

- [ ] **Step 3: Create the capability granting `core:event` for the frontend listener**

Create `src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "FactoryTool default capability (core APIs incl. event listen).",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

Note: the sidecar and file dialog are driven from Rust, so they need NO webview permission. Only `core:event` (bundled in `core:default`) is required, for the frontend's `event.listen`.

- [ ] **Step 4: Register both plugins in the builder**

In `src-tauri/src/lib.rs`, in `pub fn run()`, add the two plugins right after the existing `tauri_plugin_single_instance` plugin registration:

```rust
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
```

- [ ] **Step 5: Verify the backend still compiles**

Run: `cd src-tauri && cargo check`
Expected: `Finished`. (No new commands yet — this just confirms the deps/plugins resolve.)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/capabilities/default.json src-tauri/src/lib.rs
git commit -m "feat: register shell+dialog plugins, sidecar externalBin, default capability"
```

---

### Task 4: Safety planning logic (`plan_flash`) — TDD

**Files:**
- Create: `src-tauri/src/dloader.rs`
- Modify: `src-tauri/src/lib.rs:4-8` (module declarations)

- [ ] **Step 1: Declare the module**

In `src-tauri/src/lib.rs`, add to the `mod` list near the top (after `mod data;`):

```rust
mod dloader;
```

- [ ] **Step 2: Write the DTOs and a failing test for `plan_flash`**

Create `src-tauri/src/dloader.rs` with the DTOs and the test (implementation stub returns wrong value so the test fails first):

```rust
//! Firmware download integration: drives the 32-bit `r26-cli` sidecar.
//!
//! FactoryTool stays 64-bit and never links the Unisoc DLLs. It spawns the
//! self-contained 32-bit `r26-cli.exe` sidecar, decoding its line-delimited
//! JSON into Tauri `firmware-event`s for the UI.

use serde::{Deserialize, Serialize};

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
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test dloader::tests`
Expected: 4 tests pass (`safe_pac_proceeds_with_no_flags`, `erase_and_nv_auto_allowed`, `rf_calibration_is_blocked`, `phasecheck_is_blocked`).

> Note: the implementation in Step 2 is already correct, so these pass immediately. The tests lock the safety policy so a future refactor can't silently start allowing RF writes.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/dloader.rs src-tauri/src/lib.rs
git commit -m "feat(dloader): safety DTOs and factory-safe plan_flash with tests"
```

---

### Task 5: `pac_info` + `pick_pac_file` commands

**Files:**
- Modify: `src-tauri/src/dloader.rs` (append)

- [ ] **Step 1: Add the imports and the `pac-info` helper + commands**

Append to `src-tauri/src/dloader.rs` (above the `#[cfg(test)]` module):

```rust
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_dialog::DialogExt;

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
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: `Finished`. (Commands aren't registered yet; that's Task 6.)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/dloader.rs
git commit -m "feat(dloader): pac_info + pick_pac_file commands via sidecar"
```

---

### Task 6: `start_firmware_download` + `stop_firmware_download` + state + registration

**Files:**
- Modify: `src-tauri/src/dloader.rs` (append the state + 2 commands)
- Modify: `src-tauri/src/lib.rs` (manage state, register 4 commands)

- [ ] **Step 1: Add the sidecar-process state and the start/stop commands**

Append to `src-tauri/src/dloader.rs` (above the `#[cfg(test)]` module):

```rust
use std::sync::{Arc, Mutex};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};

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
```

- [ ] **Step 2: Manage `DloaderState` and register the 4 commands in lib.rs**

In `src-tauri/src/lib.rs`, add the state to the `.manage(...)` chain. Add this line right before the existing `.manage(AppState { ... })` call:

```rust
        .manage(dloader::DloaderState::default())
```

Then in the `tauri::generate_handler![ ... ]` list, add the four commands after `remove_factory,`:

```rust
            dloader::pick_pac_file,
            dloader::pac_info,
            dloader::start_firmware_download,
            dloader::stop_firmware_download,
```

- [ ] **Step 3: Verify the backend compiles and the safety tests still pass**

Run: `cd src-tauri && cargo check && cargo test dloader::tests`
Expected: `Finished`; 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/dloader.rs src-tauri/src/lib.rs
git commit -m "feat(dloader): start/stop firmware download with event forwarding"
```

---

## Phase 3 — FactoryTool frontend (mirror r26 DownloadPage)

> All Phase 3 edits and commits happen in the current FactoryTool worktree.

### Task 7: Add the component CSS (chips, progress bar, alerts, log console)

**Files:**
- Modify: `src/index.html` (inside `<style>`, before the closing `</style>` at line ~560)

- [ ] **Step 1: Append the firmware-page component styles**

In `src/index.html`, just before `</style>`, insert:

```css
    /* ─── Firmware page components (r26 DownloadPage parity) ─────────────── */
    .panel-header { display:flex; align-items:center; justify-content:space-between; margin-bottom:12px; }
    .panel-title { font-size:11px; font-weight:600; letter-spacing:.06em; color:var(--text-muted); text-transform:uppercase; }
    .btn-danger { background:transparent; border:1px solid var(--error); color:var(--error); }
    .btn-danger:hover:not(:disabled) { background:var(--error); color:#fff; }
    .btn:disabled { opacity:.45; cursor:not-allowed; }
    .chip { display:inline-flex; align-items:center; gap:4px; padding:3px 9px; border-radius:999px;
            font-size:12px; font-weight:600; border:1px solid var(--border-color); background:var(--bg-tertiary); color:var(--text-secondary); }
    .chip.ok   { color:var(--success); border-color:var(--success); }
    .chip.warn { color:var(--warning); border-color:var(--warning); }
    .chip.err  { color:var(--error);   border-color:var(--error); }
    .chip.info { color:var(--info);    border-color:var(--info); }
    .progress-bar { height:8px; border-radius:999px; background:var(--bg-tertiary); overflow:hidden; }
    .progress-bar-fill { height:100%; background:var(--accent); transition:width .2s ease; }
    .alert { padding:10px 12px; border-radius:var(--radius); font-size:13px; border:1px solid transparent; }
    .alert-success { background:rgba(34,197,94,.12);  border-color:var(--success); color:var(--success); }
    .alert-error   { background:rgba(239,68,68,.12);   border-color:var(--error);   color:var(--error); }
    .alert-info    { background:rgba(59,130,246,.12);  border-color:var(--info);    color:var(--text-secondary); }
    .log-console { background:#121214; color:#e4e4e7; font-family:var(--font-mono); font-size:12px;
                   padding:12px; border-radius:6px; height:220px; overflow-y:auto; white-space:pre-wrap;
                   border:1px solid var(--border-color); box-shadow:inset 0 2px 4px rgba(0,0,0,.2); }
    .risk-table { width:100%; font-size:13px; border-collapse:collapse; margin-top:8px; }
    .risk-table th, .risk-table td { text-align:left; padding:6px 8px; border-bottom:1px solid var(--border-color); }
    .risk-table th { font-size:11px; font-weight:600; letter-spacing:.06em; color:var(--text-muted); text-transform:uppercase; }
    .risk-table td.id { font-family:var(--font-mono); font-size:12px; }
```

- [ ] **Step 2: Verify the page still renders (open in browser/dev later — visual only)**

No command. Visual styles are checked in Task 10. Proceed.

- [ ] **Step 3: Commit**

```bash
git add src/index.html
git commit -m "style: add firmware-page components (chips, progress, alerts, log)"
```

---

### Task 8: Add the sidebar nav item and firmware page markup

**Files:**
- Modify: `src/index.html:597-604` (add nav item after the `about` nav item — actually before it, so 固件下载 sits among the tools)
- Modify: `src/index.html` (add the page `<div>` after the records page, before the about page)

- [ ] **Step 1: Add the sidebar nav item**

In `src/index.html`, insert this nav item immediately BEFORE the `about` nav item (`<div class="nav-item" data-page="about">`):

```html
        <div class="nav-item" data-page="firmware">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
          固件下载
        </div>
```

- [ ] **Step 2: Add the firmware page markup**

In `src/index.html`, find the records page closing and the about page opening (`<div class="page" id="page-about">`). Insert this page `<div>` immediately BEFORE the about page:

```html
      <!-- Firmware Download Page -->
      <div class="page" id="page-firmware">
        <h1 class="page-title">固件下载</h1>

        <!-- PAC selection + analysis -->
        <div class="panel">
          <div class="panel-body">
            <div class="panel-header"><div class="panel-title">PAC 固件包</div></div>
            <div style="display:flex; align-items:center; gap:12px;">
              <button class="btn btn-primary" id="fwSelectPacBtn">选择 PAC</button>
              <span id="fwPacPath" style="font-size:12px; color:var(--text-muted); word-break:break-all;">未加载 — 选择一个 PAC 文件后即可开始下载</span>
            </div>
            <div id="fwPacAlert" style="margin-top:12px;"></div>
            <div id="fwRiskWrap" style="margin-top:8px;"></div>
          </div>
        </div>

        <!-- Safety status -->
        <div class="panel">
          <div class="panel-body">
            <div class="panel-header"><div class="panel-title">安全策略状态</div></div>
            <div id="fwSafetyChips" style="display:flex; gap:8px; flex-wrap:wrap;">
              <span class="chip ok">射频校准: 已保护</span>
            </div>
          </div>
        </div>

        <!-- Download controls -->
        <div class="panel">
          <div class="panel-body">
            <div class="panel-header"><div class="panel-title">下载控制</div></div>
            <div style="display:flex; gap:12px;">
              <button class="btn btn-primary" id="fwStartBtn" disabled>开始下载</button>
              <button class="btn btn-danger" id="fwStopBtn" disabled>停止</button>
            </div>
            <div id="fwProgressWrap" style="margin-top:14px; display:none;">
              <div style="display:flex; justify-content:space-between; font-size:12px; margin-bottom:4px;">
                <span id="fwProgressLabel" style="font-weight:600;"></span>
                <span id="fwProgressPct"></span>
              </div>
              <div class="progress-bar"><div class="progress-bar-fill" id="fwProgressFill" style="width:0%;"></div></div>
            </div>
            <div id="fwResult" style="margin-top:12px;"></div>
          </div>
        </div>

        <!-- Logs -->
        <div class="panel">
          <div class="panel-body">
            <div class="panel-header"><div class="panel-title">系统运行日志</div></div>
            <div class="log-console" id="fwLog"><span style="color:var(--text-muted); font-style:italic;">暂无日志。加载 PAC 后点击"开始下载"。</span></div>
          </div>
        </div>
      </div>
```

- [ ] **Step 3: Commit**

```bash
git add src/index.html
git commit -m "feat(ui): firmware download page markup + nav item"
```

---

### Task 9: Firmware page JavaScript

**Files:**
- Modify: `src/index.html` (in the `<script>` block, after the navigation handler near line ~932)

- [ ] **Step 1: Add the firmware page controller**

In `src/index.html`, inside the `<script>`, after the theme-toggle block (line ~940) and before `initApp`, insert:

```javascript
    // ─── Firmware Download Page ────────────────────────────────────────────
    const { listen } = window.__TAURI__.event;

    const fw = {
      pacPath: null,
      report: null,
      downloading: false,
    };

    const RISK_LABEL = {
      Safe: ['安全','ok'], NvWrite: ['NV','warn'], RfCalibration: ['RF校准','err'],
      Erase: ['擦除','err'], EraseAll: ['全盘擦除','err'], PhaseCheck: ['PhaseCheck','info'],
    };

    function fwLog(line) {
      const el = document.getElementById('fwLog');
      if (el.dataset.empty !== 'false') { el.textContent = ''; el.dataset.empty = 'false'; }
      el.textContent += (el.textContent ? '\n' : '') + line;
      el.scrollTop = el.scrollHeight;
    }

    function fwRenderReport(report) {
      const alertEl = document.getElementById('fwPacAlert');
      alertEl.innerHTML = `<div class="alert alert-success">✓ PAC 加载完成（${report.total_files} 个文件）</div>`;

      // Safety chips: reflect what this PAC will actually do.
      const chips = document.getElementById('fwSafetyChips');
      const eraseOn = report.erase_files.length > 0;
      const nvOn = report.nv_files.length > 0;
      const rfBlocked = report.touches_rf_calibration || report.rf_cali_files.length > 0;
      const pcBlocked = report.phasecheck_files.length > 0;
      chips.innerHTML =
        `<span class="chip ${eraseOn ? 'warn' : 'ok'}">擦除: ${eraseOn ? '将执行' : '无'}</span>` +
        `<span class="chip ${nvOn ? 'warn' : 'ok'}">NV写入: ${nvOn ? '将执行(保留校准)' : '无'}</span>` +
        `<span class="chip ${rfBlocked ? 'err' : 'ok'}">射频校准: ${rfBlocked ? '含校准·已禁止' : '已保护'}</span>` +
        (pcBlocked ? `<span class="chip err">PhaseCheck: 含数据·已禁止</span>` : '');

      // Risk file table.
      const rows = [];
      report.safe_files.forEach(id => rows.push({ id, risk: 'Safe', reason: '' }));
      [...report.nv_files, ...report.rf_cali_files, ...report.erase_files, ...report.phasecheck_files]
        .forEach(f => rows.push({ id: f.file_id, risk: f.risk_level, reason: f.reason }));
      const body = rows.map(r => {
        const [label, cls] = RISK_LABEL[r.risk] || [r.risk, 'info'];
        return `<tr><td class="id">${r.id}</td><td><span class="chip ${cls}" title="${r.reason}">${label}</span></td></tr>`;
      }).join('');
      document.getElementById('fwRiskWrap').innerHTML =
        `<details><summary style="cursor:pointer; font-size:12.5px; color:var(--text-secondary);">文件列表 (${report.total_files})</summary>` +
        `<table class="risk-table"><thead><tr><th>文件 ID</th><th>风险</th></tr></thead><tbody>${body}</tbody></table></details>`;

      const blocked = rfBlocked || pcBlocked;
      document.getElementById('fwStartBtn').disabled = blocked || fw.downloading;
      if (blocked) {
        alertEl.innerHTML += `<div class="alert alert-error" style="margin-top:8px;">此 PAC 含受保护分区（射频校准 / PhaseCheck），出于保护已禁止刷写。</div>`;
      }
    }

    function fwSetDownloading(on) {
      fw.downloading = on;
      document.getElementById('fwStartBtn').disabled = on || !fw.report;
      document.getElementById('fwStopBtn').disabled = !on;
      document.getElementById('fwSelectPacBtn').disabled = on;
      document.getElementById('fwStartBtn').textContent = on ? '下载中…' : '开始下载';
    }

    function fwSetResult(html) {
      document.getElementById('fwResult').innerHTML = html;
    }

    function fwSetProgress(label, pct) {
      const wrap = document.getElementById('fwProgressWrap');
      if (pct == null) { wrap.style.display = 'none'; return; }
      wrap.style.display = 'block';
      document.getElementById('fwProgressLabel').textContent = label;
      document.getElementById('fwProgressPct').textContent = `${Math.round(pct)}%`;
      document.getElementById('fwProgressFill').style.width = `${Math.round(pct)}%`;
    }

    // Select + analyze a PAC.
    document.getElementById('fwSelectPacBtn').addEventListener('click', async () => {
      try {
        const path = await invoke('pick_pac_file');
        if (!path) return;
        fw.pacPath = path;
        fw.report = null;
        document.getElementById('fwPacPath').textContent = path;
        document.getElementById('fwPacAlert').innerHTML = '<div class="alert alert-info">正在分析 PAC…</div>';
        document.getElementById('fwRiskWrap').innerHTML = '';
        document.getElementById('fwStartBtn').disabled = true;
        const report = await invoke('pac_info', { path });
        fw.report = report;
        fwRenderReport(report);
      } catch (e) {
        document.getElementById('fwPacAlert').innerHTML = `<div class="alert alert-error">分析失败: ${e}</div>`;
      }
    });

    // Start download.
    document.getElementById('fwStartBtn').addEventListener('click', async () => {
      if (!fw.pacPath) { showToast('请先选择 PAC 文件', 'error'); return; }
      try {
        document.getElementById('fwLog').textContent = '';
        document.getElementById('fwLog').dataset.empty = 'false';
        fwSetResult('');
        fwSetDownloading(true);
        fwSetProgress('等待设备…', 0);
        await invoke('start_firmware_download', { path: fw.pacPath });
      } catch (e) {
        fwSetDownloading(false);
        fwSetProgress(null);
        fwSetResult(`<div class="alert alert-error">${e}</div>`);
      }
    });

    // Stop download.
    document.getElementById('fwStopBtn').addEventListener('click', async () => {
      try {
        await invoke('stop_firmware_download');
        fwSetDownloading(false);
        fwSetProgress(null);
        fwSetResult('<div class="alert alert-info">下载已手动停止</div>');
      } catch (e) {
        showToast(`停止失败: ${e}`, 'error');
      }
    });

    // Listen to forwarded sidecar events.
    listen('firmware-event', (ev) => {
      const p = ev.payload || {};
      if (p.Log) {
        fwLog(`[${p.Log.level}] ${p.Log.message}`);
      } else if (p.Progress) {
        fwSetDownloading(true);
        fwSetProgress(p.Progress.file_id, p.Progress.percent);
      } else if (p.PacLoadProgress) {
        fwSetProgress('加载 PAC…', p.PacLoadProgress.percent);
      } else if (p.StateChange) {
        fwLog(`[状态] ${p.StateChange.from} → ${p.StateChange.to}`);
      } else if (p.Completed) {
        const r = p.Completed.result;
        fwSetDownloading(false);
        fwSetProgress(null);
        fwSetResult(r.success
          ? `<div class="alert alert-success">✅ 下载完成！耗时 ${Math.round((r.duration_ms||0)/1000)} 秒</div>`
          : `<div class="alert alert-error">❌ 下载失败: ${r.error || '未知错误'}</div>`);
      } else if (p.Error) {
        fwSetDownloading(false);
        fwSetProgress(null);
        fwSetResult(`<div class="alert alert-error">❌ 错误 [${p.Error.code}]: ${p.Error.message}</div>`);
      } else if (p.Terminated) {
        // Process exited without a Completed event (e.g. killed or crashed).
        if (fw.downloading) {
          fwSetDownloading(false);
          fwSetProgress(null);
          if (p.Terminated.code && p.Terminated.code !== 0) {
            fwSetResult(`<div class="alert alert-error">刷机进程异常退出 (code ${p.Terminated.code})</div>`);
          }
        }
      }
    });
```

- [ ] **Step 2: Verify the page has no obvious JS error (build + open in Task 10)**

No command here. Proceed to commit; runtime verification is Task 10.

- [ ] **Step 3: Commit**

```bash
git add src/index.html
git commit -m "feat(ui): firmware download page controller + event handling"
```

---

## Phase 4 — Build & verification

### Task 10: Full build and manual smoke test

**Files:** none (verification only)

- [ ] **Step 1: Full Rust build**

Run: `cd src-tauri && cargo build`
Expected: `Finished`. No errors. (Confirms plugins, commands, state, and capability all resolve together.)

- [ ] **Step 2: Run the app in dev mode**

Run: `bun run dev` (or `npx tauri dev` if bun script differs — check `package.json`).
Expected: the FactoryTool window opens; sidebar shows the new "固件下载" item.

- [ ] **Step 3: Verify PAC analysis (no device needed)**

In the running app: click "固件下载" → "选择 PAC" → pick a `.pac` file.
Expected:
- "✓ PAC 加载完成（N 个文件）" appears.
- Safety chips show 擦除/NV state and "射频校准: 已保护" (or "含校准·已禁止" if the PAC has RF cali).
- "文件列表" expands to a risk-tagged table.
- "开始下载" is enabled for a safe PAC, disabled (with a red blocked alert) for an RF/PhaseCheck PAC.

- [ ] **Step 4: Verify the blocked path**

If you have an RF-calibration PAC, select it.
Expected: red "已禁止刷写" alert; "开始下载" stays disabled; no sidecar is spawned.

- [ ] **Step 5: Verify a download attempt streams events**

With a safe PAC selected and a device in download mode (or none, to confirm error surfacing): click "开始下载".
Expected: log console streams `[state]`/`[level]` lines; progress bar moves on `Progress` events; on finish, a green "✅ 下载完成" or red "❌ 下载失败" result appears; "停止" kills it mid-run and shows "下载已手动停止". With no device, expect the engine to wait or emit an error that surfaces as a red result — NOT a silent hang of the UI controls.

- [ ] **Step 6: Confirm no console window flashes**

During Step 5, confirm no separate black console window appears (the sidecar runs windowless).

- [ ] **Step 7: Final commit (if any verification fixes were needed)**

```bash
git add -A
git commit -m "fix: firmware download verification adjustments"
```

(If Steps 1–6 passed with no changes, skip this commit.)

---

## Self-Review

**Spec coverage** (against the approved 7-section design):
- §1 Architecture / data flow → Tasks 3, 6 (sidecar spawn + `firmware-event` forwarding), Task 9 (frontend listen). ✓
- §2 Safety model (erase/NV auto, RF/PhaseCheck blocked) → Task 4 `plan_flash` + tests; Task 9 chip rendering + start-button gating. ✓
- §3 Backend module + 3 commands (+ `pick_pac_file`) → Tasks 4–6. ✓
- §4 Frontend page (4 panels, nav item) → Tasks 7–9. ✓
- §5 Bundling (sidecar, externalBin, plugins, capability, refresh script) → Tasks 2, 3. ✓
- §6 Error handling (mock/blocked/abnormal-exit/stop/single-port) → Task 6 (Terminated/Error/Stderr forwarding), Task 9 (Terminated + Error handling), Task 4 (blocked). Mock mode surfaces via the sidecar's non-zero exit + stderr Log/Error → result alert (Step 5). ✓
- §7 r26-cli `--json` → Task 1. ✓

**Placeholder scan:** No TBD/TODO; every code step has complete code; every command has expected output. ✓

**Type consistency:** `SafetyReportDto`/`FileRiskDto` field names match r26-core's serde output (`total_files`, `nv_files`, `rf_cali_files`, `erase_files`, `phasecheck_files`, `touches_rf_calibration`, `summary`, `file_id`, `file_type`, `risk_level`, `reason`). `plan_flash` returns `FlashPlan { blocked_reason, allow_flags }` used identically in Task 6. Event JSON keys (`Progress`/`Log`/`Completed`/`Error`/`StateChange`/`PacLoadProgress`/`Terminated`) match between the Rust forwarder (Task 6), the r26-cli serialization (Task 1, externally-tagged `EngineEvent`), and the frontend handler (Task 9). `DloaderState.child` type `Arc<Mutex<Option<CommandChild>>>` is consistent across start/stop. ✓

**Known deviation from design §6:** The standalone pre-flight "mock mode" banner is dropped — the prebuilt sidecar embeds the DLLs (so mock mode shouldn't occur), and if it ever does, the download error surfaces clearly via the result alert. This trims scope without losing safety.
