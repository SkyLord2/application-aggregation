//! 后台代理（Windows Service 形态，预留扩展）。
//!
//! 目标：
//! - 为企业交付提供“后台常驻能力”载体（例如：健康监控、自动修复、策略下发等）
//! - 与 bootstrapper 配合：由安装程序创建/删除服务
//!
//! 当前状态：
//! - 仅提供服务框架与可停止的空循环（占位实现）
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use tracing::info;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

/// 运行参数。
///
/// 说明：
/// - `--run-console`：以控制台模式运行（用于开发调试）
/// - `--service-name`：服务名（与安装时保持一致）
#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value_t = false)]
    run_console: bool,

    #[arg(long, default_value = "XiaoHaiAssistantAgent")]
    service_name: String,
}

/// 程序入口：根据参数选择控制台模式或服务模式启动。
///
/// 异常处理：
/// - 服务调度器启动失败会返回错误
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .with_target(false)
        .init();

    let args = Args::parse();
    if args.run_console {
        run_agent_loop()?;
        return Ok(());
    }

    SERVICE_NAME.set(args.service_name).ok();
    service_dispatcher::start(SERVICE_NAME.get().unwrap(), ffi_service_main)?;
    Ok(())
}

/// 服务名（由命令行参数注入，供 `service_dispatcher` 回调使用）。
static SERVICE_NAME: once_cell::sync::OnceCell<String> = once_cell::sync::OnceCell::new();

/// 服务停止信号（由 SCM 下发 Stop 控制码触发）。
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

define_windows_service!(ffi_service_main, my_service_main);

/// Windows Service 入口（由 `service_dispatcher` 调用）。
///
/// 注意：
/// - 该函数签名由宏固定；真实逻辑在 [`run_service`]。
fn my_service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        let _ = e;
    }
}

/// 服务主流程：注册控制处理器、上报运行状态、进入主循环，退出后上报停止状态。
///
/// 异常处理：
/// - 注册/上报状态失败会返回错误（通常为服务环境异常）
fn run_service() -> Result<()> {
    let service_name = SERVICE_NAME.get().map(|s| s.as_str()).unwrap_or("XiaoHaiAssistantAgent");

    let status_handle = service_control_handler::register(service_name, move |control_event| match control_event {
        ServiceControl::Stop => {
            // SCM 请求停止：通过原子标志通知主循环退出。
            STOP_REQUESTED.store(true, Ordering::SeqCst);
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    run_agent_loop()?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

/// 代理主循环（占位实现）。
///
/// 行为：
/// - 每 30 秒打点一次（示例）
/// - 当收到服务停止信号后退出
fn run_agent_loop() -> Result<()> {
    info!("xiaohai-agent running");
    loop {
        if STOP_REQUESTED.load(Ordering::SeqCst) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(30));
    }
}
