//! 小海智能助手核心库（跨平台/业务无关）。
//!
//! 功能：
//! - 定义安装清单（bundle-manifest.json）与插件注册模型
//! - 定义安装状态落盘模型（install-state.json）
//! - 定义本机 IPC 请求/响应协议与单点登录（SSO）令牌格式
//! - 提供统一路径与目录约定（ProgramData 等）
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

pub mod auth;
pub mod ipc;
pub mod manifest;
pub mod paths;
pub mod state;
