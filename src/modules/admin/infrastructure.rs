//! admin bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use crate::{
    architecture::InfrastructureLayer,
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
            AdminMarketStrategyResponse, AdminNewsItemResponse, AdminTradingPairResponse,
            AdminUserReferralResponse, AdminUserResponse, AdminWalletAccountResponse,
            AdminWalletLedgerResponse, ConvertOrderResponse, ConvertPairResponse,
            MarketSourceCredentialSecret, NewCoinConvertRuleResponse, NewCoinDistributionResponse,
            NewCoinLockPositionResponse, NewCoinProjectResponse, NewCoinPurchaseResponse,
            NewCoinSubscriptionResponse, NewCoinUnlockResponse, RiskEventResponse,
            RiskRuleResponse, UploadConfigResponse, UploadFileInput, UploadImageResponse,
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
use axum::extract::Multipart;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};
use std::{collections::BTreeSet, path::PathBuf};
use uuid::Uuid;

const INTERNAL_USER_EMAIL_DOMAIN: &str = "@internal.local";
const INTERNAL_USER_EMAIL_PATTERN: &str = "%@internal.local";
const ADMIN_USER_INVITE_CODE_CREATE_ATTEMPTS: usize = 12;
const DEFAULT_UPLOAD_CONFIG_NAME: &str = "default";

#[derive(Debug)]
pub struct InfrastructureLayerMarker;

impl InfrastructureLayer for InfrastructureLayerMarker {}

#[derive(Debug)]
pub(crate) struct AdminCountryListFilter {
    pub(crate) country_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) registration_enabled: Option<bool>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminCountryInsert {
    pub(crate) country_code: String,
    pub(crate) country_name: String,
    pub(crate) remark: String,
    pub(crate) default_locale: String,
    pub(crate) supported_locales: Vec<String>,
    pub(crate) registration_enabled: bool,
    pub(crate) status: String,
    pub(crate) sort_order: i32,
}

#[derive(Debug)]
pub(crate) struct AdminCountryUpdate {
    pub(crate) country_name: String,
    pub(crate) remark: String,
    pub(crate) default_locale: String,
    pub(crate) supported_locales: Vec<String>,
    pub(crate) registration_enabled: bool,
    pub(crate) sort_order: i32,
}

#[derive(Debug)]
pub(crate) struct AdminAuditLogEntry {
    pub(crate) action: &'static str,
    pub(crate) target_type: &'static str,
    pub(crate) target_id: u64,
    pub(crate) before_json: Option<Value>,
    pub(crate) after_json: Option<Value>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug)]
pub(crate) struct AdminAuditLogListFilter {
    pub(crate) admin_id: Option<u64>,
    pub(crate) action: Option<String>,
    pub(crate) target_type: Option<String>,
    pub(crate) target_id: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminMarginLiquidationListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) pair_id: Option<u64>,
    pub(crate) position_id: Option<u64>,
    pub(crate) limit: u32,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminDashboardMarketCounts {
    pub(crate) active_pairs: i64,
    pub(crate) disabled_pairs: i64,
    pub(crate) external_pairs: i64,
    pub(crate) strategy_pairs: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct AdminMarketFeedConfigRow {
    id: u64,
    name: String,
    symbols_json: SqlxJson<Vec<String>>,
    intervals_json: SqlxJson<Vec<String>>,
    providers_json: SqlxJson<Vec<String>>,
    enabled: bool,
    version: u64,
    applied_version: Option<u64>,
    last_reload_status: Option<String>,
    last_reload_error: Option<String>,
    last_reloaded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct AdminMarketSourceCredentialRow {
    provider: String,
    auth_type: String,
    api_key_ciphertext: Option<String>,
    api_secret_ciphertext: Option<String>,
    passphrase_ciphertext: Option<String>,
    api_key_mask: Option<String>,
    enabled: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct AdminSmtpConfigRow {
    id: u64,
    name: String,
    host: String,
    port: u16,
    security: String,
    username_ciphertext: Option<String>,
    password_ciphertext: Option<String>,
    username_mask: Option<String>,
    from_email: String,
    from_name: Option<String>,
    verification_code_template_html: Option<String>,
    verification_code_templates_json: Option<SqlxJson<Vec<VerificationCodeTemplate>>>,
    enabled: bool,
    priority: u32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct AdminSmtpDeliverySettingsRow {
    strategy: String,
    round_robin_cursor: Option<u64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct AdminUploadConfigRow {
    id: u64,
    name: String,
    provider: String,
    endpoint: Option<String>,
    file_field: Option<String>,
    bearer_token_ciphertext: Option<String>,
    bearer_token_mask: Option<String>,
    access_key_ciphertext: Option<String>,
    access_key_mask: Option<String>,
    secret_key_ciphertext: Option<String>,
    bucket: Option<String>,
    region: Option<String>,
    public_base_url: Option<String>,
    local_root: Option<String>,
    key_prefix: Option<String>,
    max_file_size_bytes: u64,
    allowed_mime_types_json: SqlxJson<Vec<String>>,
    enabled: bool,
}

#[derive(Debug)]
pub(crate) struct AdminUserListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) include_internal: bool,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminUserInsert {
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) password_hash: String,
    pub(crate) status: String,
    pub(crate) kyc_level: i32,
}

#[derive(Debug)]
pub(crate) struct AdminAgentListFilter {
    pub(crate) agent_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) level: Option<i32>,
    pub(crate) agent_code: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct AgentHierarchyNodeRow {
    id: u64,
    parent_agent_id: Option<u64>,
    root_agent_id: Option<u64>,
    level: i32,
    path: String,
    status: String,
}

impl TryFrom<AgentHierarchyNodeRow> for AgentHierarchyNode {
    type Error = AppError;

    fn try_from(row: AgentHierarchyNodeRow) -> Result<Self, Self::Error> {
        let root_agent_id = row.root_agent_id.ok_or_else(|| {
            AppError::Conflict("agent root hierarchy is not initialized".to_owned())
        })?;
        if row.path.is_empty() {
            return Err(AppError::Conflict(
                "agent path hierarchy is not initialized".to_owned(),
            ));
        }
        Ok(Self {
            id: row.id,
            parent_agent_id: row.parent_agent_id,
            root_agent_id,
            level: row.level,
            path: row.path,
            status: row.status,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminAssetSymbolRow {
    pub(crate) symbol: String,
    pub(crate) status: String,
}

#[derive(Debug)]
pub(crate) struct AdminNewsListFilter {
    pub(crate) status: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) country_code: Option<String>,
    pub(crate) locale: Option<String>,
    pub(crate) keyword: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewsInsert {
    pub(crate) title: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) status: String,
    pub(crate) country_code: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) content_json: Value,
    pub(crate) published_at: Option<DateTime<Utc>>,
    pub(crate) admin_id: u64,
}

#[derive(Debug)]
pub(crate) struct AdminNewsUpdate {
    pub(crate) title: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) country_code: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) content_json: Value,
    pub(crate) admin_id: u64,
}

#[derive(Debug)]
pub(crate) struct AdminNewsStatusUpdate {
    pub(crate) status: String,
    pub(crate) published_at: Option<DateTime<Utc>>,
    pub(crate) admin_id: u64,
}

#[derive(Debug)]
pub(crate) struct AdminAssetListFilter {
    pub(crate) symbol: Option<String>,
    pub(crate) asset_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminAssetInsert {
    pub(crate) symbol: String,
    pub(crate) name: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) asset_type: String,
    pub(crate) status: String,
    pub(crate) deposit_enabled: bool,
    pub(crate) withdraw_enabled: bool,
    pub(crate) min_deposit_amount: BigDecimal,
    pub(crate) deposit_fee: BigDecimal,
    pub(crate) withdraw_fee: BigDecimal,
    pub(crate) withdraw_fee_tiers: Vec<WithdrawFeeTier>,
}

#[derive(Debug)]
pub(crate) struct AdminAssetUpdate {
    pub(crate) name: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) asset_type: String,
    pub(crate) status: String,
    pub(crate) deposit_enabled: bool,
    pub(crate) withdraw_enabled: bool,
    pub(crate) min_deposit_amount: BigDecimal,
    pub(crate) deposit_fee: BigDecimal,
    pub(crate) withdraw_fee: BigDecimal,
    pub(crate) withdraw_fee_tiers: Vec<WithdrawFeeTier>,
}

#[derive(Debug)]
pub(crate) struct AdminWalletAccountListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) include_empty: bool,
    pub(crate) include_internal: bool,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminWalletLedgerListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) change_type: Option<String>,
    pub(crate) ref_type: Option<String>,
    pub(crate) include_internal: bool,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminDepositNetworkConfigListFilter {
    pub(crate) network: Option<String>,
    pub(crate) address_group_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminDepositNetworkConfigWrite {
    pub(crate) network: String,
    pub(crate) display_name: String,
    pub(crate) address_group_code: String,
    pub(crate) address_group_name: Option<String>,
    pub(crate) asset_symbols: Vec<String>,
    pub(crate) status: String,
    pub(crate) sort_order: i32,
}

#[derive(Debug)]
pub(crate) struct AdminDepositAddressPoolListFilter {
    pub(crate) network: Option<String>,
    pub(crate) address_group_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) assigned_user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) address: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminDepositAddressPoolWrite {
    pub(crate) network: String,
    pub(crate) address_group_code: String,
    pub(crate) address: String,
    pub(crate) asset_symbols: Vec<String>,
    pub(crate) status: String,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
}

#[derive(Debug)]
pub(crate) struct AdminAgentCommissionListFilter {
    pub(crate) agent_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminAgentCommissionRuleListFilter {
    pub(crate) agent_id: Option<u64>,
    pub(crate) product_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminRiskRuleListFilter {
    pub(crate) rule_type: Option<String>,
    pub(crate) target_type: Option<String>,
    pub(crate) enabled: Option<bool>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminRiskEventListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) decision: Option<String>,
    pub(crate) risk_level: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminAgentCommissionRuleWrite {
    pub(crate) agent_id: u64,
    pub(crate) product_type: String,
    pub(crate) commission_rate: BigDecimal,
    pub(crate) status: String,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminWalletRow {
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct AdminWalletEmptyAssetRow {
    asset_id: u64,
    asset_symbol: String,
}

#[derive(Debug)]
pub(crate) struct AdminTradingPairListFilter {
    pub(crate) symbol: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) market_type: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminTradingPairInsert {
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
}

#[derive(Debug)]
pub(crate) struct AdminTradingPairUpdate {
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyListFilter {
    pub(crate) pair_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyInsert {
    pub(crate) pair_id: u64,
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) status: String,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyUpdate {
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinFlatListFilter {
    pub(crate) project_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinLockPositionListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinUnlockListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) fee_paid_status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinProjectInsert {
    pub(crate) asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    pub(crate) listed_at: Option<DateTime<Utc>>,
    pub(crate) unlock_type: String,
    pub(crate) fixed_unlock_at: Option<DateTime<Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinUnlockRuleUpdate {
    pub(crate) unlock_type: String,
    pub(crate) listed_at: Option<DateTime<Utc>>,
    pub(crate) fixed_unlock_at: Option<DateTime<Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinUnlockFeeRuleUpdate {
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinConvertRuleWrite {
    pub(crate) convert_pair_id: u64,
    pub(crate) rate_source: String,
    pub(crate) fixed_rate: Option<BigDecimal>,
    pub(crate) floating_rate_json: Option<Value>,
    pub(crate) status: String,
    pub(crate) admin_id: u64,
}

#[derive(Debug)]
pub(crate) struct AdminConvertOrderListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminConvertPairInsert {
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) target_min_amount: BigDecimal,
    pub(crate) target_max_amount: Option<BigDecimal>,
    pub(crate) enabled: bool,
}

#[derive(Debug)]
pub(crate) struct AdminConvertPairUpdate {
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) target_min_amount: BigDecimal,
    pub(crate) target_max_amount: Option<BigDecimal>,
    pub(crate) enabled: bool,
}

pub(crate) async fn list_admin_users(
    pool: &Pool<MySql>,
    filter: AdminUserListFilter,
) -> AppResult<Vec<AdminUserResponse>> {
    let mut builder = admin_user_query();
    builder.push(" WHERE 1 = 1");
    if !filter.include_internal {
        push_exclude_internal_user_email(&mut builder, "users.email");
    }
    if let Some(user_id) = filter.user_id {
        builder.push(" AND users.id = ");
        builder.push_bind(user_id);
    }
    if let Some(email) = filter.email {
        builder.push(" AND users.email = ");
        builder.push_bind(email);
    }
    if let Some(status) = filter.status {
        builder.push(" AND users.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY users.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminUserResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_admin_user(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<AdminUserResponse> {
    let mut builder = admin_user_query();
    builder.push(" WHERE users.id = ");
    builder.push_bind(user_id);
    builder
        .build_query_as::<AdminUserResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_admin_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminUserInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO users (email, phone, password_hash, status, kyc_level)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(input.email.as_deref())
    .bind(input.phone.as_deref())
    .bind(&input.password_hash)
    .bind(&input.status)
    .bind(input.kyc_level)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_user_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn load_admin_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<AdminUserResponse> {
    let mut builder = admin_user_query();
    builder.push(" WHERE users.id = ");
    builder.push_bind(user_id);
    builder
        .build_query_as::<AdminUserResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn ensure_admin_user_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM users WHERE id = ? LIMIT 1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(())
}

pub(crate) async fn load_admin_user_two_factor_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<UserTwoFactorSettings> {
    let settings = sqlx::query_as::<_, UserTwoFactorSettings>(
        r#"SELECT user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled,
                  confirmed_at, last_verified_at
           FROM user_two_factor_settings
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(settings.unwrap_or_else(|| UserTwoFactorSettings::empty(user_id)))
}

pub(crate) async fn reset_admin_user_two_factor_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<UserTwoFactorSettings> {
    sqlx::query(
        r#"INSERT INTO user_two_factor_settings
              (user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled, confirmed_at, last_verified_at)
           VALUES (?, NULL, FALSE, FALSE, NULL, NULL)
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = NULL,
              totp_enabled = FALSE,
              login_2fa_enabled = FALSE,
              confirmed_at = NULL,
              last_verified_at = NULL"#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(UserTwoFactorSettings::empty(user_id))
}

pub(crate) async fn create_user_invite_code_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    for _ in 0..ADMIN_USER_INVITE_CODE_CREATE_ATTEMPTS {
        let code = generate_user_invite_code()?;
        let result = sqlx::query(
            r#"INSERT INTO invite_codes (owner_type, owner_id, code, status)
               VALUES ('user', ?, ?, 'active')"#,
        )
        .bind(user_id)
        .bind(&code)
        .execute(&mut **tx)
        .await;

        match result {
            Ok(_) => return Ok(()),
            Err(error) if is_mysql_duplicate_key(&error) => continue,
            Err(error) => return Err(AppError::from(error)),
        }
    }

    Err(AppError::Internal(
        "failed to create unique user invite code".to_owned(),
    ))
}

pub(crate) async fn list_admin_agents(
    pool: &Pool<MySql>,
    filter: AdminAgentListFilter,
) -> AppResult<Vec<AdminAgentResponse>> {
    let mut builder = admin_agent_query();
    builder.push(" WHERE 1 = 1");
    if let Some(agent_id) = filter.agent_id {
        builder.push(" AND agents.id = ");
        builder.push_bind(agent_id);
    }
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "agents.user_id", user_id);
    }
    if let Some(parent_agent_id) = filter.parent_agent_id {
        builder.push(" AND agents.parent_agent_id = ");
        builder.push_bind(parent_agent_id);
    }
    if let Some(root_agent_id) = filter.root_agent_id {
        builder.push(" AND COALESCE(agents.root_agent_id, agents.id) = ");
        builder.push_bind(root_agent_id);
    }
    if let Some(level) = filter.level {
        builder.push(" AND agents.level = ");
        builder.push_bind(level);
    }
    if let Some(agent_code) = filter.agent_code {
        builder.push(" AND agents.agent_code = ");
        builder.push_bind(agent_code);
    }
    push_user_email_filter(&mut builder, "agents.user_id", filter.email);
    if let Some(status) = filter.status {
        builder.push(" AND agents.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY agents.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    builder.push(" OFFSET ");
    builder.push_bind(filter.offset as i64);

    Ok(builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_admin_agent(
    pool: &Pool<MySql>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let mut builder = admin_agent_query();
    builder.push(" WHERE agents.id = ");
    builder.push_bind(agent_id);
    builder.push(" ORDER BY agent_admin_users.id ASC LIMIT 1");
    builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_admin_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminAgentWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO agents
              (user_id, parent_agent_id, root_agent_id, agent_code, level, path, status)
           VALUES (?, ?, ?, ?, ?, '', 'active')"#,
    )
    .bind(input.user_id)
    .bind(input.parent_agent_id)
    .bind(input.root_agent_id)
    .bind(input.agent_code)
    .bind(input.level)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_agent_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn finalize_admin_agent_hierarchy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
    root_agent_id: u64,
    path: &str,
) -> AppResult<()> {
    let result = sqlx::query("UPDATE agents SET root_agent_id = ?, path = ? WHERE id = ?")
        .bind(root_agent_id)
        .bind(path)
        .bind(agent_id)
        .execute(&mut **tx)
        .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "agent hierarchy initialization changed concurrently".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) async fn lock_active_agent_hierarchy_node_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<AgentHierarchyNode> {
    let row = sqlx::query_as::<_, AgentHierarchyNodeRow>(
        r#"SELECT id, parent_agent_id, root_agent_id, level, path, status
           FROM agents
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let node = AgentHierarchyNode::try_from(row)?;

    // 创建下级代理时锁定整条祖先链，避免父级并发停用后仍创建出可登录账号。
    let ancestor_statuses = sqlx::query_scalar::<_, String>(
        r#"SELECT status
           FROM agents
           WHERE path = ? OR ? LIKE CONCAT(path, '/%')
           ORDER BY level ASC, id ASC
           FOR UPDATE"#,
    )
    .bind(&node.path)
    .bind(&node.path)
    .fetch_all(&mut **tx)
    .await?;
    if ancestor_statuses.is_empty() || ancestor_statuses.iter().any(|status| status != "active") {
        return Err(AppError::Conflict(
            "parent agent hierarchy must be active".to_owned(),
        ));
    }
    Ok(node)
}

pub(crate) async fn insert_agent_admin_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminAgentAdminUserWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO agent_admin_users (agent_id, username, password_hash, status)
           VALUES (?, ?, ?, 'active')"#,
    )
    .bind(input.agent_id)
    .bind(input.username)
    .bind(input.password_hash)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_agent_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn load_admin_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let mut builder = admin_agent_query();
    builder.push(" WHERE agents.id = ");
    builder.push_bind(agent_id);
    builder.push(" ORDER BY agent_admin_users.id ASC LIMIT 1");
    builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let mut builder = admin_agent_query();
    builder.push(" WHERE agents.id = ");
    builder.push_bind(agent_id);
    builder.push(" ORDER BY agent_admin_users.id ASC LIMIT 1 FOR UPDATE");
    builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn update_admin_agent_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE agents SET status = ? WHERE id = ?")
        .bind(status)
        .bind(agent_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn update_agent_admin_users_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE agent_admin_users SET status = ? WHERE agent_id = ?")
        .bind(status)
        .bind(agent_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn list_admin_agent_users(
    pool: &Pool<MySql>,
    agent_id: u64,
    limit: u32,
) -> AppResult<Vec<AdminAgentUserResponse>> {
    Ok(sqlx::query_as::<_, AdminAgentUserResponse>(
        r#"SELECT users.id AS user_id, users.email, users.phone, users.status, users.kyc_level,
                  owner_agents.id AS owner_agent_id, referrals.root_agent_id,
                  owner_agents.agent_code AS owner_agent_code,
                  owner_agents.level AS owner_agent_level,
                  referrals.direct_inviter_id, referrals.direct_inviter_type,
                  referrals.depth, referrals.path, referrals.created_at AS referred_at
           FROM user_referrals referrals
           INNER JOIN users ON users.id = referrals.user_id
           INNER JOIN agents owner_agents ON owner_agents.id = referrals.root_agent_id
           INNER JOIN agents scope_agent ON scope_agent.id = ?
           WHERE owner_agents.path = scope_agent.path
              OR owner_agents.path LIKE CONCAT(scope_agent.path, '/%')
           ORDER BY owner_agents.level ASC, referrals.depth ASC, users.id ASC
           LIMIT ?"#,
    )
    .bind(agent_id)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?)
}

pub(crate) async fn lock_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<AdminUserReferralResponse>> {
    Ok(sqlx::query_as::<_, AdminUserReferralResponse>(
        r#"SELECT user_id, direct_inviter_id, direct_inviter_type,
                  root_agent_id, depth, path, created_at
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?)
}

pub(crate) async fn upsert_user_agent_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: UserAgentReferralWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, 'agent', ?, 1, ?)
           ON DUPLICATE KEY UPDATE direct_inviter_id = VALUES(direct_inviter_id),
                                   direct_inviter_type = VALUES(direct_inviter_type),
                                   root_agent_id = VALUES(root_agent_id),
                                   depth = VALUES(depth),
                                   path = VALUES(path)"#,
    )
    .bind(input.user_id)
    .bind(input.agent_id)
    .bind(input.agent_id)
    .bind(input.path)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<AdminUserReferralResponse> {
    sqlx::query_as::<_, AdminUserReferralResponse>(
        r#"SELECT user_id, direct_inviter_id, direct_inviter_type,
                  root_agent_id, depth, path, created_at
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn migrate_user_referral_descendants_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    old_path: &str,
    old_depth: i32,
    old_root_agent_id: Option<u64>,
    new_root_agent_id: u64,
    new_path: &str,
) -> AppResult<()> {
    // 使用旧 path 和旧 root_agent_id 同时定位子树，避免用户 id 与代理 id 前缀碰撞误迁移其他团队。
    sqlx::query(
        r#"UPDATE user_referrals
           SET root_agent_id = ?,
               depth = depth - ? + 1,
               path = CONCAT(?, SUBSTRING(path, CHAR_LENGTH(?) + 1))
           WHERE user_id <> ?
             AND path LIKE CONCAT(?, '/%')
             AND root_agent_id <=> ?"#,
    )
    .bind(new_root_agent_id)
    .bind(old_depth)
    .bind(new_path)
    .bind(old_path)
    .bind(user_id)
    .bind(old_path)
    .bind(old_root_agent_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_active_asset_symbol_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<AdminAssetSymbolRow> {
    let asset = sqlx::query_as::<_, AdminAssetSymbolRow>(
        "SELECT symbol, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != "active" {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

pub(crate) async fn list_admin_countries(
    pool: &Pool<MySql>,
    filter: AdminCountryListFilter,
) -> AppResult<Vec<AdminCountryResponse>> {
    let mut builder = admin_country_query();
    builder.push(" WHERE 1 = 1");
    if let Some(country_code) = filter.country_code {
        builder.push(" AND country_code = ");
        builder.push_bind(country_code);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(registration_enabled) = filter.registration_enabled {
        builder.push(" AND registration_enabled = ");
        builder.push_bind(registration_enabled);
    }
    builder.push(" ORDER BY sort_order ASC, country_code ASC LIMIT ");
    builder.push_bind(filter.limit as i64);
    builder.push(" OFFSET ");
    builder.push_bind(filter.offset as i64);

    Ok(builder
        .build_query_as::<AdminCountryResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn insert_admin_country_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminCountryInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO country_configs
           (country_code, country_name, remark, default_locale, supported_locales, registration_enabled, status, sort_order)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.country_code)
    .bind(&input.country_name)
    .bind(&input.remark)
    .bind(&input.default_locale)
    .bind(SqlxJson(input.supported_locales))
    .bind(input.registration_enabled)
    .bind(&input.status)
    .bind(input.sort_order)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_country_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_country_in_tx(
    tx: &mut Transaction<'_, MySql>,
    country_id: u64,
    input: AdminCountryUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE country_configs
           SET country_name = ?, remark = ?, default_locale = ?, supported_locales = ?, registration_enabled = ?, sort_order = ?
           WHERE id = ?"#,
    )
    .bind(&input.country_name)
    .bind(&input.remark)
    .bind(&input.default_locale)
    .bind(SqlxJson(input.supported_locales))
    .bind(input.registration_enabled)
    .bind(input.sort_order)
    .bind(country_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_country_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    country_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE country_configs SET status = ? WHERE id = ?")
        .bind(status)
        .bind(country_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn load_admin_country_in_tx(
    tx: &mut Transaction<'_, MySql>,
    country_id: u64,
) -> AppResult<AdminCountryResponse> {
    let mut builder = admin_country_query();
    builder.push(" WHERE id = ");
    builder.push_bind(country_id);
    builder
        .build_query_as::<AdminCountryResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_country_in_tx(
    tx: &mut Transaction<'_, MySql>,
    country_id: u64,
) -> AppResult<AdminCountryResponse> {
    let mut builder = admin_country_query();
    builder.push(" WHERE id = ");
    builder.push_bind(country_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminCountryResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_admin_audit_log_entry_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    entry: AdminAuditLogEntry,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(entry.action)
    .bind(entry.target_type)
    .bind(entry.target_id.to_string())
    .bind(entry.before_json.map(SqlxJson))
    .bind(entry.after_json.map(SqlxJson))
    .bind(optional_audit_reason(entry.reason))
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn load_admin_market_feed_config(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminMarketFeedConfigRecord>> {
    let row =
        sqlx::query_as::<_, AdminMarketFeedConfigRow>(&select_admin_market_feed_config_sql(false))
            .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(admin_market_feed_config_record))
}

pub(crate) async fn load_enabled_admin_market_feed_config_for_bootstrap(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminMarketFeedConfigRecord>> {
    Ok(load_admin_market_feed_config(pool)
        .await?
        .filter(|record| record.enabled))
}

pub(crate) async fn lock_admin_market_feed_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<Option<AdminMarketFeedConfigRecord>> {
    sqlx::query_as::<_, AdminMarketFeedConfigRow>(&select_admin_market_feed_config_sql(true))
        .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_market_feed_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn load_admin_market_feed_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<AdminMarketFeedConfigRecord> {
    sqlx::query_as::<_, AdminMarketFeedConfigRow>(&select_admin_market_feed_config_sql(false))
        .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
        .fetch_one(&mut **tx)
        .await
        .map(admin_market_feed_config_record)
        .map_err(AppError::Database)
}

pub(crate) async fn upsert_admin_market_feed_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminMarketFeedConfigWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO market_feed_configs
           (name, symbols_json, intervals_json, providers_json, enabled, version, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE symbols_json = VALUES(symbols_json),
                                   intervals_json = VALUES(intervals_json),
                                   providers_json = VALUES(providers_json),
                                   enabled = VALUES(enabled),
                                   version = VALUES(version),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
    .bind(SqlxJson(input.symbols))
    .bind(SqlxJson(input.intervals))
    .bind(SqlxJson(input.providers))
    .bind(input.enabled)
    .bind(input.version)
    .bind(input.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn list_admin_market_source_credentials(
    pool: &Pool<MySql>,
) -> AppResult<Vec<AdminMarketSourceCredentialRecord>> {
    let rows = sqlx::query_as::<_, AdminMarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           ORDER BY provider ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(admin_market_source_credential_record)
        .collect())
}

pub(crate) async fn lock_admin_market_source_credential_in_tx(
    tx: &mut Transaction<'_, MySql>,
    provider: &str,
) -> AppResult<Option<AdminMarketSourceCredentialRecord>> {
    let row = sqlx::query_as::<_, AdminMarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           WHERE provider = ?
           FOR UPDATE"#,
    )
    .bind(provider)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(admin_market_source_credential_record))
}

pub(crate) async fn load_admin_market_source_credential_in_tx(
    tx: &mut Transaction<'_, MySql>,
    provider: &str,
) -> AppResult<AdminMarketSourceCredentialRecord> {
    sqlx::query_as::<_, AdminMarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           WHERE provider = ?"#,
    )
    .bind(provider)
    .fetch_one(&mut **tx)
    .await
    .map(admin_market_source_credential_record)
    .map_err(AppError::Database)
}

pub(crate) async fn upsert_admin_market_source_credential_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminMarketSourceCredentialWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO market_source_credentials
           (provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
            passphrase_ciphertext, api_key_mask, enabled, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE auth_type = VALUES(auth_type),
                                   api_key_ciphertext = VALUES(api_key_ciphertext),
                                   api_secret_ciphertext = VALUES(api_secret_ciphertext),
                                   passphrase_ciphertext = VALUES(passphrase_ciphertext),
                                   api_key_mask = VALUES(api_key_mask),
                                   enabled = VALUES(enabled),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(&input.provider)
    .bind(&input.auth_type)
    .bind(&input.api_key_ciphertext)
    .bind(&input.api_secret_ciphertext)
    .bind(&input.passphrase_ciphertext)
    .bind(&input.api_key_mask)
    .bind(input.enabled)
    .bind(input.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_enabled_admin_market_source_credential_secrets(
    pool: &Pool<MySql>,
    providers: &[String],
    key: Option<&str>,
) -> AppResult<Vec<MarketSourceCredentialSecret>> {
    let rows = sqlx::query_as::<_, AdminMarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           WHERE enabled = TRUE"#,
    )
    .fetch_all(pool)
    .await?;
    let records: Vec<_> = rows
        .into_iter()
        .map(admin_market_source_credential_record)
        .collect();
    let mut selected = Vec::new();
    for provider in providers {
        let provider = MarketFeedProvider::from_code(provider)?.code().to_owned();
        if let Some(record) = records.iter().find(|record| record.provider == provider) {
            if record.auth_type == MARKET_SOURCE_AUTH_TYPE_API_KEY {
                let key = key.ok_or_else(|| {
                    AppError::Internal("credential encryption key is not configured".to_owned())
                })?;
                selected.push(MarketSourceCredentialSecret {
                    provider,
                    auth_type: record.auth_type.clone(),
                    api_key: decrypt_optional_secret(record.api_key_ciphertext.as_deref(), key)?,
                    api_secret: decrypt_optional_secret(
                        record.api_secret_ciphertext.as_deref(),
                        key,
                    )?,
                    passphrase: decrypt_optional_secret(
                        record.passphrase_ciphertext.as_deref(),
                        key,
                    )?,
                });
            } else {
                selected.push(MarketSourceCredentialSecret {
                    provider,
                    auth_type: MARKET_SOURCE_AUTH_TYPE_NONE.to_owned(),
                    api_key: None,
                    api_secret: None,
                    passphrase: None,
                });
            }
        }
    }
    Ok(selected)
}

pub(crate) async fn mark_admin_market_feed_reload_success(
    pool: &Pool<MySql>,
    version: u64,
) -> AppResult<AdminMarketFeedConfigRecord> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET applied_version = ?, last_reload_status = 'success', last_reload_error = NULL,
               last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = ?"#,
    )
    .bind(version)
    .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
    .execute(pool)
    .await?;
    load_admin_market_feed_config(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn mark_admin_market_feed_reload_skipped(
    pool: &Pool<MySql>,
    version: u64,
) -> AppResult<AdminMarketFeedConfigRecord> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET applied_version = ?, last_reload_status = 'skipped', last_reload_error = NULL,
               last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = ?"#,
    )
    .bind(version)
    .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
    .execute(pool)
    .await?;
    load_admin_market_feed_config(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn mark_admin_market_feed_reload_failed(
    pool: &Pool<MySql>,
    error: &str,
) -> AppResult<AdminMarketFeedConfigRecord> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET last_reload_status = 'failed', last_reload_error = ?, last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = ?"#,
    )
    .bind(sanitize_market_feed_reload_error(error))
    .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
    .execute(pool)
    .await?;
    load_admin_market_feed_config(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn load_admin_smtp_config(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    let row = sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql(
        "WHERE name = ?",
        false,
    ))
    .bind(DEFAULT_SMTP_CONFIG_NAME)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(admin_smtp_config_record))
}

pub(crate) async fn list_admin_smtp_configs(
    pool: &Pool<MySql>,
) -> AppResult<Vec<AdminSmtpConfigRecord>> {
    let rows = sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql(
        "ORDER BY priority ASC, id ASC",
        false,
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(admin_smtp_config_record).collect())
}

pub(crate) async fn load_admin_smtp_delivery_settings(
    pool: &Pool<MySql>,
) -> AppResult<AdminSmtpDeliverySettingsRecord> {
    let row = sqlx::query_as::<_, AdminSmtpDeliverySettingsRow>(
        "SELECT strategy, round_robin_cursor FROM smtp_delivery_settings WHERE id = ?",
    )
    .bind(SMTP_DELIVERY_SETTINGS_ID)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(admin_smtp_delivery_settings_record)
        .unwrap_or_else(default_smtp_delivery_settings_record))
}

pub(crate) async fn lock_admin_smtp_delivery_settings_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<AdminSmtpDeliverySettingsRecord> {
    let row = sqlx::query_as::<_, AdminSmtpDeliverySettingsRow>(
        "SELECT strategy, round_robin_cursor FROM smtp_delivery_settings WHERE id = ? FOR UPDATE",
    )
    .bind(SMTP_DELIVERY_SETTINGS_ID)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row
        .map(admin_smtp_delivery_settings_record)
        .unwrap_or_else(default_smtp_delivery_settings_record))
}

pub(crate) async fn upsert_admin_smtp_delivery_settings_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy: &str,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO smtp_delivery_settings (id, strategy, updated_by)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE strategy = VALUES(strategy), updated_by = VALUES(updated_by)"#,
    )
    .bind(SMTP_DELIVERY_SETTINGS_ID)
    .bind(strategy)
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn lock_admin_smtp_config_by_name_in_tx(
    tx: &mut Transaction<'_, MySql>,
    name: &str,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql("WHERE name = ?", true))
        .bind(name)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_smtp_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn lock_admin_smtp_config_by_id_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql("WHERE id = ?", true))
        .bind(config_id)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_smtp_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn load_admin_smtp_config_by_name_in_tx(
    tx: &mut Transaction<'_, MySql>,
    name: &str,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql("WHERE name = ?", false))
        .bind(name)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_smtp_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn load_admin_smtp_config_by_id_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql("WHERE id = ?", false))
        .bind(config_id)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_smtp_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn load_admin_smtp_config_by_id(
    pool: &Pool<MySql>,
    config_id: u64,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    let row = sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql(
        "WHERE id = ?",
        false,
    ))
    .bind(config_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(admin_smtp_config_record))
}

pub(crate) async fn admin_smtp_config_name_exists_except(
    tx: &mut Transaction<'_, MySql>,
    name: &str,
    config_id: u64,
) -> AppResult<bool> {
    let id = sqlx::query_scalar::<_, u64>(
        "SELECT id FROM smtp_configs WHERE name = ? AND id <> ? LIMIT 1",
    )
    .bind(name)
    .bind(config_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(id.is_some())
}

pub(crate) async fn insert_admin_smtp_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminSmtpConfigWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO smtp_configs
           (name, host, port, security, username_ciphertext, password_ciphertext,
            username_mask, from_email, from_name, verification_code_template_html,
            verification_code_templates_json, enabled, priority, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.name)
    .bind(&input.host)
    .bind(input.port)
    .bind(&input.security)
    .bind(&input.username_ciphertext)
    .bind(&input.password_ciphertext)
    .bind(&input.username_mask)
    .bind(&input.from_email)
    .bind(&input.from_name)
    .bind(&input.verification_code_template_html)
    .bind(SqlxJson(input.verification_code_templates))
    .bind(input.enabled)
    .bind(input.priority)
    .bind(input.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn upsert_default_admin_smtp_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminSmtpConfigWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO smtp_configs
           (name, host, port, security, username_ciphertext, password_ciphertext,
            username_mask, from_email, from_name, verification_code_template_html,
            verification_code_templates_json, enabled, priority, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE host = VALUES(host),
                                   port = VALUES(port),
                                   security = VALUES(security),
                                   username_ciphertext = VALUES(username_ciphertext),
                                   password_ciphertext = VALUES(password_ciphertext),
                                   username_mask = VALUES(username_mask),
                                   from_email = VALUES(from_email),
                                   from_name = VALUES(from_name),
                                   verification_code_template_html = VALUES(verification_code_template_html),
                                   verification_code_templates_json = VALUES(verification_code_templates_json),
                                   enabled = VALUES(enabled),
                                   priority = VALUES(priority),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(&input.name)
    .bind(&input.host)
    .bind(input.port)
    .bind(&input.security)
    .bind(&input.username_ciphertext)
    .bind(&input.password_ciphertext)
    .bind(&input.username_mask)
    .bind(&input.from_email)
    .bind(&input.from_name)
    .bind(&input.verification_code_template_html)
    .bind(SqlxJson(input.verification_code_templates))
    .bind(input.enabled)
    .bind(input.priority)
    .bind(input.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_smtp_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
    input: AdminSmtpConfigWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE smtp_configs
           SET name = ?, host = ?, port = ?, security = ?,
               username_ciphertext = ?, password_ciphertext = ?, username_mask = ?,
               from_email = ?, from_name = ?, verification_code_template_html = ?,
               verification_code_templates_json = ?, enabled = ?, priority = ?, updated_by = ?
           WHERE id = ?"#,
    )
    .bind(&input.name)
    .bind(&input.host)
    .bind(input.port)
    .bind(&input.security)
    .bind(&input.username_ciphertext)
    .bind(&input.password_ciphertext)
    .bind(&input.username_mask)
    .bind(&input.from_email)
    .bind(&input.from_name)
    .bind(&input.verification_code_template_html)
    .bind(SqlxJson(input.verification_code_templates))
    .bind(input.enabled)
    .bind(input.priority)
    .bind(input.updated_by)
    .bind(config_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn load_enabled_admin_smtp_config_records_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<Vec<AdminSmtpConfigRecord>> {
    let rows = sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql(
        "WHERE enabled = TRUE ORDER BY priority ASC, id ASC",
        false,
    ))
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows.into_iter().map(admin_smtp_config_record).collect())
}

pub(crate) async fn load_admin_smtp_config_for_delivery(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    let mut tx = pool.begin().await?;
    let settings = lock_admin_smtp_delivery_settings_in_tx(&mut tx).await?;
    let records = load_enabled_admin_smtp_config_records_in_tx(&mut tx).await?;
    let Some(record) = select_smtp_delivery_config(&settings, &records) else {
        tx.commit().await?;
        return Ok(None);
    };
    if settings.strategy == SMTP_DELIVERY_STRATEGY_ROUND_ROBIN {
        sqlx::query(
            r#"INSERT INTO smtp_delivery_settings (id, strategy, round_robin_cursor)
               VALUES (?, ?, ?)
               ON DUPLICATE KEY UPDATE round_robin_cursor = VALUES(round_robin_cursor)"#,
        )
        .bind(SMTP_DELIVERY_SETTINGS_ID)
        .bind(SMTP_DELIVERY_STRATEGY_ROUND_ROBIN)
        .bind(record.id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Some(record))
}

pub(crate) fn admin_smtp_email_config(
    record: &AdminSmtpConfigRecord,
    key: Option<&str>,
) -> AppResult<SmtpEmailConfig> {
    let key = if record.username_ciphertext.is_some() || record.password_ciphertext.is_some() {
        Some(key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?)
    } else {
        None
    };
    let username = match key {
        Some(key) => decrypt_optional_secret(record.username_ciphertext.as_deref(), key)?,
        None => None,
    };
    let password = match key {
        Some(key) => decrypt_optional_secret(record.password_ciphertext.as_deref(), key)?,
        None => None,
    };
    Ok(SmtpEmailConfig {
        host: record.host.clone(),
        port: record.port,
        security: parse_smtp_security(&record.security)?,
        username,
        password,
        from_email: record.from_email.clone(),
        from_name: record.from_name.clone(),
        verification_code_template_html: record.verification_code_template_html.clone(),
        verification_code_templates: smtp_templates_from_record(record),
    })
}

pub(crate) async fn load_enabled_admin_smtp_email_config(
    pool: &Pool<MySql>,
    key: Option<&str>,
) -> AppResult<Option<SmtpEmailConfig>> {
    load_admin_smtp_config_for_delivery(pool)
        .await?
        .map(|record| admin_smtp_email_config(&record, key))
        .transpose()
}

pub(crate) async fn load_admin_upload_config(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminUploadConfigRecord>> {
    let row = sqlx::query_as::<_, AdminUploadConfigRow>(&select_admin_upload_config_sql(false))
        .bind(DEFAULT_UPLOAD_CONFIG_NAME)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(admin_upload_config_record))
}

pub(crate) async fn lock_admin_upload_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<Option<AdminUploadConfigRecord>> {
    sqlx::query_as::<_, AdminUploadConfigRow>(&select_admin_upload_config_sql(true))
        .bind(DEFAULT_UPLOAD_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_upload_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn load_admin_upload_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<AdminUploadConfigRecord> {
    sqlx::query_as::<_, AdminUploadConfigRow>(&select_admin_upload_config_sql(false))
        .bind(DEFAULT_UPLOAD_CONFIG_NAME)
        .fetch_one(&mut **tx)
        .await
        .map(admin_upload_config_record)
        .map_err(AppError::Database)
}

pub(crate) async fn upsert_admin_upload_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminUploadConfigWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO upload_storage_configs
           (name, provider, endpoint, file_field, bearer_token_ciphertext, bearer_token_mask,
            access_key_ciphertext, access_key_mask, secret_key_ciphertext, bucket, region,
            public_base_url, local_root, key_prefix, max_file_size_bytes, allowed_mime_types_json,
            enabled, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE provider = VALUES(provider),
                                   endpoint = VALUES(endpoint),
                                   file_field = VALUES(file_field),
                                   bearer_token_ciphertext = VALUES(bearer_token_ciphertext),
                                   bearer_token_mask = VALUES(bearer_token_mask),
                                   access_key_ciphertext = VALUES(access_key_ciphertext),
                                   access_key_mask = VALUES(access_key_mask),
                                   secret_key_ciphertext = VALUES(secret_key_ciphertext),
                                   bucket = VALUES(bucket),
                                   region = VALUES(region),
                                   public_base_url = VALUES(public_base_url),
                                   local_root = VALUES(local_root),
                                   key_prefix = VALUES(key_prefix),
                                   max_file_size_bytes = VALUES(max_file_size_bytes),
                                   allowed_mime_types_json = VALUES(allowed_mime_types_json),
                                   enabled = VALUES(enabled),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(DEFAULT_UPLOAD_CONFIG_NAME)
    .bind(&input.provider)
    .bind(&input.endpoint)
    .bind(&input.file_field)
    .bind(&input.bearer_token_ciphertext)
    .bind(&input.bearer_token_mask)
    .bind(&input.access_key_ciphertext)
    .bind(&input.access_key_mask)
    .bind(&input.secret_key_ciphertext)
    .bind(&input.bucket)
    .bind(&input.region)
    .bind(&input.public_base_url)
    .bind(&input.local_root)
    .bind(&input.key_prefix)
    .bind(input.max_file_size_bytes)
    .bind(SqlxJson(input.allowed_mime_types))
    .bind(input.enabled)
    .bind(input.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_enabled_admin_upload_config(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminUploadConfigRecord>> {
    let row = sqlx::query_as::<_, AdminUploadConfigRow>(
        r#"SELECT id, name, provider, endpoint, file_field, bearer_token_ciphertext,
                  bearer_token_mask, access_key_ciphertext, access_key_mask, secret_key_ciphertext,
                  bucket, region, public_base_url, local_root, key_prefix, max_file_size_bytes,
                  allowed_mime_types_json, enabled
           FROM upload_storage_configs
           WHERE name = ? AND enabled = TRUE"#,
    )
    .bind(DEFAULT_UPLOAD_CONFIG_NAME)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(admin_upload_config_record))
}

pub(crate) fn admin_upload_config_response(
    record: AdminUploadConfigRecord,
) -> UploadConfigResponse {
    UploadConfigResponse {
        id: record.id,
        name: record.name,
        provider: record.provider,
        endpoint: record.endpoint,
        file_field: record.file_field,
        bearer_token_mask: record.bearer_token_mask,
        bearer_token_set: record.bearer_token_ciphertext.is_some(),
        access_key_mask: record.access_key_mask,
        access_key_set: record.access_key_ciphertext.is_some(),
        secret_key_set: record.secret_key_ciphertext.is_some(),
        bucket: record.bucket,
        region: record.region,
        public_base_url: record.public_base_url,
        local_root: record.local_root,
        key_prefix: record.key_prefix,
        max_file_size_bytes: record.max_file_size_bytes,
        allowed_mime_types: record.allowed_mime_types,
        enabled: record.enabled,
    }
}

pub(crate) async fn upload_admin_file_to_storage(
    record: &AdminUploadConfigRecord,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    validate_upload_file(
        record.max_file_size_bytes,
        &record.allowed_mime_types,
        input,
    )?;
    let provider = UploadProvider::parse(&record.provider)?;
    match provider {
        UploadProvider::ImageBed => upload_to_image_bed(record, key, input).await,
        UploadProvider::Local => upload_to_local(record, input).await,
        UploadProvider::S3 => upload_to_s3(record, key, input).await,
        UploadProvider::Oss => upload_to_oss(record, key, input).await,
    }
}

pub(crate) async fn insert_admin_upload_object(
    pool: &Pool<MySql>,
    input: AdminUploadObjectWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO upload_objects
           (provider, object_key, public_url, share_url, delete_url, mime_type, size_bytes,
            original_filename, uploaded_by, uploaded_by_user)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.provider)
    .bind(&input.object_key)
    .bind(&input.public_url)
    .bind(&input.share_url)
    .bind(&input.delete_url)
    .bind(&input.mime_type)
    .bind(input.size_bytes)
    .bind(&input.original_filename)
    .bind(input.owner.admin_id())
    .bind(input.owner.user_id())
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn save_admin_security_policy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    policy: &UserSecurityPolicy,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO security_policy_configs (policy_key, policy_value, updated_by)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE
               policy_value = VALUES(policy_value),
               updated_by = VALUES(updated_by)"#,
    )
    .bind(USER_SECURITY_POLICY_KEY)
    .bind(SqlxJson(policy.clone()))
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn list_admin_news_items(
    pool: &Pool<MySql>,
    filter: AdminNewsListFilter,
) -> AppResult<Vec<AdminNewsItemResponse>> {
    let mut builder = admin_news_query();
    builder.push(" WHERE 1 = 1");
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(category) = filter.category {
        builder.push(" AND category = ");
        builder.push_bind(category);
    }
    if let Some(country_code) = filter.country_code {
        builder.push(" AND country_code = ");
        builder.push_bind(country_code);
    }
    if let Some(locale) = filter.locale {
        builder.push(" AND JSON_SEARCH(content_json, 'one', ");
        builder.push_bind(locale);
        builder.push(", NULL, '$.items[*].locale') IS NOT NULL");
    }
    if let Some(keyword) = filter.keyword {
        builder.push(" AND (title LIKE ");
        builder.push_bind(format!("%{keyword}%"));
        builder.push(" OR CAST(content_json AS CHAR) LIKE ");
        builder.push_bind(format!("%{keyword}%"));
        builder.push(")");
    }
    builder.push(" ORDER BY updated_at DESC, id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    builder.push(" OFFSET ");
    builder.push_bind(filter.offset as i64);

    Ok(builder
        .build_query_as::<AdminNewsItemResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_admin_news_item(
    pool: &Pool<MySql>,
    news_id: u64,
) -> AppResult<AdminNewsItemResponse> {
    let mut builder = admin_news_query();
    builder.push(" WHERE id = ");
    builder.push_bind(news_id);
    builder
        .build_query_as::<AdminNewsItemResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_admin_news_item_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminNewsInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO admin_news_items
           (title, banner_url, small_logo_url, category, status, country_code, default_locale, content_json, published_at,
            created_by_admin_id, updated_by_admin_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.title)
    .bind(&input.banner_url)
    .bind(&input.small_logo_url)
    .bind(&input.category)
    .bind(&input.status)
    .bind(input.country_code.as_deref())
    .bind(&input.default_locale)
    .bind(SqlxJson(input.content_json))
    .bind(input.published_at)
    .bind(input.admin_id)
    .bind(input.admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_news_item_in_tx(
    tx: &mut Transaction<'_, MySql>,
    news_id: u64,
    input: AdminNewsUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_news_items
           SET title = ?, banner_url = ?, small_logo_url = ?, category = ?, country_code = ?, default_locale = ?, content_json = ?, updated_by_admin_id = ?
           WHERE id = ?"#,
    )
    .bind(&input.title)
    .bind(&input.banner_url)
    .bind(&input.small_logo_url)
    .bind(&input.category)
    .bind(input.country_code.as_deref())
    .bind(&input.default_locale)
    .bind(SqlxJson(input.content_json))
    .bind(input.admin_id)
    .bind(news_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_news_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    news_id: u64,
    input: AdminNewsStatusUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_news_items
           SET status = ?, published_at = ?, updated_by_admin_id = ?
           WHERE id = ?"#,
    )
    .bind(&input.status)
    .bind(input.published_at)
    .bind(input.admin_id)
    .bind(news_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_admin_news_item_in_tx(
    tx: &mut Transaction<'_, MySql>,
    news_id: u64,
) -> AppResult<AdminNewsItemResponse> {
    let mut builder = admin_news_query();
    builder.push(" WHERE id = ");
    builder.push_bind(news_id);
    builder
        .build_query_as::<AdminNewsItemResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_news_item_in_tx(
    tx: &mut Transaction<'_, MySql>,
    news_id: u64,
) -> AppResult<AdminNewsItemResponse> {
    let mut builder = admin_news_query();
    builder.push(" WHERE id = ");
    builder.push_bind(news_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminNewsItemResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn list_admin_assets(
    pool: &Pool<MySql>,
    filter: AdminAssetListFilter,
) -> AppResult<Vec<AdminAssetResponse>> {
    let mut builder = admin_asset_query();
    builder.push(" WHERE 1 = 1");
    if let Some(symbol) = filter.symbol {
        builder.push(" AND symbol = ");
        builder.push_bind(symbol);
    }
    if let Some(asset_type) = filter.asset_type {
        builder.push(" AND asset_type = ");
        builder.push_bind(asset_type);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminAssetResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_admin_asset(
    pool: &Pool<MySql>,
    asset_id: u64,
) -> AppResult<AdminAssetResponse> {
    let mut builder = admin_asset_query();
    builder.push(" WHERE id = ");
    builder.push_bind(asset_id);
    builder
        .build_query_as::<AdminAssetResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminAssetInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO assets
              (symbol, name, logo_url, precision_scale, asset_type, status, deposit_enabled, withdraw_enabled,
               min_deposit_amount, deposit_fee, withdraw_fee, withdraw_fee_tiers_json)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.symbol)
    .bind(&input.name)
    .bind(&input.logo_url)
    .bind(input.precision_scale)
    .bind(&input.asset_type)
    .bind(&input.status)
    .bind(input.deposit_enabled)
    .bind(input.withdraw_enabled)
    .bind(&input.min_deposit_amount)
    .bind(&input.deposit_fee)
    .bind(&input.withdraw_fee)
    .bind(SqlxJson(input.withdraw_fee_tiers))
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_asset_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
    input: AdminAssetUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE assets
           SET name = ?,
               logo_url = ?,
               precision_scale = ?,
               asset_type = ?,
               status = ?,
               deposit_enabled = ?,
               withdraw_enabled = ?,
               min_deposit_amount = ?,
               deposit_fee = ?,
               withdraw_fee = ?,
               withdraw_fee_tiers_json = ?
           WHERE id = ?"#,
    )
    .bind(&input.name)
    .bind(&input.logo_url)
    .bind(input.precision_scale)
    .bind(&input.asset_type)
    .bind(&input.status)
    .bind(input.deposit_enabled)
    .bind(input.withdraw_enabled)
    .bind(&input.min_deposit_amount)
    .bind(&input.deposit_fee)
    .bind(&input.withdraw_fee)
    .bind(SqlxJson(input.withdraw_fee_tiers))
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn delete_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn load_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<AdminAssetResponse> {
    let mut builder = admin_asset_query();
    builder.push(" WHERE id = ");
    builder.push_bind(asset_id);
    builder
        .build_query_as::<AdminAssetResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<AdminAssetResponse> {
    let mut builder = admin_asset_query();
    builder.push(" WHERE id = ");
    builder.push_bind(asset_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminAssetResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn create_wallet_accounts_for_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           SELECT id, ?, 0, 0, 0
           FROM users"#,
    )
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn delete_zero_balance_wallet_accounts_for_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"DELETE FROM wallet_accounts
           WHERE asset_id = ? AND available = 0 AND frozen = 0 AND locked = 0"#,
    )
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn ensure_asset_has_no_references_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    let (has_references,): (i64,) = sqlx::query_as(
        r#"SELECT CASE WHEN
                  EXISTS(SELECT 1 FROM wallet_accounts WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM wallet_ledger WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM asset_lock_positions WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM asset_unlock_records WHERE asset_id = ? OR unlock_fee_asset = ?)
               OR EXISTS(SELECT 1 FROM deposit_records WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM withdraw_records WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM trading_pairs WHERE base_asset = ? OR quote_asset = ?)
               OR EXISTS(SELECT 1 FROM spot_orders WHERE reserved_asset = ?)
               OR EXISTS(SELECT 1 FROM new_coin_projects WHERE asset_id = ? OR unlock_fee_asset = ?)
               OR EXISTS(SELECT 1 FROM new_coin_subscriptions WHERE quote_asset = ?)
               OR EXISTS(SELECT 1 FROM new_coin_distributions WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM new_coin_purchase_orders WHERE base_asset = ? OR quote_asset = ?)
               OR EXISTS(SELECT 1 FROM convert_pairs WHERE from_asset = ? OR to_asset = ?)
               OR EXISTS(SELECT 1 FROM convert_quotes WHERE from_asset = ? OR to_asset = ?)
               OR EXISTS(SELECT 1 FROM convert_orders WHERE from_asset = ? OR to_asset = ?)
               OR EXISTS(SELECT 1 FROM seconds_contract_products WHERE stake_asset = ?)
               OR EXISTS(SELECT 1 FROM seconds_contract_orders WHERE stake_asset = ?)
               OR EXISTS(SELECT 1 FROM margin_products WHERE margin_asset = ?)
               OR EXISTS(SELECT 1 FROM margin_positions WHERE margin_asset = ?)
               OR EXISTS(SELECT 1 FROM margin_liquidation_records WHERE margin_asset = ?)
               OR EXISTS(SELECT 1 FROM earn_products WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM earn_subscriptions WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM quick_recharge_orders WHERE asset_id = ?)
             THEN 1 ELSE 0 END AS has_references"#,
    )
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .fetch_one(&mut **tx)
    .await?;

    if has_references != 0 {
        return Err(AppError::Validation(
            "asset with related records cannot be deleted".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) async fn list_admin_wallet_accounts(
    pool: &Pool<MySql>,
    filter: AdminWalletAccountListFilter,
) -> AppResult<Vec<AdminWalletAccountResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT accounts.id, accounts.user_id, account_users.email AS user_email,
                  accounts.asset_id, assets.symbol AS asset_symbol,
                  accounts.available, accounts.frozen, accounts.locked, TRUE AS account_exists, accounts.updated_at
           FROM wallet_accounts accounts
           INNER JOIN users account_users ON account_users.id = accounts.user_id
           INNER JOIN assets ON assets.id = accounts.asset_id
           WHERE 1 = 1"#,
    );
    if !filter.include_internal {
        push_exclude_internal_user_email(&mut builder, "account_users.email");
    }
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "accounts.user_id", user_id);
    }
    push_user_email_filter(&mut builder, "accounts.user_id", filter.email.clone());
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND accounts.asset_id = ");
        builder.push_bind(asset_id);
    }
    builder.push(" ORDER BY accounts.updated_at DESC, accounts.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    let mut accounts = builder
        .build_query_as::<AdminWalletAccountResponse>()
        .fetch_all(pool)
        .await?;
    if filter.include_empty {
        append_empty_wallet_accounts(pool, &filter, &mut accounts).await?;
    }
    Ok(accounts)
}

pub(crate) async fn list_admin_wallet_ledger(
    pool: &Pool<MySql>,
    filter: AdminWalletLedgerListFilter,
) -> AppResult<Vec<AdminWalletLedgerResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT ledger.id, ledger.user_id, ledger_users.email AS user_email,
                  ledger.asset_id, assets.symbol AS asset_symbol,
                  ledger.change_type, ledger.amount, ledger.balance_type, ledger.balance_after,
                  ledger.available_after, ledger.frozen_after, ledger.locked_after,
                  ledger.ref_type, ledger.ref_id, ledger.created_at
           FROM wallet_ledger ledger
           INNER JOIN users ledger_users ON ledger_users.id = ledger.user_id
           INNER JOIN assets ON assets.id = ledger.asset_id
           WHERE 1 = 1"#,
    );
    if !filter.include_internal {
        push_exclude_internal_user_email(&mut builder, "ledger_users.email");
    }
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "ledger.user_id", user_id);
    }
    push_user_email_filter(&mut builder, "ledger.user_id", filter.email);
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND ledger.asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(change_type) = optional_string(filter.change_type) {
        builder.push(" AND ledger.change_type = ");
        builder.push_bind(change_type);
    }
    if let Some(ref_type) = optional_string(filter.ref_type) {
        builder.push(" AND ledger.ref_type = ");
        builder.push_bind(ref_type);
    }
    builder.push(" ORDER BY ledger.created_at DESC, ledger.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminWalletLedgerResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_deposit_network_configs(
    pool: &Pool<MySql>,
    filter: AdminDepositNetworkConfigListFilter,
) -> AppResult<Vec<AdminDepositNetworkConfigResponse>> {
    let mut builder = admin_deposit_network_config_query();
    builder.push(" WHERE 1 = 1");
    if let Some(network) = filter.network {
        builder.push(" AND network = ");
        builder.push_bind(network);
    }
    if let Some(address_group_code) = filter.address_group_code {
        builder.push(" AND address_group_code = ");
        builder.push_bind(address_group_code);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(asset_symbol) = filter.asset_symbol {
        builder.push(
            " AND (asset_symbols_json IS NULL OR JSON_CONTAINS(asset_symbols_json, JSON_QUOTE(",
        );
        builder.push_bind(asset_symbol);
        builder.push(")))");
    }
    builder.push(" ORDER BY sort_order ASC, id ASC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminDepositNetworkConfigResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_deposit_network_config_by_network(
    pool: &Pool<MySql>,
    network: &str,
) -> AppResult<AdminDepositNetworkConfigResponse> {
    let mut builder = admin_deposit_network_config_query();
    builder.push(" WHERE network = ");
    builder.push_bind(network.to_owned());
    builder
        .build_query_as::<AdminDepositNetworkConfigResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Validation("deposit network config is missing".to_owned()))
}

pub(crate) async fn insert_admin_deposit_network_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminDepositNetworkConfigWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO deposit_network_configs
           (network, display_name, address_group_code, address_group_name, asset_symbols_json, status, sort_order)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.network)
    .bind(&input.display_name)
    .bind(&input.address_group_code)
    .bind(&input.address_group_name)
    .bind(deposit_asset_symbols_json(&input.asset_symbols))
    .bind(&input.status)
    .bind(input.sort_order)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_deposit_network_config_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_deposit_network_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
    input: AdminDepositNetworkConfigWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE deposit_network_configs
           SET network = ?,
               display_name = ?,
               address_group_code = ?,
               address_group_name = ?,
               asset_symbols_json = ?,
               status = ?,
               sort_order = ?
           WHERE id = ?"#,
    )
    .bind(&input.network)
    .bind(&input.display_name)
    .bind(&input.address_group_code)
    .bind(&input.address_group_name)
    .bind(deposit_asset_symbols_json(&input.asset_symbols))
    .bind(&input.status)
    .bind(input.sort_order)
    .bind(config_id)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_deposit_network_config_error)?;
    Ok(())
}

pub(crate) async fn load_deposit_network_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
) -> AppResult<AdminDepositNetworkConfigResponse> {
    let mut builder = admin_deposit_network_config_query();
    builder.push(" WHERE id = ");
    builder.push_bind(config_id);
    builder
        .build_query_as::<AdminDepositNetworkConfigResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_deposit_network_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
) -> AppResult<AdminDepositNetworkConfigResponse> {
    let mut builder = admin_deposit_network_config_query();
    builder.push(" WHERE id = ");
    builder.push_bind(config_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminDepositNetworkConfigResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn ensure_asset_symbols_exist(
    pool: &Pool<MySql>,
    symbols: &[String],
) -> AppResult<()> {
    for symbol in symbols {
        ensure_asset_symbol_exists(pool, symbol).await?;
    }
    Ok(())
}

pub(crate) async fn list_admin_deposit_address_pool(
    pool: &Pool<MySql>,
    filter: AdminDepositAddressPoolListFilter,
) -> AppResult<Vec<AdminDepositAddressPoolResponse>> {
    let mut builder = admin_deposit_address_pool_query();
    builder.push(" WHERE 1 = 1");
    if let Some(network) = filter.network {
        builder.push(" AND addresses.network = ");
        builder.push_bind(network);
    }
    if let Some(address_group_code) = filter.address_group_code {
        builder.push(" AND addresses.address_group_code = ");
        builder.push_bind(address_group_code);
    }
    if let Some(status) = filter.status {
        builder.push(" AND addresses.status = ");
        builder.push_bind(status);
    }
    if let Some(asset_symbol) = filter.asset_symbol {
        builder.push(" AND (addresses.asset_symbol = ");
        builder.push_bind(asset_symbol.clone());
        builder.push(" OR addresses.assigned_asset_symbol = ");
        builder.push_bind(asset_symbol.clone());
        builder.push(" OR JSON_CONTAINS(addresses.asset_symbols_json, JSON_QUOTE(");
        builder.push_bind(asset_symbol);
        builder.push("))");
        builder.push(")");
    }
    if let Some(user_id) = filter.assigned_user_id {
        push_user_id_filter(&mut builder, "addresses.assigned_user_id", user_id);
    }
    push_user_email_filter(&mut builder, "addresses.assigned_user_id", filter.email);
    if let Some(address) = filter.address {
        builder.push(" AND addresses.address LIKE ");
        builder.push_bind(format!("%{address}%"));
    }
    builder.push(" ORDER BY addresses.updated_at DESC, addresses.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminDepositAddressPoolResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_deposit_address_pool(
    pool: &Pool<MySql>,
    address_id: u64,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let mut builder = admin_deposit_address_pool_query();
    builder.push(" WHERE addresses.id = ");
    builder.push_bind(address_id);
    builder
        .build_query_as::<AdminDepositAddressPoolResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_deposit_address_pool_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminDepositAddressPoolWrite,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let result = sqlx::query(
        r#"INSERT INTO deposit_address_pool
           (network, address_group_code, address, asset_symbol, asset_symbols_json, status, memo, remark)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.network)
    .bind(&input.address_group_code)
    .bind(&input.address)
    .bind(deposit_address_pool_legacy_asset_symbol(&input.asset_symbols))
    .bind(deposit_asset_symbols_json(&input.asset_symbols))
    .bind(&input.status)
    .bind(&input.memo)
    .bind(&input.remark)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_deposit_address_error)?;
    load_deposit_address_pool_in_tx(tx, result.last_insert_id()).await
}

pub(crate) async fn update_deposit_address_pool_in_tx(
    tx: &mut Transaction<'_, MySql>,
    address_id: u64,
    input: AdminDepositAddressPoolWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE deposit_address_pool
           SET network = ?, address_group_code = ?, address = ?, asset_symbol = ?, asset_symbols_json = ?, status = ?, memo = ?, remark = ?
           WHERE id = ?"#,
    )
    .bind(&input.network)
    .bind(&input.address_group_code)
    .bind(&input.address)
    .bind(deposit_address_pool_legacy_asset_symbol(&input.asset_symbols))
    .bind(deposit_asset_symbols_json(&input.asset_symbols))
    .bind(&input.status)
    .bind(&input.memo)
    .bind(&input.remark)
    .bind(address_id)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_deposit_address_error)?;
    Ok(())
}

pub(crate) async fn reclaim_deposit_address_pool_in_tx(
    tx: &mut Transaction<'_, MySql>,
    address_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE deposit_address_pool
           SET status = 'available',
               assigned_user_id = NULL,
               assigned_user_email = NULL,
               assigned_asset_symbol = NULL,
               assigned_at = NULL
           WHERE id = ?"#,
    )
    .bind(address_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_deposit_address_pool_in_tx(
    tx: &mut Transaction<'_, MySql>,
    address_id: u64,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let mut builder = admin_deposit_address_pool_query();
    builder.push(" WHERE addresses.id = ");
    builder.push_bind(address_id);
    builder
        .build_query_as::<AdminDepositAddressPoolResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_deposit_address_pool_in_tx(
    tx: &mut Transaction<'_, MySql>,
    address_id: u64,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let mut builder = admin_deposit_address_pool_query();
    builder.push(" WHERE addresses.id = ");
    builder.push_bind(address_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminDepositAddressPoolResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn list_admin_agent_commission_rules(
    pool: &Pool<MySql>,
    filter: AdminAgentCommissionRuleListFilter,
) -> AppResult<Vec<AdminAgentCommissionRuleResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, agent_id, product_type, commission_rate, status, created_at, updated_at
           FROM agent_commission_rules
           WHERE 1 = 1"#,
    );
    if let Some(agent_id) = filter.agent_id {
        builder.push(" AND agent_id = ");
        builder.push_bind(agent_id);
    }
    if let Some(product_type) = filter.product_type {
        builder.push(" AND product_type = ");
        builder.push_bind(product_type);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    builder.push(" OFFSET ");
    builder.push_bind(filter.offset as i64);

    Ok(builder
        .build_query_as::<AdminAgentCommissionRuleResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn insert_agent_commission_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminAgentCommissionRuleWrite,
) -> AppResult<u64> {
    let rule_id = sqlx::query(
        r#"INSERT INTO agent_commission_rules (agent_id, product_type, commission_rate, status)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(input.agent_id)
    .bind(&input.product_type)
    .bind(&input.commission_rate)
    .bind(&input.status)
    .execute(&mut **tx)
    .await?
    .last_insert_id();
    Ok(rule_id)
}

pub(crate) async fn update_agent_commission_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
    commission_rate: Option<&BigDecimal>,
    status: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE agent_commission_rules
           SET commission_rate = COALESCE(?, commission_rate),
               status = COALESCE(?, status)
           WHERE id = ?"#,
    )
    .bind(commission_rate)
    .bind(status)
    .bind(rule_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_agent_commission_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
) -> AppResult<AdminAgentCommissionRuleResponse> {
    sqlx::query_as::<_, AdminAgentCommissionRuleResponse>(
        r#"SELECT id, agent_id, product_type, commission_rate, status, created_at, updated_at
           FROM agent_commission_rules
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(rule_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_agent_commission_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
) -> AppResult<AdminAgentCommissionRuleResponse> {
    sqlx::query_as::<_, AdminAgentCommissionRuleResponse>(
        r#"SELECT id, agent_id, product_type, commission_rate, status, created_at, updated_at
           FROM agent_commission_rules
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(rule_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn list_admin_risk_rules(
    pool: &Pool<MySql>,
    filter: AdminRiskRuleListFilter,
) -> AppResult<Vec<RiskRuleResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, rule_type, target_type, target_id, config_json, enabled,
                  created_by, created_at, updated_at
           FROM risk_rules
           WHERE 1 = 1"#,
    );
    if let Some(rule_type) = filter.rule_type {
        builder.push(" AND rule_type = ");
        builder.push_bind(rule_type);
    }
    if let Some(target_type) = filter.target_type {
        builder.push(" AND target_type = ");
        builder.push_bind(target_type);
    }
    if let Some(enabled) = filter.enabled {
        builder.push(" AND enabled = ");
        builder.push_bind(enabled);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<RiskRuleResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn insert_risk_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule: RiskRuleWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO risk_rules (rule_type, target_type, target_id, config_json, enabled, created_by)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(rule.rule_type)
    .bind(rule.target_type)
    .bind(rule.target_id)
    .bind(SqlxJson(rule.config_json))
    .bind(rule.enabled)
    .bind(rule.created_by)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn load_risk_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
) -> AppResult<RiskRuleResponse> {
    sqlx::query_as::<_, RiskRuleResponse>(
        r#"SELECT id, rule_type, target_type, target_id, config_json, enabled,
                  created_by, created_at, updated_at
           FROM risk_rules
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(rule_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_risk_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
) -> AppResult<RiskRuleResponse> {
    sqlx::query_as::<_, RiskRuleResponse>(
        r#"SELECT id, rule_type, target_type, target_id, config_json, enabled,
                  created_by, created_at, updated_at
           FROM risk_rules
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(rule_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn update_risk_rule_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
    enabled: bool,
) -> AppResult<()> {
    sqlx::query("UPDATE risk_rules SET enabled = ? WHERE id = ?")
        .bind(enabled)
        .bind(rule_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn list_admin_risk_events(
    pool: &Pool<MySql>,
    filter: AdminRiskEventListFilter,
) -> AppResult<Vec<RiskEventResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, actor_type, actor_id, event_type, risk_level,
                  decision, reason, payload_json, created_at
           FROM risk_events
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", filter.email);
    if let Some(decision) = filter.decision {
        builder.push(" AND decision = ");
        builder.push_bind(decision);
    }
    if let Some(risk_level) = filter.risk_level {
        builder.push(" AND risk_level = ");
        builder.push_bind(risk_level);
    }
    builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<RiskEventResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_agent_commissions(
    pool: &Pool<MySql>,
    filter: AdminAgentCommissionListFilter,
) -> AppResult<Vec<AdminAgentCommissionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, agent_id, user_id, source_type, source_id, source_amount, payout_asset_id,
                  commission_rate, commission_amount, status, created_at
           FROM agent_commission_records
           WHERE 1 = 1"#,
    );
    if let Some(agent_id) = filter.agent_id {
        builder.push(" AND agent_id = ");
        builder.push_bind(agent_id);
    }
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", filter.email);
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminAgentCommissionResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_agent_commission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
) -> AppResult<AdminAgentCommissionResponse> {
    sqlx::query_as::<_, AdminAgentCommissionResponse>(
        r#"SELECT id, agent_id, user_id, source_type, source_id, source_amount, payout_asset_id,
                  commission_rate, commission_amount, status, created_at
           FROM agent_commission_records
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(commission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_agent_commission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
) -> AppResult<AdminAgentCommissionResponse> {
    sqlx::query_as::<_, AdminAgentCommissionResponse>(
        r#"SELECT id, agent_id, user_id, source_type, source_id, source_amount, payout_asset_id,
                  commission_rate, commission_amount, status, created_at
           FROM agent_commission_records
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(commission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn update_agent_commission_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE agent_commission_records SET status = ? WHERE id = ?")
        .bind(status)
        .bind(commission_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn ensure_agent_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM agents WHERE id = ? LIMIT 1 FOR UPDATE")
        .bind(agent_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(())
}

pub(crate) async fn load_agent_commission_payout_target_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
) -> AppResult<AgentCommissionPayoutTarget> {
    let target = sqlx::query_as::<_, (u64, u64)>(
        r#"SELECT agents.user_id AS agent_user_id, records.payout_asset_id AS asset_id
           FROM agent_commission_records records
           INNER JOIN agents ON agents.id = records.agent_id
           WHERE records.id = ? AND records.payout_asset_id IS NOT NULL
           LIMIT 1"#,
    )
    .bind(commission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(AgentCommissionPayoutTarget {
        agent_user_id: target.0,
        asset_id: target.1,
    })
}

pub(crate) async fn credit_admin_wallet_available_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_or_create_admin_wallet_row_in_tx(tx, user_id, asset_id).await?;
    let available_after = wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_admin_wallet_ledger_in_tx(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await
}

pub(crate) async fn lock_or_create_admin_wallet_row_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<AdminWalletRow> {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    load_admin_wallet_row_in_tx(tx, user_id, asset_id).await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_admin_wallet_ledger_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn load_admin_wallet_row_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<AdminWalletRow> {
    sqlx::query_as::<_, AdminWalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("wallet account is required".to_owned()))
}

pub(crate) async fn list_admin_trading_pairs(
    pool: &Pool<MySql>,
    filter: AdminTradingPairListFilter,
) -> AppResult<Vec<AdminTradingPairResponse>> {
    let mut builder = admin_trading_pair_query();
    builder.push(" WHERE 1 = 1");
    if let Some(symbol) = filter.symbol {
        builder.push(" AND pairs.symbol = ");
        builder.push_bind(symbol);
    }
    if let Some(status) = filter.status {
        builder.push(" AND pairs.status = ");
        builder.push_bind(status);
    }
    if let Some(market_type) = filter.market_type {
        builder.push(" AND pairs.market_type = ");
        builder.push_bind(market_type);
    }
    builder.push(" ORDER BY pairs.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminTradingPairResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_admin_trading_pair(
    pool: &Pool<MySql>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    let mut builder = admin_trading_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder
        .build_query_as::<AdminTradingPairResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminTradingPairInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, logo_url, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.base_asset_id)
    .bind(input.quote_asset_id)
    .bind(&input.symbol)
    .bind(&input.logo_url)
    .bind(input.price_precision)
    .bind(input.qty_precision)
    .bind(&input.min_order_value)
    .bind(&input.status)
    .bind(&input.market_type)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_trading_pair_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    input: AdminTradingPairUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE trading_pairs
           SET logo_url = ?, price_precision = ?, qty_precision = ?, min_order_value = ?, status = ?, market_type = ?
           WHERE id = ?"#,
    )
    .bind(&input.logo_url)
    .bind(input.price_precision)
    .bind(input.qty_precision)
    .bind(&input.min_order_value)
    .bind(&input.status)
    .bind(&input.market_type)
    .bind(pair_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_trading_pair_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE trading_pairs SET status = ? WHERE id = ?")
        .bind(status)
        .bind(pair_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn load_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    let mut builder = admin_trading_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder
        .build_query_as::<AdminTradingPairResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM trading_pairs WHERE id = ? FOR UPDATE")
        .bind(pair_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    load_admin_trading_pair_in_tx(tx, pair_id).await
}

pub(crate) async fn ensure_trading_pair_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM assets WHERE id = ? AND status = 'active' LIMIT 1 FOR UPDATE",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(())
}

pub(crate) async fn load_admin_dashboard_users_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardUsersSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardUsersSummary>(
        r#"SELECT COUNT(*) AS total,
                  COUNT(CASE WHEN status = 'active' THEN 1 END) AS active,
                  COUNT(CASE WHEN created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)
                             THEN 1 END) AS new_24h
           FROM users"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn load_admin_dashboard_wallet_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardWalletSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardWalletSummary>(
        r#"SELECT (SELECT COUNT(*) FROM assets WHERE status = 'active') AS active_assets,
                  (SELECT COUNT(*) FROM wallet_accounts) AS wallet_accounts,
                  (SELECT COUNT(*) FROM wallet_accounts
                   WHERE available <> 0 OR frozen <> 0 OR locked <> 0) AS non_zero_accounts,
                  (SELECT COUNT(*) FROM asset_lock_positions
                   WHERE status = 'active' AND unlock_at <= UTC_TIMESTAMP(6)) AS pending_unlocks,
                  (SELECT COUNT(*) FROM deposit_records WHERE status = 'pending') AS pending_deposits,
                  (SELECT COUNT(*) FROM withdraw_records WHERE status = 'pending') AS pending_withdrawals,
                  'not_configured' AS custody_status"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn load_admin_dashboard_market_counts(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardMarketCounts> {
    Ok(sqlx::query_as::<_, AdminDashboardMarketCounts>(
        r#"SELECT COUNT(CASE WHEN status = 'active' THEN 1 END) AS active_pairs,
                  COUNT(CASE WHEN status = 'disabled' THEN 1 END) AS disabled_pairs,
                  COUNT(CASE WHEN market_type = 'external' THEN 1 END) AS external_pairs,
                  COUNT(CASE WHEN market_type = 'strategy' THEN 1 END) AS strategy_pairs
           FROM trading_pairs"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn load_admin_dashboard_trading_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardTradingSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardTradingSummary>(
        r#"SELECT (SELECT COUNT(*) FROM spot_orders WHERE status IN ('pending', 'partial')) AS spot_open_orders,
                  (SELECT COUNT(*) FROM spot_trades
                   WHERE created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS spot_trades_24h,
                  (SELECT COUNT(*) FROM convert_orders WHERE status = 'pending') AS convert_pending_orders,
                  (SELECT COUNT(*) FROM convert_orders
                   WHERE status = 'completed'
                     AND updated_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS convert_completed_24h"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn load_admin_dashboard_products_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardProductsSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardProductsSummary>(
        r#"SELECT (SELECT COUNT(*) FROM seconds_contract_orders WHERE status = 'opened') AS seconds_open_orders,
                  (SELECT COUNT(*) FROM margin_positions WHERE status = 'opened') AS margin_open_positions,
                  (SELECT COUNT(*) FROM margin_liquidation_records
                   WHERE liquidated_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS margin_liquidated_24h,
                  (SELECT COUNT(*) FROM earn_subscriptions WHERE status = 'subscribed') AS earn_active_subscriptions,
                  (SELECT COUNT(*) FROM earn_subscriptions
                   WHERE status = 'subscribed'
                     AND matures_at <= DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS earn_maturing_24h"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn load_admin_dashboard_risk_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardRiskSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardRiskSummary>(
        r#"SELECT (SELECT COUNT(*) FROM risk_events
                   WHERE created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS risk_events_24h,
                  (SELECT COUNT(*) FROM risk_events
                   WHERE decision IN ('block', 'blocked', 'reject', 'rejected')
                     AND created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS blocked_events_24h,
                  (SELECT COUNT(*) FROM event_outbox WHERE status = 'pending') AS pending_outbox_events,
                  (SELECT COUNT(*) FROM event_inbox WHERE status = 'retry') AS retry_inbox_events,
                  (SELECT COUNT(*) FROM event_inbox WHERE status = 'dead_letter') AS dead_letter_inbox_events"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn count_admin_dashboard_actions_24h(pool: &Pool<MySql>) -> AppResult<i64> {
    Ok(sqlx::query_as::<_, (i64,)>(
        r#"SELECT COUNT(*) FROM admin_audit_logs
           WHERE created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)"#,
    )
    .fetch_one(pool)
    .await?
    .0)
}

pub(crate) async fn list_admin_dashboard_latest_actions(
    pool: &Pool<MySql>,
) -> AppResult<Vec<AdminDashboardAuditAction>> {
    Ok(sqlx::query_as::<_, AdminDashboardAuditAction>(
        r#"SELECT id, admin_id, action, target_type, target_id, created_at
           FROM admin_audit_logs
           ORDER BY created_at DESC, id DESC
           LIMIT 5"#,
    )
    .fetch_all(pool)
    .await?)
}

pub(crate) async fn list_admin_audit_logs(
    pool: &Pool<MySql>,
    filter: AdminAuditLogListFilter,
) -> AppResult<Vec<AdminAuditLogResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, admin_id, action, target_type, target_id,
                  before_json, after_json, reason, ip, created_at
           FROM admin_audit_logs
           WHERE 1 = 1"#,
    );
    if let Some(admin_id) = filter.admin_id {
        builder.push(" AND admin_id = ");
        builder.push_bind(admin_id);
    }
    if let Some(action) = filter.action {
        builder.push(" AND action = ");
        builder.push_bind(action);
    }
    if let Some(target_type) = filter.target_type {
        builder.push(" AND target_type = ");
        builder.push_bind(target_type);
    }
    if let Some(target_id) = filter.target_id {
        builder.push(" AND target_id = ");
        builder.push_bind(target_id);
    }
    builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminAuditLogResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_margin_liquidations(
    pool: &Pool<MySql>,
    filter: AdminMarginLiquidationListFilter,
) -> AppResult<Vec<AdminMarginLiquidationResponse>> {
    let mut builder = admin_margin_liquidation_query();
    builder.push(" WHERE 1 = 1");
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", filter.email);
    if let Some(pair_id) = filter.pair_id {
        builder.push(" AND pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(position_id) = filter.position_id {
        builder.push(" AND position_id = ");
        builder.push_bind(position_id);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminMarginLiquidationResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_admin_margin_liquidation(
    pool: &Pool<MySql>,
    liquidation_id: u64,
) -> AppResult<AdminMarginLiquidationResponse> {
    let mut builder = admin_margin_liquidation_query();
    builder.push(" WHERE id = ");
    builder.push_bind(liquidation_id);
    builder.push(" LIMIT 1");
    builder
        .build_query_as::<AdminMarginLiquidationResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn list_admin_market_strategies(
    pool: &Pool<MySql>,
    filter: AdminMarketStrategyListFilter,
) -> AppResult<Vec<AdminMarketStrategyResponse>> {
    let mut builder = admin_market_strategy_query();
    builder.push(" WHERE 1 = 1");
    if let Some(pair_id) = filter.pair_id {
        builder.push(" AND strategies.pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(status) = filter.status {
        builder.push(" AND strategies.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY strategies.created_at DESC, strategies.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn ensure_market_strategy_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<String> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT market_type FROM trading_pairs WHERE id = ? AND status = 'active' FOR UPDATE",
    )
    .bind(pair_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if !matches!(row.0.as_str(), "internal" | "strategy") {
        return Err(AppError::Validation(
            "market strategy can only be bound to internal or strategy pairs".to_owned(),
        ));
    }
    Ok(row.0)
}

pub(crate) async fn insert_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminMarketStrategyInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO market_strategies
           (pair_id, strategy_type, start_price, target_price, start_time, end_time,
            volatility, volume_min, volume_max, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.pair_id)
    .bind(&input.strategy_type)
    .bind(&input.start_price)
    .bind(&input.target_price)
    .bind(input.start_time)
    .bind(input.end_time)
    .bind(&input.volatility)
    .bind(&input.volume_min)
    .bind(&input.volume_max)
    .bind(&input.status)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn insert_market_strategy_run_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
    current_price: &BigDecimal,
    start_time: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO strategy_runs
           (strategy_id, run_status, current_price, last_generated_at, last_kline_open_time, recovery_status)
           VALUES (?, ?, ?, ?, ?, 'idle')"#,
    )
    .bind(strategy_id)
    .bind(run_status)
    .bind(current_price)
    .bind(start_time)
    .bind(start_time)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn insert_market_strategy_version_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    version: i32,
    effective_time: DateTime<Utc>,
    config_json: Value,
    seed: String,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO strategy_versions (strategy_id, version, effective_time, config_json, seed, created_by)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(version)
    .bind(effective_time)
    .bind(SqlxJson(config_json))
    .bind(seed)
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    input: AdminMarketStrategyUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE market_strategies
           SET strategy_type = ?, start_price = ?, target_price = ?, start_time = ?, end_time = ?,
               volatility = ?, volume_min = ?, volume_max = ?
           WHERE id = ?"#,
    )
    .bind(&input.strategy_type)
    .bind(&input.start_price)
    .bind(&input.target_price)
    .bind(input.start_time)
    .bind(input.end_time)
    .bind(&input.volatility)
    .bind(&input.volume_min)
    .bind(&input.volume_max)
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_market_strategy_run_checkpoint_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
    current_price: &BigDecimal,
    start_time: DateTime<Utc>,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE strategy_runs
           SET run_status = ?, current_price = ?, last_generated_at = NULL,
               last_kline_open_time = ?, recovery_status = 'idle', error_message = NULL
           WHERE strategy_id = ?"#,
    )
    .bind(run_status)
    .bind(current_price)
    .bind(start_time)
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    ensure_market_strategy_run_updated(result.rows_affected())
}

pub(crate) async fn next_market_strategy_version_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<i32> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM strategy_versions WHERE strategy_id = ?",
    )
    .bind(strategy_id)
    .fetch_one(&mut **tx)
    .await?)
}

pub(crate) async fn update_market_strategy_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE market_strategies SET status = ? WHERE id = ?")
        .bind(status)
        .bind(strategy_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn update_market_strategy_run_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        "UPDATE strategy_runs SET run_status = ?, recovery_status = 'idle', error_message = NULL WHERE strategy_id = ?",
    )
    .bind(run_status)
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    ensure_market_strategy_run_updated(result.rows_affected())
}

pub(crate) async fn load_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyResponse> {
    let mut builder = admin_market_strategy_query();
    builder.push(" WHERE strategies.id = ");
    builder.push_bind(strategy_id);
    builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyResponse> {
    let mut builder = admin_market_strategy_query();
    builder.push(" WHERE strategies.id = ");
    builder.push_bind(strategy_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_market_strategy_event_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    action: &str,
    payload_json: Value,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO strategy_events (strategy_id, event_type, payload_json)
           VALUES (?, ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(action)
    .bind(SqlxJson(payload_json))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn ensure_market_strategy_run_updated(rows_affected: u64) -> AppResult<()> {
    if rows_affected != 1 {
        return Err(AppError::Conflict(
            "market strategy run checkpoint is missing".to_owned(),
        ));
    }
    Ok(())
}

fn admin_market_strategy_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT strategies.id,
                  strategies.pair_id,
                  pairs.symbol,
                  pairs.market_type,
                  strategies.strategy_type,
                  strategies.start_price,
                  strategies.target_price,
                  strategies.start_time,
                  strategies.end_time,
                  strategies.volatility,
                  strategies.volume_min,
                  strategies.volume_max,
                  strategies.status,
                  runs.run_status,
                  runs.current_price,
                  runs.last_generated_at,
                  runs.last_kline_open_time,
                  runs.recovery_status,
                  strategies.created_at
           FROM market_strategies strategies
           INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id
           LEFT JOIN strategy_runs runs ON runs.strategy_id = strategies.id"#,
    )
}

fn admin_margin_liquidation_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, position_id, user_id, product_id, pair_id, margin_asset, direction,
                  margin_amount, notional_amount, interest_amount, entry_price, mark_price,
                  maintenance_margin_rate, equity, maintenance_margin, realized_pnl,
                  payout_amount, reason, liquidated_at, created_at
           FROM margin_liquidation_records"#,
    )
}

pub(crate) async fn list_admin_new_coin_projects(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<Vec<NewCoinProjectResponse>> {
    let mut builder = admin_new_coin_project_query();
    builder.push(" ORDER BY projects.id DESC LIMIT ");
    builder.push_bind(limit as i64);
    Ok(builder
        .build_query_as::<NewCoinProjectResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_new_coin_subscriptions(
    pool: &Pool<MySql>,
    filter: AdminNewCoinFlatListFilter,
) -> AppResult<Vec<NewCoinSubscriptionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, quote_asset, quote_amount, requested_quantity,
                  allocated_quantity, status, idempotency_key, created_at
           FROM new_coin_subscriptions
           WHERE 1 = 1"#,
    );
    if let Some(project_id) = filter.project_id {
        builder.push(" AND project_id = ");
        builder.push_bind(project_id);
    }
    push_optional_user_and_status_filters(
        &mut builder,
        filter.user_id,
        filter.email,
        filter.status,
    );
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<NewCoinSubscriptionResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_new_coin_distributions(
    pool: &Pool<MySql>,
    filter: AdminNewCoinFlatListFilter,
) -> AppResult<Vec<NewCoinDistributionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, subscription_id, asset_id, quantity,
                  lock_position_id, status, idempotency_key, created_at
           FROM new_coin_distributions
           WHERE 1 = 1"#,
    );
    if let Some(project_id) = filter.project_id {
        builder.push(" AND project_id = ");
        builder.push_bind(project_id);
    }
    push_optional_user_and_status_filters(
        &mut builder,
        filter.user_id,
        filter.email,
        filter.status,
    );
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<NewCoinDistributionResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_new_coin_purchases(
    pool: &Pool<MySql>,
    filter: AdminNewCoinFlatListFilter,
) -> AppResult<Vec<NewCoinPurchaseResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                  quote_amount, lock_position_id, status, idempotency_key, created_at
           FROM new_coin_purchase_orders
           WHERE 1 = 1"#,
    );
    if let Some(project_id) = filter.project_id {
        builder.push(" AND project_id = ");
        builder.push_bind(project_id);
    }
    push_optional_user_and_status_filters(
        &mut builder,
        filter.user_id,
        filter.email,
        filter.status,
    );
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<NewCoinPurchaseResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_new_coin_lock_positions(
    pool: &Pool<MySql>,
    filter: AdminNewCoinLockPositionListFilter,
) -> AppResult<Vec<NewCoinLockPositionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, asset_id, unlock_type, unlock_at, locked_amount,
                  released_amount, remaining_amount, merge_key, status, created_at
           FROM asset_lock_positions
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", filter.email);
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<NewCoinLockPositionResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_new_coin_unlocks(
    pool: &Pool<MySql>,
    filter: AdminNewCoinUnlockListFilter,
) -> AppResult<Vec<NewCoinUnlockResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
                  unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                  unlock_fee_amount, fee_paid_status, status, idempotency_key, created_at
           FROM asset_unlock_records
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", filter.email);
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(fee_paid_status) = filter.fee_paid_status {
        builder.push(" AND fee_paid_status = ");
        builder.push_bind(fee_paid_status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<NewCoinUnlockResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn insert_admin_new_coin_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminNewCoinProjectInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
            unlock_type, fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
            unlock_fee_rate, unlock_fee_basis, unlock_fee_asset, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active')"#,
    )
    .bind(input.asset_id)
    .bind(&input.symbol)
    .bind(&input.lifecycle_status)
    .bind(&input.total_supply)
    .bind(&input.issue_price)
    .bind(input.listed_at)
    .bind(&input.unlock_type)
    .bind(input.fixed_unlock_at)
    .bind(input.relative_unlock_seconds)
    .bind(input.unlock_fee_enabled)
    .bind(&input.unlock_fee_rate)
    .bind(input.unlock_fee_basis.as_deref())
    .bind(input.unlock_fee_asset)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_new_coin_project_lifecycle_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    lifecycle_status: &str,
    listed_at: Option<DateTime<Utc>>,
) -> AppResult<()> {
    sqlx::query("UPDATE new_coin_projects SET lifecycle_status = ?, listed_at = ? WHERE id = ?")
        .bind(lifecycle_status)
        .bind(listed_at)
        .bind(project_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn update_admin_new_coin_project_unlock_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    input: AdminNewCoinUnlockRuleUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE new_coin_projects
           SET unlock_type = ?, listed_at = ?, fixed_unlock_at = ?, relative_unlock_seconds = ?
           WHERE id = ?"#,
    )
    .bind(&input.unlock_type)
    .bind(input.listed_at)
    .bind(input.fixed_unlock_at)
    .bind(input.relative_unlock_seconds)
    .bind(project_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_new_coin_project_unlock_fee_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    input: AdminNewCoinUnlockFeeRuleUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE new_coin_projects
           SET unlock_fee_enabled = ?, unlock_fee_rate = ?, unlock_fee_basis = ?, unlock_fee_asset = ?
           WHERE id = ?"#,
    )
    .bind(input.unlock_fee_enabled)
    .bind(input.unlock_fee_rate.as_ref())
    .bind(input.unlock_fee_basis.as_deref())
    .bind(input.unlock_fee_asset)
    .bind(project_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn activate_admin_new_coin_post_listing_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    sqlx::query("UPDATE trading_pairs SET status = 'active' WHERE id = ?")
        .bind(pair_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn enable_admin_new_coin_post_listing_purchase_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    pair_id: u64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE new_coin_projects SET post_listing_purchase_enabled = TRUE, post_listing_pair_id = ? WHERE id = ?",
    )
    .bind(pair_id)
    .bind(project_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn disable_admin_new_coin_post_listing_purchase_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE new_coin_projects SET post_listing_purchase_enabled = FALSE, post_listing_pair_id = NULL WHERE id = ?",
    )
    .bind(project_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn insert_admin_new_coin_distribution_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    user_id: u64,
    subscription_id: Option<u64>,
    asset_id: u64,
    quantity: &BigDecimal,
    lock_position_id: Option<u64>,
    status: &str,
    idempotency_key: &str,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO new_coin_distributions
           (project_id, user_id, subscription_id, asset_id, quantity, lock_position_id,
            status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(subscription_id)
    .bind(asset_id)
    .bind(quantity)
    .bind(lock_position_id)
    .bind(status)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_distribution_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn insert_admin_new_coin_convert_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: &AdminNewCoinConvertRuleWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO new_coin_convert_rules
           (convert_pair_id, rate_source, fixed_rate, floating_rate_json, status, created_by)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.convert_pair_id)
    .bind(&input.rate_source)
    .bind(&input.fixed_rate)
    .bind(input.floating_rate_json.clone().map(SqlxJson))
    .bind(&input.status)
    .bind(input.admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_new_coin_convert_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
    input: &AdminNewCoinConvertRuleWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE new_coin_convert_rules
           SET rate_source = ?, fixed_rate = ?, floating_rate_json = ?, status = ?, created_by = ?
           WHERE id = ?"#,
    )
    .bind(&input.rate_source)
    .bind(&input.fixed_rate)
    .bind(input.floating_rate_json.clone().map(SqlxJson))
    .bind(&input.status)
    .bind(input.admin_id)
    .bind(rule_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_admin_new_coin_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
) -> AppResult<NewCoinProjectResponse> {
    let mut builder = admin_new_coin_project_query();
    builder.push(" WHERE projects.id = ");
    builder.push_bind(project_id);
    builder.push(" LIMIT 1");
    builder
        .build_query_as::<NewCoinProjectResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_new_coin_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
) -> AppResult<NewCoinProjectResponse> {
    let mut builder = admin_new_coin_project_query();
    builder.push(" WHERE projects.id = ");
    builder.push_bind(project_id);
    builder.push(" LIMIT 1 FOR UPDATE");
    builder
        .build_query_as::<NewCoinProjectResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn load_admin_new_coin_distribution_in_tx(
    tx: &mut Transaction<'_, MySql>,
    distribution_id: u64,
) -> AppResult<NewCoinDistributionResponse> {
    sqlx::query_as::<_, NewCoinDistributionResponse>(
        r#"SELECT id, project_id, user_id, subscription_id, asset_id, quantity,
                  lock_position_id, status, idempotency_key, created_at
           FROM new_coin_distributions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(distribution_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn load_admin_new_coin_convert_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
) -> AppResult<NewCoinConvertRuleResponse> {
    sqlx::query_as::<_, NewCoinConvertRuleResponse>(
        r#"SELECT id, convert_pair_id, rate_source, fixed_rate, floating_rate_json, status, created_by
           FROM new_coin_convert_rules
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(rule_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_new_coin_convert_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    convert_pair_id: u64,
) -> AppResult<Option<NewCoinConvertRuleResponse>> {
    Ok(sqlx::query_as::<_, NewCoinConvertRuleResponse>(
        r#"SELECT id, convert_pair_id, rate_source, fixed_rate, floating_rate_json, status, created_by
           FROM new_coin_convert_rules
           WHERE convert_pair_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(convert_pair_id)
    .fetch_optional(&mut **tx)
    .await?)
}

pub(crate) async fn ensure_admin_new_coin_post_listing_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    project_asset_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>(
        r#"SELECT id
           FROM trading_pairs
           WHERE id = ? AND base_asset = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(pair_id)
    .bind(project_asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(())
}

pub(crate) async fn admin_new_coin_idempotency_key_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    table_name: &str,
    idempotency_key: &str,
) -> AppResult<bool> {
    let mut query = QueryBuilder::<MySql>::new("SELECT id FROM ");
    query
        .push(table_name)
        .push(" WHERE idempotency_key = ")
        .push_bind(idempotency_key)
        .push(" LIMIT 1 FOR UPDATE");
    let exists: Option<(u64,)> = query.build_query_as().fetch_optional(&mut **tx).await?;
    Ok(exists.is_some())
}

pub(crate) async fn insert_admin_new_coin_lifecycle_event_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    action: &str,
    payload_json: Value,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO new_coin_lifecycle_events (project_id, event_type, payload_json, created_by)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(project_id)
    .bind(action)
    .bind(SqlxJson(payload_json))
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn apply_admin_new_coin_subscription_distribution_in_tx(
    tx: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    project_id: u64,
    user_id: u64,
    quantity: &BigDecimal,
) -> AppResult<()> {
    let Some((requested_quantity, allocated_quantity)): Option<(BigDecimal, BigDecimal)> =
        sqlx::query_as(
            r#"SELECT requested_quantity, allocated_quantity
               FROM new_coin_subscriptions
               WHERE id = ? AND project_id = ? AND user_id = ?
               LIMIT 1
               FOR UPDATE"#,
        )
        .bind(subscription_id)
        .bind(project_id)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
    else {
        return Err(AppError::NotFound);
    };

    let allocated_after = allocated_quantity + quantity.clone();
    if allocated_after > requested_quantity {
        return Err(AppError::Validation(
            "distribution quantity exceeds requested subscription quantity".to_owned(),
        ));
    }
    let status = if allocated_after == requested_quantity {
        "allocated"
    } else {
        "partial_allocated"
    };

    sqlx::query(
        "UPDATE new_coin_subscriptions SET allocated_quantity = ?, status = ? WHERE id = ?",
    )
    .bind(&allocated_after)
    .bind(status)
    .bind(subscription_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn apply_admin_new_coin_distribution_allocation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    quantity: &BigDecimal,
    lock_positions: &[AdminNewCoinLockPositionWrite],
    ledger: AdminNewCoinLedgerWrite<'_>,
) -> AppResult<Option<u64>> {
    if lock_positions.is_empty() {
        credit_admin_wallet_available_in_tx(
            tx,
            user_id,
            asset_id,
            quantity,
            ledger.change_type,
            ledger.ref_type,
            ledger.ref_id,
        )
        .await?;
        return Ok(None);
    }

    let wallet = lock_or_create_admin_wallet_row_in_tx(tx, user_id, asset_id).await?;
    let locked_after = wallet.locked.clone() + quantity.clone();
    sqlx::query("UPDATE wallet_accounts SET locked = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&locked_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_admin_wallet_ledger_in_tx(
        tx,
        user_id,
        asset_id,
        quantity.clone(),
        "locked",
        &locked_after,
        &wallet.available,
        &wallet.frozen,
        &locked_after,
        ledger.change_type,
        ledger.ref_type,
        ledger.ref_id,
    )
    .await?;

    let mut first_lock_position_id = None;
    for position in lock_positions {
        let position_id = upsert_admin_new_coin_lock_position(tx, position).await?;
        if first_lock_position_id.is_none() {
            first_lock_position_id = Some(position_id);
        }
    }
    Ok(first_lock_position_id)
}

async fn upsert_admin_new_coin_lock_position(
    tx: &mut Transaction<'_, MySql>,
    position: &AdminNewCoinLockPositionWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount,
            released_amount, remaining_amount, merge_key, status)
           VALUES (?, ?, ?, ?, 0, 0, 0, ?, 'active')
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(position.user_id)
    .bind(position.asset_id)
    .bind(&position.unlock_type)
    .bind(position.unlock_at.naive_utc())
    .bind(&position.merge_key)
    .execute(&mut **tx)
    .await?;

    let position_id = if result.last_insert_id() == 0 {
        sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM asset_lock_positions WHERE merge_key = ? LIMIT 1 FOR UPDATE",
        )
        .bind(&position.merge_key)
        .fetch_one(&mut **tx)
        .await?
        .0
    } else {
        result.last_insert_id()
    };

    let inserted = sqlx::query(
        r#"INSERT IGNORE INTO asset_lock_position_sources
           (lock_position_id, source_type, source_id, source_amount, source_time)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(position_id)
    .bind(&position.source_type)
    .bind(&position.source_id)
    .bind(&position.amount)
    .bind(position.source_time.naive_utc())
    .execute(&mut **tx)
    .await?;

    if inserted.rows_affected() > 0 {
        sqlx::query(
            r#"UPDATE asset_lock_positions
               SET locked_amount = locked_amount + ?,
                   remaining_amount = remaining_amount + ?,
                   status = 'active'
               WHERE id = ?"#,
        )
        .bind(&position.amount)
        .bind(&position.amount)
        .bind(position_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(position_id)
}

fn admin_new_coin_project_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT projects.id, projects.asset_id, projects.symbol, projects.lifecycle_status,
                  projects.total_supply, projects.issue_price, projects.listed_at,
                  projects.unlock_type, projects.fixed_unlock_at, projects.relative_unlock_seconds,
                  projects.unlock_fee_enabled, projects.unlock_fee_rate, projects.unlock_fee_basis,
                  projects.unlock_fee_asset, projects.status, projects.post_listing_purchase_enabled,
                  projects.post_listing_pair_id, post_listing_pair.status AS post_listing_pair_status
           FROM new_coin_projects projects
           LEFT JOIN trading_pairs post_listing_pair ON post_listing_pair.id = projects.post_listing_pair_id"#,
    )
}

pub(crate) async fn list_admin_convert_pairs(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<Vec<ConvertPairResponse>> {
    let mut builder = admin_convert_pair_query();
    builder.push(" ORDER BY pairs.id DESC LIMIT ");
    builder.push_bind(limit as i64);
    Ok(builder
        .build_query_as::<ConvertPairResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_admin_convert_pair(
    pool: &Pool<MySql>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    let mut builder = admin_convert_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder.push(" LIMIT 1");
    builder
        .build_query_as::<ConvertPairResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn list_admin_convert_orders(
    pool: &Pool<MySql>,
    filter: AdminConvertOrderListFilter,
) -> AppResult<Vec<ConvertOrderResponse>> {
    let mut builder = admin_convert_order_query();
    builder.push(" WHERE 1 = 1");
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "orders.user_id", user_id);
    }
    push_user_email_filter(&mut builder, "orders.user_id", filter.email);
    if let Some(status) = filter.status {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY orders.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<ConvertOrderResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_admin_convert_order(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<ConvertOrderResponse> {
    let mut builder = admin_convert_order_query();
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder.push(" LIMIT 1");
    builder
        .build_query_as::<ConvertOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_admin_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminConvertPairInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, fee_rate, min_amount, max_amount,
            target_min_amount, target_max_amount, enabled)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.from_asset_id)
    .bind(input.to_asset_id)
    .bind(&input.pricing_mode)
    .bind(&input.spread_rate)
    .bind(&input.fee_rate)
    .bind(&input.min_amount)
    .bind(&input.max_amount)
    .bind(&input.target_min_amount)
    .bind(&input.target_max_amount)
    .bind(input.enabled)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_convert_pair_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    input: AdminConvertPairUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE convert_pairs
           SET from_asset = ?, to_asset = ?, pricing_mode = ?, spread_rate = ?, fee_rate = ?,
               min_amount = ?, max_amount = ?, target_min_amount = ?,
               target_max_amount = ?, enabled = ?
           WHERE id = ?"#,
    )
    .bind(input.from_asset_id)
    .bind(input.to_asset_id)
    .bind(&input.pricing_mode)
    .bind(&input.spread_rate)
    .bind(&input.fee_rate)
    .bind(&input.min_amount)
    .bind(&input.max_amount)
    .bind(&input.target_min_amount)
    .bind(&input.target_max_amount)
    .bind(input.enabled)
    .bind(pair_id)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_convert_pair_error)?;
    Ok(())
}

pub(crate) async fn delete_admin_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn ensure_convert_pair_has_no_references_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    let (quote_count, order_count, rule_count): (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
                  (SELECT COUNT(*) FROM convert_quotes WHERE convert_pair_id = ?) AS quote_count,
                  (SELECT COUNT(*) FROM convert_orders WHERE convert_pair_id = ?) AS order_count,
                  (SELECT COUNT(*) FROM new_coin_convert_rules WHERE convert_pair_id = ?) AS rule_count"#,
    )
    .bind(pair_id)
    .bind(pair_id)
    .bind(pair_id)
    .fetch_one(&mut **tx)
    .await?;

    if quote_count > 0 || order_count > 0 || rule_count > 0 {
        return Err(AppError::Validation(
            "convert pair with related records cannot be deleted".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) async fn load_admin_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    let mut builder = admin_convert_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder.push(" LIMIT 1");
    builder
        .build_query_as::<ConvertPairResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    let mut builder = admin_convert_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder.push(" LIMIT 1 FOR UPDATE");
    builder
        .build_query_as::<ConvertPairResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

fn admin_convert_pair_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT pairs.id,
                  pairs.from_asset AS from_asset_id,
                  from_assets.symbol AS from_asset_symbol,
                  pairs.to_asset AS to_asset_id,
                  to_assets.symbol AS to_asset_symbol,
                  pairs.pricing_mode, pairs.spread_rate, pairs.fee_rate, pairs.min_amount,
                  pairs.max_amount, pairs.target_min_amount, pairs.target_max_amount,
                  pairs.enabled
           FROM convert_pairs pairs
           INNER JOIN assets from_assets ON from_assets.id = pairs.from_asset
           INNER JOIN assets to_assets ON to_assets.id = pairs.to_asset"#,
    )
}

fn admin_convert_order_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT orders.id, users.email AS user_email,
                  from_assets.symbol AS from_asset_symbol,
                  to_assets.symbol AS to_asset_symbol,
                  orders.from_amount, orders.to_amount, orders.rate,
                  orders.fee_rate, orders.fee_amount, orders.status, orders.created_at
           FROM convert_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN assets from_assets ON from_assets.id = orders.from_asset
           INNER JOIN assets to_assets ON to_assets.id = orders.to_asset"#,
    )
}

fn admin_user_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT users.id, users.email, users.phone, invite_codes.code AS invite_code,
                  users.status, users.kyc_level, users.created_at, users.updated_at
           FROM users
           LEFT JOIN invite_codes
             ON invite_codes.owner_type = 'user'
            AND invite_codes.owner_id = users.id
            AND invite_codes.id = (
                SELECT MIN(user_invite_codes.id)
                FROM invite_codes user_invite_codes
                WHERE user_invite_codes.owner_type = 'user'
                  AND user_invite_codes.owner_id = users.id
            )"#,
    )
}

fn admin_agent_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT agents.id, agents.user_id, users.email,
                  agents.parent_agent_id, parent_agents.agent_code AS parent_agent_code,
                  COALESCE(agents.root_agent_id, agents.id) AS root_agent_id,
                  root_agents.agent_code AS root_agent_code,
                  agents.agent_code, agents.level, agents.path, agents.status,
                  (SELECT COUNT(*) FROM user_referrals direct_referrals
                   WHERE direct_referrals.root_agent_id = agents.id) AS direct_user_count,
                  (SELECT COUNT(*)
                   FROM user_referrals team_referrals
                   INNER JOIN agents owner_agents ON owner_agents.id = team_referrals.root_agent_id
                   WHERE owner_agents.path = agents.path
                      OR owner_agents.path LIKE CONCAT(agents.path, '/%')) AS team_user_count,
                  (SELECT COUNT(*) FROM agents child_agents
                   WHERE child_agents.parent_agent_id = agents.id) AS child_agent_count,
                  agent_admin_users.id AS admin_user_id,
                  agent_admin_users.username AS admin_username,
                  agent_admin_users.status AS admin_status,
                  agents.created_at
           FROM agents
           INNER JOIN users ON users.id = agents.user_id
           LEFT JOIN agents parent_agents ON parent_agents.id = agents.parent_agent_id
           INNER JOIN agents root_agents ON root_agents.id = COALESCE(agents.root_agent_id, agents.id)
           LEFT JOIN (
               SELECT agent_id, MIN(id) AS id
               FROM agent_admin_users
               GROUP BY agent_id
           ) first_agent_admin_users ON first_agent_admin_users.agent_id = agents.id
           LEFT JOIN agent_admin_users ON agent_admin_users.id = first_agent_admin_users.id"#,
    )
}

fn admin_country_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, country_code, country_name, remark, default_locale, supported_locales,
                  registration_enabled, status, sort_order, created_at, updated_at
           FROM country_configs"#,
    )
}

fn admin_news_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, title, banner_url, small_logo_url, category, status, country_code,
                  default_locale, content_json, published_at, created_by_admin_id,
                  updated_by_admin_id, created_at, updated_at
           FROM admin_news_items"#,
    )
}

fn admin_asset_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id,
                  symbol,
                  name,
                  logo_url,
                  precision_scale,
                  asset_type,
                  status,
                  deposit_enabled,
                  withdraw_enabled,
                  min_deposit_amount,
                  deposit_fee,
                  withdraw_fee,
                  COALESCE(withdraw_fee_tiers_json, JSON_ARRAY()) AS withdraw_fee_tiers,
                  created_at
           FROM assets"#,
    )
}

fn admin_deposit_network_config_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id,
                  network,
                  display_name,
                  address_group_code,
                  address_group_name,
                  COALESCE(asset_symbols_json, JSON_ARRAY()) AS asset_symbols,
                  status,
                  sort_order,
                  created_at,
                  updated_at
           FROM deposit_network_configs"#,
    )
}

fn admin_deposit_address_pool_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT addresses.id,
                  addresses.network,
                  addresses.address_group_code,
                  addresses.address,
                  addresses.asset_symbol,
                  COALESCE(
                      addresses.asset_symbols_json,
                      IF(addresses.asset_symbol IS NULL, JSON_ARRAY(), JSON_ARRAY(addresses.asset_symbol))
                  ) AS asset_symbols,
                  addresses.status,
                  addresses.assigned_user_id,
                  COALESCE(addresses.assigned_user_email, assigned_users.email) AS assigned_user_email,
                  addresses.assigned_asset_symbol,
                  addresses.assigned_at,
                  addresses.memo,
                  addresses.remark,
                  addresses.created_at,
                  addresses.updated_at
           FROM deposit_address_pool addresses
           LEFT JOIN users assigned_users ON assigned_users.id = addresses.assigned_user_id"#,
    )
}

fn admin_trading_pair_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT pairs.id,
                  pairs.base_asset AS base_asset_id,
                  pairs.quote_asset AS quote_asset_id,
                  pairs.symbol,
                  pairs.logo_url,
                  base.symbol AS base_asset,
                  quote.symbol AS quote_asset,
                  pairs.price_precision,
                  pairs.qty_precision,
                  pairs.min_order_value,
                  pairs.status,
                  pairs.market_type,
                  pairs.created_at
           FROM trading_pairs pairs
           INNER JOIN assets base ON base.id = pairs.base_asset
           INNER JOIN assets quote ON quote.id = pairs.quote_asset"#,
    )
}

fn map_duplicate_country_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("country already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_asset_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("asset already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_user_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("user already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_agent_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("agent already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_trading_pair_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("trading pair already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_convert_pair_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("convert pair already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_distribution_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("new coin distribution has already been created".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_deposit_network_config_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("deposit network config already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_deposit_address_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("deposit address already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

async fn ensure_asset_symbol_exists(pool: &Pool<MySql>, symbol: &str) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM assets WHERE symbol = ? AND status = 'active'",
    )
    .bind(symbol)
    .fetch_one(pool)
    .await?;
    if exists == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

fn deposit_address_pool_legacy_asset_symbol(asset_symbols: &[String]) -> Option<String> {
    match asset_symbols {
        [symbol] => Some(symbol.clone()),
        _ => None,
    }
}

fn deposit_asset_symbols_json(asset_symbols: &[String]) -> Option<SqlxJson<Vec<String>>> {
    if asset_symbols.is_empty() {
        None
    } else {
        Some(SqlxJson(asset_symbols.to_vec()))
    }
}

async fn append_empty_wallet_accounts(
    pool: &Pool<MySql>,
    filter: &AdminWalletAccountListFilter,
    accounts: &mut Vec<AdminWalletAccountResponse>,
) -> AppResult<()> {
    let Some(user_id) = resolve_user_id_filter(pool, filter.user_id, filter.email.clone()).await?
    else {
        return Ok(());
    };
    let Some(user_email) =
        sqlx::query_scalar::<_, Option<String>>("SELECT email FROM users WHERE id = ? LIMIT 1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
    else {
        return Ok(());
    };
    if !filter.include_internal && user_email.as_deref().is_some_and(is_internal_user_email) {
        return Ok(());
    }
    let existing_asset_ids = accounts
        .iter()
        .map(|account| account.asset_id)
        .collect::<BTreeSet<_>>();
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id AS asset_id, symbol AS asset_symbol
           FROM assets
           WHERE status = 'active'"#,
    );
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND id = ");
        builder.push_bind(asset_id);
    }
    builder.push(" ORDER BY symbol ASC LIMIT ");
    builder.push_bind(filter.limit as i64);

    let assets = builder
        .build_query_as::<AdminWalletEmptyAssetRow>()
        .fetch_all(pool)
        .await?;
    let zero = BigDecimal::from(0).with_scale(18);
    let now = Utc::now();
    accounts.extend(
        assets
            .into_iter()
            .filter(|asset| !existing_asset_ids.contains(&asset.asset_id))
            .map(|asset| AdminWalletAccountResponse {
                id: None,
                user_id,
                user_email: user_email.clone(),
                asset_id: asset.asset_id,
                asset_symbol: asset.asset_symbol,
                available: zero.clone(),
                frozen: zero.clone(),
                locked: zero.clone(),
                account_exists: false,
                updated_at: now,
            }),
    );
    Ok(())
}

fn push_user_id_filter(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id_column: &'static str,
    user_id: u64,
) {
    builder.push(" AND ");
    builder.push(user_id_column);
    builder.push(" = ");
    builder.push_bind(user_id);
}

fn push_user_email_filter(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id_column: &'static str,
    email: Option<String>,
) {
    if let Some(email) = optional_string(email) {
        builder.push(" AND EXISTS (SELECT 1 FROM users WHERE users.id = ");
        builder.push(user_id_column);
        builder.push(" AND users.email = ");
        builder.push_bind(email);
        builder.push(")");
    }
}

fn push_optional_user_and_status_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    status: Option<String>,
) {
    if let Some(user_id) = user_id {
        push_user_id_filter(builder, "user_id", user_id);
    }
    push_user_email_filter(builder, "user_id", email);
    if let Some(status) = optional_string(status) {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
}

fn push_exclude_internal_user_email(
    builder: &mut QueryBuilder<'_, MySql>,
    email_column: &'static str,
) {
    builder.push(" AND ");
    builder.push("(");
    builder.push(email_column);
    builder.push(" IS NULL OR ");
    builder.push(email_column);
    builder.push(" NOT LIKE ");
    builder.push_bind(INTERNAL_USER_EMAIL_PATTERN);
    builder.push(")");
}

async fn resolve_user_id_filter(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    email: Option<String>,
) -> AppResult<Option<u64>> {
    let Some(email) = optional_string(email) else {
        return Ok(user_id);
    };
    let resolved_user_id =
        sqlx::query_scalar::<_, u64>("SELECT id FROM users WHERE email = ? LIMIT 1")
            .bind(email)
            .fetch_optional(pool)
            .await?;
    Ok(match (user_id, resolved_user_id) {
        (Some(requested_user_id), Some(email_user_id)) if requested_user_id == email_user_id => {
            Some(requested_user_id)
        }
        (Some(_), _) => None,
        (None, resolved_user_id) => resolved_user_id,
    })
}

fn is_internal_user_email(email: &str) -> bool {
    email
        .trim()
        .to_ascii_lowercase()
        .ends_with(INTERNAL_USER_EMAIL_DOMAIN)
}

fn select_admin_upload_config_sql(for_update: bool) -> String {
    let mut sql = r#"SELECT id, name, provider, endpoint, file_field, bearer_token_ciphertext,
              bearer_token_mask, access_key_ciphertext, access_key_mask, secret_key_ciphertext,
              bucket, region, public_base_url, local_root, key_prefix, max_file_size_bytes,
              allowed_mime_types_json, enabled
       FROM upload_storage_configs
       WHERE name = ?"#
        .to_owned();
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sql
}

fn select_admin_market_feed_config_sql(for_update: bool) -> String {
    let mut sql = r#"SELECT id, name, symbols_json, intervals_json, providers_json, enabled,
              version, applied_version, last_reload_status, last_reload_error, last_reloaded_at
       FROM market_feed_configs
       WHERE name = ?"#
        .to_owned();
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sql
}

fn admin_market_feed_config_record(row: AdminMarketFeedConfigRow) -> AdminMarketFeedConfigRecord {
    AdminMarketFeedConfigRecord {
        id: row.id,
        name: row.name,
        symbols: row.symbols_json.0,
        intervals: row.intervals_json.0,
        providers: row.providers_json.0,
        enabled: row.enabled,
        version: row.version,
        applied_version: row.applied_version,
        last_reload_status: row.last_reload_status,
        last_reload_error: row.last_reload_error,
        last_reloaded_at: row.last_reloaded_at,
    }
}

fn admin_market_source_credential_record(
    row: AdminMarketSourceCredentialRow,
) -> AdminMarketSourceCredentialRecord {
    AdminMarketSourceCredentialRecord {
        provider: row.provider,
        auth_type: row.auth_type,
        api_key_ciphertext: row.api_key_ciphertext,
        api_secret_ciphertext: row.api_secret_ciphertext,
        passphrase_ciphertext: row.passphrase_ciphertext,
        api_key_mask: row.api_key_mask,
        enabled: row.enabled,
    }
}

fn select_admin_smtp_config_sql(clause: &str, for_update: bool) -> String {
    let mut sql = format!(
        r#"SELECT id, name, host, port, security, username_ciphertext, password_ciphertext,
                  username_mask, from_email, from_name, verification_code_template_html,
                  verification_code_templates_json, enabled, priority
           FROM smtp_configs
           {clause}"#
    );
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sql
}

fn admin_smtp_config_record(row: AdminSmtpConfigRow) -> AdminSmtpConfigRecord {
    AdminSmtpConfigRecord {
        id: row.id,
        name: row.name,
        host: row.host,
        port: row.port,
        security: row.security,
        username_ciphertext: row.username_ciphertext,
        password_ciphertext: row.password_ciphertext,
        username_mask: row.username_mask,
        from_email: row.from_email,
        from_name: row.from_name,
        verification_code_template_html: row.verification_code_template_html,
        verification_code_templates: row
            .verification_code_templates_json
            .map(|templates| templates.0)
            .unwrap_or_default(),
        enabled: row.enabled,
        priority: row.priority,
    }
}

fn admin_smtp_delivery_settings_record(
    row: AdminSmtpDeliverySettingsRow,
) -> AdminSmtpDeliverySettingsRecord {
    AdminSmtpDeliverySettingsRecord {
        strategy: row.strategy,
        round_robin_cursor: row.round_robin_cursor,
    }
}

fn admin_upload_config_record(row: AdminUploadConfigRow) -> AdminUploadConfigRecord {
    AdminUploadConfigRecord {
        id: row.id,
        name: row.name,
        provider: row.provider,
        endpoint: row.endpoint,
        file_field: row.file_field,
        bearer_token_ciphertext: row.bearer_token_ciphertext,
        bearer_token_mask: row.bearer_token_mask,
        access_key_ciphertext: row.access_key_ciphertext,
        access_key_mask: row.access_key_mask,
        secret_key_ciphertext: row.secret_key_ciphertext,
        bucket: row.bucket,
        region: row.region,
        public_base_url: row.public_base_url,
        local_root: row.local_root,
        key_prefix: row.key_prefix,
        max_file_size_bytes: row.max_file_size_bytes,
        allowed_mime_types: row.allowed_mime_types_json.0,
        enabled: row.enabled,
    }
}

async fn upload_to_image_bed(
    record: &AdminUploadConfigRecord,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let endpoint = record
        .endpoint
        .as_deref()
        .ok_or_else(|| AppError::Validation("image bed endpoint is not configured".to_owned()))?;
    let token = decrypt_required_upload_secret(
        record.bearer_token_ciphertext.as_deref(),
        key,
        "bearer token",
    )?;
    let field = record
        .file_field
        .as_deref()
        .unwrap_or(DEFAULT_UPLOAD_FILE_FIELD);
    let filename = safe_upload_filename(input.original_filename.as_deref(), &input.mime_type);
    let part = Part::bytes(input.bytes.clone())
        .file_name(filename)
        .mime_str(&input.mime_type)
        .map_err(|_| AppError::Validation("upload file mime type is invalid".to_owned()))?;
    let form = Form::new().part(field.to_owned(), part);
    let response = reqwest::Client::new()
        .post(endpoint)
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(|_| AppError::Validation("image bed upload failed".to_owned()))?;
    if !response.status().is_success() {
        return Err(AppError::Validation(format!(
            "image bed upload failed with status {}",
            response.status().as_u16()
        )));
    }
    let payload = response
        .json::<ImageBedUploadResponse>()
        .await
        .map_err(|_| AppError::Validation("image bed upload response is invalid".to_owned()))?;
    if payload.success == Some(false) {
        return Err(AppError::Validation("image bed upload failed".to_owned()));
    }
    let download_url = safe_upload_response_url(
        payload.links.download.as_deref(),
        "image bed download url",
        true,
    )?
    .ok_or_else(|| AppError::Validation("image bed download url is missing".to_owned()))?;
    let share_url =
        safe_upload_response_url(payload.links.share.as_deref(), "image bed share url", false)?;
    let delete_url = safe_upload_response_url(
        payload.links.delete.as_deref(),
        "image bed delete url",
        false,
    )?;
    let object_key = payload
        .file
        .as_ref()
        .and_then(|file| file.id.as_deref())
        .map(safe_upload_key_segment)
        .filter(|value| !value.is_empty() && value.len() <= 512)
        .unwrap_or_else(|| Uuid::now_v7().to_string());
    let size_bytes = payload
        .file
        .as_ref()
        .and_then(|file| file.size)
        .unwrap_or(input.bytes.len() as u64);
    let mime_type = payload
        .file
        .and_then(|file| file.file_type)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| UPLOAD_IMAGE_MIME_TYPES.contains(&value.as_str()))
        .unwrap_or_else(|| input.mime_type.clone());
    Ok(UploadImageResponse {
        provider: UploadProvider::ImageBed.code().to_owned(),
        object_key,
        download_url,
        share_url,
        delete_url,
        mime_type,
        size_bytes,
    })
}

async fn upload_to_local(
    record: &AdminUploadConfigRecord,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let root = record
        .local_root
        .as_deref()
        .ok_or_else(|| AppError::Validation("local_root is not configured".to_owned()))?;
    let base_url = record
        .public_base_url
        .as_deref()
        .ok_or_else(|| AppError::Validation("public_base_url is not configured".to_owned()))?;
    let object_key = generated_upload_object_key(record.key_prefix.as_deref(), &input.mime_type);
    let path = PathBuf::from(root).join(&object_key);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|_| AppError::Internal("failed to create upload directory".to_owned()))?;
    }
    tokio::fs::write(&path, &input.bytes)
        .await
        .map_err(|_| AppError::Internal("failed to write upload file".to_owned()))?;
    Ok(UploadImageResponse {
        provider: UploadProvider::Local.code().to_owned(),
        download_url: join_upload_public_url(base_url, &object_key),
        share_url: None,
        delete_url: None,
        object_key,
        mime_type: input.mime_type.clone(),
        size_bytes: input.bytes.len() as u64,
    })
}

async fn upload_to_s3(
    record: &AdminUploadConfigRecord,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let access_key =
        decrypt_required_upload_secret(record.access_key_ciphertext.as_deref(), key, "access key")?;
    let secret_key =
        decrypt_required_upload_secret(record.secret_key_ciphertext.as_deref(), key, "secret key")?;
    let bucket = record
        .bucket
        .as_deref()
        .ok_or_else(|| AppError::Validation("bucket is not configured".to_owned()))?;
    let region = record
        .region
        .as_deref()
        .ok_or_else(|| AppError::Validation("region is not configured".to_owned()))?;
    let endpoint = record
        .endpoint
        .clone()
        .unwrap_or_else(|| format!("https://s3.{region}.amazonaws.com"));
    let object_key = generated_upload_object_key(record.key_prefix.as_deref(), &input.mime_type);
    let url = join_upload_endpoint_path(&endpoint, &[bucket, &object_key])?;
    let parsed_url = reqwest::Url::parse(&url)
        .map_err(|_| AppError::Validation("s3 endpoint is invalid".to_owned()))?;
    let host = upload_url_host(&parsed_url)?;
    let now = Utc::now();
    let date = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let payload_hash = sha256_hex(&input.bytes);
    let canonical_uri = parsed_url.path();
    let canonical_request = format!(
        "PUT\n{canonical_uri}\n\ncontent-type:{}\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n\ncontent-type;host;x-amz-content-sha256;x-amz-date\n{payload_hash}",
        input.mime_type
    );
    let scope = format!("{date}/{region}/s3/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let signature = s3_upload_signature(&secret_key, &date, region, &string_to_sign);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders=content-type;host;x-amz-content-sha256;x-amz-date, Signature={signature}"
    );
    let response = reqwest::Client::new()
        .put(&url)
        .header("content-type", &input.mime_type)
        .header("x-amz-content-sha256", payload_hash)
        .header("x-amz-date", amz_date)
        .header("authorization", authorization)
        .body(input.bytes.clone())
        .send()
        .await
        .map_err(|_| AppError::Validation("s3 upload failed".to_owned()))?;
    if !response.status().is_success() {
        return Err(AppError::Validation(format!(
            "s3 upload failed with status {}",
            response.status().as_u16()
        )));
    }
    Ok(UploadImageResponse {
        provider: UploadProvider::S3.code().to_owned(),
        download_url: record
            .public_base_url
            .as_deref()
            .map(|base| join_upload_public_url(base, &object_key))
            .unwrap_or(url),
        share_url: None,
        delete_url: None,
        object_key,
        mime_type: input.mime_type.clone(),
        size_bytes: input.bytes.len() as u64,
    })
}

async fn upload_to_oss(
    record: &AdminUploadConfigRecord,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let access_key =
        decrypt_required_upload_secret(record.access_key_ciphertext.as_deref(), key, "access key")?;
    let secret_key =
        decrypt_required_upload_secret(record.secret_key_ciphertext.as_deref(), key, "secret key")?;
    let endpoint = record
        .endpoint
        .as_deref()
        .ok_or_else(|| AppError::Validation("oss endpoint is not configured".to_owned()))?;
    let bucket = record
        .bucket
        .as_deref()
        .ok_or_else(|| AppError::Validation("bucket is not configured".to_owned()))?;
    let object_key = generated_upload_object_key(record.key_prefix.as_deref(), &input.mime_type);
    let url = join_upload_endpoint_path(endpoint, &[bucket, &object_key])?;
    let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let canonical_resource = format!("/{bucket}/{object_key}");
    let string_to_sign = format!("PUT\n\n{}\n{date}\n{canonical_resource}", input.mime_type);
    let signature = hmac_sha1_base64(secret_key.as_bytes(), &string_to_sign);
    let authorization = format!("OSS {access_key}:{signature}");
    let response = reqwest::Client::new()
        .put(&url)
        .header("date", date)
        .header("content-type", &input.mime_type)
        .header("authorization", authorization)
        .body(input.bytes.clone())
        .send()
        .await
        .map_err(|_| AppError::Validation("oss upload failed".to_owned()))?;
    if !response.status().is_success() {
        return Err(AppError::Validation(format!(
            "oss upload failed with status {}",
            response.status().as_u16()
        )));
    }
    Ok(UploadImageResponse {
        provider: UploadProvider::Oss.code().to_owned(),
        download_url: record
            .public_base_url
            .as_deref()
            .map(|base| join_upload_public_url(base, &object_key))
            .unwrap_or(url),
        share_url: None,
        delete_url: None,
        object_key,
        mime_type: input.mime_type.clone(),
        size_bytes: input.bytes.len() as u64,
    })
}

pub(crate) async fn multipart_file_input(mut multipart: Multipart) -> AppResult<UploadFileInput> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::Validation("upload multipart body is invalid".to_owned()))?
    {
        if field.name() != Some(DEFAULT_UPLOAD_FILE_FIELD) {
            continue;
        }
        let original_filename = field.file_name().map(str::to_owned);
        let mime_type = field.content_type().map(str::to_owned).ok_or_else(|| {
            AppError::Validation("upload file content type is required".to_owned())
        })?;
        let bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::Validation("upload file body is invalid".to_owned()))?
            .to_vec();
        return Ok(UploadFileInput {
            original_filename,
            mime_type,
            bytes,
        });
    }

    Err(AppError::Validation("upload file is required".to_owned()))
}

fn decrypt_required_upload_secret(
    ciphertext: Option<&str>,
    key: Option<&str>,
    field: &str,
) -> AppResult<String> {
    let ciphertext =
        ciphertext.ok_or_else(|| AppError::Validation(format!("{field} is not configured")))?;
    let key = key.ok_or_else(|| {
        AppError::Internal("credential encryption key is not configured".to_owned())
    })?;
    decrypt_optional_secret(Some(ciphertext), key)?
        .ok_or_else(|| AppError::Validation(format!("{field} is not configured")))
}

#[derive(Debug, Deserialize)]
struct ImageBedUploadResponse {
    success: Option<bool>,
    file: Option<ImageBedFile>,
    links: ImageBedLinks,
}

#[derive(Debug, Deserialize)]
struct ImageBedFile {
    id: Option<String>,
    size: Option<u64>,
    #[serde(rename = "type")]
    file_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImageBedLinks {
    download: Option<String>,
    share: Option<String>,
    delete: Option<String>,
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
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
