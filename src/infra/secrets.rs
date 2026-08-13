//! 凭据加解密与脱敏工具：后台配置里的第三方密钥、SMTP 口令等敏感值以密文入库，读写都要经过本模块。
//! 加密使用 AES-256-GCM 带认证标签，密钥固定为 32 字节，由全局配置中的凭据加密主密钥提供。
//! 密文对外形态是 base64 编码的随机数与密文标签拼接，格式自解释，因此换库或迁移时不需要额外元数据。
//! 所有失败路径都不回落明文：密钥错误、密文损坏或标签校验不过一律报错，宁可功能不可用也不降级。
//! 掩码函数只用于把密钥回显给管理端界面，产生的结果不可逆也不可比较，不能拿来做任何校验。

use crate::error::{AppError, AppResult};
use base64::{Engine, engine::general_purpose::STANDARD};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};

const NONCE_LEN: usize = 12;

/// 生成仅供展示的密钥掩码；短值全部隐藏，长值只保留首尾各四个字符。
/// 掩码不可用于鉴权、比较或恢复原文，调用方仍不得把未掩码值写入日志。
pub fn mask_secret(value: &str) -> String {
    let value = value.trim();
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        return "*".repeat(chars.len());
    }
    let prefix = chars.iter().take(4).collect::<String>();
    let suffix = chars[chars.len() - 4..].iter().collect::<String>();
    format!("{prefix}****{suffix}")
}

/// 使用 32 字节密钥和每次随机 nonce 进行 AES-256-GCM 加密，输出为 `base64(nonce || ciphertext || tag)`。
/// 相同明文重复加密应产生不同密文；密钥长度、随机数生成或封装失败均必须报错，禁止明文回退。
pub fn encrypt_secret(plaintext: &str, key: &str) -> AppResult<String> {
    let key_bytes = encryption_key_bytes(key)?;
    let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, key_bytes)
        .map_err(|_| AppError::Internal("credential encryption key is invalid".to_owned()))?;
    let key = aead::LessSafeKey::new(unbound_key);
    let rng = SystemRandom::new();
    let mut nonce_bytes = [0_u8; NONCE_LEN];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| AppError::Internal("credential nonce generation failed".to_owned()))?;
    let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);
    let mut in_out = plaintext.as_bytes().to_vec();
    key.seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut in_out)
        .map_err(|_| AppError::Internal("credential encryption failed".to_owned()))?;
    let mut output = nonce_bytes.to_vec();
    output.extend(in_out);
    Ok(STANDARD.encode(output))
}

/// 解码并认证 `encrypt_secret` 生成的密文；密钥错误、格式损坏、标签校验失败或非 UTF-8 明文统一拒绝。
/// 只有认证成功的字节才会返回，调用方不得在错误日志中附带密文、密钥或解密中间值。
pub fn decrypt_secret(ciphertext: &str, key: &str) -> AppResult<String> {
    let key_bytes = encryption_key_bytes(key)?;
    let mut payload = STANDARD
        .decode(ciphertext)
        .map_err(|_| AppError::Validation("credential ciphertext is invalid".to_owned()))?;
    if payload.len() <= NONCE_LEN {
        return Err(AppError::Validation(
            "credential ciphertext is invalid".to_owned(),
        ));
    }
    let mut nonce_bytes = [0_u8; NONCE_LEN];
    nonce_bytes.copy_from_slice(&payload[..NONCE_LEN]);
    let mut encrypted = payload.split_off(NONCE_LEN);
    let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, key_bytes)
        .map_err(|_| AppError::Internal("credential encryption key is invalid".to_owned()))?;
    let key = aead::LessSafeKey::new(unbound_key);
    let plaintext = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce_bytes),
            aead::Aad::empty(),
            &mut encrypted,
        )
        .map_err(|_| AppError::Validation("credential ciphertext is invalid".to_owned()))?;
    String::from_utf8(plaintext.to_vec())
        .map_err(|_| AppError::Validation("credential plaintext is invalid utf8".to_owned()))
}

/// 更新可选密钥字段：非空新值会重新随机加密，空白或缺失输入则保留既有密文。
/// 该语义用于“留空表示不修改”的管理端表单，不代表清空密钥；显式删除应由独立业务操作处理。
pub fn encrypt_secret_field(
    key: &str,
    new_value: Option<&str>,
    existing_ciphertext: Option<String>,
) -> AppResult<Option<String>> {
    match new_value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    }) {
        Some(value) => encrypt_secret(value, key).map(Some),
        None => Ok(existing_ciphertext),
    }
}

/// 解密可选密文字段，未配置的 `None` 原样保持缺失，不会被当成解密失败。
/// 存在但无效的密文必须报错而不是降级成缺失配置，否则密钥轮换出错时会被误读为「该项从未配置」，
/// 进而让上层按未配置分支继续运行，掩盖掉真正的密钥问题。
pub fn decrypt_optional_secret(ciphertext: Option<&str>, key: &str) -> AppResult<Option<String>> {
    ciphertext
        .map(|value| decrypt_secret(value, key))
        .transpose()
}

/// 校验主密钥长度并借出其字节切片，是加密与解密共用的前置检查，保证两侧口径完全一致。
/// 长度按字节而非字符计算，必须正好 32 字节以匹配 AES-256；不足或超出都返回校验错误，不做补齐或截断。
/// 错误信息只说明长度要求，不带入密钥内容本身，避免出错时把密钥写进日志或响应。
fn encryption_key_bytes(key: &str) -> AppResult<&[u8]> {
    let key = key.as_bytes();
    if key.len() != 32 {
        return Err(AppError::Validation(
            "credential encryption key must be exactly 32 bytes".to_owned(),
        ));
    }
    Ok(key)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_infra_secrets_tests.rs"]
mod tests;
