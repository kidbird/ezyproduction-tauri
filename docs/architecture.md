# FactoryTool - 架构设计文档

> 5G 工厂序列号管理工具 | Tauri 2 + Rust + 原生 HTML/CSS/JS | v0.1.0

---

## 目录

1. [技术架构](#1-技术架构)
2. [技术栈](#2-技术栈)
3. [Code Map](#3-code-map)
4. [功能模块说明](#4-功能模块说明)
5. [函数说明](#5-函数说明)
6. [调用流程](#6-调用流程)
7. [数据结构说明](#7-数据结构说明)
8. [激活算法说明](#8-激活算法说明)

---

## 1. 技术架构

### 1.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        桌面窗口 (WebView2)                       │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                      前端 (HTML/CSS/JS)                    │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │  │
│  │  │ 生产操作  │  │ 设备配置  │  │ 生产记录  │  │   关于   │  │  │
│  │  │  页面    │  │  页面    │  │  页面    │  │  页面    │  │  │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │  │
│  │                                                           │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │              Tauri IPC Bridge (invoke)               │  │  │
│  │  └──────────────────────┬──────────────────────────────┘  │  │
│  └─────────────────────────┼─────────────────────────────────┘  │
└────────────────────────────┼────────────────────────────────────┘
                             │ JSON-RPC over IPC
┌────────────────────────────┼────────────────────────────────────┐
│                        Rust 后端 (lib.rs)                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ Tauri Commands│  │  AppState    │  │   模块层             │  │
│  │ (16 个命令)   │  │ (全局状态)    │  │  api_client.rs       │  │
│  │              │  │              │  │  sn_generator.rs      │  │
│  │              │  │              │  │  license.rs           │  │
│  │              │  │              │  │  data.rs              │  │
│  │              │  │              │  │  types.rs             │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘  │
└─────────┼─────────────────┼─────────────────────┼──────────────┘
          │                 │                     │
          ▼                 ▼                     ▼
┌─────────────────┐ ┌──────────────┐  ┌──────────────────────────┐
│  文件系统 (JSON) │ │  网络层      │  │  系统层                   │
│  basecfgdata    │ │  reqwest     │  │  mac_address (MAC获取)   │
│  selectdata     │ │  JSON-RPC    │  │  chrono (时间)           │
│  execute_sn     │ │  HTTP POST   │  │  jsonwebtoken (JWT)      │
│  device_data    │ │  设备API     │  │  serde (序列化)          │
└─────────────────┘ └──────────────┘  └──────────────────────────┘
```

### 1.2 分层架构

| 层级 | 职责 | 文件 |
|------|------|------|
| **UI 层** | 页面渲染、用户交互、状态展示 | `src/index.html` |
| **IPC 层** | 前端 ↔ 后端通信 | Tauri `invoke()` |
| **命令层** | 路由分发、参数校验、状态管理 | `lib.rs` (16 个 `#[tauri::command]`) |
| **业务层** | SN 生成、License 验证、设备通信 | `sn_generator.rs`, `license.rs`, `api_client.rs` |
| **数据层** | JSON 文件读写、数据持久化 | `data.rs` |
| **类型层** | 共享数据结构定义 | `types.rs` |

### 1.3 进程模型

```
┌─────────────────────┐
│   主进程 (Rust)      │  ← 系统托盘、窗口管理、文件系统、网络请求
│   tauri::Builder     │
│   AppState (Mutex)   │
└──────────┬──────────┘
           │ IPC (消息通道)
┌──────────┴──────────┐
│   渲染进程 (WebView) │  ← HTML/CSS/JS 渲染、DOM 操作、事件处理
│   index.html         │
└─────────────────────┘
```

---

## 2. 技术栈

### 2.1 后端 (Rust)

| 组件 | 版本 | 用途 |
|------|------|------|
| **Tauri** | 2.x | 桌面应用框架，提供 IPC、窗口管理、构建系统 |
| **reqwest** | 0.11 | HTTP 客户端，用于设备 JSON-RPC 通信 |
| **serde / serde_json** | 1.x | JSON 序列化/反序列化 |
| **jsonwebtoken** | 9.x | JWT 生成与验证 (HMAC-SHA512) |
| **chrono** | 0.4 | 日期时间处理、SN 年月码生成 |
| **mac_address** | 1.1 | 跨平台 MAC 地址获取 |
| **tokio** | 1.x | 异步运行时 (full features) |

### 2.2 前端 (原生 Web)

| 组件 | 用途 |
|------|------|
| **HTML5** | 单文件应用，无框架 |
| **CSS3** | CSS 变量主题系统、Flex/Grid 布局 |
| **Vanilla JS** | DOM 操作、事件处理、Tauri IPC 调用 |
| **@tauri-apps/api** | 2.x，前端 IPC 客户端 |

### 2.3 构建工具

| 组件 | 用途 |
|------|------|
| **Bun** | 包管理 + 脚本执行 |
| **Cargo** | Rust 编译、依赖管理 |
| **tauri-build** | Tauri 构建脚本，生成资源文件 |
| **WiX / NSIS** | Windows 安装包生成 (MSI / EXE) |

---

## 3. Code Map

### 3.1 目录结构

```
factory-tool/
├── src/
│   └── index.html              # 前端 UI (单文件, ~1050 行)
│                               #   - CSS: ~480 行 (设计系统 + 组件样式)
│                               #   - HTML: ~200 行 (4 个页面)
│                               #   - JS: ~370 行 (状态管理 + 事件处理)
├── src-tauri/
│   ├── src/
│   │   ├── main.rs             # 入口点 (5 行)
│   │   │   └── factory_tool::run()
│   │   │
│   │   ├── lib.rs              # 核心模块 (420 行)
│   │   │   ├── AppState 结构体 (7 个 Mutex 字段)
│   │   │   ├── init_app()      初始化命令
│   │   │   ├── check_license() License 验证命令
│   │   │   ├── get_base_data() 获取基础配置
│   │   │   ├── get_current_product() 获取当前产品
│   │   │   ├── set_product()   设置产品 + 更新 SN
│   │   │   ├── get_current_sn() 获取当前 SN
│   │   │   ├── get_code_set()  获取 SN 码集
│   │   │   ├── set_device_ip() 设置设备 IP
│   │   │   ├── write_sn_to_device() 写入 SN 到设备
│   │   │   ├── write_product_type() 写入型号到设备
│   │   │   ├── get_device_info_from_device() 获取设备信息
│   │   │   ├── activate_device() 激活设备
│   │   │   ├── save_execute_data() 保存执行数据
│   │   │   ├── save_device_record() 保存设备记录
│   │   │   ├── increment_sequence() 递增序列号
│   │   │   └── get_esim_iccid() 查询 eSIM ICCID
│   │   │
│   │   ├── api_client.rs       # 设备通信客户端 (~130 行)
│   │   │   ├── DeviceClient 结构体
│   │   │   ├── new()           构造函数
│   │   │   ├── update_ip()     更新 IP
│   │   │   ├── send_request()  JSON-RPC 请求发送
│   │   │   ├── set_serial_no() 写入 SN
│   │   │   ├── get_serial_no() 读取 SN
│   │   │   ├── set_product_id() 写入型号
│   │   │   ├── get_product_id() 读取型号
│   │   │   ├── get_device_info() 获取设备信息
│   │   │   ├── set_sim_slot()  切换 SIM 卡槽
│   │   │   └── validate_license() 设备端 License 验证
│   │   │
│   │   ├── sn_generator.rs     # SN 生成逻辑 (~40 行)
│   │   │   ├── get_year_code()  年份码 (年份最后一位)
│   │   │   ├── get_mon_code()   月份码 (16进制)
│   │   │   ├── increment_seq()  序列号递增
│   │   │   ├── generate_sn()    生成完整 SN
│   │   │   └── update_code_set() 更新年月码
│   │   │
│   │   ├── license.rs          # License 验证 (~87 行)
│   │   │   ├── read_license_file() 读取 lic.dat
│   │   │   ├── validate_license()  验证 JWT Token
│   │   │   ├── get_current_mac()   获取本机 MAC
│   │   │   └── generate_device_jwt() 生成设备激活 JWT
│   │   │
│   │   ├── data.rs             # 数据持久化 (~90 行)
│   │   │   ├── DataManager 结构体
│   │   │   ├── portable()      构造函数 (便携模式)
│   │   │   ├── read_json()     读取 Data/ 目录 JSON
│   │   │   ├── write_json()    写入 Data/ 目录 JSON
│   │   │   ├── read_config_json() 读取根目录配置 JSON
│   │   │   ├── write_config_json() 写入根目录配置 JSON
│   │   │   └── load_*/save_*()  业务数据读写方法
│   │   │
│   │   └── types.rs            # 类型定义 (~109 行)
│   │       ├── Product         产品选择
│   │       ├── Item            下拉选项项
│   │       ├── BaseData        基础配置
│   │       ├── CodeSet         SN 码集
│   │       ├── ExecuteData     执行数据
│   │       ├── ExecutDataList  执行数据列表
│   │       ├── DeviceInfo      设备信息
│   │       ├── DeviceRecords   设备记录
│   │       ├── JsonRpcResponse JSON-RPC 响应
│   │       ├── ProdSWLic       License 文件
│   │       └── AppConfig       应用配置
│   │
│   ├── Cargo.toml              # Rust 依赖配置
│   ├── tauri.conf.json         # Tauri 应用配置
│   ├── build.rs                # 构建脚本
│   └── icons/                  # 应用图标
│       ├── icon.ico
│       ├── 32x32.png
│       ├── 128x128.png
│       └── 256x256.png
│
├── package.json                # Node 包配置
├── tsconfig.json               # TypeScript 配置
├── .gitignore                  # Git 忽略规则
├── AGENTS.md                   # AI Agent 指令
└── README.md                   # 项目说明
```

### 3.2 数据文件布局

```
portable/
├── factory-tool.exe     # 主程序
├── basecfgdata.json            # 品牌/类型/工厂基础配置 (根目录)
├── selectdata.json             # 上次选择的产品 (根目录)
├── lic.dat                     # License 文件 (根目录, 可选)
└── Data/                       # 运行时数据目录
    ├── execute_sn_data.json    # SN 执行记录 (按前缀分组)
    └── production_device_data.json  # 设备生产记录 (按 IMEI 去重)
```

---

## 4. 功能模块说明

### 4.1 生产操作模块

**页面**: `#page-production`

| 功能 | 说明 |
|------|------|
| **产品选择** | 从 `basecfgdata.json` 加载品牌/类型/工厂下拉框，选择后自动更新 SN |
| **SN 显示** | 实时显示生成的 12 位序列号，格式: Brand(2)+Type(2)+Factory(1)+Year(1)+Month(1)+Seq(5) |
| **写入 SN** | 将 SN 写入设备，读取设备信息，保存执行数据和设备记录，自动递增序列号 |
| **获取设备信息** | 从设备读取 IMEI、ICCID、SN、软件版本、设备名称 |
| **激活设备** | 生成设备 JWT，发送到设备端进行 License 验证 |

### 4.2 设备配置模块

**页面**: `#page-config`

| 功能 | 说明 |
|------|------|
| **连接设置** | 输入设备 IP 地址，建立 HTTP 连接 |
| **eSIM ICCID 查询** | 切换到 SIM 卡槽 2，读取 ICCID，生成查询链接 |

### 4.3 生产记录模块

**页面**: `#page-records`

| 功能 | 说明 |
|------|------|
| **设备记录列表** | 显示所有已生产设备的信息 (时间、IMEI、ICCID、SN、设备名称) |

### 4.4 关于模块

**页面**: `#page-about`

| 功能 | 说明 |
|------|------|
| **版本信息** | 显示应用版本、技术栈、支持平台 |

### 4.5 全局功能

| 功能 | 说明 |
|------|------|
| **主题切换** | 暗色/亮色主题切换，通过 CSS 变量实现 |
| **连接状态** | 侧边栏底部显示设备连接状态 (红/绿点) |
| **Toast 通知** | 右上角弹出操作结果提示 |
| **Loading 遮罩** | 异步操作时显示加载动画 |

---

## 5. 函数说明

### 5.1 Tauri Commands (lib.rs)

#### 初始化

| 函数 | 签名 | 说明 |
|------|------|------|
| `init_app` | `fn(AppHandle) -> Result<bool, String>` | 应用启动初始化：加载所有 JSON 数据，解析产品选择，恢复序列号 |

**内部流程**:
1. 获取 exe 所在目录 → 创建 `DataManager::portable()`
2. 加载 `basecfgdata.json` → `base_data`
3. 加载 `selectdata.json` → `product` (失败则使用默认值)
4. 加载 `execute_sn_data.json` → `execute_data`
5. 加载 `production_device_data.json` → `device_records`
6. 初始化 `CodeSet` (全空，seq="00001")
7. 根据 product 从 base_data 解析 brand_code/type_code/fac_code
8. 调用 `update_code_set()` 设置年月码
9. 根据 execute_data 中匹配的前缀恢复序列号
10. 将所有数据写入 `AppState`

#### License

| 函数 | 签名 | 说明 |
|------|------|------|
| `check_license` | `fn() -> Result<bool, String>` | 读取 `lic.dat`，验证 JWT Token 的 MAC 地址绑定 |

#### 产品与 SN

| 函数 | 签名 | 说明 |
|------|------|------|
| `get_base_data` | `fn(State) -> Result<BaseData, String>` | 返回基础配置数据 |
| `get_current_product` | `fn(State) -> Result<Product, String>` | 返回当前选择的产品 |
| `set_product` | `fn(brand, product_type, fac, State) -> Result<String, String>` | 更新产品选择，重新解析码集，保存配置，返回新 SN |
| `get_current_sn` | `fn(State) -> Result<String, String>` | 根据当前码集生成 SN |
| `get_code_set` | `fn(State) -> Result<CodeSet, String>` | 返回当前 SN 码集 |

#### 设备通信

| 函数 | 签名 | 说明 |
|------|------|------|
| `set_device_ip` | `fn(ip, State) -> Result<(), String>` | 创建或更新 `DeviceClient` |
| `write_sn_to_device` | `async fn(sn, State) -> Result<bool, String>` | 写入 SN 到设备并验证 |
| `write_product_type` | `async fn(product_type, State) -> Result<bool, String>` | 写入型号到设备并验证 |
| `get_device_info_from_device` | `async fn(State) -> Result<DeviceInfo, String>` | 从设备获取完整信息 |
| `activate_device` | `async fn(State) -> Result<bool, String>` | 获取设备 IMEI → 生成 JWT → 发送到设备验证 |

#### 数据管理

| 函数 | 签名 | 说明 |
|------|------|------|
| `save_execute_data` | `fn(State) -> Result<(), String>` | 保存当前 SN 执行记录到 `execute_sn_data.json` |
| `save_device_record` | `fn(DeviceInfo, State) -> Result<(), String>` | 保存设备信息到 `production_device_data.json` (按 IMEI 去重) |
| `increment_sequence` | `fn(State) -> Result<String, String>` | 序列号 +1，返回新 SN |

#### eSIM

| 函数 | 签名 | 说明 |
|------|------|------|
| `get_esim_iccid` | `async fn(State) -> Result<String, String>` | 切换到 SIM 槽 2 → 读取 ICCID → 切回槽 1 → 返回前 19 位 |

### 5.2 业务层函数

#### api_client.rs

| 函数 | 签名 | 说明 |
|------|------|------|
| `DeviceClient::new` | `fn(ip: &str) -> Self` | 创建 HTTP 客户端 (禁用证书验证) |
| `DeviceClient::update_ip` | `fn(&self, ip: &str)` | 更新目标 IP |
| `DeviceClient::send_request` | `async fn(method, params) -> Result<Value, String>` | 发送 JSON-RPC 2.0 请求 |
| `DeviceClient::set_serial_no` | `async fn(sn) -> Result<bool, String>` | 写入 SN + 读取验证 |
| `DeviceClient::get_serial_no` | `async fn() -> Result<String, String>` | 读取设备 SN |
| `DeviceClient::set_product_id` | `async fn(id) -> Result<bool, String>` | 写入型号 + 读取验证 |
| `DeviceClient::get_product_id` | `async fn() -> Result<String, String>` | 读取设备型号 |
| `DeviceClient::get_device_info` | `async fn() -> Result<DeviceInfo, String>` | 获取设备完整信息 |
| `DeviceClient::set_sim_slot` | `async fn(slot) -> Result<bool, String>` | 切换 SIM 卡槽 |
| `DeviceClient::validate_license` | `async fn(lic_str) -> Result<bool, String>` | 设备端 License 验证 |

#### sn_generator.rs

| 函数 | 签名 | 说明 |
|------|------|------|
| `get_year_code` | `fn() -> String` | 取年份最后一位 (如 2026 → "6") |
| `get_mon_code` | `fn() -> String` | 月份转 16 进制大写 (如 4月 → "4", 12月 → "C") |
| `increment_seq` | `fn(seq) -> Result<String, String>` | 序列号 +1，超过 99999 报错 |
| `generate_sn` | `fn(code_set) -> String` | 拼接 6 段码生成 12 位 SN |
| `update_code_set` | `fn(code_set)` | 更新年月码到当前时间 |

#### license.rs

| 函数 | 签名 | 说明 |
|------|------|------|
| `read_license_file` | `fn(base_dir) -> Result<ProdSWLic, String>` | 读取 `lic.dat` 中的 JWT token |
| `validate_license` | `fn(token) -> Result<bool, String>` | 验证 JWT (HMAC-SHA512) + MAC 地址绑定 |
| `get_current_mac` | `fn() -> Result<String, String>` | 获取本机第一个网络接口的 MAC (大写无分隔符) |
| `generate_device_jwt` | `fn(imei, level) -> Result<String, String>` | 生成设备激活用 JWT (密钥: `AARRIIXXOO22001177`) |

#### data.rs

| 函数 | 签名 | 说明 |
|------|------|------|
| `DataManager::portable` | `fn(exe_dir) -> Self` | 创建便携模式数据管理器 |
| `DataManager::read_json` | `fn(filename) -> Result<T, String>` | 从 `Data/` 目录读取 JSON |
| `DataManager::write_json` | `fn(filename, data) -> Result<(), String>` | 写入 `Data/` 目录 JSON |
| `DataManager::read_config_json` | `fn(filename) -> Result<T, String>` | 从 exe 同级目录读取 JSON |
| `DataManager::write_config_json` | `fn(filename, data) -> Result<(), String>` | 写入 exe 同级目录 JSON |

---

## 6. 调用流程

### 6.1 应用启动流程

```
main.rs::main()
  └── lib.rs::run()
        ├── tauri::Builder::default()
        │     └── .manage(AppState { ... })    ← 初始化全局状态 (全空)
        │     └── .invoke_handler([...])       ← 注册 16 个命令
        └── .run(tauri::generate_context!())   ← 启动 WebView 窗口

WebView 加载 index.html
  └── DOMContentLoaded
        └── initApp()
              ├── invoke('init_app')                    ← 调用 Rust 初始化
              │     ├── std::env::current_exe()         ← 获取 exe 路径
              │     ├── DataManager::portable(exe_dir)  ← 创建数据管理器
              │     ├── load_base_data()                ← 读取 basecfgdata.json
              │     ├── load_product_selection()        ← 读取 selectdata.json
              │     ├── load_execute_data()             ← 读取 execute_sn_data.json
              │     ├── load_device_records()           ← 读取 production_device_data.json
              │     ├── 初始化 CodeSet                  ← 全空 + seq="00001"
              │     ├── 解析 brand/type/fac code        ← 从 base_data 匹配
              │     ├── update_code_set()               ← 设置年月码
              │     ├── 恢复序列号                      ← 从 execute_data 匹配前缀
              │     └── 写入 AppState                   ← 存储所有数据
              │
              ├── invoke('get_base_data')               ← 获取基础配置
              │     └── 填充下拉框 (brands/types/factories)
              │
              ├── invoke('get_current_product')         ← 获取当前产品
              │     └── 设置下拉框选中值
              │
              └── invoke('get_current_sn')              ← 获取当前 SN
                    └── 更新 SN 显示
```

### 6.2 写入 SN 流程

```
前端: 点击 "写入 SN" 按钮
  └── writeSnBtn click handler
        ├── invoke('write_sn_to_device', { sn })
        │     └── DeviceClient::set_serial_no(sn)
        │           ├── send_request("SetSerialNo", { serialno: sn })
        │           │     └── HTTP POST → http://{ip}/arixoapi
        │           │           └── JSON-RPC: {"jsonrpc":"2.0","method":"SetSerialNo",...}
        │           └── get_serial_no()              ← 读取验证
        │                 └── send_request("GetSerialNo", {})
        │
        ├── invoke('write_product_type', { productType })
        │     └── DeviceClient::set_product_id(productType)
        │           ├── send_request("SetProductID2", { productID })
        │           └── get_product_id()             ← 读取验证
        │
        ├── invoke('get_device_info_from_device')
        │     └── DeviceClient::get_device_info()
        │           └── send_request("GetDeviceInfo", {})
        │                 └── 解析 IMEI/ICCID/sn/SwVersion/DeviceName
        │
        ├── invoke('save_execute_data')
        │     └── 查找匹配 prefix 的记录 → 更新或新增
        │           └── DataManager::save_execute_data()
        │                 └── 写入 Data/execute_sn_data.json
        │
        ├── invoke('save_device_record', { deviceInfo })
        │     └── 查找匹配 IMEI 的记录 → 更新或新增
        │           └── DataManager::save_device_records()
        │                 └── 写入 Data/production_device_data.json
        │
        └── invoke('increment_sequence')
              └── seq_code = seq_code + 1
              └── generate_sn()                    ← 生成新 SN
              └── 更新 SN 显示
```

### 6.3 激活设备流程

```
前端: 点击 "激活设备" 按钮
  └── activateBtn click handler
        └── invoke('activate_device')
              ├── DeviceClient::get_device_info()
              │     └── 获取 IMEI
              │
              ├── generate_device_jwt(imei, 1)
              │     ├── Header: {"alg":"HS512"}
              │     ├── Payload: {"iat":timestamp, "imei":"...", "level":1, "iss":"www.arixo.cn"}
              │     └── Key: "AARRIIXXOO22001177"
              │     └── → JWT Token (HS512 签名)
              │
              └── DeviceClient::validate_license(token)
                    └── send_request("ValidateLicense", { licStr: token })
                          └── 返回 result 中无 "code" 字段 → 成功
```

### 6.4 eSIM ICCID 查询流程

```
前端: 点击 "查询 eSIM ICCID" 按钮
  └── queryEsimBtn click handler
        └── invoke('get_esim_iccid')
              ├── DeviceClient::set_sim_slot(2)     ← 切换到 eSIM 卡槽
              │     └── send_request("SetSimSlot", { slot: 2 })
              │
              ├── DeviceClient::get_device_info()   ← 读取 ICCID
              │     └── send_request("GetDeviceInfo", {})
              │
              ├── DeviceClient::set_sim_slot(1)     ← 切回物理 SIM 卡槽
              │     └── send_request("SetSimSlot", { slot: 1 })
              │
              └── 返回 ICCID 前 19 位
                    └── 前端生成查询链接:
                          http://iotv3.iot-chuanglin.com/ptdj/directQueryCard?iccid={iccid}
```

### 6.5 产品切换流程

```
前端: 选择品牌/类型/工厂下拉框
  └── onProductChange()
        └── invoke('set_product', { brand, productType, fac })
              ├── 更新 AppState.current_product
              │
              ├── 从 base_data 解析新码集
              │     ├── brand_code  ← brands 中匹配 name 的 code
              │     ├── type_code   ← product_types 中匹配 name 的 code
              │     └── fac_code    ← factories 中匹配 name 的 code
              │
              ├── update_code_set()                 ← 更新年月码
              │
              ├── DataManager::save_product_selection()
              │     └── 写入 selectdata.json
              │
              └── generate_sn() → 返回新 SN
```

---

## 7. 数据结构说明

### 7.1 配置文件

#### basecfgdata.json — 基础配置

```json
{
  "brands": [
    { "name": "AL", "index": 1, "code": "A" },
    { "name": "DK", "index": 2, "code": "D" },
    { "name": "ODM", "index": 3, "code": "M" }
  ],
  "types": [
    { "name": "D100", "index": 1, "code": "01" },
    { "name": "V100", "index": 25, "code": "25" },
    // ... 共 24 种产品类型
  ],
  "factories": [
    { "name": "北京5层", "index": 1, "code": "1" },
    { "name": "加工厂1", "index": 2, "code": "2" },
    { "name": "加工厂2", "index": 3, "code": "3" }
  ]
}
```

**Rust 映射**: `BaseData { brands: Vec<Item>, product_types: Vec<Item>, factories: Vec<Item> }`

#### selectdata.json — 产品选择

```json
{
  "Brand": "AL",
  "Type": "V100",
  "Fac": "北京5层"
}
```

**Rust 映射**: `Product { brand, product_type, fac }` (字段名首字母大写)

### 7.2 运行时数据

#### execute_sn_data.json — SN 执行记录

```json
{
  "ExeDataList": [
    {
      "DateStr": "2024/3/31 14:56:55",
      "Type": "D320-CNV1",
      "PrefixStr": "A05143",
      "CurretSeqNo": "00013"
    }
  ]
}
```

**说明**: 每个前缀组合 (Brand+Type+Fac+Year+Month) 对应一条记录，记录当前序列号。

**Rust 映射**: `ExecutDataList { exe_data_list: Vec<ExecuteData> }`

#### production_device_data.json — 设备生产记录

```json
{
  "RecordData": [
    {
      "imei": "864742052442318",
      "iccid": "89882110000000883134",
      "sn": "A0114400003",
      "swVersion": "UE1010_RG200UCNV1CA_BR1_R03.02.005",
      "deviceName": "UE1010",
      "timestamp": "2024-04-13 11:32:41"
    }
  ]
}
```

**说明**: 按 IMEI 去重，同一设备多次生产时更新记录。

**Rust 映射**: `DeviceRecords { record_data: Vec<DeviceInfo> }`

### 7.3 核心数据结构

#### CodeSet — SN 码集

| 字段 | 类型 | 示例 | 说明 |
|------|------|------|------|
| `brand_code` | String | "A" | 品牌码 (1-2 字符) |
| `type_code` | String | "25" | 类型码 (2 字符) |
| `fac_code` | String | "1" | 工厂码 (1 字符) |
| `year_code` | String | "6" | 年份码 (年份最后一位) |
| `mon_code` | String | "4" | 月份码 (16 进制大写) |
| `seq_code` | String | "00013" | 序列号 (5 位数字) |

**SN 生成**: `brand_code + type_code + fac_code + year_code + mon_code + seq_code` = 12 位

#### DeviceInfo — 设备信息

| 字段 | 来源 JSON 字段 | 说明 |
|------|---------------|------|
| `imei` | `IMEI` | 设备 IMEI |
| `iccid` | `ICCID` | SIM 卡 ICCID |
| `sn` | `sn` | 设备序列号 |
| `sw_version` | `SwVersion` | 软件版本 |
| `device_name` | `DeviceName` | 设备名称 |
| `timestamp` | (本地生成) | 记录时间 |

#### JsonRpcResponse — JSON-RPC 响应

```json
{
  "jsonrpc": "2.0",
  "result": { "serialno": "A2514900011" },
  "id": "22.01"
}
```

### 7.4 AppState — 全局状态

```rust
pub struct AppState {
    device_client: Mutex<Option<DeviceClient>>,    // 设备 HTTP 客户端
    data_manager: Mutex<Option<DataManager>>,      // 数据持久化管理器
    current_product: Mutex<Product>,               // 当前选择的产品
    current_code_set: Mutex<CodeSet>,              // 当前 SN 码集
    device_records: Mutex<Option<DeviceRecords>>,  // 设备生产记录
    execute_data: Mutex<Option<ExecutDataList>>,   // SN 执行记录
    base_data: Mutex<Option<BaseData>>,            // 基础配置数据
}
```

所有字段使用 `Mutex` 保护，因为 Tauri 命令在多线程环境中执行。

---

## 8. 激活算法说明

### 8.1 概述

设备激活采用 **JWT (JSON Web Token)** 机制，使用 HMAC-SHA512 签名算法。激活流程包含两个方向:

1. **本地 License 验证**: 验证 `lic.dat` 中的 Token 是否绑定当前机器 MAC 地址
2. **设备端激活**: 生成包含设备 IMEI 的 JWT，发送到设备端验证

### 8.2 本地 License 验证

#### 流程

```
启动时 (可选)
  └── check_license()
        └── read_license_file(exe_dir)
              └── 读取 lic.dat → { "token": "eyJ..." }
        └── validate_license(token)
              ├── 使用密钥 "PPRROODDUUCCTT123456" 创建 DecodingKey
              ├── 设置算法为 HS512，跳过 exp 验证
              ├── decode::<LicenseClaims>(token)
              │     └── 解析 JWT Payload:
              │           {
              │             "mac": "AABBCCDDEEFF",    // 绑定的 MAC 地址
              │             "level": "admin",           // 权限级别 (可选)
              │             "exp": 1735689600           // 过期时间 (可选)
              │           }
              └── get_current_mac()
                    └── mac_address::get_mac_address()
                          └── 获取第一个网络接口的 MAC
                    └── 格式化为大写无分隔符: "AABBCCDDEEFF"
              └── 比较 claims.mac == current_mac
                    ├── 匹配 → 验证通过
                    └── 不匹配 → 返回 "MAC address mismatch" 错误
```

#### JWT 结构

```
Header:  {"alg": "HS512"}
Payload: {"mac": "AABBCCDDEEFF", "level": "admin", "exp": 1735689600}
Signature: HMAC-SHA512(Header.Payload, "PPRROODDUUCCTT123456")
```

### 8.3 设备端激活

#### 流程

```
前端: 点击 "激活设备"
  └── activate_device()
        ├── get_device_info()
        │     └── 获取设备 IMEI (如 "864742052442318")
        │
        ├── generate_device_jwt(imei, level=1)
        │     ├── Header:  {"alg": "HS512"}
        │     ├── Payload: {
        │     │              "iat": 1713600000,          // 当前 Unix 时间戳
        │     │              "imei": "864742052442318",  // 设备 IMEI
        │     │              "level": 1,                 // 权限级别
        │     │              "iss": "www.arixo.cn"       // 签发者
        │     │            }
        │     └── Key: "AARRIIXXOO22001177"
        │     └── → JWT Token (HS512 签名)
        │
        └── validate_license(token) → 发送到设备
              └── send_request("ValidateLicense", { licStr: token })
                    └── HTTP POST → http://{ip}/arixoapi
                          └── 设备端验证 JWT 签名和 IMEI
                          └── 返回结果:
                                ├── 无 "code" 字段 → 激活成功
                                └── 有 "code" 字段 → 激活失败 (返回错误码)
```

#### 设备端 JWT 结构

```
Header:  {"alg": "HS512"}
Payload: {"iat": <timestamp>, "imei": "<device_imei>", "level": 1, "iss": "www.arixo.cn"}
Signature: HMAC-SHA512(Header.Payload, "AARRIIXXOO22001177")
```

### 8.4 密钥说明

| 用途 | 密钥 | 算法 |
|------|------|------|
| License 验证 (本地) | `PPRROODDUUCCTT123456` | HMAC-SHA512 |
| 设备激活 (发送到设备) | `AARRIIXXOO22001177` | HMAC-SHA512 |

### 8.5 MAC 地址获取

```rust
mac_address::get_mac_address()
  ├── Windows: GetAdaptersAddresses API
  ├── Linux:   /sys/class/net/*/address
  └── macOS:   ioctl SIOCGIFHWADDR

返回第一个非回环网络接口的 MAC 地址
格式: MacAddress { bytes: [u8; 6] }
显示: "AA:BB:CC:DD:EE:FF" (Display trait)
验证: "AABBCCDDEEFF" (大写无分隔符)
```

### 8.6 安全考虑

| 项目 | 说明 |
|------|------|
| **JWT 密钥** | 硬编码在源码中，编译后存在于二进制文件中 |
| **MAC 绑定** | 防止 License 文件在不同机器间复制使用 |
| **证书验证** | 设备 API 通信禁用证书验证 (`danger_accept_invalid_certs`) |
| **Token 过期** | 本地 License 验证跳过 `exp` 检查，设备端可能检查 |
