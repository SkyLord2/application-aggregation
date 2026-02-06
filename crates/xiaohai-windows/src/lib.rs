//! Windows 平台能力封装（注册表、快捷方式、DPAPI、服务、防火墙等）。
//!
//! 目标：
//! - 将 Windows 专有 API 与系统操作集中封装，避免上层业务代码直接依赖 Win32 细节
//! - 统一错误处理风格（以 `anyhow::Result` 形式向上返回）
//!
//! 安全注意：
//! - 涉及注册表/服务/防火墙等操作通常需要管理员权限
//! - 涉及密钥时应使用 DPAPI 等系统能力保护落盘数据
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

pub mod dpapi;
pub mod elevation;
pub mod firewall;
pub mod prereq;
pub mod process;
pub mod registry;
pub mod service;
pub mod shortcut;
