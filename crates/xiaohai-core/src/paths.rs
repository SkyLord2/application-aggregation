//! 统一路径与目录约定（主要面向 Windows ProgramData）。
//!
//! 目标：
//! - 将落盘路径集中管理，避免散落在各模块中
//! - 统一插件目录、数据目录与状态文件路径，便于企业部署与排障
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

/// ProgramData 下的供应商/产品顶层目录名。
///
/// 示例（默认）：
/// - `%ProgramData%\XiaoHaiAssistant`
pub const VENDOR_DIR: &str = "XiaoHaiAssistant";

/// 获取本项目在 ProgramData 下的根目录。
///
/// 返回值：
/// - 成功：`%ProgramData%\XiaoHaiAssistant`
///
/// 异常处理：
/// - 当环境变量 `ProgramData` 不存在或不可读时，返回错误。
pub fn program_data_dir() -> Result<PathBuf> {
    let program_data = std::env::var("ProgramData").context("读取 ProgramData 环境变量失败")?;
    Ok(PathBuf::from(program_data).join(VENDOR_DIR))
}

/// 确保目录存在（不存在则递归创建）。
///
/// 参数：
/// - `path`：目标目录路径
///
/// 异常处理：
/// - 目录创建失败（权限、路径非法等）会返回错误。
pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("创建目录失败: {}", path.display()))?;
    Ok(())
}

/// 默认数据根目录。
///
/// 返回值：
/// - `%ProgramData%\XiaoHaiAssistant\data`
pub fn default_data_root() -> Result<PathBuf> {
    Ok(program_data_dir()?.join("data"))
}

/// 默认插件目录。
///
/// 返回值：
/// - `%ProgramData%\XiaoHaiAssistant\plugins`
pub fn default_plugin_dir() -> Result<PathBuf> {
    Ok(program_data_dir()?.join("plugins"))
}

/// 默认安装状态文件路径。
///
/// 返回值：
/// - `%ProgramData%\XiaoHaiAssistant\install-state.json`
pub fn default_state_file() -> Result<PathBuf> {
    Ok(program_data_dir()?.join("install-state.json"))
}

/// 将清单中的路径字段解析为实际路径。
///
/// 参数：
/// - `base`：相对路径的基准目录（通常是清单文件所在目录或安装根目录）
/// - `raw`：清单中的路径字符串
///
/// 返回值：
/// - `raw` 为绝对路径：直接返回
/// - `raw` 为相对路径：返回 `base.join(raw)`
///
/// 异常处理：
/// - `raw` 为空字符串时返回错误，避免误用导致写入基准目录本身。
pub fn resolve_path(base: &Path, raw: &str) -> Result<PathBuf> {
    if raw.is_empty() {
        return Err(anyhow!("空路径"));
    }
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        Ok(p)
    } else {
        Ok(base.join(p))
    }
}
