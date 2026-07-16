//! security bounded context 聚合模块。
//!
//! 统一导出安全验证相关的领域、应用与基础设施能力，保留内聚而清晰的分层边界。
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;

pub use application::{
    ensure_login_challenge_usable, verify_user_security_action, verify_user_totp,
};
pub use domain::{
    CreatedLoginTwoFactorChallenge, LoginTwoFactorChallenge, LoginTwoFactorChallengeType,
    LoginTwoFactorMode, PaymentPolicies, PaymentPolicy, SecurityAction, SecurityVerificationInput,
    SecurityVerificationMethod, TOTP_DIGITS, TOTP_STEP_SECONDS, ThirdPartyBindingPolicy,
    UserSecurityPolicy, UserTwoFactorSettings, base32_decode_no_padding, base32_encode_no_padding,
    decode_security_policy_value, generate_totp_secret, totp_code_for_time, totp_otpauth_uri,
    verify_totp_code,
};
pub use infrastructure::{
    LOGIN_CHALLENGE_TTL_SECONDS, USER_SECURITY_POLICY_KEY, confirm_user_totp,
    consume_login_two_factor_challenge, create_login_two_factor_challenge,
    load_login_two_factor_challenge, load_security_policy, load_user_two_factor,
    reset_user_two_factor, save_pending_totp_secret, save_security_policy,
    set_user_login_two_factor,
};

#[cfg(test)]
#[path = "../../tests/unit_src/src_modules_security_tests.rs"]
mod tests;
