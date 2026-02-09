//! Windows 防火墙规则管理（基于 `netsh`）。
//!
//! 说明：
//! - 使用 `netsh advfirewall` 创建/删除规则，避免直接绑定复杂 Win32 防火墙 COM API
//! - 适合企业部署场景，便于排障（命令行输出可直接复现）
//!
//! 权限要求：
//! - 需要管理员权限
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use std::process::Command;

use anyhow::{anyhow, Context, Result};
use xiaohai_core::manifest::{FirewallAction, FirewallDirection, FirewallProfile, FirewallRule};

/// 创建一条防火墙规则。
///
/// 参数：
/// - `rule`：规则定义（名称、方向、动作、程序路径、profile）
///
/// 异常处理：
/// - `netsh` 启动失败/退出码非 0 会返回错误，并附带 stdout/stderr 便于排障。
pub fn add_rule(rule: &FirewallRule) -> Result<()> {
    let dir = match rule.direction {
        FirewallDirection::In => "in",
        FirewallDirection::Out => "out",
    };
    let action = match rule.action {
        FirewallAction::Allow => "allow",
        FirewallAction::Block => "block",
    };
    let profile = match rule.profile {
        FirewallProfile::Any => "any",
        FirewallProfile::Domain => "domain",
        FirewallProfile::Private => "private",
        FirewallProfile::Public => "public",
    };

    run_netsh(&[
        "advfirewall",
        "firewall",
        "add",
        "rule",
        &format!("name={}", rule.name),
        &format!("dir={dir}"),
        &format!("action={action}"),
        &format!("program={}", rule.program),
        "enable=yes",
        &format!("profile={profile}"),
    ])
}

/// 删除指定名称的防火墙规则。
///
/// 参数：
/// - `rule_name`：规则名称（与创建时一致）
///
/// 异常处理：
/// - `netsh` 启动失败/退出码非 0 会返回错误，并附带 stdout/stderr。
pub fn delete_rule(rule_name: &str) -> Result<()> {
    run_netsh(&[
        "advfirewall",
        "firewall",
        "delete",
        "rule",
        &format!("name={rule_name}"),
    ])
}

/// 执行 `netsh` 子命令并将错误输出汇总为 `anyhow::Error`。
///
/// 参数：
/// - `args`：netsh 参数数组（不包含程序名）
///
/// 异常处理：
/// - 启动失败：返回错误（通常是系统缺失或权限问题）
/// - 执行失败：返回错误并携带 stdout/stderr，便于日志与人工复现
fn run_netsh(args: &[&str]) -> Result<()> {
    let out = Command::new("netsh")
        .args(args)
        .output()
        .context("执行 netsh 失败")?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    Err(anyhow!(
        "netsh 执行失败: {}\n{}\n{}",
        out.status,
        stdout,
        stderr
    ))
}
