//! 进程状态检测（用于统一入口展示“运行中/未运行”）。
//!
//! 实现策略：
//! - 当前实现按可执行文件名进行匹配（忽略路径）
//! - 该策略适合企业套件中“文件名唯一”的场景；如存在同名进程，建议升级为 PID 记录或完整路径校验
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use std::path::Path;

use anyhow::Result;
use sysinfo::{ProcessRefreshKind, RefreshKind, System};

/// 判断指定可执行文件对应的进程是否正在运行。
///
/// 参数：
/// - `exe_path`：目标可执行文件路径（用于提取文件名）
///
/// 返回值：
/// - `Ok(true)`：检测到同名进程正在运行
/// - `Ok(false)`：未检测到
///
/// 异常处理：
/// - 当前实现理论上不会返回错误（sysinfo API 本身不抛错）；保留 `Result` 以统一上层接口
///
/// 限制：
/// - 仅按文件名匹配，无法区分不同路径的同名进程
pub fn is_process_running_by_exe(exe_path: &Path) -> Result<bool> {
    let mut system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    system.refresh_processes();
    let needle = exe_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if needle.is_empty() {
        return Ok(false);
    }
    for (_pid, proc_) in system.processes() {
        let name = proc_.name().to_ascii_lowercase();
        if name == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

