# FactoryTool - Agent Instructions

## Project Overview
Cross-platform 5G factory SN management tool built with Tauri 2 + Rust. Refactored from C# WinForms (.NET Framework 4.7.2).

## Architecture
- **Frontend**: Vanilla HTML/CSS/JS, no framework — `src/index.html` (markup + JS) + `src/styles.css` (theme, mirrors modem-cat's design language) + `src/fonts/` (Inter, JetBrains Mono)
- **Backend**: Rust in `src-tauri/src/`
- **Communication**: Tauri `invoke()` bridge
- **State**: `AppState` struct with `Mutex`-wrapped fields, managed by Tauri's state system

### Layers
| Layer | Responsibility | Files |
|-------|---------------|-------|
| UI | Page rendering, user interaction, theme | `src/index.html` |
| IPC | Frontend ↔ Backend | Tauri `invoke()` |
| Commands | Route dispatch, state management | `lib.rs`, `dloader.rs` (24 `#[tauri::command]` functions) |
| Business | SN generation, License, device comm, firmware flash | `sn_generator.rs`, `license.rs`, `api_client.rs`, `dloader.rs` |
| Data | JSON file read/write | `data.rs` |
| Types | Shared serde structs | `types.rs` |

## Key Files
- `src/index.html` — Complete UI markup + JS (sidebar nav, 5 pages incl. firmware download)
- `src/styles.css` — Theme + components (modem-cat midnight-blue design language, dark/light)
- `src-tauri/src/lib.rs` — Tauri commands + AppState (6 Mutex fields)
- `src-tauri/src/dloader.rs` — Firmware download: safety policy (`plan_flash`), sidecar bridge, `DloaderState`
- `src-tauri/src/api_client.rs` — REST HTTP client (reqwest, 5s timeout)
- `src-tauri/src/sn_generator.rs` — SN generation logic
- `src-tauri/src/license.rs` — JWT license validation
- `src-tauri/src/data.rs` — JSON file persistence
- `src-tauri/src/types.rs` — Shared Rust types (serde)
- `src-tauri/capabilities/default.json` — Webview capability (`core:default`, enables `event.listen`)
- `src-tauri/binaries/` — Vendored 32-bit `r26-cli` sidecar + `r26-cli.version.txt` stamp
- `scripts/update-dloader-cli.ps1` — Rebuild + re-vendor the sidecar from the sibling r26-dloader repo

## Developer Commands
```bash
# Install dependencies
bun install

# Dev mode (hot reload)
bun run dev

# Build for production
bun run build

# Check Rust compilation without full build
cd src-tauri && cargo check

# Build Rust backend only
cd src-tauri && cargo build

# Build Rust backend (release)
cd src-tauri && cargo build --release
```

Tests: `cd src-tauri && cargo test` (unit tests live in `dloader.rs` — safety policy + event contract). No linter is configured.

## Build Prerequisites
- Rust toolchain (rustup)
- Node.js/Bun
- Platform-specific: WebView2 (Windows), WebKitGTK (Linux), WebKit (macOS)

## API Communication
- Base URL: `http://{ip}/api`
- Protocol: REST (GET/POST), not JSON-RPC
- Response format: `ApiResponse<T> { code: i32, message: String, data: T }` (code 200 = success)
- Endpoints:
  - GET `device_activate_info` → IMEI, ICCID, firmware version
  - GET `device_name_get` → device name
  - GET `device_sn_get` / POST `device_sn_set` → serial number (write + readback verify)
  - POST `device_activate` → send license JWT
  - GET `license_get` → activation status

## License Validation
Two separate JWT systems with different keys and algorithms:

| Purpose | Key | Algorithm | Function |
|---------|-----|-----------|----------|
| Local License (`lic.dat`) | `PPRROODDUUCCTT123456` | HS512 | `validate_license()` — verifies MAC binding |
| Device Activation | `AARRIIXXOO22001177` | HS256 | `generate_device_jwt()` — generates token sent to device |

- Local license binds to MAC address; file: `lic.dat` (in exe directory)
- Device activation: IMEI + level → JWT → POST to device → verify via `license_get`

## Data Storage
- **Config + runtime data** — Tauri `app_data_dir()` (e.g. `%APPDATA%\com.arixo.factorytool`), flat, no subdirectory: `basecfgdata.json`, `selectdata.json`, `execute_sn_data.json`, `YYYY-MM-DD.csv` (device records, append-only, one file per day)
- **License file** (exe directory): `lic.dat`
- JSON key `CurretSeqNo` in `execute_sn_data.json` is a frozen legacy misspelling (see `types.rs`) — do not "fix" it

## SN Format
`Brand(1-2) + Type(2) + Factory(1) + Year(1) + Month(1 hex) + Sequence(5)` = ~12 chars

Year code = last digit of year (e.g. 2026 → "6")
Month code = uppercase hex (e.g. December → "C")
Sequence = 5-digit zero-padded, max 99999

## Managed State
`AppState` — all fields use `Mutex` for thread safety (acquire in field order, never hold across `.await`):
- `device_client: Mutex<Option<DeviceClient>>`
- `data_manager: Mutex<Option<DataManager>>`
- `current_product: Mutex<Product>`
- `current_code_set: Mutex<CodeSet>`
- `execute_data: Mutex<Option<ExecuteDataList>>`
- `base_data: Mutex<BaseData>`

`DloaderState` (managed separately): `child: Arc<Mutex<Option<CommandChild>>>` — the running firmware sidecar; cleared on process exit, stop, or window close.

## Key Patterns
- **DeviceClient clone**: Uses manual `Clone` impl that clones `reqwest::Client` and recreates `Mutex<String>` for IP — needed because `reqwest::Client` is already `Clone` but `Mutex<String>` is not `Clone`
- **Write-then-verify**: `set_device_sn()` writes SN then reads it back to confirm
- **Fallback defaults**: `data.rs` has `default_base_data()` with hardcoded brands/types/factories if `basecfgdata.json` is missing
- **Serde rename**: `Product` fields use PascalCase JSON keys (`"Brand"`, `"Type"`, `"Fac"`); `BaseData.types` field is renamed from `"types"` in JSON
- **withGlobalTauri**: `tauri.conf.json` has `"withGlobalTauri": true` — frontend uses `window.__TAURI__.invoke()` directly without ES module imports
- **CSP disabled**: `"csp": null` in tauri.conf.json

## Tauri Commands (24 total)
- **Core** (`lib.rs`): `init_app`, `check_license`, `get_base_data`, `get_current_product`, `set_product`, `get_current_sn`, `get_code_set`, `set_device_ip`, `write_sn_to_device`, `get_device_info_from_device`, `activate_device`, `save_execute_data`, `save_device_record`, `increment_sequence`
- **Base-data CRUD** (generated by the `crud_commands!` macro in `lib.rs`): `add_brand`, `remove_brand`, `add_product_type`, `remove_product_type`, `add_factory`, `remove_factory`
- **Firmware** (`dloader.rs`): `pick_pac_file`, `pac_info`, `start_firmware_download`, `stop_firmware_download`

## Firmware Download (r26-cli sidecar)
- The 32-bit `r26-cli.exe` (Unisoc DLLs embedded) runs as a Tauri sidecar under the 64-bit app (WOW64). The binary is named with the **host** triple (`r26-cli-x86_64-pc-windows-msvc.exe`) because Tauri selects sidecars by host triple.
- Sidecar and file dialog are driven from **Rust**, so no webview permission is needed beyond `core:default` (which provides `event.listen` for `firmware-event`).
- Downloads always pass `--port 0` — the sidecar auto-detects the COM port.
- Safety policy (`plan_flash`, unit-tested): erase/NV writes auto-allowed; **RF-calibration and PhaseCheck PACs are blocked before the sidecar is spawned**. Never pass `--allow-rf-calibration` / `--allow-phasecheck`.
- Event contract: sidecar stdout JSON (`EngineEvent`) is deserialized into the local `FirmwareEvent` enum at the bridge — upstream field renames surface as a "契约漂移" warn log instead of silent UI corruption. `SIDECAR_EXPECTED_VERSION` in `dloader.rs` is checked against `r26-cli --version` once per process; `binaries/r26-cli.version.txt` records the source commit.
- Sibling repo: `D:/code/r26-dloader` (no source dependency — only the compiled exe is vendored; refresh via `scripts/update-dloader-cli.ps1`).

## Notes
- Disables cert validation for device API (`danger_accept_invalid_certs`); 5s request timeout
- UI style matches modem-cat project (dark theme, sidebar nav, CSS variables); the firmware page mirrors r26-dloader's DownloadPage via `#page-firmware`-scoped styles
