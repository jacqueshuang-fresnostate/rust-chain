//! security bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::Sha1;

pub const TOTP_STEP_SECONDS: u64 = 30;
pub const TOTP_DIGITS: u32 = 6;

const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const DEFAULT_TOTP_SECRET_BYTES: usize = 20;

type HmacSha1 = Hmac<Sha1>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginTwoFactorMode {
    None,
    UserEnabled,
    Mandatory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityVerificationMethod {
    FundPassword,
    TwoFactor,
    FundPasswordAndTwoFactor,
}

impl SecurityVerificationMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FundPassword => "fund_password",
            Self::TwoFactor => "two_factor",
            Self::FundPasswordAndTwoFactor => "fund_password_and_two_factor",
        }
    }

    pub(crate) fn requires_fund_password(self) -> bool {
        matches!(self, Self::FundPassword | Self::FundPasswordAndTwoFactor)
    }

    pub(crate) fn requires_two_factor(self) -> bool {
        matches!(self, Self::TwoFactor | Self::FundPasswordAndTwoFactor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityAction {
    Withdraw,
    SpotOrder,
    Convert,
    EarnSubscribe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentPolicy {
    pub enabled: bool,
    pub method: SecurityVerificationMethod,
}

impl PaymentPolicy {
    fn disabled() -> Self {
        Self {
            enabled: false,
            method: SecurityVerificationMethod::FundPassword,
        }
    }

    fn fund_password_required() -> Self {
        Self {
            enabled: true,
            method: SecurityVerificationMethod::FundPassword,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentPolicies {
    pub withdraw: PaymentPolicy,
    pub spot_order: PaymentPolicy,
    pub convert: PaymentPolicy,
    pub earn_subscribe: PaymentPolicy,
}

impl PaymentPolicies {
    pub fn policy_for(&self, action: SecurityAction) -> &PaymentPolicy {
        match action {
            SecurityAction::Withdraw => &self.withdraw,
            SecurityAction::SpotOrder => &self.spot_order,
            SecurityAction::Convert => &self.convert,
            SecurityAction::EarnSubscribe => &self.earn_subscribe,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ThirdPartyBindingPolicy {
    #[serde(default)]
    pub coinbase_wallet_enabled: bool,
    #[serde(default)]
    pub telegram_account_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserSecurityPolicy {
    pub login_2fa_mode: LoginTwoFactorMode,
    #[serde(default)]
    pub registration_invite_required: bool,
    #[serde(default)]
    pub username_login_enabled: bool,
    pub payment_policies: PaymentPolicies,
    #[serde(default)]
    pub third_party_bindings: ThirdPartyBindingPolicy,
}

impl DomainLayer for UserSecurityPolicy {}

impl Default for UserSecurityPolicy {
    fn default() -> Self {
        Self {
            login_2fa_mode: LoginTwoFactorMode::UserEnabled,
            registration_invite_required: false,
            username_login_enabled: false,
            payment_policies: PaymentPolicies {
                withdraw: PaymentPolicy::fund_password_required(),
                spot_order: PaymentPolicy::disabled(),
                convert: PaymentPolicy::disabled(),
                earn_subscribe: PaymentPolicy::disabled(),
            },
            third_party_bindings: ThirdPartyBindingPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserTwoFactorSettings {
    pub user_id: u64,
    pub totp_secret_encrypted: Option<String>,
    pub totp_enabled: bool,
    pub login_2fa_enabled: bool,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub last_verified_at: Option<DateTime<Utc>>,
}

impl UserTwoFactorSettings {
    pub(crate) fn empty(user_id: u64) -> Self {
        Self {
            user_id,
            totp_secret_encrypted: None,
            totp_enabled: false,
            login_2fa_enabled: false,
            confirmed_at: None,
            last_verified_at: None,
        }
    }
}

impl DomainLayer for UserTwoFactorSettings {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginTwoFactorChallengeType {
    LoginTwoFactor,
    SetupTwoFactor,
}

impl LoginTwoFactorChallengeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoginTwoFactor => "login_2fa",
            Self::SetupTwoFactor => "setup_2fa",
        }
    }

    pub(crate) fn from_storage(value: &str) -> AppResult<Self> {
        match value {
            "login_2fa" => Ok(Self::LoginTwoFactor),
            "setup_2fa" => Ok(Self::SetupTwoFactor),
            _ => Err(AppError::Validation("invalid challenge type".to_owned())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoginTwoFactorChallenge {
    pub challenge_id: String,
    pub user_id: u64,
    pub challenge_type: LoginTwoFactorChallengeType,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreatedLoginTwoFactorChallenge {
    pub challenge_id: String,
    pub expires_at: DateTime<Utc>,
    pub expires_in_seconds: i64,
}

pub struct SecurityVerificationInput<'a> {
    pub fund_password: Option<&'a str>,
    pub totp_code: Option<&'a str>,
}

pub fn generate_totp_secret() -> AppResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; DEFAULT_TOTP_SECRET_BYTES];
    rng.fill(&mut bytes)
        .map_err(|_| AppError::Internal("failed to generate TOTP secret".to_owned()))?;
    Ok(base32_encode_no_padding(&bytes))
}

pub fn totp_otpauth_uri(issuer: &str, account: &str, secret: &str) -> String {
    let label = format!("{}:{}", issuer.trim(), account.trim());
    format!(
        "otpauth://totp/{}?secret={}&issuer={}&algorithm=SHA1&digits={}&period={}",
        percent_encode(&label),
        secret,
        percent_encode(issuer.trim()),
        TOTP_DIGITS,
        TOTP_STEP_SECONDS
    )
}

pub fn verify_totp_code(secret_base32: &str, code: &str, now: DateTime<Utc>) -> AppResult<bool> {
    let code = code.trim();
    if code.len() != TOTP_DIGITS as usize || !code.chars().all(|value| value.is_ascii_digit()) {
        return Ok(false);
    }
    let secret = base32_decode_no_padding(secret_base32)?;
    let timestamp = now.timestamp().max(0) as u64;
    for offset in [-1_i64, 0, 1] {
        let candidate_timestamp = if offset.is_negative() {
            timestamp.saturating_sub(TOTP_STEP_SECONDS)
        } else if offset.is_positive() {
            timestamp.saturating_add(TOTP_STEP_SECONDS)
        } else {
            timestamp
        };
        if totp_code_for_time(&secret, candidate_timestamp, TOTP_STEP_SECONDS, TOTP_DIGITS) == code
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn totp_code_for_time(secret: &[u8], timestamp: u64, step_seconds: u64, digits: u32) -> String {
    let counter = timestamp / step_seconds;
    hotp_code(secret, counter, digits)
}

pub fn base32_encode_no_padding(bytes: &[u8]) -> String {
    let mut output = String::new();
    let mut buffer = 0_u32;
    let mut bits_left = 0_u8;

    for byte in bytes {
        buffer = (buffer << 8) | u32::from(*byte);
        bits_left += 8;
        while bits_left >= 5 {
            let index = ((buffer >> (bits_left - 5)) & 0b11111) as usize;
            output.push(BASE32_ALPHABET[index] as char);
            bits_left -= 5;
        }
    }

    if bits_left > 0 {
        let index = ((buffer << (5 - bits_left)) & 0b11111) as usize;
        output.push(BASE32_ALPHABET[index] as char);
    }

    output
}

pub fn base32_decode_no_padding(value: &str) -> AppResult<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = 0_u32;
    let mut bits_left = 0_u8;

    for character in value.trim().chars() {
        if character == '=' {
            continue;
        }
        let index = base32_value(character).ok_or_else(|| {
            AppError::Validation("TOTP secret contains invalid base32 character".to_owned())
        })?;
        buffer = (buffer << 5) | u32::from(index);
        bits_left += 5;
        if bits_left >= 8 {
            output.push(((buffer >> (bits_left - 8)) & 0xff) as u8);
            bits_left -= 8;
        }
    }

    Ok(output)
}

pub fn decode_security_policy_value(value: Value) -> AppResult<UserSecurityPolicy> {
    let mut value = match value {
        Value::Null => return Ok(UserSecurityPolicy::default()),
        Value::String(text) if text.trim_start().starts_with('{') => {
            serde_json::from_str::<Value>(&text).map_err(|error| {
                AppError::Internal(format!(
                    "failed to parse user security policy JSON: {error}"
                ))
            })?
        }
        other => other,
    };
    normalize_security_policy_bool_fields(&mut value);
    serde_json::from_value(value).map_err(|error| {
        AppError::Internal(format!("failed to decode user security policy: {error}"))
    })
}

pub(crate) fn required_security_field(value: Option<&str>) -> AppResult<&str> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::security_validation("security_verification_required", "请完成安全校验")
        })
}

pub(crate) fn login_challenge_expired() -> AppError {
    AppError::security_validation("login_2fa_challenge_expired", "登录验证已过期，请重新登录")
}

fn normalize_security_policy_bool_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if is_security_policy_bool_key(key)
                    && let Some(normalized) = coerce_legacy_bool_value(child)
                {
                    *child = Value::Bool(normalized);
                    continue;
                }
                normalize_security_policy_bool_fields(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_security_policy_bool_fields(item);
            }
        }
        _ => {}
    }
}

fn is_security_policy_bool_key(key: &str) -> bool {
    key == "enabled" || key.ends_with("_enabled") || key == "registration_invite_required"
}

fn coerce_legacy_bool_value(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .and_then(|value| match value {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            }),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "0" | "false" | "no" | "off" => Some(false),
            "1" | "true" | "yes" | "on" => Some(true),
            _ => None,
        },
        _ => None,
    }
}

fn hotp_code(secret: &[u8], counter: u64, digits: u32) -> String {
    let mut mac = HmacSha1::new_from_slice(secret).expect("HMAC supports variable key length");
    mac.update(&counter.to_be_bytes());
    let hash = mac.finalize().into_bytes();
    let offset = usize::from(hash[hash.len() - 1] & 0x0f);
    let binary = ((u32::from(hash[offset]) & 0x7f) << 24)
        | (u32::from(hash[offset + 1]) << 16)
        | (u32::from(hash[offset + 2]) << 8)
        | u32::from(hash[offset + 3]);
    let modulus = 10_u32.pow(digits);
    format!("{:0width$}", binary % modulus, width = digits as usize)
}

fn base32_value(character: char) -> Option<u8> {
    match character.to_ascii_uppercase() {
        'A'..='Z' => Some(character.to_ascii_uppercase() as u8 - b'A'),
        '2'..='7' => Some(character as u8 - b'2' + 26),
        _ => None,
    }
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
