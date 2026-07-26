//! admin bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::{
    architecture::ServiceLayer,
    config::Settings,
    error::{AppError, AppResult},
    infra::email::{VerificationCodeTemplate, parse_smtp_security, smtp_security_code},
    modules::{
        admin::presentation::{
            AdminAgentCommissionResponse, AdminAgentCommissionRuleResponse, AdminAgentResponse,
            AdminAssetResponse, AdminCountryResponse, AdminDepositAddressPoolResponse,
            AdminDepositNetworkConfigResponse, AdminMarketStrategyResponse, AdminNewsItemResponse,
            AdminTradingPairResponse, AdminUserRechargeRequest, AdminUserRechargeResponse,
            AdminUserReferralResponse, AdminUserResponse, ConvertPairResponse,
            CreateAdminUserRequest, CreateAgentRequest, CreateAssetRequest,
            CreateConvertPairRequest, CreateDepositAddressPoolEntryRequest,
            CreateMarketStrategyRequest, CreateNewCoinProjectRequest, CreateRiskRuleRequest,
            CreateTradingPairRequest, DistributeNewCoinRequest, MarketFeedConfigResponse,
            MarketSourceCredentialResponse, NewCoinConvertRuleResponse,
            NewCoinDistributionResponse, NewCoinProjectResponse, RiskRuleResponse,
            SaveSmtpConfigRequest, SaveUploadConfigRequest, SmtpConfigResponse,
            SmtpDeliverySettingsResponse, UpdateAssetRequest, UpdateMarketStrategyRequest,
            UpdateNewCoinPostListingPurchaseRequest, UpdateNewCoinUnlockFeeRuleRequest,
            UpdateNewCoinUnlockRuleRequest, UpdateTradingPairRequest, UploadFileInput,
            UpsertNewCoinConvertRuleRequest,
        },
        admin::repository::{
            AdminMarketFeedConfigRecord, AdminMarketSourceCredentialRecord,
            AdminNewCoinLockPositionWrite, AdminSmtpConfigRecord, AdminSmtpDeliverySettingsRecord,
            AdminUploadConfigRecord,
        },
        auth::domain::{required_string, validate_reset_password},
        auth::hash_password,
        countries::{
            ensure_default_locale_supported, normalize_country_code, normalize_locale,
            normalize_supported_locales,
        },
        market::{KlineUpsertKey, ValidatedMarketSymbol, adapters::MarketFeedProvider},
        new_coin::{LifecycleStatus, UnlockRule, UnlockSource, apply_unlock_rule},
        security::{PaymentPolicies, SecurityAction, UserSecurityPolicy, UserTwoFactorSettings},
        wallet::{WithdrawFeeTier, normalize_withdraw_fee_tiers},
    },
    state::AppState,
    workers::market_feed::{MarketFeedRuntimeConfig, MarketFeedRuntimeStatus},
};
use base64::{Engine as _, engine::general_purpose};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Digest;
use std::collections::HashSet;
use std::default::Default;
use uuid::Uuid;

mod agents;
mod convert;
mod market;
mod market_feed;
mod new_coin;
mod news;
mod risk_security;
mod system_config;
mod users;
mod wallet_assets;

pub(crate) use self::agents::*;
pub(crate) use self::convert::*;
pub(crate) use self::market::*;
pub use self::market_feed::*;
pub(crate) use self::new_coin::*;
pub(crate) use self::news::*;
pub(crate) use self::risk_security::*;
pub(crate) use self::system_config::*;
pub(crate) use self::users::*;
pub(crate) use self::wallet_assets::*;

const ADMIN_AUDIT_REASON_MAX_LEN: usize = 512;

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

pub(crate) fn required_admin_audit_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

fn optional_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 从 admin 认证 subject 中提取管理员 ID。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}
