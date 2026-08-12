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

/// 解密可选密文字段；`None` 保持缺失，存在但无效的密文必须报错而非降级为缺失配置。
pub fn decrypt_optional_secret(ciphertext: Option<&str>, key: &str) -> AppResult<Option<String>> {
    ciphertext
        .map(|value| decrypt_secret(value, key))
        .transpose()
}

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
