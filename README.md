# FactoryTool

5G 工厂序列号管理工具 — 跨平台桌面应用

## 技术栈

- **前端**: 原生 HTML/CSS/JS (无框架)
- **后端**: Rust + Tauri 2
- **通信**: Tauri IPC (`invoke`)
- **HTTP**: reqwest (JSON-RPC 2.0)
- **JWT**: jsonwebtoken (HMACSHA512)

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
- 设备 SN/型号写入 (JSON-RPC)
- 设备信息查询与激活
- eSIM ICCID 查询
- 生产记录管理
- JWT License 验证 (MAC 绑定)

## 平台支持

- Windows 10/11
- macOS 12+
- Ubuntu 20.04+

## 项目结构

```
├── src/
│   └── index.html          # 前端 UI (单文件)
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs          # Tauri 命令入口
│   │   ├── main.rs         # 应用入口
│   │   ├── api_client.rs   # JSON-RPC 客户端
│   │   ├── sn_generator.rs # SN 生成逻辑
│   │   ├── license.rs      # License 验证
│   │   ├── data.rs         # 数据持久化
│   │   └── types.rs        # 类型定义
│   ├── Cargo.toml
│   └── tauri.conf.json
└── package.json
```

## 从 WinForms 迁移说明

本项目是从 C# WinForms (.NET Framework 4.7.2) 重构而来，保留了原有业务逻辑:
- SN 生成算法完全一致
- JSON-RPC 协议不变
- License 验证逻辑相同
- 数据文件格式兼容
