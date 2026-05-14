# FactoryTool - Agent Instructions

## Project Overview
Cross-platform 5G factory SN management tool built with Tauri 2 + Rust. Refactored from C# WinForms (.NET Framework 4.7.2).

## Architecture
- **Frontend**: Single-file vanilla HTML/CSS/JS in `src/index.html` (no framework)
- **Backend**: Rust in `src-tauri/src/`
- **Communication**: Tauri `invoke()` bridge
- **State**: `AppState` struct with `Mutex`-wrapped fields, managed by Tauri's state system

### Layers
| Layer | Responsibility | Files |
|-------|---------------|-------|
| UI | Page rendering, user interaction, theme | `src/index.html` |
| IPC | Frontend ↔ Backend | Tauri `invoke()` |
| Commands | Route dispatch, state management | `lib.rs` (13 `#[tauri::command]` functions) |
| Business | SN generation, License, device comm | `sn_generator.rs`, `license.rs`, `api_client.rs` |
| Data | JSON file read/write | `data.rs` |
| Types | Shared serde structs | `types.rs` |

## Key Files
- `src/index.html` — Complete UI (sidebar nav, dark/light theme, 4 pages)
- `src-tauri/src/lib.rs` — Tauri commands + AppState (7 Mutex fields)
- `src-tauri/src/api_client.rs` — REST HTTP client (reqwest)
- `src-tauri/src/sn_generator.rs` — SN generation logic
- `src-tauri/src/license.rs` — JWT license validation
- `src-tauri/src/data.rs` — JSON file persistence
- `src-tauri/src/types.rs` — Shared Rust types (serde)

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

No test framework is configured. No linter is configured.

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
Portable mode — all data relative to the executable directory:
- **Config files** (exe directory): `basecfgdata.json`, `selectdata.json`, `lic.dat`
- **Runtime data** (`Data/` subdirectory): `execute_sn_data.json`, `YYYY-MM-DD.csv` (device records, append-only)
- Device records are appended as CSV rows (date-named file, one file per day)

## SN Format
`Brand(1-2) + Type(2) + Factory(1) + Year(1) + Month(1 hex) + Sequence(5)` = ~12 chars

Year code = last digit of year (e.g. 2026 → "6")
Month code = uppercase hex (e.g. December → "C")
Sequence = 5-digit zero-padded, max 99999

## AppState Fields
All fields use `Mutex` for thread safety:
- `device_client: Mutex<Option<DeviceClient>>`
- `data_manager: Mutex<Option<DataManager>>`
- `current_product: Mutex<Product>`
- `current_code_set: Mutex<CodeSet>`
- `execute_data: Mutex<Option<ExecutDataList>>`
- `execute_data: Mutex<Option<ExecutDataList>>`
- `base_data: Mutex<Option<BaseData>>`

## Key Patterns
- **DeviceClient clone**: Uses manual `Clone` impl that clones `reqwest::Client` and recreates `Mutex<String>` for IP — needed because `reqwest::Client` is already `Clone` but `Mutex<String>` is not `Clone`
- **Write-then-verify**: `set_device_sn()` writes SN then reads it back to confirm
- **Fallback defaults**: `data.rs` has `default_base_data()` with hardcoded brands/types/factories if `basecfgdata.json` is missing
- **Serde rename**: `Product` fields use PascalCase JSON keys (`"Brand"`, `"Type"`, `"Fac"`); `BaseData.types` field is renamed from `"types"` in JSON
- **withGlobalTauri**: `tauri.conf.json` has `"withGlobalTauri": true` — frontend uses `window.__TAURI__.invoke()` directly without ES module imports
- **CSP disabled**: `"csp": null` in tauri.conf.json

## Tauri Commands (13 total)
`init_app`, `check_license`, `get_base_data`, `get_current_product`, `set_product`, `get_current_sn`, `get_code_set`, `set_device_ip`, `write_sn_to_device`, `get_device_info_from_device`, `activate_device`, `save_execute_data`, `save_device_record`, `increment_sequence`

## Notes
- Disables cert validation for device API (`danger_accept_invalid_certs`)
- UI style matches modem-cat project (dark theme, sidebar nav, CSS variables)
- No test framework configured yet
