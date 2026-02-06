//! 注册表读写与依赖检测。
//!
//! 主要用途：
//! - 根据清单中的注册表检测规则判断组件是否已安装
//! - 检测常见前置依赖（.NET Framework 4.8、VC++ 运行库）
//! - 写入/删除 Windows 登录自启动项（HKLM Run）
//!
//! 权限要求：
//! - 读取大多数系统键通常不需要管理员，但某些机器策略可能限制
//! - 写入 HKLM Run 通常需要管理员权限
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use anyhow::{Context, Result};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;
use xiaohai_core::manifest::{RegistryExpectedValue, RegistryHive, RegistryValueKind, RegistryValueRule};

/// 按清单规则检测注册表值是否满足期望。
///
/// 参数：
/// - `rule`：注册表检测规则（根键、子键路径、值名、类型、期望值）
///
/// 返回值：
/// - `Ok(true)`：满足期望（视为“已安装/已配置”）
/// - `Ok(false)`：不满足期望
///
/// 异常处理：
/// - 打开键或读取值失败会返回错误（常见原因：权限不足、键不存在、类型不匹配）。
pub fn detect_registry_rule(rule: &RegistryValueRule) -> Result<bool> {
    let root = match rule.hive {
        RegistryHive::Hklm => RegKey::predef(HKEY_LOCAL_MACHINE),
        RegistryHive::Hkcu => RegKey::predef(HKEY_CURRENT_USER),
    };
    let key = root.open_subkey(&rule.key).with_context(|| format!("打开注册表键失败: {}\\{}", hive_name(rule.hive), rule.key))?;
    match rule.kind {
        RegistryValueKind::Dword => {
            let v: u32 = key.get_value(&rule.value_name).with_context(|| format!("读取 DWORD 失败: {}", rule.value_name))?;
            Ok(match &rule.expected {
                RegistryExpectedValue::DwordAtLeast(min) => v >= *min,
                RegistryExpectedValue::DwordEquals(eq) => v == *eq,
                RegistryExpectedValue::SzEquals(_) => false,
            })
        }
        RegistryValueKind::Sz => {
            let v: String = key.get_value(&rule.value_name).with_context(|| format!("读取 SZ 失败: {}", rule.value_name))?;
            Ok(match &rule.expected {
                RegistryExpectedValue::SzEquals(eq) => v == *eq,
                RegistryExpectedValue::DwordAtLeast(_) | RegistryExpectedValue::DwordEquals(_) => false,
            })
        }
    }
}

/// 将 [`RegistryHive`] 转换为可读字符串（用于错误信息）。
///
/// 参数：
/// - `h`：根键枚举
///
/// 返回值：
/// - `"HKLM"` 或 `"HKCU"`
fn hive_name(h: RegistryHive) -> &'static str {
    match h {
        RegistryHive::Hklm => "HKLM",
        RegistryHive::Hkcu => "HKCU",
    }
}

/// 检测 .NET Framework 4.8 是否已安装。
///
/// 检测逻辑：
/// - 读取 `HKLM\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full` 的 `Release` 值
/// - Release >= 528040 视为已安装（微软官方定义）
///
/// 异常处理：
/// - 键或值不存在/读取失败会返回错误。
pub fn detect_dotnet_fx48_installed() -> Result<bool> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey("SOFTWARE\\Microsoft\\NET Framework Setup\\NDP\\v4\\Full")
        .context("打开 .NET Framework v4\\Full 注册表键失败")?;
    let release: u32 = key.get_value("Release").context("读取 .NET Release 值失败")?;
    Ok(release >= 528040)
}

/// 检测 VC++ 2015-2022 (x64) 运行库是否已安装。
///
/// 检测逻辑：
/// - 读取 `HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64` 的 `Installed` 值
/// - Installed == 1 视为已安装
///
/// 异常处理：
/// - 键或值不存在/读取失败会返回错误。
pub fn detect_vcredist_2015_2022_x64_installed() -> Result<bool> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey("SOFTWARE\\Microsoft\\VisualStudio\\14.0\\VC\\Runtimes\\x64")
        .context("打开 VC++ Runtime x64 注册表键失败")?;
    let installed: u32 = key.get_value("Installed").context("读取 Installed 值失败")?;
    Ok(installed == 1)
}

/// 写入 Windows 登录自启动项（HKLM Run）。
///
/// 参数：
/// - `name`：注册表值名（建议使用产品标识）
/// - `command`：启动命令（通常包含引号包裹的 exe 路径与参数）
///
/// 异常处理：
/// - 打开/创建键或写入值失败会返回错误（常见原因：权限不足）。
pub fn set_hklm_run(name: &str, command: &str) -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _disp) = hklm
        .create_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run")
        .context("打开/创建 HKLM Run 键失败")?;
    key.set_value(name, &command)
        .with_context(|| format!("写入 HKLM Run 值失败: {name}"))?;
    Ok(())
}

/// 删除 Windows 登录自启动项（HKLM Run）。
///
/// 参数：
/// - `name`：注册表值名
///
/// 异常处理：
/// - 打开键失败会返回错误（常见原因：权限不足/键不存在）
/// - 删除值失败会被忽略（值不存在时视为已删除）
pub fn delete_hklm_run(name: &str) -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey_with_flags(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
            winreg::enums::KEY_WRITE,
        )
        .context("打开 HKLM Run 键失败")?;
    let _ = key.delete_value(name);
    Ok(())
}

