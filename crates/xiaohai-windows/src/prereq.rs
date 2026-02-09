//! 前置依赖检测（基于注册表）。
//!
//! 说明：
//! - 本模块只负责“检测是否安装”，不负责安装本身；安装由 bootstrapper 按清单执行。
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use anyhow::Result;

use crate::registry;

/// 前置依赖是否已安装。
#[derive(Debug, Clone, Copy)]
pub enum PrereqStatus {
    /// 已安装。
    Installed,
    /// 未安装。
    Missing,
}

/// 检测 .NET Framework 4.8 是否已安装。
///
/// 返回值：
/// - `Installed`：检测到已安装
/// - `Missing`：未检测到安装
///
/// 异常处理：
/// - 注册表读取失败时返回错误（常见原因：权限不足）。
pub fn dotnet_fx48_status() -> Result<PrereqStatus> {
    Ok(if registry::detect_dotnet_fx48_installed()? {
        PrereqStatus::Installed
    } else {
        PrereqStatus::Missing
    })
}

/// 检测 VC++ 2015-2022 x64 运行库是否已安装。
///
/// 返回值：
/// - `Installed`：检测到已安装
/// - `Missing`：未检测到安装
///
/// 异常处理：
/// - 注册表读取失败时返回错误（常见原因：权限不足）。
pub fn vcredist_2015_2022_x64_status() -> Result<PrereqStatus> {
    Ok(if registry::detect_vcredist_2015_2022_x64_installed()? {
        PrereqStatus::Installed
    } else {
        PrereqStatus::Missing
    })
}
