# AGENTS.md

> 项目入口。只保留任务路由、硬约束、项目级约定。架构、命令清单、r26 集成细节写在 `docs/ARCHITECTURE.md`，本文件不重复。
> 与兄弟项目 `modem-cat` 共享同一套 Tauri 2 + 原生前端的设计语言与约定；分歧只在本文件列出的 factory-tool 特有部分。

## Required Read Order

1. 本文件
2. `docs/ARCHITECTURE.md`（模块、命令清单、r26 集成、SN 格式、License、API、持久化）
3. `docs/factory-tool-api.md`（设备侧 REST 接口）
4. 改动若涉及架构/命令/固件/持久化，必须同一次更新 `docs/ARCHITECTURE.md`

## Project Overview

跨平台 5G 工厂 SN 管理工具。Tauri 2 + Rust，从 C# WinForms (.NET Framework 4.7.2) 重构而来。

## Hard Constraints

- **单一真相源**：配置以 `AppState` 为准，持久化以 `app_data_dir()` 为准。不要维护第二份 live 镜像，也不要在 handler 里写“读不到就 fallback 默认值”——读不到就直接报错。
- **写后必读（write-then-verify）**：`set_device_sn` 写入后必须回读确认；类似写设备字段都要遵循。
- **固件下载只走 r26-cli sidecar**：禁止新增第二条刷机路径，禁止在前端直连 spawn；sidecar 由 Rust 侧经 `tauri-plugin-shell` 拉起，前端只能通过 `firmware-event` 事件观察进度。
- **安全策略不可绕过**：`dloader::plan_flash` 判定 RF 校准 / PhaseCheck PAC 必须 BLOCKED；**永远不要**传 `--allow-rf-calibration` 或 `--allow-phasecheck`。新增 PAC 类型先补 `dloader.rs` 的单测。
- **Sidecar 版本闸门**：`SIDECAR_EXPECTED_VERSION` 必须与 `binaries/r26-cli.version.txt` 一致；刷新 sidecar 只能走 `scripts/update-dloader-cli.ps1`，不要手改二进制。
- **Mutex 锁顺序**：`AppState` 各字段按声明顺序获取，**不得跨 `.await` 持有**；`DloaderState.child` 锁要覆盖“查重—spawn—写入”整个序列，防止并发重复拉起。
- **冻结的拼写**：`types.rs` 中 `CurretSeqNo` 是遗留字段名，**禁止**重命名（会破坏 `execute_sn_data.json` 兼容）。
- **AT/字符串参数校验**：所有由用户输入拼进命令行的字符串，必须做非空 + 长度 + 字符白名单校验（参考 `validate_at_string` 思路）。
- **前端只用 `window.__TAURI__` 全局**：`withGlobalTauri: true`，禁止引入 `@tauri-apps/api` 的 ES 模块导入；IPC 统一走 `tauriInvoke`（见 `src/index.html`），不要新写一份 raw `invoke` 包装。
- **单一 UI 设计语言**：沿用 `modem-cat` 的 midnight-blue 主题与 CSS 变量；不要新增硬编码颜色、图标字体、emoji 图标。
- **设备 API 的 TLS 校验是关的**（`danger_accept_invalid_certs`），超时 5s——不要“修”它。

## Task Routing

| 任务 | 工作目录 | 参考 |
|---|---|---|
| 前端（HTML/CSS/JS，5 页） | `src/` | `docs/ARCHITECTURE.md` §Frontend |
| Tauri 命令 / AppState | `src-tauri/src/{lib,types,data}.rs` | `docs/ARCHITECTURE.md` §Commands |
| 设备 REST 客户端 | `src-tauri/src/api_client.rs` | `docs/factory-tool-api.md` |
| SN / License | `src-tauri/src/{sn_generator,license}.rs` | `docs/ARCHITECTURE.md` §SN §License |
| 固件下载（r26 sidecar） | `src-tauri/src/dloader.rs` + `src-tauri/binaries/` | `docs/ARCHITECTURE.md` §Firmware |
| Sidecar 重新打包 | 仓库外 `D:/code/r26-dloader` | `scripts/update-dloader-cli.ps1` |
| 构建 / 发布 | 仓库根 | `bun run build` / `src-tauri/Cargo.toml` release profile |

## Key Files

- `src/index.html` —— HTML 骨架（5 页：production / product-config / records / firmware / about）
- `src/styles.css` —— 主题与组件（沿用 modem-cat 设计语言）
- `src/js/core.js` —— `state` / invoke 包装 / toast / loading / 导航 / 主题 / `escHtml`
- `src/js/firmware.js` —— 固件下载页 + `firmware-event` 监听
- `src/js/app.js` —— `initApp` / 产品配置 / 生产页事件
- `src-tauri/src/lib.rs` —— `AppState`、24 条 Tauri 命令、窗口关闭时杀 sidecar
- `src-tauri/src/dloader.rs` —— `plan_flash` 安全策略、sidecar 桥、事件契约、`DloaderState`
- `src-tauri/binaries/r26-cli*.exe` + `r26-cli.version.txt` —— 32 位刷机 sidecar + 版本戳
- `src-tauri/capabilities/default.json` —— 仅 `core:default`（提供 `event.listen`）
- `scripts/update-dloader-cli.ps1` —— sidecar 重新打包脚本

## Developer Commands

```bash
bun install              # 安装前端依赖
bun run dev              # 开发模式
bun run build            # 生产构建
cd src-tauri && cargo check
cd src-tauri && cargo build --release
cd src-tauri && cargo test     # 单测集中在 dloader.rs（安全策略 + 事件契约）
```

无 linter 配置；无 CI 配置。

## Ownership Rules

- `docs/ARCHITECTURE.md` —— 架构、模块、命令清单、r26 集成、SN、License、API、持久化
- `docs/factory-tool-api.md` —— 设备侧 REST 接口规格
- 架构、命令、固件、持久化格式变化必须同一次更新 `docs/ARCHITECTURE.md`
- 不要把 `AGENTS.md` 写成实现细节清单；细节去 owner 文档
