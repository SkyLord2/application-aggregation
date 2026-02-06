//! DPAPI（Windows 数据保护 API）封装。
//!
//! 用途：
//! - 将敏感数据（例如令牌签名密钥）绑定到“本机”进行加密落盘
//! - 使密钥即使被拷贝到其他机器也无法解密（LocalMachine 范围）
//!
//! 安全注意：
//! - DPAPI 并不替代权限控制；应确保密文文件的 ACL 合理
//! - 本实现未附带可选熵（entropy）；如需要更强隔离可扩展
//!
//! 作者：小海智能助手项目组（自动生成）
//! 创建时间：2026-02-04
//! 修改时间：2026-02-04

use anyhow::{Context, Result};
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_LOCAL_MACHINE, CRYPT_INTEGER_BLOB,
};

/// 使用 DPAPI（LocalMachine）加密字节数据。
///
/// 参数：
/// - `plain`：明文字节
///
/// 返回值：
/// - 加密后的密文字节（可安全落盘）
///
/// 异常处理：
/// - Win32 API 调用失败时返回错误
///
/// 安全/内存说明：
/// - `CryptProtectData` 返回的密文缓冲区由系统分配，需要使用 `LocalFree` 释放
pub fn protect_local_machine(plain: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let in_blob = CRYPT_INTEGER_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(
            &in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_LOCAL_MACHINE,
            &mut out_blob,
        )
        .ok()
        .context("CryptProtectData 失败")?;
        // 将系统分配的缓冲区复制到 Rust Vec，随后释放系统缓冲区，避免内存泄漏。
        let bytes = std::slice::from_raw_parts(out_blob.pbData as *const u8, out_blob.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out_blob.pbData as *mut core::ffi::c_void));
        Ok(bytes)
    }
}

/// 使用 DPAPI（LocalMachine）解密字节数据。
///
/// 参数：
/// - `cipher`：密文字节（由 [`protect_local_machine`] 生成）
///
/// 返回值：
/// - 解密后的明文字节
///
/// 异常处理：
/// - Win32 API 调用失败时返回错误（例如密文损坏、非本机生成的密文等）
///
/// 安全/内存说明：
/// - `CryptUnprotectData` 返回的明文缓冲区由系统分配，需要使用 `LocalFree` 释放
pub fn unprotect_local_machine(cipher: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let in_blob = CRYPT_INTEGER_BLOB {
            cbData: cipher.len() as u32,
            pbData: cipher.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(&in_blob, None, None, None, None, 0, &mut out_blob)
            .ok()
            .context("CryptUnprotectData 失败")?;
        // 将系统分配的缓冲区复制到 Rust Vec，随后释放系统缓冲区，避免内存泄漏。
        let bytes = std::slice::from_raw_parts(out_blob.pbData as *const u8, out_blob.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out_blob.pbData as *mut core::ffi::c_void));
        Ok(bytes)
    }
}

/// 加密字符串（UTF-8 字节）并返回密文。
///
/// 参数：
/// - `s`：待保护字符串
///
/// 返回值：
/// - 密文字节
///
/// 异常处理：
/// - 加密失败时返回错误
pub fn protect_string_local_machine(s: &str) -> Result<Vec<u8>> {
    protect_local_machine(s.as_bytes()).context("保护字符串失败")
}

