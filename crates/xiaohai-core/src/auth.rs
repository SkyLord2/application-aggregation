//! 单点登录（SSO）令牌：签发与校验。
//!
//! 令牌格式（文本）：
//! - `v1.<payload_b64url>.<sig_b64url>`
//! - payload 为 JSON 序列化后的 [`TokenClaims`]
//! - sig 为 `HMAC-SHA256(secret, payload)` 的结果
//!
//! 设计目标：
//! - 便于在本机 IPC/HTTP 场景下快速签发短期令牌
//! - 避免引入复杂的 PKI/JWT 依赖（此处是轻量定制格式）
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

/// HMAC-SHA256 签名算法别名（用于令牌签名）。
type HmacSha256 = Hmac<Sha256>;

/// 令牌载荷（Claims）。
///
/// 字段说明：
/// - `token_id`：令牌唯一 ID，用于审计/去重（如需）
/// - `subject`：令牌主体（通常是用户/应用标识）
/// - `product_code`：产品线/套件标识，用于多产品隔离
/// - `issued_at_unix`：签发时间（Unix 秒）
/// - `expires_at_unix`：过期时间（Unix 秒）
///
/// 异常处理：
/// - 时间戳解析失败时，会回退到 `UNIX_EPOCH`（见 [`TokenClaims::issued_at`] / [`TokenClaims::expires_at`]）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub token_id: Uuid,
    pub subject: String,
    pub product_code: String,
    pub issued_at_unix: i64,
    pub expires_at_unix: i64,
}

impl TokenClaims {
    /// 将 `issued_at_unix` 转换为 [`OffsetDateTime`]。
    ///
    /// 返回值：
    /// - 解析成功：对应时间
    /// - 解析失败：返回 `UNIX_EPOCH` 作为降级值（避免因坏数据 panic）
    pub fn issued_at(&self) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(self.issued_at_unix).unwrap_or_else(|_| OffsetDateTime::UNIX_EPOCH)
    }

    /// 将 `expires_at_unix` 转换为 [`OffsetDateTime`]。
    ///
    /// 返回值：
    /// - 解析成功：对应时间
    /// - 解析失败：返回 `UNIX_EPOCH` 作为降级值（避免因坏数据 panic）
    pub fn expires_at(&self) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(self.expires_at_unix).unwrap_or_else(|_| OffsetDateTime::UNIX_EPOCH)
    }
}

/// 令牌校验错误类型。
///
/// 用途：
/// - 用于 IPC/HTTP 返回明确的失败原因（格式错误、解码失败、签名错误、过期等）。
#[derive(Debug, Error)]
pub enum TokenError {
    #[error("令牌格式不正确")]
    BadFormat,
    #[error("令牌解码失败")]
    Decode,
    #[error("令牌签名校验失败")]
    BadSignature,
    #[error("令牌已过期")]
    Expired,
    #[error("令牌尚未生效")]
    NotYetValid,
}

/// 令牌签发器。
///
/// 安全注意：
/// - `secret` 必须来自安全随机源，并应使用 OS 级保护（本项目在 Windows 下用 DPAPI 加密落盘）。
/// - `secret` 仅用于 HMAC，不应输出到日志。
#[derive(Debug, Clone)]
pub struct TokenIssuer {
    secret: Vec<u8>,
    product_code: String,
}

impl TokenIssuer {
    /// 创建签发器。
    ///
    /// 参数：
    /// - `secret`：HMAC 密钥（建议 32 字节以上）
    /// - `product_code`：产品标识（写入 claims，用于多套件隔离）
    pub fn new(secret: Vec<u8>, product_code: String) -> Self {
        Self { secret, product_code }
    }

    /// 签发一个短期令牌。
    ///
    /// 参数：
    /// - `subject`：主体标识（用户/应用/会话等）
    /// - `ttl`：有效期（从当前 UTC 时间起算）
    ///
    /// 返回值：
    /// - 符合 `v1.<payload>.<sig>` 格式的字符串
    ///
    /// 异常处理：
    /// - 该函数返回 `String`，内部使用 `expect` 断言序列化与 HMAC 初始化不会失败；
    ///   若未来需要将错误返回给调用方，可将签发接口调整为 `Result<String, TokenError>`。
    pub fn issue(&self, subject: impl Into<String>, ttl: Duration) -> String {
        let now = OffsetDateTime::now_utc();
        let claims = TokenClaims {
            token_id: Uuid::new_v4(),
            subject: subject.into(),
            product_code: self.product_code.clone(),
            issued_at_unix: now.unix_timestamp(),
            expires_at_unix: (now + ttl).unix_timestamp(),
        };
        let payload = serde_json::to_vec(&claims).expect("claims serialize");

        let mut mac = HmacSha256::new_from_slice(&self.secret).expect("hmac key");
        mac.update(&payload);
        let sig = mac.finalize().into_bytes();

        format!(
            "v1.{}.{}",
            URL_SAFE_NO_PAD.encode(payload),
            URL_SAFE_NO_PAD.encode(sig)
        )
    }

    /// 校验令牌并返回解析后的 claims。
    ///
    /// 参数：
    /// - `token`：待校验令牌文本
    /// - `allowed_clock_skew`：允许的时钟偏差（用于处理端到端时间不一致）
    ///
    /// 返回值：
    /// - 成功：返回 [`TokenClaims`]
    /// - 失败：返回 [`TokenError`]
    ///
    /// 异常处理逻辑：
    /// - 格式错误（分段数不对、版本不对）：`BadFormat`
    /// - Base64 解码失败或 JSON 反序列化失败：`Decode`
    /// - HMAC 校验失败：`BadSignature`
    /// - 时间窗口校验失败：`Expired` / `NotYetValid`
    pub fn verify(&self, token: &str, allowed_clock_skew: Duration) -> Result<TokenClaims, TokenError> {
        // 期望格式：v1.payload.sig（分隔符为 '.'）
        let mut parts = token.split('.');
        let version = parts.next().ok_or(TokenError::BadFormat)?;
        if version != "v1" {
            return Err(TokenError::BadFormat);
        }
        let payload_b64 = parts.next().ok_or(TokenError::BadFormat)?;
        let sig_b64 = parts.next().ok_or(TokenError::BadFormat)?;
        if parts.next().is_some() {
            return Err(TokenError::BadFormat);
        }

        // payload/sig 都使用 URL-safe base64（无 padding），以便在 URL/命令行/配置中传递。
        let payload = URL_SAFE_NO_PAD
            .decode(payload_b64.as_bytes())
            .map_err(|_| TokenError::Decode)?;
        let sig = URL_SAFE_NO_PAD
            .decode(sig_b64.as_bytes())
            .map_err(|_| TokenError::Decode)?;

        // 先验签再反序列化，避免对不可信 payload 做昂贵/危险解析。
        let mut mac = HmacSha256::new_from_slice(&self.secret).map_err(|_| TokenError::BadSignature)?;
        mac.update(&payload);
        mac.verify_slice(&sig).map_err(|_| TokenError::BadSignature)?;

        let claims: TokenClaims = serde_json::from_slice(&payload).map_err(|_| TokenError::Decode)?;
        let now = OffsetDateTime::now_utc();
        let issued_at = claims.issued_at();
        let expires_at = claims.expires_at();
        // 使用 clock skew 放宽时间窗口：减少客户端/服务端时间不一致造成的误判。
        if now + allowed_clock_skew < issued_at {
            return Err(TokenError::NotYetValid);
        }
        if now - allowed_clock_skew > expires_at {
            return Err(TokenError::Expired);
        }
        Ok(claims)
    }
}

