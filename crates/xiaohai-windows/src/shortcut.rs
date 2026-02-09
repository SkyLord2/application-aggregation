//! Windows 快捷方式（.lnk）创建与删除。
//!
//! 实现方式：
//! - 使用 COM：`IShellLinkW` + `IPersistFile::Save`
//! - 通过 Known Folder 获取桌面与开始菜单 Programs 目录
//!
//! 异常处理：
//! - COM 初始化/对象创建/保存失败会返回错误
//! - 删除快捷方式若不存在会返回 `Ok(false)`（幂等）
//!
//! 安全注意：
//! - 本模块只操作指定路径下的 `.lnk` 文件；上层应避免传入不可信的 name 以免路径注入
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use windows::core::{Interface, PCWSTR, PWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Com::{CoTaskMemFree, IPersistFile};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Programs, IShellLinkW, SHGetKnownFolderPath, ShellLink,
    KF_FLAG_DEFAULT,
};

/// 快捷方式放置位置。
#[derive(Debug, Clone, Copy)]
pub enum ShortcutLocation {
    /// 当前用户桌面目录。
    Desktop,
    /// 当前用户开始菜单 Programs 目录。
    StartMenuPrograms,
}

/// 创建快捷方式（.lnk）。
///
/// 参数：
/// - `location`：放置位置（桌面/开始菜单）
/// - `name`：快捷方式显示名称（不含 `.lnk`）
/// - `target_exe`：目标可执行文件路径
/// - `args`：启动参数
/// - `working_dir`：工作目录（可选）
/// - `icon`：图标路径与索引（可选）
///
/// 返回值：
/// - 成功：返回创建出的 `.lnk` 完整路径
///
/// 异常处理：
/// - 目录创建、COM 初始化、ShellLink 创建、属性设置或保存失败会返回错误
pub fn create_shortcut(
    location: ShortcutLocation,
    name: &str,
    target_exe: &Path,
    args: &[String],
    working_dir: Option<&Path>,
    icon: Option<(&Path, i32)>,
) -> Result<PathBuf> {
    let folder = known_folder(location)?;
    std::fs::create_dir_all(&folder)
        .with_context(|| format!("创建快捷方式目录失败: {}", folder.display()))?;

    let link_path = folder.join(format!("{name}.lnk"));

    unsafe {
        // ShellLink 相关 COM 接口通常要求 STA（单线程单元）。
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .context("COM 初始化失败")?;
        let _guard = ComGuard;

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .context("创建 ShellLink 实例失败")?;

        // COM 接口以宽字符串（UTF-16，NUL 结尾）接收路径与参数。
        link.SetPath(PCWSTR(to_wide(target_exe.as_os_str()).as_ptr()))
            .context("设置快捷方式路径失败")?;

        if !args.is_empty() {
            let joined = args.join(" ");
            link.SetArguments(PCWSTR(to_wide(OsStr::new(&joined)).as_ptr()))
                .context("设置快捷方式参数失败")?;
        }

        if let Some(dir) = working_dir {
            link.SetWorkingDirectory(PCWSTR(to_wide(dir.as_os_str()).as_ptr()))
                .context("设置快捷方式工作目录失败")?;
        }

        if let Some((icon_path, index)) = icon {
            link.SetIconLocation(PCWSTR(to_wide(icon_path.as_os_str()).as_ptr()), index)
                .context("设置快捷方式图标失败")?;
        }

        let persist: IPersistFile = link.cast().context("获取 IPersistFile 失败")?;
        persist
            .Save(PCWSTR(to_wide(link_path.as_os_str()).as_ptr()), true)
            .context("保存快捷方式失败")?;
    }

    Ok(link_path)
}

/// 根据名称删除指定位置的快捷方式。
///
/// 参数：
/// - `location`：放置位置
/// - `name`：快捷方式名称（不含 `.lnk`）
///
/// 返回值：
/// - `Ok(true)`：发现并删除了文件
/// - `Ok(false)`：文件不存在（幂等）
///
/// 异常处理：
/// - 删除失败（权限/文件被占用等）会返回错误
pub fn remove_shortcut_by_name(location: ShortcutLocation, name: &str) -> Result<bool> {
    let folder = known_folder(location)?;
    let link_path = folder.join(format!("{name}.lnk"));
    if link_path.exists() {
        std::fs::remove_file(&link_path)
            .with_context(|| format!("删除快捷方式失败: {}", link_path.display()))?;
        return Ok(true);
    }
    Ok(false)
}

/// 批量删除桌面快捷方式。
///
/// 参数：
/// - `names`：快捷方式名称列表（不含 `.lnk`）
///
/// 返回值：
/// - 返回实际删除的 `.lnk` 路径列表
///
/// 异常处理：
/// - 删除任意一个文件失败会返回错误（并中断）
pub fn remove_shortcuts_from_desktop(names: &[String]) -> Result<Vec<PathBuf>> {
    let desktop = known_folder(ShortcutLocation::Desktop)?;
    let mut removed = Vec::new();
    for n in names {
        let p = desktop.join(format!("{n}.lnk"));
        if p.exists() {
            std::fs::remove_file(&p)
                .with_context(|| format!("删除桌面快捷方式失败: {}", p.display()))?;
            removed.push(p);
        }
    }
    Ok(removed)
}

/// 获取 Known Folder 对应的目录路径。
///
/// 参数：
/// - `location`：快捷方式位置枚举
///
/// 返回值：
/// - 对应目录的绝对路径
///
/// 异常处理：
/// - Known Folder 查询失败或返回路径无法解码时返回错误
fn known_folder(location: ShortcutLocation) -> Result<PathBuf> {
    let folder_id = match location {
        ShortcutLocation::Desktop => &FOLDERID_Desktop,
        ShortcutLocation::StartMenuPrograms => &FOLDERID_Programs,
    };
    unsafe {
        let path_ptr: PWSTR = SHGetKnownFolderPath(folder_id, KF_FLAG_DEFAULT, None)
            .context("读取 Known Folder 失败")?;
        // SHGetKnownFolderPath 返回的内存由 COM 分配，必须用 CoTaskMemFree 释放。
        let _guard = CoTaskMemGuard(path_ptr);
        let s = path_ptr.to_string().context("Known Folder 路径解码失败")?;
        Ok(PathBuf::from(s))
    }
}

/// 将 Windows 宽字符串（UTF-16）编码并追加 NUL 结尾。
///
/// 参数：
/// - `s`：待转换的 OS 字符串
///
/// 返回值：
/// - UTF-16 编码的 `Vec<u16>`，最后一个元素为 0（NUL）
fn to_wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// COM 初始化守卫：离开作用域时自动调用 `CoUninitialize`。
struct ComGuard;
impl Drop for ComGuard {
    /// 自动调用 `CoUninitialize`，与 [`CoInitializeEx`] 成对。
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}

/// COM 内存释放守卫：释放 `SHGetKnownFolderPath` 返回的 `PWSTR`。
struct CoTaskMemGuard(PWSTR);
impl Drop for CoTaskMemGuard {
    /// 自动释放 COM 分配的字符串内存，避免泄漏。
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                CoTaskMemFree(Some(self.0 .0 as *const core::ffi::c_void));
            }
        }
    }
}
