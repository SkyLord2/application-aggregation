//! 本机 IPC 协议定义（请求/响应）。
//!
//! 协议形态：
//! - 以 JSON 序列化 [`IpcRequest`] / [`IpcResponse`]，按“单行一条消息”的方式传输
//! - 每条消息携带 `request_id` 用于请求-响应关联
//!
//! 约束与注意事项：
//! - `message` 字段不应包含敏感信息（密钥/令牌明文等）
//! - 若未来迁移到 Named Pipe/HTTP，本协议仍可复用
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// IPC 请求消息。
///
/// 序列化格式：
/// - 使用 `#[serde(tag = "type")]`，在 JSON 中通过 `type` 字段区分请求类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
    /// 连通性探测。
    ///
    /// 参数：
    /// - `request_id`：请求 ID
    Ping { request_id: Uuid },
    /// 获取单点登录（SSO）令牌。
    ///
    /// 参数：
    /// - `request_id`：请求 ID
    /// - `subject`：令牌主体（用户/应用标识）
    GetSsoToken { request_id: Uuid, subject: String },
    /// 获取应用运行状态。
    ///
    /// 参数：
    /// - `request_id`：请求 ID
    /// - `app_id`：应用/插件 ID（通常对应插件文件名）
    GetAppStatus { request_id: Uuid, app_id: String },
}

/// IPC 响应消息。
///
/// 异常处理：
/// - 通用错误通过 [`IpcResponse::Error`] 返回；`request_id` 应尽量回传原始请求 ID。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    /// `Ping` 的响应。
    Pong { request_id: Uuid },
    /// `GetSsoToken` 的响应。
    SsoToken {
        request_id: Uuid,
        token: String,
        expires_at_unix: i64,
    },
    /// `GetAppStatus` 的响应。
    AppStatus {
        request_id: Uuid,
        app_id: String,
        running: bool,
    },
    /// 请求处理失败的通用错误。
    ///
    /// 参数：
    /// - `request_id`：请求 ID
    /// - `message`：错误描述（避免包含敏感信息）
    Error { request_id: Uuid, message: String },
}
