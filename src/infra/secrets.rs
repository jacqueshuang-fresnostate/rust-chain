use crate::error::{AppError, AppResult};
use base64::{Engine, engine::general_purpose::STANDARD};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};

const NONCE_LEN: usize = 12;

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
