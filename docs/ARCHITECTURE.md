# FactoryTool — Architecture

> 实现细节的唯一真相源。`AGENTS.md` 只保留入口与硬约束，不回填本文件内容。
> 修改架构、命令、固件集成、持久化格式时，必须同一次更新本文件。

## 1. 技术栈

| 层 | 选型 | 说明 |
|---|---|---|
| 框架 | Tauri 2 | `src-tauri/tauri.conf.json`；`withGlobalTauri: true`，`csp: null` |
| 插件 | `single-instance` / `shell` / `dialog` | `shell` 只用于 sidecar；`dialog` 只用于 PAC 选择 |
| 后端 | Rust（2021 edition） | 见 §3 |
| 前端 | 原生 HTML/CSS/JS（**无框架、无打包**） | 见 §4 |
| HTTP | `reqwest 0.11` rustls-tls | TLS 校验关闭（`danger_accept_invalid_certs`），超时 5s |
| JWT | `jsonwebtoken 9` | 本地 HS512 + 设备 HS256 |
| 异步 | tokio 1 (full) | |
| 时间 | chrono 0.4 | SN 年/月码 + CSV 日期 |

## 2. 顶层布局

```
factory-tool/
├── AGENTS.md                         入口与硬约束
├── README.md                         项目自述
├── package.json                      仅 Tauri CLI
├── test_api.sh                       对物理设备的 curl 冒烟测试（手工执行）
├── src/
│   ├── index.html                    HTML 骨架（5 页）
│   ├── styles.css                    主题 + 组件
│   ├── fonts/                        Inter + JetBrains Mono（本地 woff2）
│   └── js/
│       ├── core.js                   state / invoke 包装 / toast / 导航 / 主题
│       ├── firmware.js               固件下载页 + firmware-event 监听
│       └── app.js                    initApp / 产品配置 / 生产页事件
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/default.json     仅 core:default（event.listen）
│   ├── binaries/                     r26-cli sidecar + 版本戳
│   ├── icons/
│   └── src/
│       ├── main.rs                   5 行入口
│       ├── lib.rs                    AppState、20 条命令、窗口关闭杀 sidecar
│       ├── dloader.rs                固件下载（4 命令 + 7 单测）
│       ├── api_client.rs             DeviceClient（REST）
│       ├── sn_generator.rs           SN 生成 / 序号递增
│       ├── license.rs                本地 + 设备 JWT
│       ├── data.rs                   JSON + CSV 持久化
│       └── types.rs                  serde 结构
├── scripts/update-dloader-cli.ps1    从 sibling r26-dloader 重新打包 sidecar
├── docs/
│   ├── ARCHITECTURE.md               本文件
│   └── factory-tool-api.md           设备侧 REST 接口
└── portable/
    └── basecfgdata.json              被 data.rs `include_str!` 引用（默认值）
```

## 3. 后端模块

### 3.1 `lib.rs` — AppState 与 20 条命令

```rust
pub struct AppState {
    pub device_client:   Mutex<Option<DeviceClient>>,
    pub data_manager:    Mutex<Option<DataManager>>,
    pub current_product: Mutex<Product>,
    pub current_code_set: Mutex<CodeSet>,
    pub execute_data:    Mutex<Option<ExecuteDataList>>,
    pub base_data:       Mutex<BaseData>,
}
```

**锁顺序**：按字段声明顺序获取；不得跨 `.await` 持有。

`DloaderState` 单独管理：`child: Arc<Mutex<Option<CommandChild>>>`。

**命令清单（20 条，全部 `#[tauri::command]`）**：

- 初始化与许可：`init_app`、`check_license`
- 产品 / 码集：`get_base_data`、`get_current_product`、`set_product`、`get_current_sn`、`get_code_set`
- 设备通信：`set_device_ip`、`write_sn_to_device`、`get_device_info_from_device`、`activate_device`
- 数据：`save_execute_data`、`save_device_record`、`increment_sequence`
- CRUD 宏（`crud_commands!`）：`add_brand`、`remove_brand`、`add_product_type`、`remove_product_type`、`add_factory`、`remove_factory`

**窗口关闭安全**：`CloseRequested` / `Destroyed` 时从 `DloaderState.child` 取出并 `kill()`，防止 sidecar 在 UI 消失后继续控制设备。

### 3.2 `dloader.rs` — r26-cli 集成（4 命令）

- `pick_pac_file` — 经 `tauri-plugin-dialog` 选择 `.pac`
- `pac_info` — `r26-cli pac-info --json`，stdout 最后一行 JSON 解析为 `SafetyReportDto`
- `start_firmware_download` — 重跑 `pac_info` → `plan_flash` 安全闸门 → `r26-cli download --port 0 --json ...` → 行解析为 `FirmwareEvent` → 经 Tauri 事件 `firmware-event` 转发前端
- `stop_firmware_download` — `child.kill()`

**安全策略 `plan_flash`**（已单测）：

| PAC 风险 | 处理 |
|---|---|
| Safe | 放行，无额外标志 |
| Erase / EraseAll | 自动 `--allow-erase` |
| NvWrite | 自动 `--allow-nv-write` |
| RfCalibration | **BLOCKED**（永不传 `--allow-rf-calibration`） |
| PhaseCheck | **BLOCKED**（永不传 `--allow-phasecheck`） |
| Unknown(v) | 默认 BLOCKED 并 warn |

**版本闸门**：`SIDECAR_EXPECTED_VERSION = "0.1.0"`，进程内 `OnceLock` 缓存；`binaries/r26-cli.version.txt` 记录源 commit。版本不匹配 → 报错 `"刷机组件版本不匹配"` 并指引运行 `scripts/update-dloader-cli.ps1`。

**事件契约**：sidecar stdout 行 JSON → `FirmwareEvent`；未识别对象打 `"契约漂移"` warn（不丢数据）；非 JSON 行作为 Log 事件；进程退出合成 `Terminated`。

**重入保护**：`DloaderState.child` 锁跨“查重—spawn—写入”三步，已存在 child → `"下载正在进行中"`。

**Sidecar 命名**：二进制按 **host** triple 命名（`r26-cli-x86_64-pc-windows-msvc.exe`），Tauri 按 host triple 选择 sidecar；32 位 sidecar 在 64 位进程下 WOW64 运行。

### 3.3 `api_client.rs` — DeviceClient

- `reqwest::Client` 复用连接池；IP 用 `Mutex<String>` 持有
- 手动 `Clone`：克隆 Client + 重建 IP Mutex
- 方法：`get_device_activate_info`、`get_device_name`、`get_device_sn`、`set_device_sn`（写后回读验证）、`activate_device`、`get_license_status`
- 响应结构：`ApiResponse<T> { code, message, data }`，`code == 200` 为成功

### 3.4 `sn_generator.rs` — SN 格式

```
Brand(1-2) + Type(2) + Factory(1) + Year(1) + Month(1 hex) + Sequence(5)
```

- Year = 年份末位（2026 → "6"）
- Month = 大写 hex（12 月 → "C"）
- Sequence = 5 位补零，上限 99999

### 3.5 `license.rs` — 两套 JWT

| 用途 | Key | 算法 | 函数 |
|---|---|---|---|
| 本地许可（`lic.dat`） | `PPRROODDUUCCTT123456` | HS512 | `validate_license`（校验 MAC 绑定） |
| 设备激活 | `AARRIIXXOO22001177` | HS256 | `generate_device_jwt` |

本地许可绑定 MAC，文件随 exe 目录；设备激活流程：取 IMEI + level → 签 JWT → POST 到设备 → `license_get` 验证。

### 3.6 `data.rs` — 持久化

- 目录：Tauri `app_data_dir()`（例 `%APPDATA%\com.arixo.factorytool`）
- 文件：
  - `basecfgdata.json`（品牌 / 产品类型 / 工厂）
  - `selectdata.json`（当前选择）
  - `execute_sn_data.json`（**冻结字段名 `CurretSeqNo`**，禁止重命名）
  - `YYYY-MM-DD.csv`（设备记录，追加式，一天一份）
- 兜底：`default_base_data()` 在 JSON 缺失时返回硬编码默认值（仅用于首次启动）

## 4. 前端

- `src/index.html` —— HTML 骨架（5 页：`production` / `product-config` / `records` / `firmware` / `about`）
- `src/styles.css` —— modem-cat midnight-blue 设计语言，暗/亮主题
- `src/fonts/` —— Inter + JetBrains Mono（本地 woff2，无 CDN）
- `src/js/core.js` —— `state` 全局对象、`tauriInvoke` / `invoke` 包装、toast、loading、导航、主题切换、`escHtml`
- `src/js/firmware.js` —— 固件页 UI、`firmware-event` 监听、`fw` 局部状态
- `src/js/app.js` —— `initApp`、产品配置表、生产页事件；末尾挂 `DOMContentLoaded`
- IPC：统一 `window.__TAURI__.core.invoke`（`withGlobalTauri: true`），按文件加载顺序执行：`core.js` → `firmware.js` → `app.js`
- **两种 IPC 包装并存（有意）**：
  - `tauriInvoke(cmd, args)` —— 自动 `showLoading` / `hideLoading` + 错误 toast；用于事件驱动的短命令
  - 直接 `invoke(cmd, args)` —— 用于长流程（固件下载的进度由 `firmware-event` 独立上报，加载遮罩会冲突）与启动期 `initApp`（UI 尚未就绪）

新增前端调用默认用 `tauriInvoke`；只有当调用方自己管理进度 UI 时才用 `invoke`，并在调用点加一行注释说明。

## 5. 开发者命令

```bash
bun install
bun run dev
bun run build
cd src-tauri && cargo check
cd src-tauri && cargo build --release
cd src-tauri && cargo test     # 单测集中在 dloader.rs
```

无 linter、无 CI。

## 6. 平台前置

- Rust 工具链（rustup）
- Node.js / Bun
- WebView2（Windows）/ WebKitGTK（Linux）/ WebKit（macOS）
- 固件下载：**仅 Windows**（sidecar 为 32 位 PE，依赖 Unisoc DLL；Tauri 会按 host triple 校验 `binaries/r26-cli-<triple>` 存在，macOS/Linux 上构建会因缺少对应 sidecar 而失败——要么为对应平台提供 dummy sidecar，要么在 `tauri.conf.json` 的 `bundle.externalBin` 外加条件判断）
