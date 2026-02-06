//! 安装状态落盘模型（install-state.json）。
//!
//! 目的：
//! - 记录“本次安装做过哪些系统修改”，以便卸载时可精准回滚（快捷方式/防火墙/服务/自启动等）
//! - 记录已安装模块清单，便于统一入口展示与健康检查
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// 安装状态（会序列化为 JSON 存储到 ProgramData）。
///
/// 字段说明：
/// - `state_id`：本次安装状态文件 ID（用于区分多次安装）
/// - `product_code`：产品标识（与清单一致）
/// - `version`：版本号（与清单一致）
/// - `installed_at`：安装时间（UTC）
/// - `modules`：已安装模块清单
/// - `created_shortcuts`：安装时创建的快捷方式（卸载时删除）
/// - `firewall_rules`：安装时创建的防火墙规则名（卸载时删除）
/// - `service_name`：安装时创建的服务名（卸载时删除）
/// - `autorun_name`：安装时创建的自启动项名（卸载时删除）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallState {
    pub state_id: Uuid,
    pub product_code: String,
    pub version: String,
    pub installed_at: OffsetDateTime,
    #[serde(default)]
    pub modules: Vec<InstalledModule>,
    #[serde(default)]
    pub created_shortcuts: Vec<CreatedShortcut>,
    #[serde(default)]
    pub firewall_rules: Vec<String>,
    #[serde(default)]
    pub service_name: Option<String>,
    #[serde(default)]
    pub autorun_name: Option<String>,
}

impl InstallState {
    /// 创建一份新的安装状态。
    ///
    /// 参数：
    /// - `product_code`：产品标识
    /// - `version`：版本号
    ///
    /// 返回值：
    /// - 初始化后的 [`InstallState`]，其中 `state_id` 为随机 UUID，`installed_at` 为当前 UTC 时间。
    pub fn new(product_code: String, version: String) -> Self {
        Self {
            state_id: Uuid::new_v4(),
            product_code,
            version,
            installed_at: OffsetDateTime::now_utc(),
            modules: Vec::new(),
            created_shortcuts: Vec::new(),
            firewall_rules: Vec::new(),
            service_name: None,
            autorun_name: None,
        }
    }
}

/// 已安装模块信息（用于展示/卸载辅助）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModule {
    /// 模块 ID（清单中的 `modules[].id`）。
    pub id: String,
    /// 模块显示名称（清单中的 `modules[].display_name`）。
    pub display_name: String,
    /// 模块类型描述（MSI/EXE/FileCopy 等）。
    pub kind: String,
    #[serde(default)]
    /// 是否已安装（部分场景会写入“检测为已安装但未执行安装”的状态）。
    pub installed: bool,
    #[serde(default)]
    /// 安装根目录（可用于统一入口定位）。
    pub install_root: Option<String>,
    #[serde(default)]
    /// 卸载提示（预留字段，可用于写入卸载参数/注意事项）。
    pub uninstall_hint: Option<String>,
}

/// 安装过程中创建的快捷方式记录。
///
/// 用途：
/// - 卸载时按记录删除，避免误删用户自建快捷方式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedShortcut {
    /// 创建位置（例如 `desktop` / `start_menu`）。
    pub location: String,
    /// 快捷方式文件完整路径（`.lnk`）。
    pub path: String,
}

