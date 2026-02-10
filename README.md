# 智能助手：企业级应用聚合与统一安装（Rust）

本仓库提供一套“单一安装程序 + 统一桌面入口 + 插件化聚合”的参考实现，用于将 HUES、iHaier、智能助手、VDI 等独立系统以**模块化子包**方式组合交付，并在安装后仅保留“智能助手”桌面快捷方式。

## 目录结构

- `crates/xiaohai-bootstrapper`：统一安装/卸载引导程序（支持静默模式、依赖检测、模块安装顺序编排、快捷方式治理、服务/防火墙配置）
- `crates/xiaohai-assistant`：统一启动入口（GUI），动态加载 `plugins/*.json` 插件并启动各应用
- `crates/xiaohai-core`：清单/插件/IPC/SSO Token 协议与通用路径定义
- `crates/xiaohai-windows`：Windows 专用能力（注册表检测、快捷方式 COM、DPAPI、服务、进程状态、netsh 防火墙）
- `bundle-manifest.json`：统一安装清单（模块、依赖、快捷方式、服务、网络/路径等）

## 快速开始（开发态）

1. 构建

```bash
cargo build -p xiaohai-bootstrapper
cargo build -p xiaohai-assistant
```

2. 安装（需要管理员权限；静默安装用 `--silent`）

```bash
target\\debug\\xiaohai-bootstrapper.exe --manifest bundle-manifest.json install --silent
```

3. 启动统一入口

```bash
target\\debug\\xiaohai-assistant.exe
```

## 交付打包建议

本仓库的“单一安装程序”核心是 `xiaohai-bootstrapper`，建议最终交付时：
- 将各组件安装包/离线依赖放入 `payload/` 并与 bootstrapper 同目录交付，或进一步将 payload 作为资源嵌入到 bootstrapper 中（后续可扩展）。
- 每个组件通过 `bundle-manifest.json` 声明：安装方式（MSI/EXE/FileCopy）、静默参数、检测规则、卸载方式、桌面图标治理、插件注册与安装后配置。

