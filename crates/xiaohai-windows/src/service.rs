//! Windows 服务安装/卸载封装（基于 `windows-service` crate）。
//!
//! 用途：
//! - 为“后台守护进程/代理（agent）”提供企业部署所需的服务化能力
//! - 与 bootstrapper 配合：安装时创建服务，卸载时删除服务
//!
//! 权限要求：
//! - 创建/删除服务通常需要管理员权限
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use std::ffi::OsString;

use anyhow::{Context, Result};
use windows_service::service::{
    ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

/// 安装或更新 Windows 服务。
///
/// 参数：
/// - `service_name`：服务名（唯一标识）
/// - `display_name`：显示名
/// - `description`：描述（为空则不设置）
/// - `exe`：服务可执行文件路径
/// - `args`：服务启动参数
///
/// 异常处理：
/// - 打开服务管理器失败：返回错误
/// - 创建失败：返回错误；若错误码为 1073（服务已存在），则改为“打开并更新描述”
pub fn install_service(
    service_name: &str,
    display_name: &str,
    description: &str,
    exe: &str,
    args: &[String],
) -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access).context("打开 ServiceManager 失败")?;

    let mut launch_arguments: Vec<OsString> = Vec::new();
    for a in args {
        launch_arguments.push(OsString::from(a));
    }

    let service_info = ServiceInfo {
        name: OsString::from(service_name),
        display_name: OsString::from(display_name),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe.into(),
        launch_arguments,
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };

    let service = service_manager
        .create_service(&service_info, ServiceAccess::CHANGE_CONFIG)
        .or_else(|e| match e {
            windows_service::Error::Winapi(e) if e.raw_os_error() == Some(1073) => {
                // 1073 = ERROR_SERVICE_EXISTS：允许幂等安装（重复执行 install 时更新描述等信息）。
                Ok(service_manager.open_service(service_name, ServiceAccess::CHANGE_CONFIG)?)
            }
            other => Err(other),
        })
        .context("创建/打开服务失败")?;

    if !description.is_empty() {
        service.set_description(description).context("设置服务描述失败")?;
    }
    Ok(())
}

/// 卸载 Windows 服务。
///
/// 参数：
/// - `service_name`：服务名
///
/// 异常处理：
/// - 打开服务或删除服务失败时返回错误（通常是权限不足或服务不存在/被占用）。
pub fn uninstall_service(service_name: &str) -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access).context("打开 ServiceManager 失败")?;
    let service = service_manager
        .open_service(service_name, ServiceAccess::DELETE)
        .with_context(|| format!("打开服务失败: {service_name}"))?;
    service.delete().with_context(|| format!("删除服务失败: {service_name}"))?;
    Ok(())
}

