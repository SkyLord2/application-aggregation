//! 安装清单（bundle-manifest.json）与插件/模块模型定义。
//!
//! 该模块描述“单一安装程序”需要的全部输入：
//! - 产品信息（名称/版本/安装根目录）
//! - 前置依赖（.NET/VC++ 运行库）
//! - 子模块（MSI/EXE/FileCopy）的安装/检测/卸载/配置
//! - 快捷方式治理与插件注册
//! - 安装后配置（数据目录、插件目录、服务/防火墙、自启动）
//!
//! 约定：
//! - 大部分字段通过 `#[serde(default)]` 提供默认值，以便清单向前兼容
//! - 该模块仅定义数据结构，不执行任何 IO/系统修改
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use serde::{Deserialize, Serialize};

/// 安装清单根对象（对应 `bundle-manifest.json`）。
///
/// 说明：
/// - `install_root` 是目标安装目录（通常在 `Program Files` 下）
/// - `modules` 描述各子系统/组件如何安装与注册到统一入口
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    /// 产品显示名称。
    pub product_name: String,
    /// 产品标识（用于状态落盘/令牌隔离等）。
    pub product_code: String,
    /// 版本号（用于展示/审计）。
    pub version: String,
    /// 安装根目录（绝对路径字符串）。
    pub install_root: String,
    /// 依赖项（.NET/VC++ 等）。
    pub prerequisites: PrerequisitesManifest,
    /// 组件/模块列表。
    pub modules: Vec<ModuleManifest>,
    /// 快捷方式治理与统一入口快捷方式配置。
    pub shortcuts: ShortcutManifest,
    /// 安装后配置（数据目录、插件目录、服务器地址等）。
    pub post_config: PostConfigManifest,
    /// 防火墙规则配置。
    pub firewall: FirewallManifest,
    /// Windows 服务配置。
    pub service: ServiceManifest,
    #[serde(default)]
    /// Windows 登录后自启动配置（HKLM Run）。
    pub autorun: AutorunManifest,
}

/// 前置依赖清单。
///
/// 说明：
/// - 依赖项均可通过 `enabled` 开关关闭
/// - `installer` 为可选项，开启但未提供安装器时应由上层报错
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrerequisitesManifest {
    #[serde(default)]
    /// .NET Framework 4.8（通过注册表 Release 值检测）。
    pub dotnet_fx48: PrerequisiteItem,
    #[serde(default)]
    /// Visual C++ 2015-2022 Redistributable (x64)。
    pub vcredist_2015_2022_x64: PrerequisiteItem,
}

/// 单个依赖项定义。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrerequisiteItem {
    #[serde(default)]
    /// 是否启用该依赖项。
    pub enabled: bool,
    #[serde(default)]
    /// 依赖安装器（路径与参数）。
    pub installer: Option<PayloadInstaller>,
}

/// 单个模块定义（一个独立子系统/组件）。
///
/// 安装方式：
/// - `kind = msi/exe`：通过外部安装器执行（建议提供静默参数）
/// - `kind = file_copy`：将 payload 目录直接复制到 `install_root` 下
///
/// 检测方式：
/// - `detect`：用于判断“是否已安装”，避免重复安装
///
/// 快捷方式治理：
/// - `remove_desktop_shortcuts`：用于删除该模块安装器创建的桌面快捷方式（按 `.lnk` 文件名，不含扩展名）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    /// 模块 ID（唯一）。
    pub id: String,
    /// 模块显示名称。
    pub display_name: String,
    #[serde(default)]
    /// 是否启用该模块（关闭后不会安装/注册）。
    pub enabled: bool,
    /// 模块类型（MSI/EXE/FileCopy）。
    pub kind: ModuleKind,
    #[serde(default)]
    /// 安装检测规则（默认 `none`）。
    pub detect: DetectRule,
    #[serde(default)]
    /// FileCopy 模式的 payload 配置。
    pub payload: Option<ModulePayload>,
    #[serde(default)]
    /// MSI/EXE 模式的安装器配置。
    pub installer: Option<PayloadInstaller>,
    #[serde(default)]
    /// MSI/EXE 模式的卸载器配置。
    pub uninstaller: Option<PayloadInstaller>,
    #[serde(default)]
    /// 需要从桌面移除的快捷方式名称列表。
    pub remove_desktop_shortcuts: Vec<String>,
    #[serde(default)]
    /// 注册到统一入口的插件描述（为空则不在统一入口展示）。
    pub plugin: Option<PluginRegistration>,
    #[serde(default)]
    /// 安装后配置（写入 server_url、创建数据目录、替换配置文件等）。
    pub config: ModuleConfig,
}

/// 模块安装类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleKind {
    /// MSI 安装包。
    Msi,
    /// EXE 安装包。
    Exe,
    /// 目录/文件复制安装。
    FileCopy,
}

/// FileCopy 模式的 payload 配置。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulePayload {
    #[serde(default)]
    /// payload 相对/绝对路径。
    pub path: String,
    #[serde(default)]
    /// 安装到 `install_root` 下的子目录名；为空则默认使用模块 ID。
    pub install_subdir: Option<String>,
}

/// 安装检测规则。
///
/// 说明：
/// - 默认 `none`，表示不做检测（始终视为未安装）
/// - `registry_value`/`file_exists` 用于企业部署常见的“幂等安装”需求
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DetectRule {
    #[default]
    /// 不检测。
    None,
    /// 注册表值检测。
    RegistryValue(RegistryValueRule),
    /// 文件存在检测。
    FileExists(FileExistsRule),
}

/// 注册表检测规则：读取指定键值并与期望值比较。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryValueRule {
    /// 根键（HKLM/HKCU）。
    pub hive: RegistryHive,
    /// 子键路径（不含根键）。
    pub key: String,
    /// 值名。
    pub value_name: String,
    /// 值类型（DWORD/SZ）。
    pub kind: RegistryValueKind,
    /// 期望值（支持等于/大于等于）。
    pub expected: RegistryExpectedValue,
}

/// 注册表根键枚举。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryHive {
    /// HKEY_LOCAL_MACHINE。
    Hklm,
    /// HKEY_CURRENT_USER。
    Hkcu,
}

/// 注册表值类型枚举。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryValueKind {
    /// DWORD（u32）。
    Dword,
    /// 字符串（REG_SZ）。
    Sz,
}

/// 注册表期望值比较规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryExpectedValue {
    /// DWORD 值大于等于给定阈值。
    DwordAtLeast(u32),
    /// DWORD 值等于给定值。
    DwordEquals(u32),
    /// 字符串值等于给定字符串。
    SzEquals(String),
}

/// 文件存在检测规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileExistsRule {
    /// 目标文件路径（可为绝对路径，或相对清单基准目录）。
    pub path: String,
}

/// 外部安装器（或卸载器）定义。
///
/// 约定：
/// - `path` 可为相对路径（相对清单文件目录）或绝对路径
/// - `args` 建议提供静默安装参数
/// - `success_exit_codes` 为空时由上层提供默认成功码（例如 0/3010/1641）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadInstaller {
    /// 安装器可执行文件路径。
    pub path: String,
    #[serde(default)]
    /// 安装器命令行参数列表。
    pub args: Vec<String>,
    #[serde(default)]
    /// 视为成功的退出码列表。
    pub success_exit_codes: Vec<i32>,
}

/// 插件注册信息：用于统一入口加载并展示可启动的应用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRegistration {
    /// 插件 ID（建议与模块 ID 一致或可映射）。
    pub id: String,
    /// 展示名称。
    pub name: String,
    /// 可执行文件路径（相对安装根目录或绝对路径）。
    pub exe: String,
    #[serde(default)]
    /// 启动参数。
    pub args: Vec<String>,
    #[serde(default)]
    /// 图标路径（可选）。
    pub icon: Option<String>,
    #[serde(default)]
    /// 健康检查方式（可选）。
    pub healthcheck: Option<Healthcheck>,
}

/// 插件健康检查策略。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Healthcheck {
    /// 通过进程名/可执行文件判断是否运行。
    Process,
    /// 通过命名管道检查（预留）。
    Pipe { name: String },
    /// 通过 HTTP 探活（预留）。
    Http { url: String },
}

/// 模块安装后配置。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModuleConfig {
    #[serde(default)]
    /// 服务器地址（可用于替换模块配置文件或写入模块侧配置）。
    pub server_url: Option<String>,
    #[serde(default)]
    /// 在统一数据根目录下为模块创建的子目录名。
    pub data_subdir: Option<String>,
    #[serde(default)]
    /// 配置文件替换规则集合。
    pub file_replacements: Vec<FileReplacement>,
}

/// 单个配置文件替换规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReplacement {
    /// 目标文件路径（相对安装根目录或绝对路径）。
    pub file: String,
    /// 替换键值集合（按字符串替换）。
    pub replacements: Vec<KeyValue>,
}

/// 字符串替换键值对。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    /// 被替换的字符串（建议使用占位符形式，如 `{{SERVER_URL}}`）。
    pub key: String,
    /// 替换后的字符串。
    pub value: String,
}

/// 快捷方式与统一入口相关配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutManifest {
    /// 统一入口可执行文件相对安装根目录的路径。
    pub assistant_exe: String,
    /// 统一入口快捷方式显示名称。
    pub assistant_name: String,
    #[serde(default)]
    /// 统一入口快捷方式图标路径（可选）。
    pub icon_path: Option<String>,
    #[serde(default)]
    /// 是否创建开始菜单快捷方式。
    pub start_menu: bool,
    #[serde(default)]
    /// 是否创建桌面快捷方式。
    pub desktop: bool,
}

/// 安装后全局配置（作用于整个套件）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PostConfigManifest {
    #[serde(default)]
    /// 全局服务器地址（可为模块提供默认值）。
    pub server_url: Option<String>,
    #[serde(default)]
    /// 数据根目录（覆盖默认 ProgramData\data）。
    pub data_root: Option<String>,
    #[serde(default)]
    /// 插件目录（覆盖默认 ProgramData\plugins）。
    pub plugin_dir: Option<String>,
}

/// 防火墙配置。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FirewallManifest {
    #[serde(default)]
    /// 是否启用防火墙规则管理。
    pub enabled: bool,
    #[serde(default)]
    /// 防火墙规则列表。
    pub rules: Vec<FirewallRule>,
}

/// 单条防火墙规则定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallRule {
    /// 规则名称（用于创建/删除）。
    pub name: String,
    /// 目标程序路径（通常是可执行文件绝对路径）。
    pub program: String,
    #[serde(default)]
    /// 方向（入站/出站）。
    pub direction: FirewallDirection,
    #[serde(default)]
    /// 动作（允许/阻止）。
    pub action: FirewallAction,
    #[serde(default)]
    /// 生效配置文件（域/专用/公用/任意）。
    pub profile: FirewallProfile,
}

/// 防火墙方向。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FirewallDirection {
    #[default]
    /// 入站。
    In,
    /// 出站。
    Out,
}

/// 防火墙动作。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FirewallAction {
    #[default]
    /// 允许。
    Allow,
    /// 阻止。
    Block,
}

/// 防火墙配置文件类型。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FirewallProfile {
    #[default]
    /// 任意配置文件。
    Any,
    /// 域网络。
    Domain,
    /// 专用网络。
    Private,
    /// 公用网络。
    Public,
}

/// Windows 服务安装配置。
///
/// 说明：
/// - 当前项目提供服务安装能力与示例 agent；是否启用由 `enabled` 控制。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceManifest {
    #[serde(default)]
    /// 是否启用服务安装。
    pub enabled: bool,
    #[serde(default)]
    /// 服务名（唯一标识）。
    pub name: String,
    #[serde(default)]
    /// 服务显示名。
    pub display_name: String,
    #[serde(default)]
    /// 服务描述。
    pub description: String,
    #[serde(default)]
    /// 服务可执行文件路径（相对安装根目录）。
    pub exe: String,
    #[serde(default)]
    /// 服务启动参数。
    pub args: Vec<String>,
}

/// Windows 登录后自启动配置（HKLM Run）。
///
/// 注意：
/// - 仅建议用于启动“统一入口”或轻量后台程序；GUI 程序由服务拉起会受 Session 0 隔离影响。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutorunManifest {
    #[serde(default)]
    /// 是否启用自启动写入。
    pub enabled: bool,
    #[serde(default)]
    /// 自启动项名称（注册表值名）。
    pub name: String,
    #[serde(default)]
    /// 自启动命令（通常包含可执行文件路径与参数）。
    pub command: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// 验证 `DetectRule::FileExists` 的 JSON 反序列化是否正确。
    fn detect_rule_serde_file_exists() {
        let json = r#"{ "file_exists": { "path": "C:\\test.txt" } }"#;
        let v: DetectRule = serde_json::from_str(json).unwrap();
        match v {
            DetectRule::FileExists(r) => assert_eq!(r.path, r"C:\test.txt"),
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    /// 验证 `DetectRule::None` 的 JSON 反序列化是否正确。
    fn detect_rule_serde_none() {
        let v: DetectRule = serde_json::from_str(r#""none""#).unwrap();
        assert!(matches!(v, DetectRule::None));
    }
}
