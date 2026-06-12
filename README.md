# FactoryTool

5G 工厂序列号管理工具 — 跨平台桌面应用

## 技术栈

- **前端**: 原生 HTML/CSS/JS (无框架)
- **后端**: Rust + Tauri 2
- **通信**: Tauri IPC (`invoke`)
- **HTTP**: reqwest (REST, `http://{ip}/api`)
- **JWT**: jsonwebtoken (HMACSHA512)
- **刷机**: 32 位 `r26-cli` sidecar (Unisoc PAC)

## 快速开始

```bash
# 安装依赖
bun install

# 开发模式
bun run dev

# 构建生产版本
bun run build
```

## 功能

- 产品选择与 SN 自动生成
- 设备 SN/型号写入 (REST)
- 设备信息查询与激活
- eSIM ICCID 查询
- 生产记录管理
- JWT License 验证 (MAC 绑定)
- 固件下载 (Unisoc PAC 刷机，射频校准默认保护)

## 平台支持

- Windows 10/11
- macOS 12+
- Ubuntu 20.04+

## 项目结构

```
├── src/
│   ├── index.html          # 前端 UI (标记 + JS)
│   ├── styles.css          # 主题样式 (与 modem-cat 风格一致)
│   └── fonts/              # Inter / JetBrains Mono 本地字体
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs          # Tauri 命令入口
│   │   ├── main.rs         # 应用入口
│   │   ├── dloader.rs      # 固件下载 (r26-cli sidecar 桥接 + 安全策略)
│   │   ├── api_client.rs   # REST 客户端
│   │   ├── sn_generator.rs # SN 生成逻辑
│   │   ├── license.rs      # License 验证
│   │   ├── data.rs         # 数据持久化
│   │   └── types.rs        # 类型定义
│   ├── binaries/           # 内置 32 位 r26-cli sidecar + 版本印记
│   ├── capabilities/       # Webview 权限声明
│   ├── Cargo.toml
│   └── tauri.conf.json
├── scripts/
│   └── update-dloader-cli.ps1  # 从 r26-dloader 仓库重新构建并同步 sidecar
└── package.json
```

## 从 WinForms 迁移说明

本项目是从 C# WinForms (.NET Framework 4.7.2) 重构而来，保留了原有业务逻辑:
- SN 生成算法完全一致
- 设备 REST 协议不变
- License 验证逻辑相同
- 数据文件格式兼容
