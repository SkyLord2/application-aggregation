//! 提权/权限相关检测。
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use anyhow::Result;
use windows::Win32::UI::Shell::IsUserAnAdmin;

/// 判断当前进程是否以管理员权限运行。
///
/// 返回值：
/// - `Ok(true)`：当前为管理员
/// - `Ok(false)`：当前非管理员
///
/// 异常处理：
/// - 该 Win32 API 本身不返回错误码；此处保留 `Result` 以统一上层调用风格。
///
/// 安全注意：
/// - 该检查仅用于“是否应继续执行需要管理员权限的系统修改”，不能作为完整的安全边界。
pub fn is_running_as_admin() -> Result<bool> {
    unsafe { Ok(IsUserAnAdmin().as_bool()) }
}

