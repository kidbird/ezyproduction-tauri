# FactoryTool - Agent Instructions

## Project Overview
Cross-platform 5G factory SN management tool built with Tauri 2 + TypeScript. Refactored from C# WinForms (.NET Framework 4.7.2).

## Architecture
- **Frontend**: Single-file vanilla HTML/CSS/JS in `src/index.html` (no framework)
- **Backend**: Rust in `src-tauri/src/`
- **Communication**: Tauri `invoke()` bridge

## Key Files
- `src/index.html` — Complete UI (sidebar nav, dark/light theme, panels)
- `src-tauri/src/lib.rs` — Tauri commands + AppState
- `src-tauri/src/api_client.rs` — JSON-RPC HTTP client (reqwest)
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
```

## Build Prerequisites
- Rust toolchain (rustup)
- Node.js/Bun
- Platform-specific: WebView2 (Windows), WebKitGTK (Linux), WebKit (macOS)

## API Communication
- Endpoint: `http://{ip}/arixoapi`
- Protocol: JSON-RPC 2.0 over HTTP POST
- Methods: SetSerialNo, GetSerialNo, SetProductID2, GetProductID2, GetDeviceInfo, ValidateLicense, SetSimSlot

## License Validation
- JWT HMACSHA512, secret: `PPRROODDUUCCTT123456`
- Binds to MAC address
- License file: `lic.dat` (in app data dir)

## Data Storage
- JSON files in Tauri app data directory (`~/.config/factorytool/Data/` or platform equivalent)
- Files: `basecfgdata.json`, `selectdata.json`, `execute_sn_data.json`, `production_device_data.json`

## SN Format
Brand(2) + Type(2) + Factory(1) + Year(1) + Month(1 hex) + Sequence(5) = 12 chars

## Notes
- Disables cert validation for device API (danger_accept_invalid_certs)
- UI style matches modem-cat project (dark theme, sidebar nav, CSS variables)
- No test framework configured yet
