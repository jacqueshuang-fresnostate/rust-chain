use super::*;

#[test]
fn default_security_policy_requires_fund_password_for_withdraw_only() {
    let policy = UserSecurityPolicy::default();
    assert_eq!(policy.login_2fa_mode, LoginTwoFactorMode::UserEnabled);
    assert!(policy.payment_policies.withdraw.enabled);
    assert_eq!(
        policy.payment_policies.withdraw.method,
        SecurityVerificationMethod::FundPassword
    );
    assert!(!policy.payment_policies.convert.enabled);
}

#[test]
fn security_policy_decodes_legacy_string_and_numeric_bools() {
    let policy = decode_security_policy_value(serde_json::json!({
        "login_2fa_mode": "user_enabled",
        "registration_invite_required": "0",
        "username_login_enabled": "1",
        "payment_policies": {
            "withdraw": { "enabled": "1", "method": "fund_password" },
            "spot_order": { "enabled": 0, "method": "fund_password" },
            "convert": { "enabled": "false", "method": "fund_password" },
            "earn_subscribe": { "enabled": true, "method": "two_factor" }
        },
        "third_party_bindings": {
            "coinbase_wallet_enabled": "1",
            "telegram_account_enabled": 0
        }
    }))
    .unwrap();

    assert!(!policy.registration_invite_required);
    assert!(policy.username_login_enabled);
    assert!(policy.payment_policies.withdraw.enabled);
    assert!(!policy.payment_policies.spot_order.enabled);
    assert!(!policy.payment_policies.convert.enabled);
    assert!(policy.payment_policies.earn_subscribe.enabled);
    assert!(policy.third_party_bindings.coinbase_wallet_enabled);
    assert!(!policy.third_party_bindings.telegram_account_enabled);
}

#[test]
fn totp_matches_rfc_6238_sha1_vector() {
    let secret = b"12345678901234567890";
    assert_eq!(totp_code_for_time(secret, 59, 30, 6), "287082");
    assert_eq!(totp_code_for_time(secret, 1_111_111_109, 30, 6), "081804");
}

#[test]
fn base32_roundtrip_preserves_random_secret_bytes() {
    let bytes = b"exchange-2fa-secret";
    let encoded = base32_encode_no_padding(bytes);
    assert_eq!(base32_decode_no_padding(&encoded).unwrap(), bytes);
}
