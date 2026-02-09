//! 统一安装/卸载引导程序（bootstrapper）。
//!
//! 职责：
//! - 读取 `bundle-manifest.json`，按模块编排安装/卸载流程
//! - 前置依赖检测与安装（.NET Framework、VC++ 运行库）
//! - 安装后治理：只保留“小海智能助手”快捷方式，移除各组件桌面图标
//! - 安装后配置：创建数据/插件目录、写入插件注册、可选服务/防火墙/自启动
//! - 生成/更新 `install-state.json`，用于卸载精准回滚
//!
//! 权限要求：
//! - 安装/卸载建议以管理员权限运行（写 Program Files、写 HKLM、自启动、服务、防火墙等）
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use tracing::{info, warn};
use xiaohai_core::manifest::{BundleManifest, DetectRule, ModuleKind, PayloadInstaller};
use xiaohai_core::paths;
use xiaohai_core::state::{CreatedShortcut, InstallState, InstalledModule};
use xiaohai_windows::{elevation, firewall, prereq, registry, service, shortcut};

/// 命令行参数。
///
/// 说明：
/// - `manifest` 指向安装清单文件（默认 `bundle-manifest.json`）
/// - `silent` 用于企业部署场景（减少提示输出）
#[derive(Debug, Parser)]
#[command(name = "xiaohai-bootstrapper", version)]
struct Cli {
    #[arg(long, default_value = "bundle-manifest.json")]
    manifest: PathBuf,

    #[arg(long, default_value_t = false)]
    silent: bool,

    #[command(subcommand)]
    command: Commands,
}

/// bootstrapper 支持的子命令。
#[derive(Debug, Subcommand)]
enum Commands {
    /// 安装（幂等：已安装模块会跳过）。
    Install,
    /// 卸载（按状态文件回滚 + 按清单执行模块卸载）。
    Uninstall,
    /// 仅执行检测并输出结果（不做系统修改）。
    Detect,
    /// 环境自检（管理员权限、依赖安装状态等）。
    Doctor,
}

/// 程序入口：解析参数并分发子命令。
///
/// 异常处理：
/// - 任意子命令执行失败会返回 `Err` 并输出日志（由调用方/控制台显示）。
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Install => install(&cli),
        Commands::Uninstall => uninstall(&cli),
        Commands::Detect => detect(&cli),
        Commands::Doctor => doctor(&cli),
    }
}

/// 读取并解析安装清单（JSON）。
///
/// 参数：
/// - `path`：清单文件路径
///
/// 返回值：
/// - 成功：返回解析后的 [`BundleManifest`]
///
/// 异常处理：
/// - 文件读取失败（不存在/权限/IO）返回错误
/// - JSON 解析失败返回错误
fn load_manifest(path: &Path) -> Result<BundleManifest> {
    let bytes = std::fs::read(path).with_context(|| format!("读取清单失败: {}", path.display()))?;
    let manifest: BundleManifest = serde_json::from_slice(&bytes).context("解析清单 JSON 失败")?;
    Ok(manifest)
}

fn allow_non_admin_for_tests() -> bool {
    matches!(
        std::env::var("XIAOHAI_TEST_ALLOW_NON_ADMIN").as_deref(),
        Ok("1")
    )
}

/// 执行安装流程（按清单编排）。
///
/// 参数：
/// - `cli`：命令行参数（包含 manifest 路径、silent 标志）
///
/// 主要步骤：
/// 1) 权限检查（需要管理员）
/// 2) 加载清单并创建 ProgramData 目录结构
/// 3) 检测并安装前置依赖
/// 4) 按模块顺序执行安装（支持幂等跳过）
/// 5) 写入插件注册、创建统一入口快捷方式、可选配置服务/防火墙/自启动
/// 6) 落盘 `install-state.json`（用于卸载回滚）
///
/// 异常处理：
/// - 任一模块安装失败将终止流程并返回错误；上层可据此中止批量部署。
fn install(cli: &Cli) -> Result<()> {
    if !allow_non_admin_for_tests() && !elevation::is_running_as_admin()? {
        return Err(anyhow!("安装需要管理员权限，请以管理员方式运行"));
    }

    let manifest = load_manifest(&cli.manifest)?;
    let base_dir = cli
        .manifest
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    info!("开始安装: {} {}", manifest.product_name, manifest.version);

    ensure_programdata_layout()?;

    install_prerequisites(&manifest, &base_dir)?;

    let mut state = InstallState::new(manifest.product_code.clone(), manifest.version.clone());
    for module in &manifest.modules {
        if !module.enabled {
            continue;
        }
        let already = detect_module_installed(&base_dir, module)?;
        if already {
            info!("模块已安装，跳过: {} ({})", module.display_name, module.id);
            state.modules.push(InstalledModule {
                id: module.id.clone(),
                display_name: module.display_name.clone(),
                kind: format!("{:?}", module.kind),
                installed: true,
                install_root: None,
                uninstall_hint: None,
            });
            continue;
        }
        info!("安装模块: {} ({})", module.display_name, module.id);
        let install_root = PathBuf::from(&manifest.install_root);
        match module.kind {
            ModuleKind::Msi | ModuleKind::Exe => {
                let installer = module
                    .installer
                    .clone()
                    .ok_or_else(|| anyhow!("模块缺少 installer 配置: {}", module.id))?;
                run_installer(&base_dir, &installer)?;
            }
            ModuleKind::FileCopy => {
                let payload = module
                    .payload
                    .clone()
                    .ok_or_else(|| anyhow!("FileCopy 模块缺少 payload 配置: {}", module.id))?;
                let src = paths::resolve_path(&base_dir, &payload.path)?;
                let dst = if let Some(subdir) = payload.install_subdir.as_deref() {
                    install_root.join(subdir)
                } else {
                    install_root.join(&module.id)
                };
                copy_recursively(&src, &dst)?;
            }
        }

        apply_module_config(&base_dir, &manifest, module)?;

        state.modules.push(InstalledModule {
            id: module.id.clone(),
            display_name: module.display_name.clone(),
            kind: format!("{:?}", module.kind),
            installed: true,
            install_root: Some(manifest.install_root.clone()),
            uninstall_hint: None,
        });
    }

    write_plugins(&base_dir, &manifest)?;
    manage_shortcuts(&manifest, &mut state)?;
    install_service_and_firewall(&manifest, &mut state)?;

    persist_state(&state)?;
    info!("安装完成");
    if !cli.silent {
        info!("提示：可运行 xiaohai-assistant 启动统一入口");
    }
    Ok(())
}

/// 执行卸载流程。
///
/// 参数：
/// - `cli`：命令行参数
///
/// 主要步骤：
/// 1) 权限检查（需要管理员）
/// 2) 读取状态文件并尽可能回滚（防火墙/服务/自启动/快捷方式）
/// 3) 删除插件注册
/// 4) 按模块执行卸载（若模块未提供卸载器则跳过并提示）
/// 5) 删除安装目录与 ProgramData 落盘目录
///
/// 异常处理：
/// - 回滚阶段以“尽力而为”为主（失败不阻塞后续卸载）
/// - 模块卸载阶段若执行卸载器失败会返回错误
fn uninstall(cli: &Cli) -> Result<()> {
    if !allow_non_admin_for_tests() && !elevation::is_running_as_admin()? {
        return Err(anyhow!("卸载需要管理员权限，请以管理员方式运行"));
    }

    let manifest = load_manifest(&cli.manifest)?;
    let base_dir = cli
        .manifest
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    info!("开始卸载: {} {}", manifest.product_name, manifest.version);

    let state_path = paths::default_state_file()?;
    let mut state: Option<InstallState> = None;
    if state_path.exists() {
        let bytes = std::fs::read(&state_path).context("读取 install-state.json 失败")?;
        state = Some(serde_json::from_slice(&bytes).context("解析 install-state.json 失败")?);
    }

    if let Some(st) = &state {
        for rule in &st.firewall_rules {
            let _ = firewall::delete_rule(rule);
        }
        if let Some(name) = &st.autorun_name {
            let _ = registry::delete_hklm_run(name);
        }
        if let Some(svc) = &st.service_name {
            let _ = service::uninstall_service(svc);
        }
        for s in &st.created_shortcuts {
            let p = PathBuf::from(&s.path);
            let _ = std::fs::remove_file(&p);
        }
    }
    if state.is_none() && manifest.autorun.enabled {
        let name = if manifest.autorun.name.is_empty() {
            "XiaoHaiAssistant"
        } else {
            manifest.autorun.name.as_str()
        };
        let _ = registry::delete_hklm_run(name);
    }

    remove_plugins()?;

    for module in &manifest.modules {
        if !module.enabled {
            continue;
        }
        match module.kind {
            ModuleKind::Msi | ModuleKind::Exe => {
                if let Some(uninstaller) = module.uninstaller.clone() {
                    info!("卸载模块: {} ({})", module.display_name, module.id);
                    run_installer(&base_dir, &uninstaller)?;
                } else {
                    warn!(
                        "模块未提供卸载配置，跳过: {} ({})",
                        module.display_name, module.id
                    );
                }
            }
            ModuleKind::FileCopy => {
                let install_root = PathBuf::from(&manifest.install_root);
                let dir = module
                    .payload
                    .as_ref()
                    .and_then(|p| p.install_subdir.as_deref())
                    .map(|subdir| install_root.join(subdir))
                    .unwrap_or_else(|| install_root.join(&module.id));
                if dir.exists() {
                    info!("删除模块目录: {}", dir.display());
                    let _ = std::fs::remove_dir_all(&dir);
                }
            }
        }
    }

    let install_root = PathBuf::from(&manifest.install_root);
    if install_root.exists() {
        let _ = std::fs::remove_dir_all(&install_root);
    }

    let data_dir = paths::program_data_dir()?;
    if data_dir.exists() {
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    info!("卸载完成");
    Ok(())
}

/// 仅检测清单中各模块是否已安装并输出结果。
///
/// 参数：
/// - `cli`：命令行参数
///
/// 返回值：
/// - `Ok(())`：检测完成；结果输出到 stdout
///
/// 异常处理：
/// - 清单读取/解析失败会返回错误
/// - 检测过程中若出现注册表/路径解析错误会返回错误
fn detect(cli: &Cli) -> Result<()> {
    let manifest = load_manifest(&cli.manifest)?;
    let base_dir = cli
        .manifest
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    for module in &manifest.modules {
        if !module.enabled {
            continue;
        }
        let installed = detect_module_installed(&base_dir, module)?;
        println!("{} ({}) = {}", module.display_name, module.id, installed);
    }
    Ok(())
}

/// 环境自检（用于排障）。
///
/// 输出：
/// - 是否管理员运行
/// - .NET Framework 4.8 状态
/// - VC++ 2015-2022 x64 状态
fn doctor(_cli: &Cli) -> Result<()> {
    println!("admin = {}", elevation::is_running_as_admin()?);
    println!("dotnet_fx48 = {:?}", prereq::dotnet_fx48_status()?);
    println!(
        "vcredist_2015_2022_x64 = {:?}",
        prereq::vcredist_2015_2022_x64_status()?
    );
    Ok(())
}

/// 创建 ProgramData 目录结构（数据/插件/状态文件所在目录）。
///
/// 异常处理：
/// - 目录创建失败（权限、磁盘等）会返回错误
fn ensure_programdata_layout() -> Result<()> {
    let base = paths::program_data_dir()?;
    paths::ensure_dir(&base)?;
    paths::ensure_dir(&paths::default_plugin_dir()?)?;
    paths::ensure_dir(&paths::default_data_root()?)?;
    Ok(())
}

/// 安装前置依赖（若缺失则按清单执行安装器）。
///
/// 参数：
/// - `manifest`：安装清单（依赖项配置）
/// - `base_dir`：清单所在目录（用于解析相对路径 payload）
///
/// 异常处理：
/// - 依赖开启但缺少 installer 配置会返回错误
/// - 安装器执行失败会返回错误
fn install_prerequisites(manifest: &BundleManifest, base_dir: &Path) -> Result<()> {
    if manifest.prerequisites.dotnet_fx48.enabled {
        if matches!(prereq::dotnet_fx48_status()?, prereq::PrereqStatus::Missing) {
            let installer = manifest
                .prerequisites
                .dotnet_fx48
                .installer
                .clone()
                .ok_or_else(|| anyhow!("dotnet_fx48 缺少 installer 配置"))?;
            info!(".NET Framework 4.8 缺失，开始安装");
            run_installer(base_dir, &installer)?;
        } else {
            info!(".NET Framework 4.8 已安装");
        }
    }
    if manifest.prerequisites.vcredist_2015_2022_x64.enabled {
        if matches!(
            prereq::vcredist_2015_2022_x64_status()?,
            prereq::PrereqStatus::Missing
        ) {
            let installer = manifest
                .prerequisites
                .vcredist_2015_2022_x64
                .installer
                .clone()
                .ok_or_else(|| anyhow!("vcredist_2015_2022_x64 缺少 installer 配置"))?;
            info!("VC++ 2015-2022 x64 缺失，开始安装");
            run_installer(base_dir, &installer)?;
        } else {
            info!("VC++ 2015-2022 x64 已安装");
        }
    }
    Ok(())
}

/// 按模块检测规则判断是否已安装。
///
/// 参数：
/// - `base_dir`：清单所在目录（用于解析 `file_exists` 相对路径）
/// - `module`：模块清单
///
/// 返回值：
/// - `Ok(true)`：检测为已安装
/// - `Ok(false)`：检测为未安装或未提供检测规则
///
/// 异常处理：
/// - 注册表读取/路径解析失败会返回错误
fn detect_module_installed(
    base_dir: &Path,
    module: &xiaohai_core::manifest::ModuleManifest,
) -> Result<bool> {
    match &module.detect {
        DetectRule::None => Ok(false),
        DetectRule::RegistryValue(rule) => registry::detect_registry_rule(rule),
        DetectRule::FileExists(rule) => {
            let p = paths::resolve_path(base_dir, &rule.path)?;
            Ok(p.exists())
        }
    }
}

/// 执行安装器/卸载器并检查退出码。
///
/// 参数：
/// - `base_dir`：清单所在目录（用于解析相对路径）
/// - `installer`：安装器定义（路径、参数、成功退出码）
///
/// 异常处理：
/// - 进程启动失败返回错误
/// - 退出码不在允许列表中返回错误，并附带 stdout/stderr 便于排障
fn run_installer(base_dir: &Path, installer: &PayloadInstaller) -> Result<()> {
    let exe = paths::resolve_path(base_dir, &installer.path)?;
    let mut cmd = Command::new(&exe);
    cmd.args(&installer.args);
    let out = cmd
        .output()
        .with_context(|| format!("启动安装程序失败: {}", exe.display()))?;
    let code = out.status.code().unwrap_or(-1);
    let mut ok_codes = installer.success_exit_codes.clone();
    if ok_codes.is_empty() {
        // 约定的默认成功码：
        // - 0：成功
        // - 3010：成功但需要重启（MSI 常见）
        // - 1641：成功并已触发重启（MSI 常见）
        ok_codes = vec![0, 3010, 1641];
    }
    if ok_codes.contains(&code) {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    Err(anyhow!(
        "安装程序退出码异常: {} ({})\n{}\n{}",
        exe.display(),
        code,
        stdout,
        stderr
    ))
}

/// 递归复制文件/目录（用于 FileCopy 模式）。
///
/// 参数：
/// - `src`：源路径（文件或目录）
/// - `dst`：目标路径（文件或目录）
///
/// 异常处理：
/// - 读目录/创建目录/复制文件失败会返回错误
fn copy_recursively(src: &Path, dst: &Path) -> Result<()> {
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dst)
            .with_context(|| format!("复制文件失败: {} -> {}", src.display(), dst.display()))?;
        return Ok(());
    }

    std::fs::create_dir_all(dst).with_context(|| format!("创建目录失败: {}", dst.display()))?;
    for entry in
        std::fs::read_dir(src).with_context(|| format!("读取目录失败: {}", src.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_recursively(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .with_context(|| format!("复制文件失败: {} -> {}", from.display(), to.display()))?;
        }
    }
    Ok(())
}

/// 执行模块级安装后配置。
///
/// 当前实现：
/// - 创建模块数据目录（如配置了 `data_subdir`）
/// - 对指定配置文件执行字符串替换（`file_replacements`）
///
/// 参数：
/// - `base_dir`：清单所在目录（保留，用于后续扩展）
/// - `manifest`：全局清单（用于获取安装根目录与全局数据目录）
/// - `module`：模块清单（用于获取模块级配置）
///
/// 异常处理：
/// - 读写配置文件失败会返回错误
fn apply_module_config(
    base_dir: &Path,
    manifest: &BundleManifest,
    module: &xiaohai_core::manifest::ModuleManifest,
) -> Result<()> {
    let install_root = PathBuf::from(&manifest.install_root);
    let data_root = manifest
        .post_config
        .data_root
        .clone()
        .map(PathBuf::from)
        .unwrap_or(paths::default_data_root()?);

    if let Some(subdir) = &module.config.data_subdir {
        let dir = data_root.join(subdir);
        paths::ensure_dir(&dir)?;
    }

    for fr in &module.config.file_replacements {
        let target = paths::resolve_path(&install_root, &fr.file)?;
        if !target.exists() {
            warn!("配置文件不存在，跳过: {}", target.display());
            continue;
        }
        let mut content = std::fs::read_to_string(&target)
            .with_context(|| format!("读取配置文件失败: {}", target.display()))?;
        for kv in &fr.replacements {
            content = content.replace(&kv.key, &kv.value);
        }
        std::fs::write(&target, content)
            .with_context(|| format!("写入配置文件失败: {}", target.display()))?;
    }

    if let Some(url) = &module.config.server_url {
        let _ = (base_dir, url);
    }

    Ok(())
}

/// 将启用模块的插件信息写入 ProgramData 插件目录。
///
/// 输出：
/// - 每个启用模块若配置了 `plugin`，会生成一个 `<plugin.id>.json` 文件
///
/// 异常处理：
/// - 插件目录创建失败或写文件失败会返回错误
fn write_plugins(base_dir: &Path, manifest: &BundleManifest) -> Result<()> {
    let plugin_dir = manifest
        .post_config
        .plugin_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or(paths::default_plugin_dir()?);
    paths::ensure_dir(&plugin_dir)?;

    for module in &manifest.modules {
        if !module.enabled {
            continue;
        }
        let Some(plugin) = &module.plugin else {
            continue;
        };
        let mut plugin_value = serde_json::to_value(plugin).context("序列化插件失败")?;
        if let Some(obj) = plugin_value.as_object_mut() {
            obj.insert(
                "module_id".to_string(),
                serde_json::Value::String(module.id.clone()),
            );
        }
        let bytes = serde_json::to_vec_pretty(&plugin_value)?;
        let file = plugin_dir.join(format!("{}.json", plugin.id));
        std::fs::write(&file, bytes)
            .with_context(|| format!("写入插件文件失败: {}", file.display()))?;

        let _ = base_dir;
    }
    Ok(())
}

/// 删除 ProgramData 插件目录下的插件注册文件（*.json）。
///
/// 异常处理：
/// - 读取目录失败会返回错误
/// - 删除文件失败会被忽略（尽力而为）
fn remove_plugins() -> Result<()> {
    let plugin_dir = paths::default_plugin_dir()?;
    if !plugin_dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&plugin_dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

/// 快捷方式治理：移除模块桌面图标并创建统一入口快捷方式。
///
/// 参数：
/// - `manifest`：安装清单
/// - `state`：安装状态（用于记录创建的快捷方式以便卸载回滚）
///
/// 异常处理：
/// - 创建/删除快捷方式失败会返回错误
fn manage_shortcuts(manifest: &BundleManifest, state: &mut InstallState) -> Result<()> {
    for module in &manifest.modules {
        if !module.enabled {
            continue;
        }
        let _ = shortcut::remove_shortcuts_from_desktop(&module.remove_desktop_shortcuts)?;
    }

    let assistant_exe =
        PathBuf::from(&manifest.install_root).join(&manifest.shortcuts.assistant_exe);
    let icon = manifest
        .shortcuts
        .icon_path
        .as_deref()
        .map(|p| (PathBuf::from(&manifest.install_root).join(p), 0));

    if manifest.shortcuts.desktop {
        let p = shortcut::create_shortcut(
            shortcut::ShortcutLocation::Desktop,
            &manifest.shortcuts.assistant_name,
            &assistant_exe,
            &[],
            assistant_exe.parent(),
            icon.as_ref().map(|(p, i)| (p.as_path(), *i)),
        )?;
        state.created_shortcuts.push(CreatedShortcut {
            location: "desktop".to_string(),
            path: p.to_string_lossy().to_string(),
        });
    }

    if manifest.shortcuts.start_menu {
        let p = shortcut::create_shortcut(
            shortcut::ShortcutLocation::StartMenuPrograms,
            &manifest.shortcuts.assistant_name,
            &assistant_exe,
            &[],
            assistant_exe.parent(),
            icon.as_ref().map(|(p, i)| (p.as_path(), *i)),
        )?;
        state.created_shortcuts.push(CreatedShortcut {
            location: "start_menu".to_string(),
            path: p.to_string_lossy().to_string(),
        });
    }

    Ok(())
}

/// 配置系统级能力：自启动/服务/防火墙。
///
/// 参数：
/// - `manifest`：安装清单
/// - `state`：安装状态（用于记录已配置项，便于卸载清理）
///
/// 异常处理：
/// - 写注册表/安装服务/添加防火墙规则失败会返回错误
fn install_service_and_firewall(manifest: &BundleManifest, state: &mut InstallState) -> Result<()> {
    if manifest.autorun.enabled {
        let name = if manifest.autorun.name.is_empty() {
            "XiaoHaiAssistant".to_string()
        } else {
            manifest.autorun.name.clone()
        };
        let command = if manifest.autorun.command.is_empty() {
            let assistant_exe =
                PathBuf::from(&manifest.install_root).join(&manifest.shortcuts.assistant_exe);
            format!("\"{}\"", assistant_exe.display())
        } else {
            manifest.autorun.command.clone()
        };
        registry::set_hklm_run(&name, &command)?;
        state.autorun_name = Some(name);
    }

    if manifest.service.enabled {
        let exe = PathBuf::from(&manifest.install_root).join(&manifest.service.exe);
        service::install_service(
            &manifest.service.name,
            &manifest.service.display_name,
            &manifest.service.description,
            &exe.to_string_lossy(),
            &manifest.service.args,
        )?;
        state.service_name = Some(manifest.service.name.clone());
    }

    if manifest.firewall.enabled {
        for rule in &manifest.firewall.rules {
            firewall::add_rule(rule)?;
            state.firewall_rules.push(rule.name.clone());
        }
    }

    Ok(())
}

/// 将安装状态序列化并写入 ProgramData。
///
/// 参数：
/// - `state`：安装状态
///
/// 异常处理：
/// - 序列化失败或写文件失败会返回错误
fn persist_state(state: &InstallState) -> Result<()> {
    let path = paths::default_state_file()?;
    let bytes = serde_json::to_vec_pretty(state).context("序列化 install-state.json 失败")?;
    std::fs::write(&path, bytes)
        .with_context(|| format!("写入状态文件失败: {}", path.display()))?;
    Ok(())
}
