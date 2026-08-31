//! admin bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use crate::{
    error::{AppError, AppResult},
    infra::{
        email::{SmtpEmailConfig, VerificationCodeTemplate, parse_smtp_security},
        secrets::decrypt_optional_secret,
    },
    modules::admin::{
        presentation::{
            AdminAgentCommissionResponse, AdminAgentCommissionRuleResponse, AdminAgentResponse,
            AdminAgentUserResponse, AdminAssetResponse, AdminAuditLogResponse,
            AdminCountryResponse, AdminDashboardAuditAction, AdminDashboardProductsSummary,
            AdminDashboardRiskSummary, AdminDashboardTradingSummary, AdminDashboardUsersSummary,
            AdminDashboardWalletSummary, AdminDepositAddressPoolResponse,
            AdminDepositNetworkConfigResponse, AdminMarginLiquidationResponse,
            AdminMarketStrategyNodeResponse, AdminMarketStrategyResponse, AdminNewsItemResponse,
            AdminTradingPairResponse, AdminUserReferralResponse, AdminUserResponse,
            AdminWalletAccountResponse, AdminWalletLedgerResponse, ConvertOrderResponse,
            ConvertPairResponse, MarketSourceCredentialSecret, MarketStrategyNodeRequest,
            MarketStrategyRecoveryJobResponse, NewCoinConvertRuleResponse,
            NewCoinDistributionResponse, NewCoinLockPositionResponse, NewCoinProjectResponse,
            NewCoinPurchaseResponse, NewCoinSubscriptionResponse, NewCoinUnlockResponse,
            RiskEventResponse, RiskRuleResponse, UploadConfigResponse, UploadFileInput,
            UploadImageResponse,
        },
        repository::{
            AdminAgentAdminUserWrite, AdminAgentWrite, AdminMarketFeedConfigRecord,
            AdminMarketFeedConfigWrite, AdminMarketSourceCredentialRecord,
            AdminMarketSourceCredentialWrite, AdminNewCoinLedgerWrite,
            AdminNewCoinLockPositionWrite, AdminSmtpConfigRecord, AdminSmtpConfigWrite,
            AdminSmtpDeliverySettingsRecord, AdminUploadConfigRecord, AdminUploadConfigWrite,
            AdminUploadObjectWrite, AgentCommissionPayoutTarget, RiskRuleWrite,
            UserAgentReferralWrite,
        },
        service::{
            DEFAULT_MARKET_FEED_CONFIG_NAME, DEFAULT_SMTP_CONFIG_NAME, DEFAULT_UPLOAD_FILE_FIELD,
            MARKET_SOURCE_AUTH_TYPE_API_KEY, MARKET_SOURCE_AUTH_TYPE_NONE,
            SMTP_DELIVERY_SETTINGS_ID, SMTP_DELIVERY_STRATEGY_ROUND_ROBIN, UPLOAD_IMAGE_MIME_TYPES,
            UploadProvider, default_smtp_delivery_settings_record, generated_upload_object_key,
            hmac_sha1_base64, join_upload_endpoint_path, join_upload_public_url,
            s3_upload_signature, safe_upload_filename, safe_upload_key_segment,
            safe_upload_response_url, sanitize_market_feed_reload_error,
            select_smtp_delivery_config, sha256_hex, smtp_templates_from_record, upload_url_host,
            validate_upload_file,
        },
    },
    modules::agent::domain::AgentHierarchyNode,
    modules::market::adapters::MarketFeedProvider,
    modules::security::{USER_SECURITY_POLICY_KEY, UserSecurityPolicy, UserTwoFactorSettings},
    modules::user::service::generate_user_invite_code,
    modules::wallet::WithdrawFeeTier,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};
use std::{collections::BTreeSet, path::PathBuf};
use uuid::Uuid;

mod access_control;
mod agents;
mod config_center;
mod config_changes;
mod convert;
mod dashboard_audit;
mod financial_idempotency;
mod margin;
mod market;
mod market_feed;
mod market_settings;
mod new_coin;
mod news;
mod risk_security;
mod system_config;
mod users;
mod wallet_assets;

pub(crate) use self::access_control::*;
pub(crate) use self::agents::*;
pub(crate) use self::config_center::*;
pub(crate) use self::config_changes::*;
pub(crate) use self::convert::*;
pub(crate) use self::dashboard_audit::*;
pub(crate) use self::financial_idempotency::*;
pub(crate) use self::margin::*;
pub(crate) use self::market::*;
pub(crate) use self::market_feed::*;
pub(crate) use self::market_settings::*;
pub(crate) use self::new_coin::*;
pub(crate) use self::news::*;
pub(crate) use self::risk_security::*;
pub(crate) use self::system_config::*;
pub(crate) use self::users::*;
pub(crate) use self::wallet_assets::*;

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 行查询与 COUNT 查询必须由同一组过滤谓词构建，返回总数才能与当前筛选一致。
async fn fetch_admin_page<T>(
    pool: &Pool<MySql>,
    mut rows: QueryBuilder<'_, MySql>,
    mut total: QueryBuilder<'_, MySql>,
    order_by: &str,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<T>, i64)>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    rows.push(order_by);
    rows.push(" LIMIT ");
    rows.push_bind(limit as i64);
    rows.push(" OFFSET ");
    rows.push_bind(offset as i64);

    let items = rows.build_query_as::<T>().fetch_all(pool).await?;
    let total = total.build_query_scalar::<i64>().fetch_one(pool).await?;

    Ok((items, total))
}

fn is_mysql_duplicate_key(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(database_error) = error else {
        return false;
    };
    if database_error.code().as_deref() == Some("1062") {
        return true;
    }
    database_error.code().as_deref() == Some("23000")
        && (database_error.message().contains("1062")
            || database_error.message().contains("Duplicate entry"))
}

fn optional_audit_reason(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
