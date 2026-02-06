#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! 小海智能助手（统一入口 GUI）。
//!
//! 职责：
//! - 从 ProgramData 插件目录动态加载应用插件（`plugins/*.json`）
//! - 为各插件提供“统一启动入口”，并在 UI 中展示运行状态
//! - 启动本机 IPC 服务：签发/校验 SSO 令牌、查询应用状态
//!
//! 安全注意：
//! - IPC 当前实现为 127.0.0.1 TCP，仅用于本机；企业交付建议升级为 Named Pipe + ACL
//! - SSO 签名密钥使用 DPAPI(LocalMachine) 保护落盘
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use eframe::egui;
use rand::RngCore;
use time::Duration;
use tracing::{info, warn};
use uuid::Uuid;
use xiaohai_core::auth::{TokenIssuer, TokenClaims};
use xiaohai_core::ipc::{IpcRequest, IpcResponse};
use xiaohai_core::paths;
use xiaohai_core::state::InstallState;
use xiaohai_windows::{dpapi, process};

/// 插件文件的落盘结构。
///
/// 说明：
/// - `module_id` 用于标识该插件属于哪个安装模块
/// - `plugin` 使用 `flatten`，使 JSON 结构更扁平，便于人工维护
#[derive(Debug, Clone, serde::Deserialize)]
struct PluginFile {
    module_id: String,
    #[serde(flatten)]
    plugin: xiaohai_core::manifest::PluginRegistration,
}

/// 已加载的插件（带来源文件路径）。
#[derive(Debug, Clone)]
struct LoadedPlugin {
    module_id: String,
    plugin: xiaohai_core::manifest::PluginRegistration,
    file_path: PathBuf,
}

/// 程序入口：初始化日志、加载安装状态、启动 IPC 服务并启动 GUI。
///
/// 异常处理：
/// - 关键步骤（状态文件读取/密钥读取/IPC 启动/GUI 启动）失败会返回错误
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .with_target(false)
        .init();

    let install_state = load_install_state().ok();
    let install_root = install_state
        .as_ref()
        .and_then(|s| s.modules.iter().find_map(|m| m.install_root.clone()))
        .map(PathBuf::from)
        .unwrap_or_else(|| current_exe_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let secret = load_or_create_auth_secret()?;
    let issuer = TokenIssuer::new(secret, install_state.as_ref().map(|s| s.product_code.clone()).unwrap_or_else(|| "xiaohai".to_string()));

    let server = IpcServer::start(issuer.clone())?;
    info!("IPC server listening on {}", server.addr);

    let app_state = AppState::new(install_root, server.addr, issuer);
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "小海智能助手",
        options,
        Box::new(|_cc| Box::new(app_state)),
    )
    .map_err(|e| anyhow::anyhow!("启动 GUI 失败: {e}"))?;
    Ok(())
}

/// 读取安装状态文件（install-state.json）。
///
/// 返回值：
/// - 成功：返回 [`InstallState`]
///
/// 异常处理：
/// - 文件不存在/读取失败/解析失败会返回错误
fn load_install_state() -> Result<InstallState> {
    let path = paths::default_state_file()?;
    let bytes = std::fs::read(&path).with_context(|| format!("读取状态文件失败: {}", path.display()))?;
    Ok(serde_json::from_slice(&bytes).context("解析状态文件失败")?)
}

/// 获取当前可执行文件所在目录。
///
/// 返回值：
/// - 成功：返回 exe 所在目录
///
/// 异常处理：
/// - 无法获取当前 exe 路径时返回错误
fn current_exe_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("读取当前可执行文件路径失败")?;
    Ok(exe.parent().unwrap_or_else(|| Path::new(".")).to_path_buf())
}

/// 加载或生成 SSO 签名密钥，并使用 DPAPI(LocalMachine) 保护落盘。
///
/// 返回值：
/// - 成功：返回明文密钥字节（仅用于进程内 HMAC）
///
/// 异常处理：
/// - ProgramData 目录创建失败/文件读写失败/DPAPI 解密失败会返回错误
///
/// 安全注意：
/// - 密钥明文只在内存中使用，不应写日志
fn load_or_create_auth_secret() -> Result<Vec<u8>> {
    let base = paths::program_data_dir()?;
    paths::ensure_dir(&base)?;
    let file = base.join("auth-secret.bin");
    if file.exists() {
        let cipher = std::fs::read(&file).context("读取 auth-secret.bin 失败")?;
        return dpapi::unprotect_local_machine(&cipher).context("解密 auth-secret.bin 失败");
    }
    let mut secret = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut secret);
    let cipher = dpapi::protect_local_machine(&secret).context("加密 auth secret 失败")?;
    std::fs::write(&file, cipher).context("写入 auth-secret.bin 失败")?;
    Ok(secret)
}

/// IPC 服务句柄。
///
/// 说明：
/// - `addr`：监听地址（当前为本机回环随机端口）
/// - `_join`：后台线程句柄（保持线程生命周期）
struct IpcServer {
    addr: SocketAddr,
    _join: std::thread::JoinHandle<()>,
}

impl IpcServer {
    /// 启动 IPC 服务并返回句柄。
    ///
    /// 参数：
    /// - `issuer`：SSO 令牌签发器（用于处理 GetSsoToken 请求）
    ///
    /// 返回值：
    /// - 成功：返回服务句柄（包含监听地址）
    ///
    /// 异常处理：
    /// - Tokio Runtime 创建失败、端口绑定失败等会返回错误
    fn start(issuer: TokenIssuer) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new().context("创建 Tokio Runtime 失败")?;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").context("绑定 IPC 端口失败")?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let join = std::thread::spawn(move || {
            let _ = rt.block_on(async move { run_ipc_loop(listener, issuer).await });
        });
        Ok(Self { addr, _join: join })
    }
}

/// IPC 监听主循环：接收连接并为每个连接启动异步任务。
///
/// 参数：
/// - `listener`：标准库 TcpListener（会转换为 tokio listener）
/// - `issuer`：令牌签发器
///
/// 异常处理：
/// - `accept()` 失败会直接向上传播（通常为系统资源问题）
async fn run_ipc_loop(listener: std::net::TcpListener, issuer: TokenIssuer) -> Result<()> {
    let listener = tokio::net::TcpListener::from_std(listener).context("转换 TcpListener 失败")?;
    loop {
        let (mut stream, _addr) = listener.accept().await?;
        let issuer = issuer.clone();
        tokio::spawn(async move {
            let (reader, mut writer) = stream.split();
            let mut reader = tokio::io::BufReader::new(reader);
            let mut line = String::new();
            loop {
                line.clear();
                let n = match tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line).await {
                    Ok(n) => n,
                    Err(_) => return,
                };
                if n == 0 {
                    return;
                }
                // 协议采用“单行一条 JSON”，便于调试与跨语言实现。
                let req: IpcRequest = match serde_json::from_str(line.trim()) {
                    Ok(v) => v,
                    Err(e) => {
                        let resp = IpcResponse::Error {
                            request_id: Uuid::nil(),
                            message: format!("bad request: {e}"),
                        };
                        let _ = write_resp(&mut writer, &resp).await;
                        continue;
                    }
                };
                let resp = handle_ipc(req, &issuer);
                let _ = write_resp(&mut writer, &resp).await;
            }
        });
    }
}

/// 处理单条 IPC 请求并返回响应。
///
/// 参数：
/// - `req`：请求
/// - `issuer`：令牌签发器
///
/// 返回值：
/// - 总是返回 [`IpcResponse`]；错误通过 `IpcResponse::Error` 表达
fn handle_ipc(req: IpcRequest, issuer: &TokenIssuer) -> IpcResponse {
    match req {
        IpcRequest::Ping { request_id } => IpcResponse::Pong { request_id },
        IpcRequest::GetSsoToken { request_id, subject } => {
            let ttl = Duration::minutes(30);
            let token = issuer.issue(subject, ttl);
            let claims: TokenClaims = match issuer.verify(&token, Duration::seconds(30)) {
                Ok(c) => c,
                Err(e) => {
                    return IpcResponse::Error {
                        request_id,
                        message: format!("token verify failed: {e}"),
                    }
                }
            };
            IpcResponse::SsoToken {
                request_id,
                token,
                expires_at_unix: claims.expires_at_unix,
            }
        }
        IpcRequest::GetAppStatus { request_id, app_id } => {
            match get_app_running_status(&app_id) {
                Ok(running) => IpcResponse::AppStatus {
                    request_id,
                    app_id,
                    running,
                },
                Err(e) => IpcResponse::Error {
                    request_id,
                    message: e.to_string(),
                },
            }
        }
    }
}

/// 根据插件 ID 获取应用运行状态。
///
/// 参数：
/// - `app_id`：插件 ID（默认对应 `plugins/<app_id>.json`）
///
/// 返回值：
/// - `Ok(true)`：检测为运行中
/// - `Ok(false)`：检测为未运行
///
/// 异常处理：
/// - 插件文件读取/解析失败会返回错误
/// - 进程检测失败时返回错误（当前实现一般不会触发）
fn get_app_running_status(app_id: &str) -> Result<bool> {
    let install_state = load_install_state().ok();
    let install_root = install_state
        .as_ref()
        .and_then(|s| s.modules.iter().find_map(|m| m.install_root.clone()))
        .map(PathBuf::from)
        .unwrap_or_else(|| current_exe_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let plugin_dir = paths::default_plugin_dir()?;
    let plugin_file = plugin_dir.join(format!("{app_id}.json"));
    let raw = std::fs::read_to_string(&plugin_file)
        .with_context(|| format!("读取插件文件失败: {}", plugin_file.display()))?;
    let pf: PluginFile = serde_json::from_str(&raw).context("解析插件文件失败")?;
    let exe = resolve_under_install_root(&install_root, &pf.plugin.exe);
    process::is_process_running_by_exe(&exe)
}

/// 将响应序列化为 JSON 并写回连接。
///
/// 参数：
/// - `writer`：TCP 写端
/// - `resp`：响应对象
///
/// 异常处理：
/// - 序列化失败或写入失败会返回错误
async fn write_resp(writer: &mut tokio::net::tcp::WriteHalf<'_>, resp: &IpcResponse) -> Result<()> {
    let mut s = serde_json::to_string(resp)?;
    s.push('\n');
    tokio::io::AsyncWriteExt::write_all(writer, s.as_bytes()).await?;
    Ok(())
}

/// GUI 应用状态（eframe App）。
///
/// 说明：
/// - `install_root`：安装根目录（用于解析插件 exe 相对路径）
/// - `ipc_addr`：IPC 监听地址（通过环境变量注入到被启动应用）
/// - `plugins`：当前加载到的插件列表
/// - `last_error`：最近一次启动失败的错误信息（用于 UI 展示）
struct AppState {
    install_root: PathBuf,
    ipc_addr: SocketAddr,
    plugins: Arc<Mutex<Vec<LoadedPlugin>>>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl AppState {
    /// 创建应用状态并加载插件。
    ///
    /// 参数：
    /// - `install_root`：安装根目录
    /// - `ipc_addr`：IPC 地址
    /// - `issuer`：令牌签发器（预留，后续可在 GUI 内直接签发/校验）
    fn new(install_root: PathBuf, ipc_addr: SocketAddr, issuer: TokenIssuer) -> Self {
        let _ = issuer;
        let plugins = Arc::new(Mutex::new(Vec::new()));
        let last_error = Arc::new(Mutex::new(None));
        let s = Self {
            install_root,
            ipc_addr,
            plugins,
            last_error,
        };
        s.reload_plugins();
        s
    }

    /// 重新加载插件目录下的所有插件文件。
    ///
    /// 异常处理：
    /// - 当前实现以“尽力而为”为主：读取/解析失败的文件会被忽略，不影响其他插件加载
    fn reload_plugins(&self) {
        let plugin_dir = paths::default_plugin_dir().ok();
        let mut loaded = Vec::new();
        if let Some(dir) = plugin_dir {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|s| s.to_str()) != Some("json") {
                        continue;
                    }
                    match std::fs::read_to_string(&p)
                        .ok()
                        .and_then(|s| serde_json::from_str::<PluginFile>(&s).ok())
                    {
                        Some(f) => loaded.push(LoadedPlugin {
                            module_id: f.module_id,
                            plugin: f.plugin,
                            file_path: p,
                        }),
                        None => {}
                    }
                }
            }
        }
        *self.plugins.lock().unwrap() = loaded;
    }

    /// 启动指定插件。
    ///
    /// 参数：
    /// - `p`：已加载插件
    ///
    /// 异常处理：
    /// - exe 不存在或进程启动失败会返回错误
    ///
    /// 行为：
    /// - 通过环境变量 `XIAOHAI_IPC_ADDR` 将 IPC 地址注入子进程，便于插件侧调用统一 IPC/SSO
    fn launch_plugin(&self, p: &LoadedPlugin) -> Result<()> {
        let exe = resolve_under_install_root(&self.install_root, &p.plugin.exe);
        if !exe.exists() {
            return Err(anyhow::anyhow!("应用不存在: {}", exe.display()));
        }
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(&p.plugin.args);
        cmd.env("XIAOHAI_IPC_ADDR", self.ipc_addr.to_string());
        cmd.spawn().with_context(|| format!("启动应用失败: {}", exe.display()))?;
        Ok(())
    }
}

/// 将插件中的路径解析为安装目录下的实际路径。
///
/// 规则：
/// - 若 `raw` 是绝对路径：直接返回
/// - 若 `raw` 是相对路径：返回 `install_root.join(raw)`
fn resolve_under_install_root(install_root: &Path, raw: &str) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else {
        install_root.join(p)
    }
}

impl eframe::App for AppState {
    /// GUI 渲染与交互逻辑（每帧调用）。
    ///
    /// 实现要点：
    /// - 顶部栏提供“刷新”按钮，用于重新扫描插件目录
    /// - 中央区域展示插件列表、运行状态与“启动”按钮
    ///
    /// 异常处理：
    /// - 进程状态检测失败时降级为 `false`（未运行）
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("小海智能助手");
                if ui.button("刷新").clicked() {
                    self.reload_plugins();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(err) = self.last_error.lock().unwrap().as_ref() {
                ui.colored_label(egui::Color32::RED, err);
            }
            ui.separator();

            let plugins = self.plugins.lock().unwrap().clone();
            if plugins.is_empty() {
                ui.label("未发现可用应用插件（请检查 ProgramData\\XiaoHaiAssistant\\plugins）");
                return;
            }
            for p in plugins {
                ui.group(|ui| {
                    let exe = resolve_under_install_root(&self.install_root, &p.plugin.exe);
                    let running = process::is_process_running_by_exe(&exe).unwrap_or(false);
                    ui.horizontal(|ui| {
                        ui.label(&p.plugin.name);
                        ui.label(if running { "运行中" } else { "未运行" });
                        if ui.button("启动").clicked() {
                            if let Err(e) = self.launch_plugin(&p) {
                                warn!("{e}");
                                *self.last_error.lock().unwrap() = Some(e.to_string());
                            } else {
                                *self.last_error.lock().unwrap() = None;
                            }
                        }
                    });
                    ui.label(exe.display().to_string());
                    ui.label(format!("module_id = {}", p.module_id));
                    ui.label(format!("plugin = {}", p.file_path.display()));
                });
                ui.add_space(8.0);
            }
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}
