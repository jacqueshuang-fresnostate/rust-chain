use super::*;

#[test]
fn masks_secret_without_exposing_middle() {
    assert_eq!(mask_secret("abcd1234wxyz"), "abcd****wxyz");
    assert_eq!(mask_secret("用户abcdwxyz"), "用户ab****wxyz");
    assert_eq!(mask_secret("abcdefgh"), "********");
    assert_eq!(mask_secret("short"), "*****");
}

#[test]
fn encrypts_and_decrypts_secret() {
    let key = "0123456789abcdef0123456789abcdef";
    let ciphertext = encrypt_secret("secret-value", key).unwrap();

    assert_ne!(ciphertext, "secret-value");
    assert!(!ciphertext.contains("secret-value"));
    assert_eq!(decrypt_secret(&ciphertext, key).unwrap(), "secret-value");
}

#[test]
fn blank_secret_field_keeps_existing_ciphertext() {
    assert_eq!(
        encrypt_secret_field(
            "0123456789abcdef0123456789abcdef",
            Some("   "),
            Some("existing".to_owned()),
        )
        .unwrap(),
        Some("existing".to_owned())
    );
}
