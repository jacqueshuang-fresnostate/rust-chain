use crate::{
    error::{AppError, AppResult},
    modules::{
        admin::{
            market_feed_config::{
                MarketFeedStatusResponse, MarketSourceCredentialSecret,
                MarketSourceCredentialsResponse, ReloadMarketFeedRequest, ReloadMarketFeedResponse,
                SaveMarketFeedConfigRequest, UpsertMarketSourceCredentialRequest,
                insert_reload_audit_log, list_credentials, load_config, load_enabled_credentials,
                mark_reload_failed, mark_reload_skipped, mark_reload_success,
                runtime_config_from_response, save_config, upsert_credential,
            },
            smtp_config::{
                SaveSmtpConfigRequest, SendSmtpTestRequest, SendSmtpTestResponse,
                SmtpConfigResponse, load_smtp_config, save_smtp_config, send_smtp_test_email,
            },
            upload_config::{
                MAX_UPLOAD_BODY_SIZE_BYTES, SaveUploadConfigRequest, UploadConfigResponse,
                UploadFileInput, UploadImageResponse, load_upload_config, save_upload_config,
                upload_image,
            },
        },
        auth::{AdminAuth, hash_password},
        new_coin::{LifecycleStatus, UnlockRule, UnlockSource, apply_unlock_rule},
    },
    state::AppState,
    time::{option_unix_millis, unix_millis},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    routing::{get, patch, post},
};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Value, json};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};
use std::collections::{BTreeSet, HashSet};
use uuid::Uuid;

struct AdminAuditEntry<'a> {
    action: &'a str,
    target_type: &'a str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
}

#[derive(Debug)]
struct AdminAgentAuditEntry<'a> {
    action: &'a str,
    target_type: &'a str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
}

const ADMIN_AUDIT_REASON_MAX_LEN: usize = 512;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard", get(get_admin_dashboard))
        .route(
            "/news",
            get(list_admin_news_items).post(create_admin_news_item),
        )
        .route(
            "/news/:id",
            get(get_admin_news_item).patch(update_admin_news_item),
        )
        .route("/news/:id/status", patch(update_admin_news_status))
        .route("/users", get(list_admin_users).post(create_admin_user))
        .route("/users/:id", get(get_admin_user))
        .route("/users/:id/recharge", post(recharge_admin_user_wallet))
        .route("/assets", get(list_assets).post(create_asset))
        .route("/assets/:id", get(get_asset).patch(update_asset))
        .route("/wallet/accounts", get(list_wallet_accounts))
        .route("/wallet/ledger", get(list_wallet_ledger))
        .route("/risk/rules", get(list_risk_rules).post(create_risk_rule))
        .route("/risk/rules/:id/status", patch(update_risk_rule_status))
        .route("/risk/events", get(list_risk_events))
        .route(
            "/market-pairs",
            get(list_trading_pairs).post(create_trading_pair),
        )
        .route(
            "/market-pairs/:id",
            get(get_trading_pair).patch(update_trading_pair),
        )
        .route(
            "/market-pairs/:id/status",
            patch(update_trading_pair_status),
        )
        .route(
            "/market-feed/config",
            get(get_market_feed_config).patch(save_market_feed_config),
        )
        .route("/market-feed/reload", post(reload_market_feed_config))
        .route("/market-feed/status", get(get_market_feed_status))
        .route(
            "/market-feed/credentials",
            get(list_market_feed_credentials),
        )
        .route(
            "/market-feed/credentials/:provider",
            patch(upsert_market_feed_credential),
        )
        .route(
            "/smtp/config",
            get(get_smtp_config).patch(save_smtp_config_route),
        )
        .route("/smtp/test", post(send_smtp_test))
        .route(
            "/upload/config",
            get(get_upload_config).patch(save_upload_config_route),
        )
        .route(
            "/uploads/images",
            post(upload_image_route).layer(DefaultBodyLimit::max(MAX_UPLOAD_BODY_SIZE_BYTES)),
        )
        .route(
            "/new-coins",
            get(list_new_coin_projects).post(create_new_coin_project),
        )
        .route("/new-coins/:id/lifecycle", patch(update_new_coin_lifecycle))
        .route("/new-coins/:id/distribute", post(distribute_new_coin))
        .route(
            "/new-coins/:id/unlock-rule",
            patch(update_new_coin_unlock_rule),
        )
        .route(
            "/new-coins/:id/post-listing-purchase",
            patch(update_new_coin_post_listing_purchase),
        )
        .route(
            "/new-coins/:id/unlock-fee-rule",
            patch(update_new_coin_unlock_fee_rule),
        )
        .route(
            "/new-coins/:id/subscriptions",
            get(list_new_coin_subscriptions),
        )
        .route(
            "/new-coins/:id/distributions",
            get(list_new_coin_distributions),
        )
        .route(
            "/new-coins/subscriptions",
            get(list_all_new_coin_subscriptions),
        )
        .route(
            "/new-coins/distributions",
            get(list_all_new_coin_distributions),
        )
        .route("/new-coins/purchases", get(list_new_coin_purchases))
        .route(
            "/new-coins/lock-positions",
            get(list_new_coin_lock_positions),
        )
        .route("/new-coins/unlocks", get(list_new_coin_unlocks))
        .route(
            "/convert/pairs",
            get(list_convert_pairs).post(create_convert_pair),
        )
        .route(
            "/convert/pairs/:id",
            get(get_convert_pair).patch(update_convert_pair_status),
        )
        .route(
            "/convert/new-coin-rules",
            post(upsert_new_coin_convert_rule),
        )
        .route("/convert/orders", get(list_convert_orders))
        .route("/convert/orders/:id", get(get_convert_order))
        .route(
            "/market-strategies",
            get(list_market_strategies).post(create_market_strategy),
        )
        .route("/market-strategies/:id", patch(update_market_strategy))
        .route(
            "/market-strategies/:id/status",
            patch(update_market_strategy_status),
        )
        .route("/audit-logs", get(list_admin_audit_logs))
        .route("/margin/liquidations", get(list_margin_liquidations))
        .route("/margin/liquidations/:id", get(get_margin_liquidation))
        .route("/agents", get(list_agents).post(create_agent))
        .route("/agents/:id", get(get_agent))
        .route("/agents/:id/status", patch(update_agent_status))
        .route("/agents/:id/users", get(list_agent_users))
        .route("/users/:id/agent", patch(assign_user_agent))
        .route(
            "/agent-commission-rules",
            get(list_agent_commission_rules).post(create_agent_commission_rule),
        )
        .route(
            "/agent-commission-rules/:id",
            patch(update_agent_commission_rule),
        )
        .route("/agent-commissions", get(list_agent_commissions))
        .route(
            "/agent-commissions/:id/status",
            patch(update_agent_commission_status),
        )
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ConvertOrdersQuery {
    user_id: Option<u64>,
    email: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminUserQuery {
    user_id: Option<u64>,
    email: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminWalletAccountQuery {
    user_id: Option<u64>,
    email: Option<String>,
    asset_id: Option<u64>,
    include_empty: Option<bool>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminWalletLedgerQuery {
    user_id: Option<u64>,
    email: Option<String>,
    asset_id: Option<u64>,
    change_type: Option<String>,
    ref_type: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminAssetQuery {
    symbol: Option<String>,
    asset_type: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminRiskRuleQuery {
    rule_type: Option<String>,
    target_type: Option<String>,
    enabled: Option<bool>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminRiskEventQuery {
    user_id: Option<u64>,
    email: Option<String>,
    decision: Option<String>,
    risk_level: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminTradingPairQuery {
    symbol: Option<String>,
    status: Option<String>,
    market_type: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminMarketStrategyQuery {
    pair_id: Option<u64>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminMarginLiquidationQuery {
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    position_id: Option<u64>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminAuditLogsQuery {
    admin_id: Option<u64>,
    action: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminScopedListQuery {
    user_id: Option<u64>,
    email: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminNewCoinPurchaseQuery {
    project_id: Option<u64>,
    user_id: Option<u64>,
    email: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminNewCoinFlatListQuery {
    project_id: Option<u64>,
    user_id: Option<u64>,
    email: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminNewCoinLockPositionQuery {
    user_id: Option<u64>,
    email: Option<String>,
    asset_id: Option<u64>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminNewCoinUnlockQuery {
    user_id: Option<u64>,
    email: Option<String>,
    asset_id: Option<u64>,
    status: Option<String>,
    fee_paid_status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CreateAdminUserRequest {
    email: Option<String>,
    phone: Option<String>,
    password: String,
    status: Option<String>,
    kyc_level: Option<i32>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdminUserRechargeRequest {
    asset_id: u64,
    amount: BigDecimal,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateConvertPairRequest {
    from_asset_id: u64,
    to_asset_id: u64,
    pricing_mode: String,
    spread_rate: BigDecimal,
    min_amount: BigDecimal,
    max_amount: Option<BigDecimal>,
    enabled: Option<bool>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateTradingPairRequest {
    base_asset_id: u64,
    quote_asset_id: u64,
    symbol: String,
    price_precision: i32,
    qty_precision: i32,
    min_order_value: BigDecimal,
    status: Option<String>,
    market_type: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateTradingPairStatusRequest {
    status: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateTradingPairRequest {
    price_precision: i32,
    qty_precision: i32,
    min_order_value: BigDecimal,
    market_type: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateAssetRequest {
    symbol: String,
    name: String,
    precision_scale: i32,
    asset_type: Option<String>,
    status: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateAssetRequest {
    name: String,
    precision_scale: i32,
    asset_type: String,
    status: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateNewCoinProjectRequest {
    asset_id: u64,
    symbol: String,
    lifecycle_status: String,
    total_supply: BigDecimal,
    issue_price: BigDecimal,
    #[serde(default, with = "option_unix_millis")]
    listed_at: Option<chrono::DateTime<chrono::Utc>>,
    unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    relative_unlock_seconds: Option<u64>,
    unlock_fee_enabled: Option<bool>,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateNewCoinLifecycleRequest {
    lifecycle_status: String,
    #[serde(default, with = "option_unix_millis")]
    listed_at: Option<chrono::DateTime<chrono::Utc>>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DistributeNewCoinRequest {
    user_id: u64,
    subscription_id: Option<u64>,
    quantity: BigDecimal,
    idempotency_key: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateNewCoinUnlockRuleRequest {
    unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    listed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, with = "option_unix_millis")]
    fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    relative_unlock_seconds: Option<u64>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateNewCoinUnlockFeeRuleRequest {
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateNewCoinPostListingPurchaseRequest {
    enabled: bool,
    pair_id: Option<u64>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpsertNewCoinConvertRuleRequest {
    convert_pair_id: u64,
    rate_source: String,
    fixed_rate: Option<BigDecimal>,
    floating_rate_json: Option<Value>,
    status: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateConvertPairStatusRequest {
    enabled: bool,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateAgentRequest {
    user_id: u64,
    agent_code: String,
    admin_username: String,
    admin_password: Option<String>,
    admin_password_hash: Option<String>,
    level: Option<i32>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateAgentStatusRequest {
    status: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateAgentCommissionStatusRequest {
    status: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateAgentCommissionRuleRequest {
    agent_id: u64,
    product_type: String,
    commission_rate: BigDecimal,
    status: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateAgentCommissionRuleRequest {
    commission_rate: Option<BigDecimal>,
    status: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateRiskRuleRequest {
    rule_type: String,
    target_type: String,
    target_id: Option<String>,
    config_json: Value,
    enabled: Option<bool>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateRiskRuleStatusRequest {
    enabled: bool,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateMarketStrategyRequest {
    pair_id: u64,
    strategy_type: String,
    start_price: BigDecimal,
    target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    start_time: chrono::DateTime<chrono::Utc>,
    #[serde(with = "unix_millis")]
    end_time: chrono::DateTime<chrono::Utc>,
    volatility: BigDecimal,
    volume_min: BigDecimal,
    volume_max: BigDecimal,
    status: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateMarketStrategyRequest {
    strategy_type: String,
    start_price: BigDecimal,
    target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    start_time: chrono::DateTime<chrono::Utc>,
    #[serde(with = "unix_millis")]
    end_time: chrono::DateTime<chrono::Utc>,
    volatility: BigDecimal,
    volume_min: BigDecimal,
    volume_max: BigDecimal,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateMarketStrategyStatusRequest {
    status: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AssignUserAgentRequest {
    agent_id: u64,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdminAgentQuery {
    agent_id: Option<u64>,
    user_id: Option<u64>,
    agent_code: Option<String>,
    email: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminAgentCommissionQuery {
    agent_id: Option<u64>,
    user_id: Option<u64>,
    email: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminAgentCommissionRuleQuery {
    agent_id: Option<u64>,
    product_type: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminNewsQuery {
    status: Option<String>,
    category: Option<String>,
    country_code: Option<String>,
    locale: Option<String>,
    q: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CreateAdminNewsItemRequest {
    title: String,
    category: String,
    status: Option<String>,
    country_code: Option<String>,
    default_locale: String,
    content_json: Value,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateAdminNewsItemRequest {
    title: String,
    category: String,
    country_code: Option<String>,
    default_locale: String,
    content_json: Value,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateAdminNewsStatusRequest {
    status: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminUserResponse {
    id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    kyc_level: i32,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "unix_millis")]
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminWalletAccountResponse {
    id: Option<u64>,
    user_id: u64,
    asset_id: u64,
    asset_symbol: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    available: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    frozen: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    locked: BigDecimal,
    account_exists: bool,
    #[serde(with = "unix_millis")]
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct AdminWalletEmptyAssetRow {
    asset_id: u64,
    asset_symbol: String,
}

#[derive(Debug, Serialize)]
struct AdminUserRechargeResponse {
    recharge_id: String,
    user_id: u64,
    asset_id: u64,
    asset_symbol: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    available: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    frozen: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    locked: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct AdminAssetSymbolRow {
    symbol: String,
    status: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminWalletLedgerResponse {
    id: u64,
    user_id: u64,
    asset_id: u64,
    asset_symbol: String,
    change_type: String,
    amount: BigDecimal,
    balance_type: String,
    balance_after: BigDecimal,
    available_after: BigDecimal,
    frozen_after: BigDecimal,
    locked_after: BigDecimal,
    ref_type: String,
    ref_id: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminAssetResponse {
    id: u64,
    symbol: String,
    name: String,
    precision_scale: i32,
    asset_type: String,
    status: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminNewsItemResponse {
    id: u64,
    title: String,
    category: String,
    status: String,
    country_code: Option<String>,
    default_locale: String,
    content_json: SqlxJson<Value>,
    #[serde(default, with = "option_unix_millis")]
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    created_by_admin_id: Option<u64>,
    updated_by_admin_id: Option<u64>,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "unix_millis")]
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct RiskRuleResponse {
    id: u64,
    rule_type: String,
    target_type: String,
    target_id: Option<String>,
    config_json: SqlxJson<Value>,
    enabled: bool,
    created_by: Option<u64>,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "unix_millis")]
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct RiskEventResponse {
    id: u64,
    user_id: Option<u64>,
    actor_type: String,
    actor_id: Option<u64>,
    event_type: String,
    risk_level: String,
    decision: String,
    reason: Option<String>,
    payload_json: SqlxJson<Value>,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ConvertPairResponse {
    id: u64,
    from_asset_id: u64,
    to_asset_id: u64,
    pricing_mode: String,
    spread_rate: BigDecimal,
    min_amount: BigDecimal,
    max_amount: Option<BigDecimal>,
    enabled: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminTradingPairResponse {
    id: u64,
    base_asset_id: u64,
    quote_asset_id: u64,
    symbol: String,
    base_asset: String,
    quote_asset: String,
    price_precision: i32,
    qty_precision: i32,
    min_order_value: BigDecimal,
    status: String,
    market_type: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinProjectResponse {
    id: u64,
    asset_id: u64,
    symbol: String,
    lifecycle_status: String,
    total_supply: BigDecimal,
    issue_price: BigDecimal,
    #[serde(default, with = "option_unix_millis")]
    listed_at: Option<chrono::DateTime<chrono::Utc>>,
    unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    relative_unlock_seconds: Option<u64>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    status: String,
    post_listing_purchase_enabled: bool,
    post_listing_pair_id: Option<u64>,
    post_listing_pair_status: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ConvertOrderResponse {
    id: u64,
    quote_id: String,
    convert_pair_id: u64,
    user_id: u64,
    from_asset_id: u64,
    to_asset_id: u64,
    from_amount: BigDecimal,
    to_amount: BigDecimal,
    rate: BigDecimal,
    status: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminMarketStrategyResponse {
    id: u64,
    pair_id: u64,
    symbol: String,
    market_type: String,
    strategy_type: String,
    start_price: BigDecimal,
    target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    start_time: chrono::DateTime<chrono::Utc>,
    #[serde(with = "unix_millis")]
    end_time: chrono::DateTime<chrono::Utc>,
    volatility: BigDecimal,
    volume_min: BigDecimal,
    volume_max: BigDecimal,
    status: String,
    run_status: Option<String>,
    current_price: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    last_generated_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, with = "option_unix_millis")]
    last_kline_open_time: Option<chrono::DateTime<chrono::Utc>>,
    recovery_status: Option<String>,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminMarginLiquidationResponse {
    id: u64,
    position_id: u64,
    user_id: u64,
    product_id: u64,
    pair_id: u64,
    margin_asset: u64,
    direction: String,
    margin_amount: BigDecimal,
    notional_amount: BigDecimal,
    interest_amount: BigDecimal,
    entry_price: BigDecimal,
    mark_price: BigDecimal,
    maintenance_margin_rate: BigDecimal,
    equity: BigDecimal,
    maintenance_margin: BigDecimal,
    realized_pnl: BigDecimal,
    payout_amount: BigDecimal,
    reason: String,
    #[serde(with = "unix_millis")]
    liquidated_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinSubscriptionResponse {
    id: u64,
    project_id: u64,
    user_id: u64,
    quote_asset: u64,
    quote_amount: BigDecimal,
    requested_quantity: BigDecimal,
    allocated_quantity: BigDecimal,
    status: String,
    idempotency_key: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinDistributionResponse {
    id: u64,
    project_id: u64,
    user_id: u64,
    subscription_id: Option<u64>,
    asset_id: u64,
    quantity: BigDecimal,
    lock_position_id: Option<u64>,
    status: String,
    idempotency_key: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinPurchaseResponse {
    id: u64,
    project_id: u64,
    user_id: u64,
    pair_id: u64,
    base_asset: u64,
    quote_asset: u64,
    price: BigDecimal,
    quantity: BigDecimal,
    quote_amount: BigDecimal,
    lock_position_id: Option<u64>,
    status: String,
    idempotency_key: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinLockPositionResponse {
    id: u64,
    user_id: u64,
    asset_id: u64,
    unlock_type: String,
    #[serde(with = "unix_millis")]
    unlock_at: chrono::DateTime<chrono::Utc>,
    locked_amount: BigDecimal,
    released_amount: BigDecimal,
    remaining_amount: BigDecimal,
    merge_key: String,
    status: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinUnlockResponse {
    id: u64,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    unlock_quantity: BigDecimal,
    unlock_price: Option<BigDecimal>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    unlock_fee_amount: Option<BigDecimal>,
    fee_paid_status: String,
    status: String,
    idempotency_key: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminAgentResponse {
    id: u64,
    user_id: u64,
    email: Option<String>,
    agent_code: String,
    level: i32,
    status: String,
    admin_user_id: Option<u64>,
    admin_username: Option<String>,
    admin_status: Option<String>,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminAgentUserResponse {
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    kyc_level: i32,
    root_agent_id: u64,
    direct_inviter_id: Option<u64>,
    direct_inviter_type: Option<String>,
    depth: i32,
    path: String,
    #[serde(with = "unix_millis")]
    referred_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminUserReferralResponse {
    user_id: u64,
    direct_inviter_id: Option<u64>,
    direct_inviter_type: Option<String>,
    root_agent_id: Option<u64>,
    depth: i32,
    path: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminAgentCommissionResponse {
    id: u64,
    agent_id: u64,
    user_id: u64,
    source_type: String,
    source_id: String,
    source_amount: BigDecimal,
    commission_amount: BigDecimal,
    status: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminAgentCommissionRuleResponse {
    id: u64,
    agent_id: u64,
    product_type: String,
    commission_rate: BigDecimal,
    status: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "unix_millis")]
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct AgentCommissionPayoutTargetRow {
    agent_user_id: u64,
    asset_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct AdminWalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug)]
struct AdminNewCoinLockPositionInsert {
    user_id: u64,
    asset_id: u64,
    unlock_type: String,
    unlock_at: chrono::DateTime<chrono::Utc>,
    amount: BigDecimal,
    merge_key: String,
    source_time: chrono::DateTime<chrono::Utc>,
    source_type: String,
    source_id: String,
}

#[derive(Debug, Clone, Copy)]
struct AdminNewCoinLedgerMetadata<'a> {
    change_type: &'a str,
    ref_type: &'a str,
    ref_id: &'a str,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinConvertRuleResponse {
    id: u64,
    convert_pair_id: u64,
    rate_source: String,
    fixed_rate: Option<BigDecimal>,
    floating_rate_json: Option<SqlxJson<Value>>,
    status: String,
    created_by: Option<u64>,
}

#[derive(Debug, Serialize)]
struct AdminDashboardResponse {
    #[serde(with = "unix_millis")]
    generated_at: chrono::DateTime<chrono::Utc>,
    users: AdminDashboardUsersSummary,
    wallet: AdminDashboardWalletSummary,
    market: AdminDashboardMarketSummary,
    trading: AdminDashboardTradingSummary,
    products: AdminDashboardProductsSummary,
    risk: AdminDashboardRiskSummary,
    audit: AdminDashboardAuditSummary,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminDashboardUsersSummary {
    total: i64,
    active: i64,
    new_24h: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminDashboardWalletSummary {
    active_assets: i64,
    wallet_accounts: i64,
    non_zero_accounts: i64,
    pending_unlocks: i64,
    pending_deposits: i64,
    pending_withdrawals: i64,
    custody_status: String,
}

#[derive(Debug, Serialize)]
struct AdminDashboardMarketSummary {
    active_pairs: i64,
    disabled_pairs: i64,
    external_pairs: i64,
    strategy_pairs: i64,
    feed_runtime_status: String,
    feed_needs_reload: bool,
    feed_symbols: Vec<String>,
    feed_providers: Vec<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct AdminDashboardMarketCounts {
    active_pairs: i64,
    disabled_pairs: i64,
    external_pairs: i64,
    strategy_pairs: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminDashboardTradingSummary {
    spot_open_orders: i64,
    spot_trades_24h: i64,
    convert_pending_orders: i64,
    convert_completed_24h: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminDashboardProductsSummary {
    seconds_open_orders: i64,
    margin_open_positions: i64,
    margin_liquidated_24h: i64,
    earn_active_subscriptions: i64,
    earn_maturing_24h: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminDashboardRiskSummary {
    risk_events_24h: i64,
    blocked_events_24h: i64,
    pending_outbox_events: i64,
    retry_inbox_events: i64,
    dead_letter_inbox_events: i64,
}

#[derive(Debug, Serialize)]
struct AdminDashboardAuditSummary {
    admin_actions_24h: i64,
    latest_actions: Vec<AdminDashboardAuditAction>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminDashboardAuditAction {
    id: u64,
    admin_id: u64,
    action: String,
    target_type: String,
    target_id: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct AdminUsersResponse {
    users: Vec<AdminUserResponse>,
}

#[derive(Debug, Serialize)]
struct AdminWalletAccountsResponse {
    accounts: Vec<AdminWalletAccountResponse>,
}

#[derive(Debug, Serialize)]
struct AdminWalletLedgerResponseList {
    ledger: Vec<AdminWalletLedgerResponse>,
}

#[derive(Debug, Serialize)]
struct AdminAssetsResponse {
    assets: Vec<AdminAssetResponse>,
}

#[derive(Debug, Serialize)]
struct AdminNewsItemsResponse {
    news: Vec<AdminNewsItemResponse>,
}

#[derive(Debug, Serialize)]
struct RiskRulesResponse {
    rules: Vec<RiskRuleResponse>,
}

#[derive(Debug, Serialize)]
struct RiskEventsResponse {
    events: Vec<RiskEventResponse>,
}

#[derive(Debug, Serialize)]
struct ConvertPairsResponse {
    pairs: Vec<ConvertPairResponse>,
}

#[derive(Debug, Serialize)]
struct AdminTradingPairsResponse {
    pairs: Vec<AdminTradingPairResponse>,
}

#[derive(Debug, Serialize)]
struct NewCoinProjectsResponse {
    projects: Vec<NewCoinProjectResponse>,
}

#[derive(Debug, Serialize)]
struct ConvertOrdersResponse {
    orders: Vec<ConvertOrderResponse>,
}

#[derive(Debug, Serialize)]
struct AdminMarketStrategiesResponse {
    strategies: Vec<AdminMarketStrategyResponse>,
}

#[derive(Debug, Serialize)]
struct AdminAuditLogsResponse {
    logs: Vec<AdminAuditLogResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminAuditLogResponse {
    id: u64,
    admin_id: u64,
    action: String,
    target_type: String,
    target_id: String,
    before_json: Option<SqlxJson<Value>>,
    after_json: Option<SqlxJson<Value>>,
    reason: Option<String>,
    ip: Option<String>,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct AdminMarginLiquidationsResponse {
    liquidations: Vec<AdminMarginLiquidationResponse>,
}

#[derive(Debug, Serialize)]
struct NewCoinSubscriptionsResponse {
    subscriptions: Vec<NewCoinSubscriptionResponse>,
}

#[derive(Debug, Serialize)]
struct NewCoinDistributionsResponse {
    distributions: Vec<NewCoinDistributionResponse>,
}

#[derive(Debug, Serialize)]
struct NewCoinPurchasesResponse {
    purchases: Vec<NewCoinPurchaseResponse>,
}

#[derive(Debug, Serialize)]
struct NewCoinLockPositionsResponse {
    lock_positions: Vec<NewCoinLockPositionResponse>,
}

#[derive(Debug, Serialize)]
struct NewCoinUnlocksResponse {
    unlocks: Vec<NewCoinUnlockResponse>,
}

#[derive(Debug, Serialize)]
struct AdminAgentsResponse {
    agents: Vec<AdminAgentResponse>,
}

#[derive(Debug, Serialize)]
struct AdminAgentUsersResponse {
    users: Vec<AdminAgentUserResponse>,
}

#[derive(Debug, Serialize)]
struct AdminAgentCommissionsResponse {
    commissions: Vec<AdminAgentCommissionResponse>,
}

#[derive(Debug, Serialize)]
struct AdminAgentCommissionRulesResponse {
    rules: Vec<AdminAgentCommissionRuleResponse>,
}

async fn list_agents(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentQuery>,
) -> AppResult<Json<AdminAgentsResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = base_agent_query();
    builder.push(" WHERE 1 = 1");
    if let Some(agent_id) = query.agent_id {
        builder.push(" AND agents.id = ");
        builder.push_bind(agent_id);
    }
    if let Some(user_id) = query.user_id {
        push_user_id_filter(&mut builder, "agents.user_id", user_id);
    }
    if let Some(agent_code) = optional_string(query.agent_code) {
        builder.push(" AND agents.agent_code = ");
        builder.push_bind(agent_code);
    }
    push_user_email_filter(&mut builder, "agents.user_id", query.email);
    if let Some(status) = optional_string(query.status) {
        builder.push(" AND agents.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY agents.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    builder.push(" OFFSET ");
    builder.push_bind(route_offset(query.offset) as i64);

    let agents = builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_all(&pool)
        .await?;

    Ok(Json(AdminAgentsResponse { agents }))
}

async fn get_agent(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
) -> AppResult<Json<AdminAgentResponse>> {
    let mut builder = base_agent_query();
    builder.push(" WHERE agents.id = ");
    builder.push_bind(agent_id);
    builder.push(" ORDER BY agent_admin_users.id ASC LIMIT 1");
    let agent = builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_optional(&mysql_pool(&state)?)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(agent))
}

async fn create_agent(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAgentRequest>,
) -> AppResult<Json<AdminAgentResponse>> {
    // 先校验代理编码、后台账号和层级，避免无效代理资料进入事务。
    validate_create_agent(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let agent_code = optional_string(Some(request.agent_code.clone())).unwrap();
    let admin_username = optional_string(Some(request.admin_username.clone())).unwrap();
    let admin_password_hash = agent_password_hash(&request)?;
    let level = request.level.unwrap_or(1);

    // 创建代理前先锁定并确认归属用户存在，避免外键错误泄露为内部错误。
    ensure_user_exists_in_tx(&mut tx, request.user_id).await?;

    // 创建代理主表与代理后台账号必须同事务提交，避免出现无后台账号的半成品代理。
    let agent_id = sqlx::query(
        r#"INSERT INTO agents (user_id, agent_code, level, status)
           VALUES (?, ?, ?, 'active')"#,
    )
    .bind(request.user_id)
    .bind(&agent_code)
    .bind(level)
    .execute(&mut *tx)
    .await
    .map_err(map_duplicate_agent)?
    .last_insert_id();
    let agent_admin_id = sqlx::query(
        r#"INSERT INTO agent_admin_users (agent_id, username, password_hash, status)
           VALUES (?, ?, ?, 'active')"#,
    )
    .bind(agent_id)
    .bind(&admin_username)
    .bind(&admin_password_hash)
    .execute(&mut *tx)
    .await
    .map_err(map_duplicate_agent)?
    .last_insert_id();
    let after = load_agent_in_tx(&mut tx, agent_id).await?;
    let _ = agent_admin_id;
    insert_admin_agent_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAgentAuditEntry {
            action: "agent.create",
            target_type: "agent",
            target_id: agent_id,
            before_json: None,
            after_json: Some(agent_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn update_agent_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
    Json(request): Json<UpdateAgentStatusRequest>,
) -> AppResult<Json<AdminAgentResponse>> {
    let status = validate_agent_status(&request.status)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    // 锁定代理行后再更新状态，确保审计 before/after 与业务状态一致。
    let before = lock_agent_in_tx(&mut tx, agent_id).await?;
    sqlx::query("UPDATE agents SET status = ? WHERE id = ?")
        .bind(&status)
        .bind(agent_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE agent_admin_users SET status = ? WHERE agent_id = ?")
        .bind(&status)
        .bind(agent_id)
        .execute(&mut *tx)
        .await?;
    let after = load_agent_in_tx(&mut tx, agent_id).await?;
    insert_admin_agent_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAgentAuditEntry {
            action: "agent.status.update",
            target_type: "agent",
            target_id: agent_id,
            before_json: Some(agent_audit_json(&before)),
            after_json: Some(agent_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn list_agent_users(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<AdminAgentUsersResponse>> {
    // 后台按指定 root_agent_id 查看团队用户，只读接口不写审计。
    let users = sqlx::query_as::<_, AdminAgentUserResponse>(
        r#"SELECT users.id AS user_id, users.email, users.phone, users.status, users.kyc_level,
                  referrals.root_agent_id, referrals.direct_inviter_id, referrals.direct_inviter_type,
                  referrals.depth, referrals.path, referrals.created_at AS referred_at
           FROM user_referrals referrals
           INNER JOIN users ON users.id = referrals.user_id
           WHERE referrals.root_agent_id = ?
           ORDER BY referrals.depth ASC, users.id ASC
           LIMIT ?"#,
    )
    .bind(agent_id)
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;

    Ok(Json(AdminAgentUsersResponse { users }))
}

async fn assign_user_agent(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
    Json(request): Json<AssignUserAgentRequest>,
) -> AppResult<Json<AdminUserReferralResponse>> {
    if request.agent_id == 0 {
        return Err(AppError::Validation("agent_id is required".to_owned()));
    }
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    // 锁定目标用户、代理和既有归属，防止后台并发改派导致团队归属覆盖。
    ensure_user_exists_in_tx(&mut tx, user_id).await?;
    let agent = lock_agent_in_tx(&mut tx, request.agent_id).await?;
    if agent.status != "active" {
        return Err(AppError::Conflict(
            "only active agents can receive assigned users".to_owned(),
        ));
    }
    let before = lock_user_referral_in_tx(&mut tx, user_id).await?;
    let previous_tree = before.as_ref().map(|referral| {
        (
            referral.path.clone(),
            referral.depth,
            referral.root_agent_id,
        )
    });
    let path = format!("/{}/{}/{}", request.agent_id, request.agent_id, user_id);
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
    .bind(user_id)
    .bind(request.agent_id)
    .bind(request.agent_id)
    .bind(&path)
    .execute(&mut *tx)
    .await?;
    if let Some((old_path, old_depth, old_root_agent_id)) = previous_tree.as_ref() {
        // 改派目标用户后，同步迁移同一旧归属下的邀请子树，避免 path 前缀碰撞误迁移其他团队。
        migrate_user_referral_descendants_in_tx(
            &mut tx,
            user_id,
            old_path,
            *old_depth,
            *old_root_agent_id,
            request.agent_id,
            &path,
        )
        .await?;
    }
    let after = load_user_referral_in_tx(&mut tx, user_id).await?;
    insert_admin_agent_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAgentAuditEntry {
            action: "user_referral.assign_agent",
            target_type: "user_referral",
            target_id: user_id,
            before_json: before.as_ref().map(user_referral_audit_json),
            after_json: Some(user_referral_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn list_agent_commission_rules(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentCommissionRuleQuery>,
) -> AppResult<Json<AdminAgentCommissionRulesResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, agent_id, product_type, commission_rate, status, created_at, updated_at
           FROM agent_commission_rules
           WHERE 1 = 1"#,
    );
    if let Some(agent_id) = query.agent_id {
        builder.push(" AND agent_id = ");
        builder.push_bind(agent_id);
    }
    if let Some(product_type) = optional_string(query.product_type) {
        builder.push(" AND product_type = ");
        builder.push_bind(product_type);
    }
    if let Some(status) = optional_string(query.status) {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    builder.push(" OFFSET ");
    builder.push_bind(route_offset(query.offset) as i64);

    let rules = builder
        .build_query_as::<AdminAgentCommissionRuleResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminAgentCommissionRulesResponse { rules }))
}

async fn create_agent_commission_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAgentCommissionRuleRequest>,
) -> AppResult<Json<AdminAgentCommissionRuleResponse>> {
    let reason = required_reason(request.reason.clone())?;
    let product_type = validate_agent_commission_rule_product_type(&request.product_type)?;
    let status =
        validate_agent_commission_rule_status(request.status.as_deref().unwrap_or("active"))?;
    validate_agent_commission_rate(&request.commission_rate)?;
    if request.agent_id == 0 {
        return Err(AppError::Validation("agent_id is required".to_owned()));
    }
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    lock_agent_in_tx(&mut tx, request.agent_id).await?;
    let rule_id = sqlx::query(
        r#"INSERT INTO agent_commission_rules (agent_id, product_type, commission_rate, status)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(request.agent_id)
    .bind(&product_type)
    .bind(&request.commission_rate)
    .bind(&status)
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    let after = load_agent_commission_rule_in_tx(&mut tx, rule_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "agent_commission_rule.create",
            target_type: "agent_commission_rule",
            target_id: rule_id,
            before_json: None,
            after_json: Some(agent_commission_rule_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn update_agent_commission_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(rule_id): Path<u64>,
    Json(request): Json<UpdateAgentCommissionRuleRequest>,
) -> AppResult<Json<AdminAgentCommissionRuleResponse>> {
    let reason = required_reason(request.reason.clone())?;
    let commission_rate = if let Some(commission_rate) = request.commission_rate {
        validate_agent_commission_rate(&commission_rate)?;
        Some(commission_rate)
    } else {
        None
    };
    let status = request
        .status
        .as_deref()
        .map(validate_agent_commission_rule_status)
        .transpose()?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_agent_commission_rule_in_tx(&mut tx, rule_id).await?;
    sqlx::query(
        r#"UPDATE agent_commission_rules
           SET commission_rate = COALESCE(?, commission_rate),
               status = COALESCE(?, status)
           WHERE id = ?"#,
    )
    .bind(commission_rate.as_ref())
    .bind(status.as_ref())
    .bind(rule_id)
    .execute(&mut *tx)
    .await?;
    let after = load_agent_commission_rule_in_tx(&mut tx, rule_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "agent_commission_rule.update",
            target_type: "agent_commission_rule",
            target_id: rule_id,
            before_json: Some(agent_commission_rule_audit_json(&before)),
            after_json: Some(agent_commission_rule_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn list_agent_commissions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentCommissionQuery>,
) -> AppResult<Json<AdminAgentCommissionsResponse>> {
    // 后台佣金列表支持代理、用户和状态过滤，所有动态条件均使用 bind 参数。
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, agent_id, user_id, source_type, source_id, source_amount, commission_amount, status, created_at
           FROM agent_commission_records
           WHERE 1 = 1"#,
    );
    if let Some(agent_id) = query.agent_id {
        builder.push(" AND agent_id = ");
        builder.push_bind(agent_id);
    }
    if let Some(user_id) = query.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", query.email);
    if let Some(status) = optional_string(query.status) {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let commissions = builder
        .build_query_as::<AdminAgentCommissionResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminAgentCommissionsResponse { commissions }))
}

async fn update_agent_commission_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(commission_id): Path<u64>,
    Json(request): Json<UpdateAgentCommissionStatusRequest>,
) -> AppResult<Json<AdminAgentCommissionResponse>> {
    let status = validate_agent_commission_status(&request.status)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    // 锁定佣金记录后只允许 pending 状态进入结算/拒绝，避免重复结算或事后覆盖。
    let before = lock_agent_commission_in_tx(&mut tx, commission_id).await?;
    if before.status != "pending" {
        return Err(AppError::Conflict(
            "agent commission status can only be updated from pending".to_owned(),
        ));
    }
    if status == "settled" {
        settle_agent_commission_payout_in_tx(&mut tx, &before).await?;
    }
    sqlx::query("UPDATE agent_commission_records SET status = ? WHERE id = ?")
        .bind(&status)
        .bind(commission_id)
        .execute(&mut *tx)
        .await?;
    let after = load_agent_commission_in_tx(&mut tx, commission_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "agent_commission.status.update",
            target_type: "agent_commission",
            target_id: commission_id,
            before_json: Some(agent_commission_audit_json(&before)),
            after_json: Some(agent_commission_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn get_market_feed_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<crate::modules::admin::market_feed_config::MarketFeedConfigResponse>>> {
    Ok(Json(load_config(&mysql_pool(&state)?).await?))
}

async fn save_market_feed_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveMarketFeedConfigRequest>,
) -> AppResult<Json<crate::modules::admin::market_feed_config::MarketFeedConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = save_config(&mysql_pool(&state)?, admin_id, request).await?;
    Ok(Json(config))
}

async fn get_market_feed_status(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarketFeedStatusResponse>> {
    let saved_config = load_config(&mysql_pool(&state)?).await?;
    let runtime = match &state.market_feed_supervisor {
        Some(supervisor) => supervisor.status().await,
        None => Default::default(),
    };
    Ok(Json(MarketFeedStatusResponse {
        saved_config,
        runtime,
    }))
}

async fn list_market_feed_credentials(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarketSourceCredentialsResponse>> {
    let credentials = list_credentials(&mysql_pool(&state)?).await?;
    Ok(Json(MarketSourceCredentialsResponse { credentials }))
}

async fn upsert_market_feed_credential(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Json(request): Json<UpsertMarketSourceCredentialRequest>,
) -> AppResult<Json<crate::modules::admin::market_feed_config::MarketSourceCredentialResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let key = state.settings.exposed_credential_encryption_key();
    let credential =
        upsert_credential(&mysql_pool(&state)?, admin_id, provider, key, request).await?;
    Ok(Json(credential))
}

async fn get_smtp_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<SmtpConfigResponse>>> {
    Ok(Json(load_smtp_config(&mysql_pool(&state)?).await?))
}

async fn save_smtp_config_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveSmtpConfigRequest>,
) -> AppResult<Json<SmtpConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = save_smtp_config(
        &mysql_pool(&state)?,
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        request,
    )
    .await?;
    Ok(Json(config))
}

async fn send_smtp_test(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SendSmtpTestRequest>,
) -> AppResult<Json<SendSmtpTestResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let sender = state
        .email_sender
        .clone()
        .ok_or_else(|| AppError::Internal("email sender is not configured".to_owned()))?;
    let response = send_smtp_test_email(
        &pool,
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        sender.as_ref(),
        request,
    )
    .await?;
    Ok(Json(response))
}

async fn get_upload_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<UploadConfigResponse>>> {
    Ok(Json(load_upload_config(&mysql_pool(&state)?).await?))
}

async fn save_upload_config_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveUploadConfigRequest>,
) -> AppResult<Json<UploadConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = save_upload_config(
        &mysql_pool(&state)?,
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        request,
    )
    .await?;
    Ok(Json(config))
}

async fn upload_image_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    multipart: Multipart,
) -> AppResult<Json<UploadImageResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let input = multipart_file_input(multipart).await?;
    let response = upload_image(
        &mysql_pool(&state)?,
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        input,
    )
    .await?;
    Ok(Json(response))
}

async fn multipart_file_input(mut multipart: Multipart) -> AppResult<UploadFileInput> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::Validation("upload multipart body is invalid".to_owned()))?
    {
        if field.name() != Some("file") {
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

async fn reload_market_feed_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<ReloadMarketFeedRequest>,
) -> AppResult<Json<ReloadMarketFeedResponse>> {
    let reason = optional_string(Some(request.reason))
        .ok_or_else(|| AppError::Validation("operation reason is required".to_owned()))?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let config = load_config(&pool).await?.ok_or(AppError::NotFound)?;
    let supervisor = state
        .market_feed_supervisor
        .clone()
        .ok_or_else(|| AppError::Internal("market feed supervisor is not configured".to_owned()))?;

    if !config.enabled {
        supervisor.stop().await;
        let config = mark_reload_skipped(&pool, config.version).await?;
        let runtime = supervisor.status().await;
        insert_reload_audit_log(&pool, admin_id, &config, &runtime, reason).await?;
        return Ok(Json(ReloadMarketFeedResponse { config, runtime }));
    }

    let credentials = load_enabled_credentials(
        &pool,
        &config.providers,
        state.settings.exposed_credential_encryption_key(),
    )
    .await?;
    validate_loaded_market_feed_credentials(&config.providers, &credentials)?;
    drop(credentials);
    let runtime_config = match runtime_config_from_response(&state.settings, &config) {
        Ok(runtime_config) => runtime_config,
        Err(error) => {
            let config = mark_reload_failed(&pool, &error.to_string()).await?;
            let runtime = supervisor.record_failure(error.to_string()).await;
            insert_reload_audit_log(&pool, admin_id, &config, &runtime, reason).await?;
            return Err(error);
        }
    };

    let runtime = match supervisor
        .reload(state.clone(), runtime_config, config.version)
        .await
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let config = mark_reload_failed(&pool, &error.to_string()).await?;
            let runtime = supervisor.record_failure(error.to_string()).await;
            insert_reload_audit_log(&pool, admin_id, &config, &runtime, reason).await?;
            return Err(error);
        }
    };
    let config = mark_reload_success(&pool, config.version).await?;
    insert_reload_audit_log(&pool, admin_id, &config, &runtime, reason).await?;
    Ok(Json(ReloadMarketFeedResponse { config, runtime }))
}

fn validate_loaded_market_feed_credentials(
    providers: &[String],
    credentials: &[MarketSourceCredentialSecret],
) -> AppResult<()> {
    for provider in providers {
        let missing_api_key = credentials
            .iter()
            .find(|credential| credential.provider == *provider)
            .is_some_and(|credential| {
                credential.auth_type == "api_key"
                    && credential.api_key.as_deref().unwrap_or("").is_empty()
            });
        if missing_api_key {
            return Err(AppError::Validation(format!(
                "market feed provider {provider} api_key is required"
            )));
        }
    }
    Ok(())
}

async fn get_admin_dashboard(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AdminDashboardResponse>> {
    let pool = mysql_pool(&state)?;
    let generated_at = chrono::Utc::now();
    let users = sqlx::query_as::<_, AdminDashboardUsersSummary>(
        r#"SELECT COUNT(*) AS total,
                  COUNT(CASE WHEN status = 'active' THEN 1 END) AS active,
                  COUNT(CASE WHEN created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)
                             THEN 1 END) AS new_24h
           FROM users"#,
    )
    .fetch_one(&pool)
    .await?;
    let wallet = sqlx::query_as::<_, AdminDashboardWalletSummary>(
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
    .fetch_one(&pool)
    .await?;
    let market_counts = sqlx::query_as::<_, AdminDashboardMarketCounts>(
        r#"SELECT COUNT(CASE WHEN status = 'active' THEN 1 END) AS active_pairs,
                  COUNT(CASE WHEN status = 'disabled' THEN 1 END) AS disabled_pairs,
                  COUNT(CASE WHEN market_type = 'external' THEN 1 END) AS external_pairs,
                  COUNT(CASE WHEN market_type = 'strategy' THEN 1 END) AS strategy_pairs
           FROM trading_pairs"#,
    )
    .fetch_one(&pool)
    .await?;
    let saved_feed_config = load_config(&pool).await?;
    let runtime = match &state.market_feed_supervisor {
        Some(supervisor) => supervisor.status().await,
        None => Default::default(),
    };
    let feed_runtime_status = runtime
        .last_reload_status
        .clone()
        .unwrap_or_else(|| "not_started".to_owned());
    let market = AdminDashboardMarketSummary {
        active_pairs: market_counts.active_pairs,
        disabled_pairs: market_counts.disabled_pairs,
        external_pairs: market_counts.external_pairs,
        strategy_pairs: market_counts.strategy_pairs,
        feed_runtime_status,
        feed_needs_reload: saved_feed_config
            .as_ref()
            .is_some_and(|config| config.needs_reload),
        feed_symbols: runtime.symbols,
        feed_providers: runtime.providers,
    };
    let trading = sqlx::query_as::<_, AdminDashboardTradingSummary>(
        r#"SELECT (SELECT COUNT(*) FROM spot_orders WHERE status IN ('pending', 'partial')) AS spot_open_orders,
                  (SELECT COUNT(*) FROM spot_trades
                   WHERE created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS spot_trades_24h,
                  (SELECT COUNT(*) FROM convert_orders WHERE status = 'pending') AS convert_pending_orders,
                  (SELECT COUNT(*) FROM convert_orders
                   WHERE status = 'completed'
                     AND updated_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS convert_completed_24h"#,
    )
    .fetch_one(&pool)
    .await?;
    let products = sqlx::query_as::<_, AdminDashboardProductsSummary>(
        r#"SELECT (SELECT COUNT(*) FROM seconds_contract_orders WHERE status = 'opened') AS seconds_open_orders,
                  (SELECT COUNT(*) FROM margin_positions WHERE status = 'opened') AS margin_open_positions,
                  (SELECT COUNT(*) FROM margin_liquidation_records
                   WHERE liquidated_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS margin_liquidated_24h,
                  (SELECT COUNT(*) FROM earn_subscriptions WHERE status = 'subscribed') AS earn_active_subscriptions,
                  (SELECT COUNT(*) FROM earn_subscriptions
                   WHERE status = 'subscribed'
                     AND matures_at <= DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS earn_maturing_24h"#,
    )
    .fetch_one(&pool)
    .await?;
    let risk = sqlx::query_as::<_, AdminDashboardRiskSummary>(
        r#"SELECT (SELECT COUNT(*) FROM risk_events
                   WHERE created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS risk_events_24h,
                  (SELECT COUNT(*) FROM risk_events
                   WHERE decision IN ('block', 'blocked', 'reject', 'rejected')
                     AND created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS blocked_events_24h,
                  (SELECT COUNT(*) FROM event_outbox WHERE status = 'pending') AS pending_outbox_events,
                  (SELECT COUNT(*) FROM event_inbox WHERE status = 'retry') AS retry_inbox_events,
                  (SELECT COUNT(*) FROM event_inbox WHERE status = 'dead_letter') AS dead_letter_inbox_events"#,
    )
    .fetch_one(&pool)
    .await?;
    let admin_actions_24h = sqlx::query_as::<_, (i64,)>(
        r#"SELECT COUNT(*) FROM admin_audit_logs
           WHERE created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)"#,
    )
    .fetch_one(&pool)
    .await?
    .0;
    let latest_actions = sqlx::query_as::<_, AdminDashboardAuditAction>(
        r#"SELECT id, admin_id, action, target_type, target_id, created_at
           FROM admin_audit_logs
           ORDER BY created_at DESC, id DESC
           LIMIT 5"#,
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(AdminDashboardResponse {
        generated_at,
        users,
        wallet,
        market,
        trading,
        products,
        risk,
        audit: AdminDashboardAuditSummary {
            admin_actions_24h,
            latest_actions,
        },
    }))
}

async fn create_admin_user(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAdminUserRequest>,
) -> AppResult<Json<AdminUserResponse>> {
    validate_create_admin_user(&request)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let email = optional_string(request.email);
    let phone = optional_string(request.phone);
    let status = request
        .status
        .as_deref()
        .map(validate_user_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    let kyc_level = request.kyc_level.unwrap_or(0);
    let password_hash = hash_password(&request.password)?;
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        r#"INSERT INTO users (email, phone, password_hash, status, kyc_level)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(email.as_deref())
    .bind(phone.as_deref())
    .bind(password_hash)
    .bind(&status)
    .bind(kyc_level)
    .execute(&mut *tx)
    .await
    .map_err(map_duplicate_user)?;
    let user = load_admin_user_in_tx(&mut tx, result.last_insert_id()).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "user.create",
            target_type: "user",
            target_id: user.id,
            before_json: None,
            after_json: Some(user_audit_json(&user)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(user))
}

async fn list_admin_users(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminUserQuery>,
) -> AppResult<Json<AdminUsersResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, email, phone, status, kyc_level, created_at, updated_at
           FROM users
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = query.user_id {
        builder.push(" AND id = ");
        builder.push_bind(user_id);
    }
    if let Some(email) = optional_string(query.email) {
        builder.push(" AND email = ");
        builder.push_bind(email);
    }
    if let Some(status) = optional_string(query.status) {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let users = builder
        .build_query_as::<AdminUserResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminUsersResponse { users }))
}

async fn get_admin_user(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
) -> AppResult<Json<AdminUserResponse>> {
    let user = load_admin_user(&mysql_pool(&state)?, user_id).await?;
    Ok(Json(user))
}

async fn recharge_admin_user_wallet(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
    Json(request): Json<AdminUserRechargeRequest>,
) -> AppResult<Json<AdminUserRechargeResponse>> {
    validate_admin_user_recharge(&request)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let recharge_id = Uuid::now_v7().to_string();
    let mut tx = pool.begin().await?;
    ensure_user_exists_in_tx(&mut tx, user_id).await?;
    let asset = load_active_asset_symbol_in_tx(&mut tx, request.asset_id).await?;
    credit_admin_wallet_available(
        &mut tx,
        user_id,
        request.asset_id,
        &request.amount,
        "admin_recharge",
        "admin_recharge",
        &recharge_id,
    )
    .await?;
    let wallet = load_admin_wallet_row_in_tx(&mut tx, user_id, request.asset_id).await?;
    let response = AdminUserRechargeResponse {
        recharge_id,
        user_id,
        asset_id: request.asset_id,
        asset_symbol: asset.symbol,
        amount: request.amount,
        available: wallet.available,
        frozen: wallet.frozen,
        locked: wallet.locked,
    };
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "wallet.recharge",
            target_type: "wallet_account",
            target_id: user_id,
            before_json: None,
            after_json: Some(recharge_audit_json(&response)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(response))
}

async fn load_admin_user(pool: &Pool<MySql>, user_id: u64) -> AppResult<AdminUserResponse> {
    sqlx::query_as::<_, AdminUserResponse>(
        r#"SELECT id, email, phone, status, kyc_level, created_at, updated_at
           FROM users
           WHERE id = ?"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

async fn load_admin_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<AdminUserResponse> {
    sqlx::query_as::<_, AdminUserResponse>(
        r#"SELECT id, email, phone, status, kyc_level, created_at, updated_at
           FROM users
           WHERE id = ?"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn list_admin_news_items(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewsQuery>,
) -> AppResult<Json<AdminNewsItemsResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = admin_news_query();
    builder.push(" WHERE 1 = 1");
    if let Some(status) = optional_string(query.status) {
        let status = validate_news_status(&status)?;
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(category) = optional_string(query.category) {
        let category = validate_news_category(&category)?;
        builder.push(" AND category = ");
        builder.push_bind(category);
    }
    if let Some(country_code) = optional_string(query.country_code) {
        let country_code = normalize_news_country_code(&country_code)?;
        builder.push(" AND country_code = ");
        builder.push_bind(country_code);
    }
    if let Some(locale) = optional_string(query.locale) {
        let locale = validate_news_locale(&locale)?;
        builder.push(" AND JSON_SEARCH(content_json, 'one', ");
        builder.push_bind(locale);
        builder.push(", NULL, '$.items[*].locale') IS NOT NULL");
    }
    if let Some(keyword) = optional_string(query.q) {
        builder.push(" AND (title LIKE ");
        builder.push_bind(format!("%{keyword}%"));
        builder.push(" OR CAST(content_json AS CHAR) LIKE ");
        builder.push_bind(format!("%{keyword}%"));
        builder.push(")");
    }
    builder.push(" ORDER BY updated_at DESC, id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    builder.push(" OFFSET ");
    builder.push_bind(route_offset(query.offset) as i64);

    let news = builder
        .build_query_as::<AdminNewsItemResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminNewsItemsResponse { news }))
}

async fn get_admin_news_item(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    let news = load_admin_news_item(&mysql_pool(&state)?, news_id).await?;
    Ok(Json(news))
}

async fn create_admin_news_item(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAdminNewsItemRequest>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let title = validate_news_title(&request.title)?;
    let category = validate_news_category(&request.category)?;
    let status = request
        .status
        .as_deref()
        .map(validate_news_status)
        .transpose()?
        .unwrap_or_else(|| "draft".to_owned());
    let country_code = normalize_optional_news_country_code(request.country_code)?;
    let default_locale = validate_news_locale(&request.default_locale)?;
    let content_json = validate_news_content_document(request.content_json, &default_locale)?;
    let published_at = (status == "published").then(chrono::Utc::now);
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        r#"INSERT INTO admin_news_items
           (title, category, status, country_code, default_locale, content_json, published_at,
            created_by_admin_id, updated_by_admin_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&title)
    .bind(&category)
    .bind(&status)
    .bind(country_code.as_deref())
    .bind(&default_locale)
    .bind(SqlxJson(content_json))
    .bind(published_at)
    .bind(admin_id)
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;
    let news = load_admin_news_item_in_tx(&mut tx, result.last_insert_id()).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "admin_news_item.create",
            target_type: "admin_news_item",
            target_id: news.id,
            before_json: None,
            after_json: Some(admin_news_item_audit_json(&news)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(news))
}

async fn update_admin_news_item(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
    Json(request): Json<UpdateAdminNewsItemRequest>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let title = validate_news_title(&request.title)?;
    let category = validate_news_category(&request.category)?;
    let country_code = normalize_optional_news_country_code(request.country_code)?;
    let default_locale = validate_news_locale(&request.default_locale)?;
    let content_json = validate_news_content_document(request.content_json, &default_locale)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_news_item_in_tx(&mut tx, news_id).await?;
    sqlx::query(
        r#"UPDATE admin_news_items
           SET title = ?, category = ?, country_code = ?, default_locale = ?, content_json = ?, updated_by_admin_id = ?
           WHERE id = ?"#,
    )
    .bind(&title)
    .bind(&category)
    .bind(country_code.as_deref())
    .bind(&default_locale)
    .bind(SqlxJson(content_json))
    .bind(admin_id)
    .bind(news_id)
    .execute(&mut *tx)
    .await?;
    let after = load_admin_news_item_in_tx(&mut tx, news_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "admin_news_item.update",
            target_type: "admin_news_item",
            target_id: after.id,
            before_json: Some(admin_news_item_audit_json(&before)),
            after_json: Some(admin_news_item_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}

async fn update_admin_news_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
    Json(request): Json<UpdateAdminNewsStatusRequest>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let status = validate_news_status(&request.status)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_news_item_in_tx(&mut tx, news_id).await?;
    let published_at = if status == "published" && before.published_at.is_none() {
        Some(chrono::Utc::now())
    } else {
        before.published_at
    };
    sqlx::query(
        r#"UPDATE admin_news_items
           SET status = ?, published_at = ?, updated_by_admin_id = ?
           WHERE id = ?"#,
    )
    .bind(&status)
    .bind(published_at)
    .bind(admin_id)
    .bind(news_id)
    .execute(&mut *tx)
    .await?;
    let after = load_admin_news_item_in_tx(&mut tx, news_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "admin_news_item.status.update",
            target_type: "admin_news_item",
            target_id: after.id,
            before_json: Some(admin_news_item_audit_json(&before)),
            after_json: Some(admin_news_item_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}

async fn list_assets(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAssetQuery>,
) -> AppResult<Json<AdminAssetsResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = admin_asset_query();
    builder.push(" WHERE 1 = 1");
    if let Some(symbol) = optional_string(query.symbol) {
        builder.push(" AND symbol = ");
        builder.push_bind(normalize_asset_symbol(&symbol)?);
    }
    if let Some(asset_type) = optional_string(query.asset_type) {
        validate_asset_type(&asset_type)?;
        builder.push(" AND asset_type = ");
        builder.push_bind(asset_type);
    }
    if let Some(status) = optional_string(query.status) {
        validate_asset_status(&status)?;
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let assets = builder
        .build_query_as::<AdminAssetResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminAssetsResponse { assets }))
}

async fn get_asset(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
) -> AppResult<Json<AdminAssetResponse>> {
    let asset = load_asset(&mysql_pool(&state)?, asset_id).await?;
    Ok(Json(asset))
}

async fn update_asset(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
    Json(request): Json<UpdateAssetRequest>,
) -> AppResult<Json<AdminAssetResponse>> {
    validate_update_asset(&request)?;
    let asset_type = validate_asset_type(&request.asset_type)?;
    let status = validate_asset_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let name = optional_string(Some(request.name)).unwrap();
    let mut tx = pool.begin().await?;
    let before = lock_asset_in_tx(&mut tx, asset_id).await?;
    sqlx::query(
        r#"UPDATE assets
           SET name = ?, precision_scale = ?, asset_type = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(&name)
    .bind(request.precision_scale)
    .bind(&asset_type)
    .bind(&status)
    .bind(asset_id)
    .execute(&mut *tx)
    .await?;
    let after = load_asset_in_tx(&mut tx, asset_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "asset.config.update",
            target_type: "asset",
            target_id: after.id,
            before_json: Some(asset_audit_json(&before)),
            after_json: Some(asset_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}

async fn create_asset(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAssetRequest>,
) -> AppResult<Json<AdminAssetResponse>> {
    validate_create_asset(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let symbol = normalize_asset_symbol(&request.symbol)?;
    let name = optional_string(Some(request.name.clone())).unwrap();
    let asset_type = request
        .asset_type
        .as_deref()
        .map(validate_asset_type)
        .transpose()?
        .unwrap_or_else(|| "coin".to_owned());
    let status = request
        .status
        .as_deref()
        .map(validate_asset_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        r#"INSERT INTO assets (symbol, name, precision_scale, asset_type, status)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&symbol)
    .bind(&name)
    .bind(request.precision_scale)
    .bind(&asset_type)
    .bind(&status)
    .execute(&mut *tx)
    .await
    .map_err(map_duplicate_asset)?;
    let asset = load_asset_in_tx(&mut tx, result.last_insert_id()).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "asset.create",
            target_type: "asset",
            target_id: asset.id,
            before_json: None,
            after_json: Some(asset_audit_json(&asset)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(asset))
}

async fn list_wallet_accounts(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminWalletAccountQuery>,
) -> AppResult<Json<AdminWalletAccountsResponse>> {
    let pool = mysql_pool(&state)?;
    let include_empty = query.include_empty.unwrap_or(false);
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT accounts.id, accounts.user_id, accounts.asset_id, assets.symbol AS asset_symbol,
                  accounts.available, accounts.frozen, accounts.locked, TRUE AS account_exists, accounts.updated_at
           FROM wallet_accounts accounts
           INNER JOIN assets ON assets.id = accounts.asset_id
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = query.user_id {
        push_user_id_filter(&mut builder, "accounts.user_id", user_id);
    }
    push_user_email_filter(&mut builder, "accounts.user_id", query.email.clone());
    if let Some(asset_id) = query.asset_id {
        builder.push(" AND accounts.asset_id = ");
        builder.push_bind(asset_id);
    }
    builder.push(" ORDER BY accounts.updated_at DESC, accounts.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let mut accounts = builder
        .build_query_as::<AdminWalletAccountResponse>()
        .fetch_all(&pool)
        .await?;
    if include_empty {
        append_empty_wallet_accounts(&pool, &query, &mut accounts).await?;
    }
    Ok(Json(AdminWalletAccountsResponse { accounts }))
}

fn serialize_decimal_amount<S>(amount: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{amount:.18}"))
}

async fn append_empty_wallet_accounts(
    pool: &Pool<MySql>,
    query: &AdminWalletAccountQuery,
    accounts: &mut Vec<AdminWalletAccountResponse>,
) -> AppResult<()> {
    let Some(user_id) = resolve_user_id_filter(pool, query.user_id, query.email.clone()).await?
    else {
        return Ok(());
    };
    let existing_asset_ids = accounts
        .iter()
        .map(|account| account.asset_id)
        .collect::<BTreeSet<_>>();
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id AS asset_id, symbol AS asset_symbol
           FROM assets
           WHERE status = 'active'"#,
    );
    if let Some(asset_id) = query.asset_id {
        builder.push(" AND id = ");
        builder.push_bind(asset_id);
    }
    builder.push(" ORDER BY symbol ASC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let assets = builder
        .build_query_as::<AdminWalletEmptyAssetRow>()
        .fetch_all(pool)
        .await?;
    let zero = BigDecimal::from(0).with_scale(18);
    let now = chrono::Utc::now();
    accounts.extend(
        assets
            .into_iter()
            .filter(|asset| !existing_asset_ids.contains(&asset.asset_id))
            .map(|asset| AdminWalletAccountResponse {
                id: None,
                user_id,
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

async fn list_wallet_ledger(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminWalletLedgerQuery>,
) -> AppResult<Json<AdminWalletLedgerResponseList>> {
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT ledger.id, ledger.user_id, ledger.asset_id, assets.symbol AS asset_symbol,
                  ledger.change_type, ledger.amount, ledger.balance_type, ledger.balance_after,
                  ledger.available_after, ledger.frozen_after, ledger.locked_after,
                  ledger.ref_type, ledger.ref_id, ledger.created_at
           FROM wallet_ledger ledger
           INNER JOIN assets ON assets.id = ledger.asset_id
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = query.user_id {
        push_user_id_filter(&mut builder, "ledger.user_id", user_id);
    }
    push_user_email_filter(&mut builder, "ledger.user_id", query.email);
    if let Some(asset_id) = query.asset_id {
        builder.push(" AND ledger.asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(change_type) = optional_string(query.change_type) {
        builder.push(" AND ledger.change_type = ");
        builder.push_bind(change_type);
    }
    if let Some(ref_type) = optional_string(query.ref_type) {
        builder.push(" AND ledger.ref_type = ");
        builder.push_bind(ref_type);
    }
    builder.push(" ORDER BY ledger.created_at DESC, ledger.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let ledger = builder
        .build_query_as::<AdminWalletLedgerResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminWalletLedgerResponseList { ledger }))
}

async fn list_risk_rules(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminRiskRuleQuery>,
) -> AppResult<Json<RiskRulesResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, rule_type, target_type, target_id, config_json, enabled,
                  created_by, created_at, updated_at
           FROM risk_rules
           WHERE 1 = 1"#,
    );
    if let Some(rule_type) = optional_string(query.rule_type) {
        builder.push(" AND rule_type = ");
        builder.push_bind(rule_type);
    }
    if let Some(target_type) = optional_string(query.target_type) {
        builder.push(" AND target_type = ");
        builder.push_bind(target_type);
    }
    if let Some(enabled) = query.enabled {
        builder.push(" AND enabled = ");
        builder.push_bind(enabled);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let rules = builder
        .build_query_as::<RiskRuleResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(RiskRulesResponse { rules }))
}

async fn create_risk_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateRiskRuleRequest>,
) -> AppResult<Json<RiskRuleResponse>> {
    validate_create_risk_rule(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let rule_type = optional_string(Some(request.rule_type.clone())).unwrap();
    let target_type = optional_string(Some(request.target_type.clone())).unwrap();
    let target_id = optional_string(request.target_id.clone());
    let result = sqlx::query(
        r#"INSERT INTO risk_rules (rule_type, target_type, target_id, config_json, enabled, created_by)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&rule_type)
    .bind(&target_type)
    .bind(&target_id)
    .bind(SqlxJson(request.config_json))
    .bind(request.enabled.unwrap_or(true))
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;
    let rule_id = result.last_insert_id();
    let rule = load_risk_rule_in_tx(&mut tx, rule_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "risk_rule.create",
            target_type: "risk_rule",
            target_id: rule_id,
            before_json: None,
            after_json: Some(risk_rule_audit_json(&rule)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(rule))
}

async fn update_risk_rule_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(rule_id): Path<u64>,
    Json(request): Json<UpdateRiskRuleStatusRequest>,
) -> AppResult<Json<RiskRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_risk_rule_in_tx(&mut tx, rule_id).await?;
    sqlx::query("UPDATE risk_rules SET enabled = ? WHERE id = ?")
        .bind(request.enabled)
        .bind(rule_id)
        .execute(&mut *tx)
        .await?;
    let after = load_risk_rule_in_tx(&mut tx, rule_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "risk_rule.status.update",
            target_type: "risk_rule",
            target_id: rule_id,
            before_json: Some(risk_rule_audit_json(&before)),
            after_json: Some(risk_rule_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}

async fn list_risk_events(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminRiskEventQuery>,
) -> AppResult<Json<RiskEventsResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, actor_type, actor_id, event_type, risk_level,
                  decision, reason, payload_json, created_at
           FROM risk_events
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = query.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", query.email);
    if let Some(decision) = optional_string(query.decision) {
        builder.push(" AND decision = ");
        builder.push_bind(decision);
    }
    if let Some(risk_level) = optional_string(query.risk_level) {
        builder.push(" AND risk_level = ");
        builder.push_bind(risk_level);
    }
    builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let events = builder
        .build_query_as::<RiskEventResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(RiskEventsResponse { events }))
}

async fn list_trading_pairs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminTradingPairQuery>,
) -> AppResult<Json<AdminTradingPairsResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = admin_trading_pair_query();
    builder.push(" WHERE 1 = 1");
    if let Some(symbol) = optional_string(query.symbol) {
        builder.push(" AND pairs.symbol = ");
        builder.push_bind(normalize_trading_pair_symbol(&symbol)?);
    }
    if let Some(status) = optional_string(query.status) {
        validate_trading_pair_status(&status)?;
        builder.push(" AND pairs.status = ");
        builder.push_bind(status);
    }
    if let Some(market_type) = optional_string(query.market_type) {
        validate_trading_pair_market_type(&market_type)?;
        builder.push(" AND pairs.market_type = ");
        builder.push_bind(market_type);
    }
    builder.push(" ORDER BY pairs.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let pairs = builder
        .build_query_as::<AdminTradingPairResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminTradingPairsResponse { pairs }))
}

async fn get_trading_pair(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    let pair = load_trading_pair_by_id(&mysql_pool(&state)?, pair_id).await?;
    Ok(Json(pair))
}

async fn update_trading_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<UpdateTradingPairRequest>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    validate_update_trading_pair_request(&request)?;
    let market_type = validate_trading_pair_market_type(&request.market_type)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_trading_pair_in_tx(&mut tx, pair_id).await?;
    sqlx::query(
        r#"UPDATE trading_pairs
           SET price_precision = ?, qty_precision = ?, min_order_value = ?, market_type = ?
           WHERE id = ?"#,
    )
    .bind(request.price_precision)
    .bind(request.qty_precision)
    .bind(&request.min_order_value)
    .bind(&market_type)
    .bind(pair_id)
    .execute(&mut *tx)
    .await?;
    let after = load_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "trading_pair.config.update",
            target_type: "trading_pair",
            target_id: after.id,
            before_json: Some(trading_pair_audit_json(&before)),
            after_json: Some(trading_pair_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}

async fn update_trading_pair_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<UpdateTradingPairStatusRequest>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    let status = validate_trading_pair_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_trading_pair_in_tx(&mut tx, pair_id).await?;
    sqlx::query("UPDATE trading_pairs SET status = ? WHERE id = ?")
        .bind(&status)
        .bind(pair_id)
        .execute(&mut *tx)
        .await?;
    let after = load_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "trading_pair.status.update",
            target_type: "trading_pair",
            target_id: after.id,
            before_json: Some(trading_pair_audit_json(&before)),
            after_json: Some(trading_pair_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}

async fn create_trading_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateTradingPairRequest>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    validate_create_trading_pair(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let symbol = normalize_trading_pair_symbol(&request.symbol)?;
    let status = request
        .status
        .as_deref()
        .map(validate_trading_pair_status)
        .transpose()?
        .unwrap_or_else(|| "disabled".to_owned());
    let market_type = request
        .market_type
        .as_deref()
        .map(validate_trading_pair_market_type)
        .transpose()?
        .unwrap_or_else(|| "external".to_owned());
    let mut tx = pool.begin().await?;
    ensure_trading_pair_asset_in_tx(&mut tx, request.base_asset_id).await?;
    ensure_trading_pair_asset_in_tx(&mut tx, request.quote_asset_id).await?;
    let result = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(request.base_asset_id)
    .bind(request.quote_asset_id)
    .bind(&symbol)
    .bind(request.price_precision)
    .bind(request.qty_precision)
    .bind(&request.min_order_value)
    .bind(&status)
    .bind(&market_type)
    .execute(&mut *tx)
    .await
    .map_err(map_duplicate_trading_pair)?;
    let pair = load_trading_pair_in_tx(&mut tx, result.last_insert_id()).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "trading_pair.create",
            target_type: "trading_pair",
            target_id: pair.id,
            before_json: None,
            after_json: Some(trading_pair_audit_json(&pair)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(pair))
}

async fn list_new_coin_projects(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinProjectsResponse>> {
    let projects = sqlx::query_as::<_, NewCoinProjectResponse>(
        r#"SELECT projects.id, projects.asset_id, projects.symbol, projects.lifecycle_status,
                  projects.total_supply, projects.issue_price, projects.listed_at,
                  projects.unlock_type, projects.fixed_unlock_at, projects.relative_unlock_seconds,
                  projects.unlock_fee_enabled, projects.unlock_fee_rate, projects.unlock_fee_basis,
                  projects.unlock_fee_asset, projects.status, projects.post_listing_purchase_enabled,
                  projects.post_listing_pair_id, post_listing_pair.status AS post_listing_pair_status
           FROM new_coin_projects projects
           LEFT JOIN trading_pairs post_listing_pair ON post_listing_pair.id = projects.post_listing_pair_id
           ORDER BY projects.id DESC
           LIMIT ?"#,
    )
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;

    Ok(Json(NewCoinProjectsResponse { projects }))
}

async fn list_convert_pairs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<ConvertPairsResponse>> {
    let pairs = sqlx::query_as::<_, ConvertPairResponse>(
        r#"SELECT id, from_asset AS from_asset_id, to_asset AS to_asset_id, pricing_mode,
                  spread_rate, min_amount, max_amount, enabled
           FROM convert_pairs
           ORDER BY id DESC
           LIMIT ?"#,
    )
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;

    Ok(Json(ConvertPairsResponse { pairs }))
}

async fn get_convert_pair(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
) -> AppResult<Json<ConvertPairResponse>> {
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let pair = load_convert_pair_in_tx(&mut tx, pair_id).await?;
    tx.commit().await?;
    Ok(Json(pair))
}

async fn list_convert_orders(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<ConvertOrdersQuery>,
) -> AppResult<Json<ConvertOrdersResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, quote_id, convert_pair_id, user_id, from_asset AS from_asset_id,
                  to_asset AS to_asset_id, from_amount, to_amount, rate, status, created_at
           FROM convert_orders
           WHERE 1 = 1"#,
    );

    if let Some(user_id) = query.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", query.email);
    if let Some(status) = optional_string(query.status) {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }

    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let orders = builder
        .build_query_as::<ConvertOrderResponse>()
        .fetch_all(&pool)
        .await?;

    Ok(Json(ConvertOrdersResponse { orders }))
}

async fn get_convert_order(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<ConvertOrderResponse>> {
    let order = sqlx::query_as::<_, ConvertOrderResponse>(
        r#"SELECT id, quote_id, convert_pair_id, user_id, from_asset AS from_asset_id,
                  to_asset AS to_asset_id, from_amount, to_amount, rate, status, created_at
           FROM convert_orders
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(order_id)
    .fetch_optional(&mysql_pool(&state)?)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(order))
}

async fn list_market_strategies(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminMarketStrategyQuery>,
) -> AppResult<Json<AdminMarketStrategiesResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = base_market_strategy_query();
    builder.push(" WHERE 1 = 1");
    if let Some(pair_id) = query.pair_id {
        builder.push(" AND strategies.pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(status) = optional_string(query.status) {
        builder.push(" AND strategies.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY strategies.created_at DESC, strategies.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let strategies = builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminMarketStrategiesResponse { strategies }))
}

async fn list_admin_audit_logs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAuditLogsQuery>,
) -> AppResult<Json<AdminAuditLogsResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, admin_id, action, target_type, target_id,
                  before_json, after_json, reason, ip, created_at
           FROM admin_audit_logs
           WHERE 1 = 1"#,
    );
    if let Some(admin_id) = query.admin_id {
        builder.push(" AND admin_id = ");
        builder.push_bind(admin_id);
    }
    if let Some(action) = optional_string(query.action) {
        builder.push(" AND action = ");
        builder.push_bind(action);
    }
    if let Some(target_type) = optional_string(query.target_type) {
        builder.push(" AND target_type = ");
        builder.push_bind(target_type);
    }
    if let Some(target_id) = optional_string(query.target_id) {
        builder.push(" AND target_id = ");
        builder.push_bind(target_id);
    }
    builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let logs = builder
        .build_query_as::<AdminAuditLogResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminAuditLogsResponse { logs }))
}

async fn create_market_strategy(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateMarketStrategyRequest>,
) -> AppResult<Json<AdminMarketStrategyResponse>> {
    validate_create_market_strategy(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let market_type = ensure_strategy_pair_in_tx(&mut tx, request.pair_id).await?;
    let status = request
        .status
        .as_deref()
        .map(validate_market_strategy_status)
        .transpose()?
        .unwrap_or_else(|| "draft".to_owned());
    let strategy_type = optional_string(Some(request.strategy_type.clone())).unwrap();
    let result = sqlx::query(
        r#"INSERT INTO market_strategies
           (pair_id, strategy_type, start_price, target_price, start_time, end_time,
            volatility, volume_min, volume_max, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(request.pair_id)
    .bind(&strategy_type)
    .bind(&request.start_price)
    .bind(&request.target_price)
    .bind(request.start_time)
    .bind(request.end_time)
    .bind(&request.volatility)
    .bind(&request.volume_min)
    .bind(&request.volume_max)
    .bind(&status)
    .execute(&mut *tx)
    .await?;
    let strategy_id = result.last_insert_id();
    let run_status = market_strategy_run_status(&status);
    sqlx::query(
        r#"INSERT INTO strategy_runs
           (strategy_id, run_status, current_price, last_generated_at, last_kline_open_time, recovery_status)
           VALUES (?, ?, ?, ?, ?, 'idle')"#,
    )
    .bind(strategy_id)
    .bind(run_status)
    .bind(&request.start_price)
    .bind(request.start_time)
    .bind(request.start_time)
    .execute(&mut *tx)
    .await?;
    let config_json = market_strategy_config_json(&request, &status, &market_type);
    sqlx::query(
        r#"INSERT INTO strategy_versions (strategy_id, version, effective_time, config_json, seed, created_by)
           VALUES (?, 1, ?, ?, ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(request.start_time)
    .bind(SqlxJson(config_json))
    .bind(Uuid::now_v7().to_string())
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;
    let strategy = load_market_strategy_in_tx(&mut tx, strategy_id).await?;
    persist_market_strategy_change(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.create",
        None,
        Some(&strategy),
        request.reason,
    )
    .await?;
    tx.commit().await?;

    Ok(Json(strategy))
}

async fn update_market_strategy(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Json(request): Json<UpdateMarketStrategyRequest>,
) -> AppResult<Json<AdminMarketStrategyResponse>> {
    validate_update_market_strategy(&request)?;
    let reason = required_reason(request.reason.clone())?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_market_strategy_in_tx(&mut tx, strategy_id).await?;
    if before.status == "active" {
        return Err(AppError::Conflict(
            "active market strategy must be paused or disabled before update".to_owned(),
        ));
    }
    let strategy_type = optional_string(Some(request.strategy_type.clone())).unwrap();
    sqlx::query(
        r#"UPDATE market_strategies
           SET strategy_type = ?, start_price = ?, target_price = ?, start_time = ?, end_time = ?,
               volatility = ?, volume_min = ?, volume_max = ?
           WHERE id = ?"#,
    )
    .bind(&strategy_type)
    .bind(&request.start_price)
    .bind(&request.target_price)
    .bind(request.start_time)
    .bind(request.end_time)
    .bind(&request.volatility)
    .bind(&request.volume_min)
    .bind(&request.volume_max)
    .bind(strategy_id)
    .execute(&mut *tx)
    .await?;
    let run_update = sqlx::query(
        r#"UPDATE strategy_runs
           SET run_status = ?, current_price = ?, last_generated_at = NULL,
               last_kline_open_time = ?, recovery_status = 'idle', error_message = NULL
           WHERE strategy_id = ?"#,
    )
    .bind(market_strategy_run_status(&before.status))
    .bind(&request.start_price)
    .bind(request.start_time)
    .bind(strategy_id)
    .execute(&mut *tx)
    .await?;
    if run_update.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "market strategy run checkpoint is missing".to_owned(),
        ));
    }
    let next_version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM strategy_versions WHERE strategy_id = ?",
    )
    .bind(strategy_id)
    .fetch_one(&mut *tx)
    .await?;
    let after = load_market_strategy_in_tx(&mut tx, strategy_id).await?;
    let config_json =
        market_strategy_update_config_json(&request, &after.status, &after.market_type);
    sqlx::query(
        r#"INSERT INTO strategy_versions (strategy_id, version, effective_time, config_json, seed, created_by)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(next_version)
    .bind(request.start_time)
    .bind(SqlxJson(config_json))
    .bind(Uuid::now_v7().to_string())
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;
    persist_market_strategy_change(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.update",
        Some(&before),
        Some(&after),
        Some(reason),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn update_market_strategy_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Json(request): Json<UpdateMarketStrategyStatusRequest>,
) -> AppResult<Json<AdminMarketStrategyResponse>> {
    let status = validate_market_strategy_status(&request.status)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_market_strategy_in_tx(&mut tx, strategy_id).await?;
    sqlx::query("UPDATE market_strategies SET status = ? WHERE id = ?")
        .bind(&status)
        .bind(strategy_id)
        .execute(&mut *tx)
        .await?;
    let run_update = sqlx::query(
        "UPDATE strategy_runs SET run_status = ?, recovery_status = 'idle', error_message = NULL WHERE strategy_id = ?",
    )
    .bind(market_strategy_run_status(&status))
    .bind(strategy_id)
    .execute(&mut *tx)
    .await?;
    if run_update.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "market strategy run checkpoint is missing".to_owned(),
        ));
    }
    let after = load_market_strategy_in_tx(&mut tx, strategy_id).await?;
    persist_market_strategy_change(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.status.update",
        Some(&before),
        Some(&after),
        request.reason,
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn list_margin_liquidations(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminMarginLiquidationQuery>,
) -> AppResult<Json<AdminMarginLiquidationsResponse>> {
    // 后台强平记录列表只读查询，支持按用户、交易对和仓位精确过滤，便于风控复盘。
    let pool = mysql_pool(&state)?;
    let mut builder = margin_liquidation_query();
    builder.push(" WHERE 1 = 1");
    if let Some(user_id) = query.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", query.email);
    if let Some(pair_id) = query.pair_id {
        builder.push(" AND pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(position_id) = query.position_id {
        builder.push(" AND position_id = ");
        builder.push_bind(position_id);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let liquidations = builder
        .build_query_as::<AdminMarginLiquidationResponse>()
        .fetch_all(&pool)
        .await?;

    Ok(Json(AdminMarginLiquidationsResponse { liquidations }))
}

async fn get_margin_liquidation(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(liquidation_id): Path<u64>,
) -> AppResult<Json<AdminMarginLiquidationResponse>> {
    let mut builder = margin_liquidation_query();
    builder.push(" WHERE id = ");
    builder.push_bind(liquidation_id);
    builder.push(" LIMIT 1");
    let liquidation = builder
        .build_query_as::<AdminMarginLiquidationResponse>()
        .fetch_optional(&mysql_pool(&state)?)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(liquidation))
}

fn margin_liquidation_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, position_id, user_id, product_id, pair_id, margin_asset, direction,
                  margin_amount, notional_amount, interest_amount, entry_price, mark_price,
                  maintenance_margin_rate, equity, maintenance_margin, realized_pnl,
                  payout_amount, reason, liquidated_at, created_at
           FROM margin_liquidation_records"#,
    )
}

async fn list_new_coin_subscriptions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Query(query): Query<AdminScopedListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    let query = AdminNewCoinFlatListQuery {
        project_id: Some(project_id),
        user_id: query.user_id,
        status: query.status,
        email: query.email,
        limit: query.limit,
    };
    list_new_coin_subscriptions_by_query(&state, query).await
}

async fn list_all_new_coin_subscriptions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinFlatListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    list_new_coin_subscriptions_by_query(&state, query).await
}

async fn list_new_coin_distributions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Query(query): Query<AdminScopedListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    let query = AdminNewCoinFlatListQuery {
        project_id: Some(project_id),
        user_id: query.user_id,
        status: query.status,
        email: query.email,
        limit: query.limit,
    };
    list_new_coin_distributions_by_query(&state, query).await
}

async fn list_all_new_coin_distributions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinFlatListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    list_new_coin_distributions_by_query(&state, query).await
}

async fn list_new_coin_subscriptions_by_query(
    state: &AppState,
    query: AdminNewCoinFlatListQuery,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    let pool = mysql_pool(state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, quote_asset, quote_amount, requested_quantity,
                  allocated_quantity, status, idempotency_key, created_at
           FROM new_coin_subscriptions
           WHERE 1 = 1"#,
    );
    if let Some(project_id) = query.project_id {
        builder.push(" AND project_id = ");
        builder.push_bind(project_id);
    }
    push_optional_user_and_status_filters(&mut builder, query.user_id, query.email, query.status);
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let subscriptions = builder
        .build_query_as::<NewCoinSubscriptionResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(NewCoinSubscriptionsResponse { subscriptions }))
}

async fn list_new_coin_distributions_by_query(
    state: &AppState,
    query: AdminNewCoinFlatListQuery,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    let pool = mysql_pool(state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, subscription_id, asset_id, quantity,
                  lock_position_id, status, idempotency_key, created_at
           FROM new_coin_distributions
           WHERE 1 = 1"#,
    );
    if let Some(project_id) = query.project_id {
        builder.push(" AND project_id = ");
        builder.push_bind(project_id);
    }
    push_optional_user_and_status_filters(&mut builder, query.user_id, query.email, query.status);
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let distributions = builder
        .build_query_as::<NewCoinDistributionResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(NewCoinDistributionsResponse { distributions }))
}

async fn list_new_coin_purchases(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinPurchaseQuery>,
) -> AppResult<Json<NewCoinPurchasesResponse>> {
    // 后台认购订单列表支持项目、用户和状态过滤，所有条件均使用 bind 参数。
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                  quote_amount, lock_position_id, status, idempotency_key, created_at
           FROM new_coin_purchase_orders
           WHERE 1 = 1"#,
    );
    if let Some(project_id) = query.project_id {
        builder.push(" AND project_id = ");
        builder.push_bind(project_id);
    }
    push_optional_user_and_status_filters(&mut builder, query.user_id, query.email, query.status);
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let purchases = builder
        .build_query_as::<NewCoinPurchaseResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(NewCoinPurchasesResponse { purchases }))
}

async fn list_new_coin_lock_positions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinLockPositionQuery>,
) -> AppResult<Json<NewCoinLockPositionsResponse>> {
    // 后台锁仓列表按用户、资产和状态过滤，用于核对 locked 汇总与明细。
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, asset_id, unlock_type, unlock_at, locked_amount,
                  released_amount, remaining_amount, merge_key, status, created_at
           FROM asset_lock_positions
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = query.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", query.email);
    if let Some(asset_id) = query.asset_id {
        builder.push(" AND asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(status) = optional_string(query.status) {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let lock_positions = builder
        .build_query_as::<NewCoinLockPositionResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(NewCoinLockPositionsResponse { lock_positions }))
}

async fn list_new_coin_unlocks(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinUnlockQuery>,
) -> AppResult<Json<NewCoinUnlocksResponse>> {
    // 后台解禁列表按用户、资产、解禁状态和矿工费支付状态过滤。
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
                  unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                  unlock_fee_amount, fee_paid_status, status, idempotency_key, created_at
           FROM asset_unlock_records
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = query.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", query.email);
    if let Some(asset_id) = query.asset_id {
        builder.push(" AND asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(status) = optional_string(query.status) {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(fee_paid_status) = optional_string(query.fee_paid_status) {
        builder.push(" AND fee_paid_status = ");
        builder.push_bind(fee_paid_status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let unlocks = builder
        .build_query_as::<NewCoinUnlockResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(NewCoinUnlocksResponse { unlocks }))
}

async fn create_new_coin_project(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateNewCoinProjectRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    validate_create_new_coin_project(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let unlock_fee_enabled = request.unlock_fee_enabled.unwrap_or(false);
    let result = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
            unlock_type, fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
            unlock_fee_rate, unlock_fee_basis, unlock_fee_asset, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active')"#,
    )
    .bind(request.asset_id)
    .bind(request.symbol.trim())
    .bind(request.lifecycle_status.trim())
    .bind(&request.total_supply)
    .bind(&request.issue_price)
    .bind(request.listed_at)
    .bind(request.unlock_type.trim())
    .bind(request.fixed_unlock_at)
    .bind(request.relative_unlock_seconds)
    .bind(unlock_fee_enabled)
    .bind(&request.unlock_fee_rate)
    .bind(request.unlock_fee_basis.as_deref().map(str::trim))
    .bind(request.unlock_fee_asset)
    .execute(&mut *tx)
    .await?;
    let project = load_new_coin_project_in_tx(&mut tx, result.last_insert_id()).await?;
    let event_payload = new_coin_project_audit_json(&project);
    sqlx::query(
        r#"INSERT INTO new_coin_lifecycle_events (project_id, event_type, payload_json, created_by)
           VALUES (?, 'new_coin_project.create', ?, ?)"#,
    )
    .bind(project.id)
    .bind(SqlxJson(event_payload.clone()))
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "new_coin_project.create",
            target_type: "new_coin_project",
            target_id: project.id,
            before_json: None,
            after_json: Some(event_payload),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(project))
}

async fn update_new_coin_lifecycle(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinLifecycleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let target_status = parse_lifecycle_status_from_request(&request.lifecycle_status)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_new_coin_project_in_tx(&mut tx, project_id).await?;
    let current_status = parse_lifecycle_status_from_db(&before.lifecycle_status)?;
    current_status
        .transition_to(target_status)
        .map_err(|_| AppError::Validation("invalid new coin lifecycle transition".to_owned()))?;

    let listed_at = if target_status == LifecycleStatus::Listed {
        Some(request.listed_at.unwrap_or_else(chrono::Utc::now))
    } else {
        before.listed_at
    };

    sqlx::query("UPDATE new_coin_projects SET lifecycle_status = ?, listed_at = ? WHERE id = ?")
        .bind(lifecycle_status_value(target_status))
        .bind(listed_at)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
    let after = load_new_coin_project_in_tx(&mut tx, project_id).await?;
    let before_json = new_coin_project_audit_json(&before);
    let after_json = new_coin_project_audit_json(&after);
    let event_payload = json!({
        "before": before_json,
        "after": after_json,
    });

    sqlx::query(
        r#"INSERT INTO new_coin_lifecycle_events (project_id, event_type, payload_json, created_by)
           VALUES (?, 'new_coin_project.lifecycle.update', ?, ?)"#,
    )
    .bind(project_id)
    .bind(SqlxJson(event_payload))
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "new_coin_project.lifecycle.update",
            target_type: "new_coin_project",
            target_id: project_id,
            before_json: Some(before_json),
            after_json: Some(after_json),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn update_new_coin_unlock_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinUnlockRuleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    // 先校验解禁规则形态，避免互斥字段或缺失字段进入事务。
    validate_update_new_coin_unlock_rule(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    // 锁定项目后再更新规则，避免后台并发修改导致审计 before/after 失真。
    let before = lock_new_coin_project_in_tx(&mut tx, project_id).await?;
    let listed_at = if request.unlock_type.trim() == "immediate_on_listing" {
        request.listed_at
    } else {
        before.listed_at
    };
    sqlx::query(
        r#"UPDATE new_coin_projects
           SET unlock_type = ?, listed_at = ?, fixed_unlock_at = ?, relative_unlock_seconds = ?
           WHERE id = ?"#,
    )
    .bind(request.unlock_type.trim())
    .bind(listed_at)
    .bind(request.fixed_unlock_at)
    .bind(request.relative_unlock_seconds)
    .bind(project_id)
    .execute(&mut *tx)
    .await?;
    let after = load_new_coin_project_in_tx(&mut tx, project_id).await?;
    persist_new_coin_project_rule_change(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.unlock_rule.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn update_new_coin_unlock_fee_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinUnlockFeeRuleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    // 先校验矿工费开关与费率/计费依据，确保关闭时清空费用字段。
    validate_update_new_coin_unlock_fee_rule(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    // 锁定项目后写入矿工费规则，并把规则变更与事件/审计放在同一事务。
    let before = lock_new_coin_project_in_tx(&mut tx, project_id).await?;
    sqlx::query(
        r#"UPDATE new_coin_projects
           SET unlock_fee_enabled = ?, unlock_fee_rate = ?, unlock_fee_basis = ?, unlock_fee_asset = ?
           WHERE id = ?"#,
    )
    .bind(request.unlock_fee_enabled)
    .bind(if request.unlock_fee_enabled {
        request.unlock_fee_rate.as_ref()
    } else {
        None
    })
    .bind(if request.unlock_fee_enabled {
        request.unlock_fee_basis.as_deref().map(str::trim)
    } else {
        None
    })
    .bind(if request.unlock_fee_enabled {
        request.unlock_fee_asset
    } else {
        None
    })
    .bind(project_id)
    .execute(&mut *tx)
    .await?;
    let after = load_new_coin_project_in_tx(&mut tx, project_id).await?;
    persist_new_coin_project_rule_change(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.unlock_fee_rule.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn update_new_coin_post_listing_purchase(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinPostListingPurchaseRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    // 先校验后台认购开关请求，避免启用时缺少交易对进入事务。
    validate_update_new_coin_post_listing_purchase(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    // 锁定新币项目和目标交易对，确保认购开关、交易对启用和审计一致提交。
    let before = lock_new_coin_project_in_tx(&mut tx, project_id).await?;
    ensure_post_listing_purchase_lifecycle(&before)?;
    if request.enabled {
        let pair_id = request.pair_id.ok_or_else(|| {
            AppError::Validation(
                "pair_id is required when post-listing purchase is enabled".to_owned(),
            )
        })?;
        ensure_post_listing_pair_in_tx(&mut tx, pair_id, before.asset_id).await?;
        sqlx::query("UPDATE trading_pairs SET status = 'active' WHERE id = ?")
            .bind(pair_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE new_coin_projects SET post_listing_purchase_enabled = TRUE, post_listing_pair_id = ? WHERE id = ?",
        )
        .bind(pair_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            "UPDATE new_coin_projects SET post_listing_purchase_enabled = FALSE, post_listing_pair_id = NULL WHERE id = ?",
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
    }
    let after = load_new_coin_project_in_tx(&mut tx, project_id).await?;
    persist_new_coin_project_rule_change(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.post_listing_purchase.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn distribute_new_coin(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<DistributeNewCoinRequest>,
) -> AppResult<Json<NewCoinDistributionResponse>> {
    // 先完成请求级校验，避免无效派发参数进入数据库事务。
    validate_distribute_new_coin(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let idempotency_key = request.idempotency_key.trim().to_owned();
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;

    // 锁定项目行，确保生命周期和解禁规则在派发期间不被并发修改。
    let project = lock_new_coin_project_in_tx(&mut tx, project_id).await?;
    ensure_distribution_lifecycle(&project)?;
    if idempotency_key_exists_in_tx(&mut tx, "new_coin_distributions", &idempotency_key).await? {
        return Err(AppError::Conflict(
            "new coin distribution has already been created".to_owned(),
        ));
    }

    // 如派发来源于申购单，先锁定申购单并累计已派发数量。
    if let Some(subscription_id) = request.subscription_id {
        apply_subscription_distribution_in_tx(
            &mut tx,
            subscription_id,
            project_id,
            request.user_id,
            &request.quantity,
        )
        .await?;
    }

    let source_time = chrono::Utc::now();
    let lock_positions = lock_positions_for_distribution(
        &project,
        request.user_id,
        project.asset_id,
        &idempotency_key,
        request.quantity.clone(),
        source_time,
    )?;

    // 根据解禁规则入账：立即解禁进入可用余额，否则进入锁定余额和锁仓明细。
    let lock_position_id = apply_new_coin_distribution_allocation(
        &mut tx,
        request.user_id,
        project.asset_id,
        &request.quantity,
        &lock_positions,
        AdminNewCoinLedgerMetadata {
            change_type: "new_coin_distribution_lock",
            ref_type: "new_coin_distribution",
            ref_id: &idempotency_key,
        },
    )
    .await?;
    let status = if lock_position_id.is_some() {
        "locked"
    } else {
        "completed"
    };

    // 写入派发记录，记录锁仓位置和幂等键，作为后台派发的业务凭证。
    let distribution_id = sqlx::query(
        r#"INSERT INTO new_coin_distributions
           (project_id, user_id, subscription_id, asset_id, quantity, lock_position_id,
            status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(project_id)
    .bind(request.user_id)
    .bind(request.subscription_id)
    .bind(project.asset_id)
    .bind(&request.quantity)
    .bind(lock_position_id)
    .bind(status)
    .bind(&idempotency_key)
    .execute(&mut *tx)
    .await
    .map_err(map_duplicate_distribution)?
    .last_insert_id();
    let distribution = load_new_coin_distribution_in_tx(&mut tx, distribution_id).await?;
    let distribution_json = new_coin_distribution_audit_json(&distribution);

    // 同事务写生命周期事件和后台审计，确保业务变更与追踪记录一致提交。
    sqlx::query(
        r#"INSERT INTO new_coin_lifecycle_events (project_id, event_type, payload_json, created_by)
           VALUES (?, 'new_coin_distribution.create', ?, ?)"#,
    )
    .bind(project_id)
    .bind(SqlxJson(
        json!({ "distribution": distribution_json.clone() }),
    ))
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: "new_coin_distribution.create",
            target_type: "new_coin_distribution",
            target_id: distribution.id,
            before_json: None,
            after_json: Some(distribution_json),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(distribution))
}

async fn upsert_new_coin_convert_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<UpsertNewCoinConvertRuleRequest>,
) -> AppResult<Json<NewCoinConvertRuleResponse>> {
    validate_new_coin_convert_rule(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let status = optional_string(request.status.clone()).unwrap_or_else(|| "active".to_owned());
    let mut tx = pool.begin().await?;
    let before = lock_new_coin_convert_rule_in_tx(&mut tx, request.convert_pair_id).await?;

    let rule_id = if let Some(before) = before.as_ref() {
        sqlx::query(
            r#"UPDATE new_coin_convert_rules
               SET rate_source = ?, fixed_rate = ?, floating_rate_json = ?, status = ?, created_by = ?
               WHERE id = ?"#,
        )
        .bind(request.rate_source.trim())
        .bind(&request.fixed_rate)
        .bind(request.floating_rate_json.clone().map(SqlxJson))
        .bind(&status)
        .bind(admin_id)
        .bind(before.id)
        .execute(&mut *tx)
        .await?;
        before.id
    } else {
        sqlx::query(
            r#"INSERT INTO new_coin_convert_rules
               (convert_pair_id, rate_source, fixed_rate, floating_rate_json, status, created_by)
               VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(request.convert_pair_id)
        .bind(request.rate_source.trim())
        .bind(&request.fixed_rate)
        .bind(request.floating_rate_json.clone().map(SqlxJson))
        .bind(&status)
        .bind(admin_id)
        .execute(&mut *tx)
        .await?
        .last_insert_id()
    };

    let after = load_new_coin_convert_rule_in_tx(&mut tx, rule_id).await?;
    insert_typed_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        AdminAuditEntry {
            action: if before.is_some() {
                "new_coin_convert_rule.update"
            } else {
                "new_coin_convert_rule.create"
            },
            target_type: "new_coin_convert_rule",
            target_id: after.id,
            before_json: before.as_ref().map(new_coin_convert_rule_audit_json),
            after_json: Some(new_coin_convert_rule_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn create_convert_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateConvertPairRequest>,
) -> AppResult<Json<ConvertPairResponse>> {
    validate_create_convert_pair(&request)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let enabled = request.enabled.unwrap_or(true);
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, min_amount, max_amount, enabled)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(request.from_asset_id)
    .bind(request.to_asset_id)
    .bind(request.pricing_mode.trim())
    .bind(&request.spread_rate)
    .bind(&request.min_amount)
    .bind(&request.max_amount)
    .bind(enabled)
    .execute(&mut *tx)
    .await
    .map_err(map_duplicate_pair)?;
    let pair = load_convert_pair_in_tx(&mut tx, result.last_insert_id()).await?;
    insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "convert_pair.create",
        pair.id,
        None,
        Some(convert_pair_audit_json(&pair)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(pair))
}

async fn update_convert_pair_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<UpdateConvertPairStatusRequest>,
) -> AppResult<Json<ConvertPairResponse>> {
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_convert_pair_in_tx(&mut tx, pair_id).await?;

    sqlx::query("UPDATE convert_pairs SET enabled = ? WHERE id = ?")
        .bind(request.enabled)
        .bind(pair_id)
        .execute(&mut *tx)
        .await?;
    let after = load_convert_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "convert_pair.update_status",
        pair_id,
        Some(convert_pair_audit_json(&before)),
        Some(convert_pair_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(after))
}

async fn load_risk_rule_in_tx(
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

async fn lock_risk_rule_in_tx(
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

async fn load_new_coin_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
) -> AppResult<NewCoinProjectResponse> {
    sqlx::query_as::<_, NewCoinProjectResponse>(
        r#"SELECT projects.id, projects.asset_id, projects.symbol, projects.lifecycle_status,
                  projects.total_supply, projects.issue_price, projects.listed_at,
                  projects.unlock_type, projects.fixed_unlock_at, projects.relative_unlock_seconds,
                  projects.unlock_fee_enabled, projects.unlock_fee_rate, projects.unlock_fee_basis,
                  projects.unlock_fee_asset, projects.status, projects.post_listing_purchase_enabled,
                  projects.post_listing_pair_id, post_listing_pair.status AS post_listing_pair_status
           FROM new_coin_projects projects
           LEFT JOIN trading_pairs post_listing_pair ON post_listing_pair.id = projects.post_listing_pair_id
           WHERE projects.id = ?
           LIMIT 1"#,
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn lock_new_coin_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
) -> AppResult<NewCoinProjectResponse> {
    sqlx::query_as::<_, NewCoinProjectResponse>(
        r#"SELECT projects.id, projects.asset_id, projects.symbol, projects.lifecycle_status,
                  projects.total_supply, projects.issue_price, projects.listed_at,
                  projects.unlock_type, projects.fixed_unlock_at, projects.relative_unlock_seconds,
                  projects.unlock_fee_enabled, projects.unlock_fee_rate, projects.unlock_fee_basis,
                  projects.unlock_fee_asset, projects.status, projects.post_listing_purchase_enabled,
                  projects.post_listing_pair_id, post_listing_pair.status AS post_listing_pair_status
           FROM new_coin_projects projects
           LEFT JOIN trading_pairs post_listing_pair ON post_listing_pair.id = projects.post_listing_pair_id
           WHERE projects.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn load_new_coin_distribution_in_tx(
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

async fn load_new_coin_convert_rule_in_tx(
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

async fn lock_new_coin_convert_rule_in_tx(
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

async fn load_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    sqlx::query_as::<_, ConvertPairResponse>(
        r#"SELECT id, from_asset AS from_asset_id, to_asset AS to_asset_id, pricing_mode,
                  spread_rate, min_amount, max_amount, enabled
           FROM convert_pairs
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(pair_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn lock_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    sqlx::query_as::<_, ConvertPairResponse>(
        r#"SELECT id, from_asset AS from_asset_id, to_asset AS to_asset_id, pricing_mode,
                  spread_rate, min_amount, max_amount, enabled
           FROM convert_pairs
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(pair_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

fn admin_news_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, title, category, status, country_code, default_locale, content_json,
                  published_at, created_by_admin_id, updated_by_admin_id, created_at, updated_at
           FROM admin_news_items"#,
    )
}

async fn load_admin_news_item(
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

async fn load_admin_news_item_in_tx(
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

async fn lock_admin_news_item_in_tx(
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

fn admin_asset_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, symbol, name, precision_scale, asset_type, status, created_at
           FROM assets"#,
    )
}

async fn load_asset(pool: &Pool<MySql>, asset_id: u64) -> AppResult<AdminAssetResponse> {
    let mut builder = admin_asset_query();
    builder.push(" WHERE id = ");
    builder.push_bind(asset_id);
    builder
        .build_query_as::<AdminAssetResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

async fn load_asset_in_tx(
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

async fn lock_asset_in_tx(
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

fn admin_trading_pair_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT pairs.id,
                  pairs.base_asset AS base_asset_id,
                  pairs.quote_asset AS quote_asset_id,
                  pairs.symbol,
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

async fn load_trading_pair_by_id(
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

async fn load_trading_pair_in_tx(
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

async fn lock_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM trading_pairs WHERE id = ? FOR UPDATE")
        .bind(pair_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    load_trading_pair_in_tx(tx, pair_id).await
}

async fn ensure_trading_pair_asset_in_tx(
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

fn base_market_strategy_query() -> QueryBuilder<'static, MySql> {
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

async fn load_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyResponse> {
    let mut builder = base_market_strategy_query();
    builder.push(" WHERE strategies.id = ");
    builder.push_bind(strategy_id);
    builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

async fn lock_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyResponse> {
    let mut builder = base_market_strategy_query();
    builder.push(" WHERE strategies.id = ");
    builder.push_bind(strategy_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

async fn ensure_strategy_pair_in_tx(
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

fn base_agent_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT agents.id, agents.user_id, users.email, agents.agent_code, agents.level, agents.status,
                  agent_admin_users.id AS admin_user_id,
                  agent_admin_users.username AS admin_username,
                  agent_admin_users.status AS admin_status,
                  agents.created_at
           FROM agents
           INNER JOIN users ON users.id = agents.user_id
           LEFT JOIN (
               SELECT agent_id, MIN(id) AS id
               FROM agent_admin_users
               GROUP BY agent_id
           ) first_agent_admin_users ON first_agent_admin_users.agent_id = agents.id
           LEFT JOIN agent_admin_users ON agent_admin_users.id = first_agent_admin_users.id"#,
    )
}

async fn load_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let mut builder = base_agent_query();
    builder.push(" WHERE agents.id = ");
    builder.push_bind(agent_id);
    builder.push(" ORDER BY agent_admin_users.id ASC LIMIT 1");
    builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

async fn ensure_post_listing_pair_in_tx(
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

async fn lock_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let mut builder = base_agent_query();
    builder.push(" WHERE agents.id = ");
    builder.push_bind(agent_id);
    builder.push(" ORDER BY agent_admin_users.id ASC LIMIT 1 FOR UPDATE");
    builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

async fn ensure_user_exists_in_tx(tx: &mut Transaction<'_, MySql>, user_id: u64) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM users WHERE id = ? LIMIT 1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(())
}

async fn lock_user_referral_in_tx(
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

async fn load_user_referral_in_tx(
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

async fn settle_agent_commission_payout_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission: &AdminAgentCommissionResponse,
) -> AppResult<()> {
    if commission.source_type != "convert_order" {
        return Err(AppError::Conflict(
            "agent commission source cannot be settled without payout support".to_owned(),
        ));
    }
    let target = sqlx::query_as::<_, AgentCommissionPayoutTargetRow>(
        r#"SELECT agents.user_id AS agent_user_id, orders.from_asset AS asset_id
           FROM agent_commission_records records
           INNER JOIN agents ON agents.id = records.agent_id
           INNER JOIN convert_orders orders ON orders.quote_id = records.source_id
           WHERE records.id = ?
           LIMIT 1"#,
    )
    .bind(commission.id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;

    credit_admin_wallet_available(
        tx,
        target.agent_user_id,
        target.asset_id,
        &commission.commission_amount,
        "agent_commission_payout",
        "agent_commission",
        &commission.id.to_string(),
    )
    .await
}

async fn load_agent_commission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
) -> AppResult<AdminAgentCommissionResponse> {
    sqlx::query_as::<_, AdminAgentCommissionResponse>(
        r#"SELECT id, agent_id, user_id, source_type, source_id, source_amount, commission_amount, status, created_at
           FROM agent_commission_records
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(commission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn load_agent_commission_rule_in_tx(
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

async fn lock_agent_commission_rule_in_tx(
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

async fn lock_agent_commission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
) -> AppResult<AdminAgentCommissionResponse> {
    sqlx::query_as::<_, AdminAgentCommissionResponse>(
        r#"SELECT id, agent_id, user_id, source_type, source_id, source_amount, commission_amount, status, created_at
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

async fn migrate_user_referral_descendants_in_tx(
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

async fn idempotency_key_exists_in_tx(
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

async fn insert_admin_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    insert_typed_admin_audit_log_in_tx(
        tx,
        admin_id,
        AdminAuditEntry {
            action,
            target_type: "convert_pair",
            target_id,
            before_json,
            after_json,
            reason,
        },
    )
    .await
}

async fn persist_market_strategy_change(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    strategy_id: u64,
    action: &'static str,
    before: Option<&AdminMarketStrategyResponse>,
    after: Option<&AdminMarketStrategyResponse>,
    reason: Option<String>,
) -> AppResult<()> {
    let before_json = before.map(market_strategy_audit_json);
    let after_json = after.map(market_strategy_audit_json);
    let event_payload = json!({
        "before": before_json,
        "after": after_json,
    });
    sqlx::query(
        r#"INSERT INTO strategy_events (strategy_id, event_type, payload_json)
           VALUES (?, ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(action)
    .bind(SqlxJson(event_payload))
    .execute(&mut **tx)
    .await?;
    insert_typed_admin_audit_log_in_tx(
        tx,
        admin_id,
        AdminAuditEntry {
            action,
            target_type: "market_strategy",
            target_id: strategy_id,
            before_json,
            after_json,
            reason,
        },
    )
    .await
}

async fn persist_new_coin_project_rule_change(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    project_id: u64,
    action: &'static str,
    before: &NewCoinProjectResponse,
    after: &NewCoinProjectResponse,
    reason: Option<String>,
) -> AppResult<()> {
    let before_json = new_coin_project_audit_json(before);
    let after_json = new_coin_project_audit_json(after);
    let event_payload = json!({
        "before": before_json,
        "after": after_json,
    });

    sqlx::query(
        r#"INSERT INTO new_coin_lifecycle_events (project_id, event_type, payload_json, created_by)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(project_id)
    .bind(action)
    .bind(SqlxJson(event_payload))
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    insert_typed_admin_audit_log_in_tx(
        tx,
        admin_id,
        AdminAuditEntry {
            action,
            target_type: "new_coin_project",
            target_id: project_id,
            before_json: Some(before_json),
            after_json: Some(after_json),
            reason,
        },
    )
    .await
}

async fn insert_typed_admin_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    entry: AdminAuditEntry<'_>,
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
    .bind(optional_string(entry.reason))
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn insert_admin_agent_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    entry: AdminAgentAuditEntry<'_>,
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
    .bind(optional_string(entry.reason))
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn validate_create_risk_rule(request: &CreateRiskRuleRequest) -> AppResult<()> {
    if optional_string(Some(request.rule_type.clone())).is_none() {
        return Err(AppError::Validation("rule_type is required".to_owned()));
    }
    if optional_string(Some(request.target_type.clone())).is_none() {
        return Err(AppError::Validation("target_type is required".to_owned()));
    }
    if request.config_json.is_null() {
        return Err(AppError::Validation("config_json is required".to_owned()));
    }
    Ok(())
}

fn validate_create_agent(request: &CreateAgentRequest) -> AppResult<()> {
    if request.user_id == 0 {
        return Err(AppError::Validation("user_id is required".to_owned()));
    }
    if optional_string(Some(request.agent_code.clone())).is_none() {
        return Err(AppError::Validation("agent_code is required".to_owned()));
    }
    if optional_string(Some(request.admin_username.clone())).is_none() {
        return Err(AppError::Validation(
            "admin_username is required".to_owned(),
        ));
    }
    if optional_string(request.admin_password.clone()).is_none()
        && optional_string(request.admin_password_hash.clone()).is_none()
    {
        return Err(AppError::Validation(
            "admin_password is required".to_owned(),
        ));
    }
    if request.level.unwrap_or(1) <= 0 {
        return Err(AppError::Validation("level must be positive".to_owned()));
    }
    Ok(())
}

fn agent_password_hash(request: &CreateAgentRequest) -> AppResult<String> {
    if let Some(password) = optional_string(request.admin_password.clone()) {
        return hash_password(&password);
    }
    optional_string(request.admin_password_hash.clone())
        .ok_or_else(|| AppError::Validation("admin_password is required".to_owned()))
}

fn validate_agent_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "suspended" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported agent status".to_owned())),
    }
}

fn validate_agent_commission_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "settled" | "rejected" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported agent commission status".to_owned(),
        )),
    }
}

fn validate_agent_commission_rule_product_type(value: &str) -> AppResult<String> {
    let Some(product_type) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("product_type is required".to_owned()));
    };
    if product_type == "convert" {
        Ok(product_type)
    } else {
        Err(AppError::Validation(
            "unsupported agent commission product type".to_owned(),
        ))
    }
}

fn validate_agent_commission_rule_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported agent commission rule status".to_owned(),
        )),
    }
}

fn validate_agent_commission_rate(value: &BigDecimal) -> AppResult<()> {
    if value < &BigDecimal::from(0) || value > &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "commission_rate must be between 0 and 1".to_owned(),
        ));
    }
    Ok(())
}

fn validate_create_market_strategy(request: &CreateMarketStrategyRequest) -> AppResult<()> {
    if request.pair_id == 0 {
        return Err(AppError::Validation("pair_id is required".to_owned()));
    }
    validate_market_strategy_config(MarketStrategyConfigValidation {
        strategy_type: &request.strategy_type,
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
    })?;
    if let Some(status) = request.status.as_deref() {
        validate_market_strategy_status(status)?;
    }
    Ok(())
}

fn validate_update_market_strategy(request: &UpdateMarketStrategyRequest) -> AppResult<()> {
    validate_market_strategy_config(MarketStrategyConfigValidation {
        strategy_type: &request.strategy_type,
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
    })?;
    required_reason(request.reason.clone())?;
    Ok(())
}

struct MarketStrategyConfigValidation<'a> {
    strategy_type: &'a str,
    start_price: &'a BigDecimal,
    target_price: &'a BigDecimal,
    start_time: chrono::DateTime<chrono::Utc>,
    end_time: chrono::DateTime<chrono::Utc>,
    volatility: &'a BigDecimal,
    volume_min: &'a BigDecimal,
    volume_max: &'a BigDecimal,
}

fn validate_market_strategy_config(config: MarketStrategyConfigValidation<'_>) -> AppResult<()> {
    if optional_string(Some(config.strategy_type.to_owned())).is_none() {
        return Err(AppError::Validation("strategy_type is required".to_owned()));
    }
    if config.start_price <= &BigDecimal::from(0) || config.target_price <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "strategy prices must be positive".to_owned(),
        ));
    }
    if config.end_time <= config.start_time {
        return Err(AppError::Validation(
            "end_time must be after start_time".to_owned(),
        ));
    }
    if config.volatility < &BigDecimal::from(0)
        || config.volume_min < &BigDecimal::from(0)
        || config.volume_max < &BigDecimal::from(0)
    {
        return Err(AppError::Validation(
            "volatility and volume must be non-negative".to_owned(),
        ));
    }
    if config.volume_max < config.volume_min {
        return Err(AppError::Validation(
            "volume_max must be greater than or equal to volume_min".to_owned(),
        ));
    }
    Ok(())
}

fn validate_market_strategy_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "draft" | "active" | "paused" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported market strategy status".to_owned(),
        )),
    }
}

fn market_strategy_run_status(status: &str) -> &'static str {
    match status {
        "active" => "running",
        "paused" => "paused",
        "disabled" => "stopped",
        _ => "draft",
    }
}

fn validate_distribute_new_coin(request: &DistributeNewCoinRequest) -> AppResult<()> {
    if request.quantity <= 0 {
        return Err(AppError::Validation("quantity must be positive".to_owned()));
    }
    if optional_string(Some(request.idempotency_key.clone())).is_none() {
        return Err(AppError::Validation(
            "idempotency_key must not be empty".to_owned(),
        ));
    }
    Ok(())
}

fn validate_update_new_coin_unlock_rule(request: &UpdateNewCoinUnlockRuleRequest) -> AppResult<()> {
    validate_unlock_rule_shape(
        &request.unlock_type,
        request.listed_at,
        request.fixed_unlock_at,
        request.relative_unlock_seconds,
    )
}

fn validate_update_new_coin_unlock_fee_rule(
    request: &UpdateNewCoinUnlockFeeRuleRequest,
) -> AppResult<()> {
    validate_unlock_fee_rule_shape(
        request.unlock_fee_enabled,
        request.unlock_fee_rate.as_ref(),
        request.unlock_fee_basis.clone(),
        request.unlock_fee_asset,
    )
}

fn validate_update_new_coin_post_listing_purchase(
    request: &UpdateNewCoinPostListingPurchaseRequest,
) -> AppResult<()> {
    if request.enabled && request.pair_id.unwrap_or(0) == 0 {
        return Err(AppError::Validation(
            "pair_id is required when post-listing purchase is enabled".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_post_listing_purchase_lifecycle(project: &NewCoinProjectResponse) -> AppResult<()> {
    if parse_lifecycle_status_from_db(&project.lifecycle_status)? != LifecycleStatus::Listed {
        return Err(AppError::Validation(
            "post-listing purchase can only be configured for listed projects".to_owned(),
        ));
    }
    Ok(())
}

fn validate_create_new_coin_project(request: &CreateNewCoinProjectRequest) -> AppResult<()> {
    let Some(lifecycle_status) = optional_string(Some(request.lifecycle_status.clone())) else {
        return Err(AppError::Validation(
            "lifecycle_status is required".to_owned(),
        ));
    };
    parse_lifecycle_status_from_request(&lifecycle_status)?;
    if request.total_supply <= 0 {
        return Err(AppError::Validation(
            "total_supply must be positive".to_owned(),
        ));
    }
    if request.issue_price < 0 {
        return Err(AppError::Validation(
            "issue_price must be non-negative".to_owned(),
        ));
    }
    if optional_string(Some(request.symbol.clone())).is_none() {
        return Err(AppError::Validation("symbol is required".to_owned()));
    }
    validate_unlock_rule_shape(
        &request.unlock_type,
        request.listed_at,
        request.fixed_unlock_at,
        request.relative_unlock_seconds,
    )?;
    validate_unlock_fee_rule_shape(
        request.unlock_fee_enabled.unwrap_or(false),
        request.unlock_fee_rate.as_ref(),
        request.unlock_fee_basis.clone(),
        request.unlock_fee_asset,
    )?;

    Ok(())
}

fn validate_unlock_rule_shape(
    unlock_type: &str,
    listed_at: Option<chrono::DateTime<chrono::Utc>>,
    fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    relative_unlock_seconds: Option<u64>,
) -> AppResult<()> {
    match optional_string(Some(unlock_type.to_owned())).as_deref() {
        Some("immediate_on_listing") => {
            if listed_at.is_none() {
                return Err(AppError::Validation(
                    "listed_at is required for immediate_on_listing unlock".to_owned(),
                ));
            }
            if fixed_unlock_at.is_some() || relative_unlock_seconds.is_some() {
                return Err(AppError::Validation(
                    "immediate_on_listing unlock cannot include fixed or relative unlock fields"
                        .to_owned(),
                ));
            }
        }
        Some("fixed_time") => {
            if fixed_unlock_at.is_none() {
                return Err(AppError::Validation(
                    "fixed_unlock_at is required for fixed_time unlock".to_owned(),
                ));
            }
            if listed_at.is_some() || relative_unlock_seconds.is_some() {
                return Err(AppError::Validation(
                    "fixed_time unlock cannot include listed_at or relative_unlock_seconds"
                        .to_owned(),
                ));
            }
        }
        Some("relative_period") => {
            if relative_unlock_seconds.unwrap_or(0) == 0 {
                return Err(AppError::Validation(
                    "relative_unlock_seconds is required for relative_period unlock".to_owned(),
                ));
            }
            if listed_at.is_some() || fixed_unlock_at.is_some() {
                return Err(AppError::Validation(
                    "relative_period unlock cannot include listed_at or fixed_unlock_at".to_owned(),
                ));
            }
        }
        Some(_) => {
            return Err(AppError::Validation(
                "unsupported new coin unlock_type".to_owned(),
            ));
        }
        None => return Err(AppError::Validation("unlock_type is required".to_owned())),
    }

    Ok(())
}

fn validate_unlock_fee_rule_shape(
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<&BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
) -> AppResult<()> {
    if !unlock_fee_enabled {
        return Ok(());
    }
    if unlock_fee_rate.is_none_or(|rate| rate <= 0) {
        return Err(AppError::Validation(
            "unlock_fee_rate must be positive when unlock fee is enabled".to_owned(),
        ));
    }
    match optional_string(unlock_fee_basis).as_deref() {
        Some("market_value" | "profit") => {}
        Some(_) => {
            return Err(AppError::Validation(
                "unsupported unlock_fee_basis".to_owned(),
            ));
        }
        None => {
            return Err(AppError::Validation(
                "unlock_fee_basis is required when unlock fee is enabled".to_owned(),
            ));
        }
    }
    if unlock_fee_asset.is_none() {
        return Err(AppError::Validation(
            "unlock_fee_asset is required when unlock fee is enabled".to_owned(),
        ));
    }

    Ok(())
}

fn parse_lifecycle_status_from_request(value: &str) -> AppResult<LifecycleStatus> {
    let Some(value) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "lifecycle_status is required".to_owned(),
        ));
    };
    parse_lifecycle_status(&value)
}

fn parse_lifecycle_status_from_db(value: &str) -> AppResult<LifecycleStatus> {
    parse_lifecycle_status(value).map_err(|_| {
        AppError::Internal(format!(
            "stored new coin lifecycle_status is unsupported: {value}"
        ))
    })
}

fn parse_lifecycle_status(value: &str) -> AppResult<LifecycleStatus> {
    match value {
        "preheat" => Ok(LifecycleStatus::Preheat),
        "subscription" => Ok(LifecycleStatus::Subscription),
        "distribution" => Ok(LifecycleStatus::Distribution),
        "listed" => Ok(LifecycleStatus::Listed),
        _ => Err(AppError::Validation(
            "unsupported new coin lifecycle_status".to_owned(),
        )),
    }
}

fn lifecycle_status_value(status: LifecycleStatus) -> &'static str {
    match status {
        LifecycleStatus::Preheat => "preheat",
        LifecycleStatus::Subscription => "subscription",
        LifecycleStatus::Distribution => "distribution",
        LifecycleStatus::Listed => "listed",
    }
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

fn validate_new_coin_convert_rule(request: &UpsertNewCoinConvertRuleRequest) -> AppResult<()> {
    let Some(rate_source) = optional_string(Some(request.rate_source.clone())) else {
        return Err(AppError::Validation("rate_source is required".to_owned()));
    };
    if rate_source != "fixed" {
        return Err(AppError::Validation(
            "only fixed rate_source is supported for new coin convert rules".to_owned(),
        ));
    }
    if request.fixed_rate.is_none() {
        return Err(AppError::Validation(
            "fixed_rate is required for fixed rate_source".to_owned(),
        ));
    }
    if let Some(fixed_rate) = &request.fixed_rate
        && fixed_rate <= 0
    {
        return Err(AppError::Validation(
            "fixed_rate must be positive".to_owned(),
        ));
    }
    if optional_string(request.status.clone()).is_none() && request.status.is_some() {
        return Err(AppError::Validation("status is required".to_owned()));
    }

    Ok(())
}

fn validate_admin_user_recharge(request: &AdminUserRechargeRequest) -> AppResult<()> {
    if request.asset_id == 0 {
        return Err(AppError::Validation("asset_id is required".to_owned()));
    }
    if request.amount <= 0 {
        return Err(AppError::Validation("amount must be positive".to_owned()));
    }
    required_reason(request.reason.clone())?;
    Ok(())
}

fn validate_create_admin_user(request: &CreateAdminUserRequest) -> AppResult<()> {
    if optional_string(request.email.clone()).is_none()
        && optional_string(request.phone.clone()).is_none()
    {
        return Err(AppError::Validation(
            "email or phone is required".to_owned(),
        ));
    }
    if let Some(email) = optional_string(request.email.clone())
        && (email.len() > 255 || !email.contains('@'))
    {
        return Err(AppError::Validation("email format is invalid".to_owned()));
    }
    if let Some(phone) = optional_string(request.phone.clone())
        && phone.len() > 32
    {
        return Err(AppError::Validation("phone is too long".to_owned()));
    }
    if optional_string(Some(request.password.clone())).is_none() {
        return Err(AppError::Validation("password is required".to_owned()));
    }
    if let Some(status) = request.status.as_deref() {
        validate_user_status(status)?;
    }
    if request.kyc_level.unwrap_or(0) < 0 {
        return Err(AppError::Validation(
            "kyc_level must be non-negative".to_owned(),
        ));
    }
    required_reason(request.reason.clone())?;
    Ok(())
}

fn validate_user_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "suspended" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported user status".to_owned())),
    }
}

fn validate_create_convert_pair(request: &CreateConvertPairRequest) -> AppResult<()> {
    if request.from_asset_id == request.to_asset_id {
        return Err(AppError::Validation(
            "convert pair assets must be different".to_owned(),
        ));
    }
    if optional_string(Some(request.pricing_mode.clone())).is_none() {
        return Err(AppError::Validation("pricing_mode is required".to_owned()));
    }
    if request.min_amount < 0 {
        return Err(AppError::Validation(
            "min_amount must be non-negative".to_owned(),
        ));
    }
    if request.spread_rate < 0 {
        return Err(AppError::Validation(
            "spread_rate must be non-negative".to_owned(),
        ));
    }
    if let Some(max_amount) = &request.max_amount
        && max_amount < &request.min_amount
    {
        return Err(AppError::Validation(
            "max_amount must be greater than or equal to min_amount".to_owned(),
        ));
    }

    Ok(())
}

fn validate_create_asset(request: &CreateAssetRequest) -> AppResult<()> {
    normalize_asset_symbol(&request.symbol)?;
    validate_asset_name(&request.name)?;
    validate_asset_precision(request.precision_scale)?;
    if let Some(asset_type) = request.asset_type.as_deref() {
        validate_asset_type(asset_type)?;
    }
    if let Some(status) = request.status.as_deref() {
        validate_asset_status(status)?;
    }
    Ok(())
}

fn validate_update_asset(request: &UpdateAssetRequest) -> AppResult<()> {
    validate_asset_name(&request.name)?;
    validate_asset_precision(request.precision_scale)?;
    validate_asset_type(&request.asset_type)?;
    validate_asset_status(&request.status)?;
    required_reason(request.reason.clone())?;
    Ok(())
}

fn validate_asset_name(value: &str) -> AppResult<String> {
    let Some(name) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("asset name is required".to_owned()));
    };
    if name.len() > 128 {
        return Err(AppError::Validation(
            "asset name must be at most 128 characters".to_owned(),
        ));
    }
    Ok(name)
}

fn validate_asset_precision(value: i32) -> AppResult<()> {
    if !(0..=18).contains(&value) {
        return Err(AppError::Validation(
            "asset precision_scale must be between 0 and 18".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_asset_symbol(value: &str) -> AppResult<String> {
    let Some(symbol) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("asset symbol is required".to_owned()));
    };
    if symbol.len() > 32 || !symbol.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(AppError::Validation(
            "asset symbol format is invalid".to_owned(),
        ));
    }
    Ok(symbol.to_ascii_uppercase())
}

fn validate_asset_type(value: &str) -> AppResult<String> {
    let Some(asset_type) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("asset_type is required".to_owned()));
    };
    match asset_type.as_str() {
        "coin" | "fiat" | "stablecoin" | "platform" => Ok(asset_type),
        _ => Err(AppError::Validation("unsupported asset_type".to_owned())),
    }
}

fn validate_asset_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported asset status".to_owned())),
    }
}

fn validate_news_title(value: &str) -> AppResult<String> {
    let Some(title) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news title is required".to_owned()));
    };
    if title.chars().count() > 255 {
        return Err(AppError::Validation("news title is too long".to_owned()));
    }
    Ok(title)
}

fn validate_news_category(value: &str) -> AppResult<String> {
    let Some(category) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news category is required".to_owned()));
    };
    match category.as_str() {
        "general" | "market" | "product" | "system" | "promotion" => Ok(category),
        _ => Err(AppError::Validation("unsupported news category".to_owned())),
    }
}

fn validate_news_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news status is required".to_owned()));
    };
    match status.as_str() {
        "draft" | "published" | "archived" => Ok(status),
        _ => Err(AppError::Validation("unsupported news status".to_owned())),
    }
}

fn normalize_optional_news_country_code(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|value| normalize_news_country_code(&value))
        .transpose()
}

fn normalize_news_country_code(value: &str) -> AppResult<String> {
    let Some(country_code) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "news country_code is required".to_owned(),
        ));
    };
    let country_code = country_code.to_ascii_uppercase();
    if country_code == "GLOBAL" {
        return Ok(country_code);
    }
    if country_code.len() < 2
        || country_code.len() > 16
        || !country_code.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(AppError::Validation(
            "news country_code format is invalid".to_owned(),
        ));
    }
    Ok(country_code)
}

fn validate_news_locale(value: &str) -> AppResult<String> {
    let Some(locale) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news locale is required".to_owned()));
    };
    if locale.len() < 2
        || locale.len() > 16
        || !locale
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(AppError::Validation(
            "news locale format is invalid".to_owned(),
        ));
    }
    Ok(locale)
}

fn validate_news_content_document(value: Value, default_locale: &str) -> AppResult<Value> {
    let object = value
        .as_object()
        .ok_or_else(|| AppError::Validation("news content must be an object".to_owned()))?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "version" | "default_locale" | "items"))
    {
        return Err(AppError::Validation(
            "news content field is unsupported".to_owned(),
        ));
    }
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(AppError::Validation(
            "news content version must be 1".to_owned(),
        ));
    }
    let content_default_locale = object
        .get("default_locale")
        .and_then(Value::as_str)
        .map(validate_news_locale)
        .transpose()?
        .ok_or_else(|| {
            AppError::Validation("news content default_locale is required".to_owned())
        })?;
    if content_default_locale != default_locale {
        return Err(AppError::Validation(
            "news content default_locale must match request default_locale".to_owned(),
        ));
    }
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| AppError::Validation("news content items are required".to_owned()))?;
    let mut has_default_locale = false;
    let mut seen = HashSet::new();
    for item in items {
        let item_object = item.as_object().ok_or_else(|| {
            AppError::Validation("news content item must be an object".to_owned())
        })?;
        if item_object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "locale" | "country_code" | "title" | "summary" | "content"
            )
        }) {
            return Err(AppError::Validation(
                "news content item field is unsupported".to_owned(),
            ));
        }
        let locale = required_news_content_string(item_object.get("locale"), "locale")?;
        let locale = validate_news_locale(locale)?;
        if locale == default_locale {
            has_default_locale = true;
        }
        let country_code =
            required_news_content_string(item_object.get("country_code"), "country_code")?;
        let country_code = normalize_news_country_code(country_code)?;
        if !seen.insert((locale, country_code)) {
            return Err(AppError::Validation(
                "news content locale and country_code must be unique".to_owned(),
            ));
        }
        validate_news_title(required_news_content_string(
            item_object.get("title"),
            "title",
        )?)?;
        if let Some(summary) = item_object.get("summary") {
            let summary = summary.as_str().ok_or_else(|| {
                AppError::Validation("news content summary must be a string".to_owned())
            })?;
            if summary.chars().count() > 512 {
                return Err(AppError::Validation(
                    "news content summary is too long".to_owned(),
                ));
            }
        }
        let content = item_object
            .get("content")
            .and_then(Value::as_array)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| {
                AppError::Validation("news content item content is required".to_owned())
            })?;
        if !validate_news_rich_text_content(content)? {
            return Err(AppError::Validation(
                "news content text is required".to_owned(),
            ));
        }
    }
    if !has_default_locale {
        return Err(AppError::Validation(
            "news content default_locale must exist in items".to_owned(),
        ));
    }
    Ok(value)
}

fn validate_news_rich_text_content(content: &[Value]) -> AppResult<bool> {
    let mut has_text = false;
    for node in content {
        has_text = validate_news_rich_text_block(node)? || has_text;
    }
    Ok(has_text)
}

fn validate_news_rich_text_block(node: &Value) -> AppResult<bool> {
    let object = node
        .as_object()
        .ok_or_else(invalid_news_rich_text_content)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "children"))
    {
        return Err(invalid_news_rich_text_content());
    }
    let node_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(invalid_news_rich_text_content)?;
    if !matches!(node_type, "p" | "h1" | "h2" | "h3" | "blockquote") {
        return Err(invalid_news_rich_text_content());
    }
    let children = object
        .get("children")
        .and_then(Value::as_array)
        .filter(|children| !children.is_empty())
        .ok_or_else(invalid_news_rich_text_content)?;
    let mut has_text = false;
    for child in children {
        has_text = validate_news_rich_text_child(child)? || has_text;
    }
    Ok(has_text)
}

fn validate_news_rich_text_child(node: &Value) -> AppResult<bool> {
    let object = node
        .as_object()
        .ok_or_else(invalid_news_rich_text_content)?;
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(invalid_news_rich_text_content)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "text" | "bold" | "italic" | "underline"))
    {
        return Err(invalid_news_rich_text_content());
    }
    for mark in ["bold", "italic", "underline"] {
        if let Some(value) = object.get(mark)
            && !value.is_boolean()
        {
            return Err(invalid_news_rich_text_content());
        }
    }
    Ok(!text.trim().is_empty())
}

fn invalid_news_rich_text_content() -> AppError {
    AppError::Validation("news content node is invalid".to_owned())
}

fn required_news_content_string<'a>(value: Option<&'a Value>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation(format!("news content {field} is required")))
}

fn validate_create_trading_pair(request: &CreateTradingPairRequest) -> AppResult<()> {
    if request.base_asset_id == request.quote_asset_id {
        return Err(AppError::Validation(
            "trading pair assets must be different".to_owned(),
        ));
    }
    normalize_trading_pair_symbol(&request.symbol)?;
    validate_trading_pair_config(
        request.price_precision,
        request.qty_precision,
        &request.min_order_value,
    )?;
    if let Some(status) = request.status.as_deref() {
        validate_trading_pair_status(status)?;
    }
    if let Some(market_type) = request.market_type.as_deref() {
        validate_trading_pair_market_type(market_type)?;
    }
    Ok(())
}

fn validate_update_trading_pair_request(request: &UpdateTradingPairRequest) -> AppResult<()> {
    validate_trading_pair_config(
        request.price_precision,
        request.qty_precision,
        &request.min_order_value,
    )?;
    validate_trading_pair_market_type(&request.market_type)?;
    Ok(())
}

fn validate_trading_pair_config(
    price_precision: i32,
    qty_precision: i32,
    min_order_value: &BigDecimal,
) -> AppResult<()> {
    if price_precision < 0 || qty_precision < 0 {
        return Err(AppError::Validation(
            "trading pair precision must be non-negative".to_owned(),
        ));
    }
    if min_order_value <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "min_order_value must be positive".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_trading_pair_symbol(value: &str) -> AppResult<String> {
    let Some(value) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("symbol is required".to_owned()));
    };
    if value.len() > 64
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/'))
    {
        return Err(AppError::Validation(
            "trading pair symbol format is invalid".to_owned(),
        ));
    }
    Ok(value.to_ascii_uppercase().replace(['_', '/'], "-"))
}

fn validate_trading_pair_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported trading pair status".to_owned(),
        )),
    }
}

fn validate_trading_pair_market_type(value: &str) -> AppResult<String> {
    let Some(market_type) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("market_type is required".to_owned()));
    };
    match market_type.as_str() {
        "external" | "internal" | "strategy" => Ok(market_type),
        _ => Err(AppError::Validation(
            "unsupported trading pair market_type".to_owned(),
        )),
    }
}

fn risk_rule_audit_json(rule: &RiskRuleResponse) -> Value {
    json!({
        "id": rule.id,
        "rule_type": rule.rule_type,
        "target_type": rule.target_type,
        "target_id": rule.target_id,
        "config_json": rule.config_json.0,
        "enabled": rule.enabled,
        "created_by": rule.created_by,
    })
}

fn convert_pair_audit_json(pair: &ConvertPairResponse) -> Value {
    json!({
        "id": pair.id,
        "from_asset_id": pair.from_asset_id,
        "to_asset_id": pair.to_asset_id,
        "pricing_mode": pair.pricing_mode,
        "spread_rate": pair.spread_rate,
        "min_amount": pair.min_amount,
        "max_amount": pair.max_amount,
        "enabled": pair.enabled,
    })
}

fn user_audit_json(user: &AdminUserResponse) -> Value {
    json!({
        "id": user.id,
        "email": user.email,
        "phone": user.phone,
        "status": user.status,
        "kyc_level": user.kyc_level,
        "created_at": user.created_at.timestamp_millis(),
        "updated_at": user.updated_at.timestamp_millis(),
    })
}

fn asset_audit_json(asset: &AdminAssetResponse) -> Value {
    json!({
        "id": asset.id,
        "symbol": asset.symbol,
        "name": asset.name,
        "precision_scale": asset.precision_scale,
        "asset_type": asset.asset_type,
        "status": asset.status,
        "created_at": asset.created_at.timestamp_millis(),
    })
}

fn admin_news_item_audit_json(news: &AdminNewsItemResponse) -> Value {
    json!({
        "id": news.id,
        "title": news.title,
        "category": news.category,
        "status": news.status,
        "country_code": news.country_code,
        "default_locale": news.default_locale,
        "published_at": news.published_at.map(|value| value.timestamp_millis()),
        "created_by_admin_id": news.created_by_admin_id,
        "updated_by_admin_id": news.updated_by_admin_id,
        "created_at": news.created_at.timestamp_millis(),
        "updated_at": news.updated_at.timestamp_millis(),
    })
}

fn recharge_audit_json(recharge: &AdminUserRechargeResponse) -> Value {
    json!({
        "recharge_id": recharge.recharge_id,
        "user_id": recharge.user_id,
        "asset_id": recharge.asset_id,
        "asset_symbol": recharge.asset_symbol,
        "amount": format!("{:.18}", recharge.amount),
        "available": format!("{:.18}", recharge.available),
        "frozen": format!("{:.18}", recharge.frozen),
        "locked": format!("{:.18}", recharge.locked),
    })
}

fn trading_pair_audit_json(pair: &AdminTradingPairResponse) -> Value {
    json!({
        "id": pair.id,
        "base_asset_id": pair.base_asset_id,
        "quote_asset_id": pair.quote_asset_id,
        "symbol": pair.symbol,
        "base_asset": pair.base_asset,
        "quote_asset": pair.quote_asset,
        "price_precision": pair.price_precision,
        "qty_precision": pair.qty_precision,
        "min_order_value": pair.min_order_value,
        "status": pair.status,
        "market_type": pair.market_type,
        "created_at": pair.created_at.timestamp_millis(),
    })
}

fn new_coin_project_audit_json(project: &NewCoinProjectResponse) -> Value {
    json!({
        "id": project.id,
        "asset_id": project.asset_id,
        "symbol": project.symbol,
        "lifecycle_status": project.lifecycle_status,
        "total_supply": project.total_supply,
        "issue_price": project.issue_price,
        "listed_at": project.listed_at.map(|value| value.timestamp_millis()),
        "unlock_type": project.unlock_type,
        "fixed_unlock_at": project.fixed_unlock_at.map(|value| value.timestamp_millis()),
        "relative_unlock_seconds": project.relative_unlock_seconds,
        "unlock_fee_enabled": project.unlock_fee_enabled,
        "unlock_fee_rate": project.unlock_fee_rate,
        "unlock_fee_basis": project.unlock_fee_basis,
        "unlock_fee_asset": project.unlock_fee_asset,
        "status": project.status,
        "post_listing_purchase_enabled": project.post_listing_purchase_enabled,
        "post_listing_pair_id": project.post_listing_pair_id,
        "post_listing_pair_status": project.post_listing_pair_status,
    })
}

async fn apply_subscription_distribution_in_tx(
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

fn ensure_distribution_lifecycle(project: &NewCoinProjectResponse) -> AppResult<()> {
    if parse_lifecycle_status_from_db(&project.lifecycle_status)? != LifecycleStatus::Distribution {
        return Err(AppError::Validation(
            "new coin project must be in distribution lifecycle before distribution".to_owned(),
        ));
    }
    Ok(())
}

fn lock_positions_for_distribution(
    project: &NewCoinProjectResponse,
    user_id: u64,
    asset_id: u64,
    source_id: &str,
    quantity: BigDecimal,
    source_time: chrono::DateTime<chrono::Utc>,
) -> AppResult<Vec<AdminNewCoinLockPositionInsert>> {
    let unlock_rule = unlock_rule_from_project(project)?;
    let application = apply_unlock_rule(
        &unlock_rule,
        vec![UnlockSource {
            user_id: user_id.to_string(),
            asset_id: asset_id.to_string(),
            source_id: source_id.to_owned(),
            amount: quantity,
            source_time,
        }],
    )
    .map_err(|error| AppError::Validation(format!("invalid new coin unlock rule: {error:?}")))?;

    Ok(application
        .lock_positions
        .into_iter()
        .map(|position| AdminNewCoinLockPositionInsert {
            user_id,
            asset_id,
            unlock_type: position.unlock_type,
            unlock_at: position.unlock_at,
            amount: position.remaining_amount,
            merge_key: position.merge_key,
            source_time,
            source_type: "new_coin_distribution".to_owned(),
            source_id: source_id.to_owned(),
        })
        .collect())
}

fn unlock_rule_from_project(project: &NewCoinProjectResponse) -> AppResult<UnlockRule> {
    match project.unlock_type.as_str() {
        "immediate_on_listing" => Ok(UnlockRule::ImmediateOnListing {
            listed_at: project.listed_at.ok_or_else(|| {
                AppError::Validation("listed_at is required for immediate unlock".to_owned())
            })?,
        }),
        "fixed_time" => Ok(UnlockRule::FixedTime {
            unlock_at: project.fixed_unlock_at.ok_or_else(|| {
                AppError::Validation("fixed_unlock_at is required for fixed unlock".to_owned())
            })?,
        }),
        "relative_period" => Ok(UnlockRule::RelativePeriod {
            seconds_after_source: project
                .relative_unlock_seconds
                .ok_or_else(|| {
                    AppError::Validation(
                        "relative_unlock_seconds is required for relative unlock".to_owned(),
                    )
                })?
                .try_into()
                .map_err(|_| {
                    AppError::Validation("relative unlock period is too large".to_owned())
                })?,
        }),
        _ => Err(AppError::Validation(
            "unsupported new coin unlock_type".to_owned(),
        )),
    }
}

async fn apply_new_coin_distribution_allocation(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    quantity: &BigDecimal,
    lock_positions: &[AdminNewCoinLockPositionInsert],
    ledger: AdminNewCoinLedgerMetadata<'_>,
) -> AppResult<Option<u64>> {
    if lock_positions.is_empty() {
        credit_admin_wallet_available(
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

    let wallet = lock_or_create_admin_wallet_row(tx, user_id, asset_id).await?;
    let locked_after = wallet.locked.clone() + quantity.clone();
    sqlx::query("UPDATE wallet_accounts SET locked = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&locked_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_admin_wallet_ledger(
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

async fn credit_admin_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_or_create_admin_wallet_row(tx, user_id, asset_id).await?;
    let available_after = wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_admin_wallet_ledger(
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

async fn lock_or_create_admin_wallet_row(
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

async fn load_active_asset_symbol_in_tx(
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

async fn upsert_admin_new_coin_lock_position(
    tx: &mut Transaction<'_, MySql>,
    position: &AdminNewCoinLockPositionInsert,
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

#[allow(clippy::too_many_arguments)]
async fn insert_admin_wallet_ledger(
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

fn new_coin_distribution_audit_json(distribution: &NewCoinDistributionResponse) -> Value {
    json!({
        "id": distribution.id,
        "project_id": distribution.project_id,
        "user_id": distribution.user_id,
        "subscription_id": distribution.subscription_id,
        "asset_id": distribution.asset_id,
        "quantity": distribution.quantity,
        "lock_position_id": distribution.lock_position_id,
        "status": distribution.status,
        "idempotency_key": distribution.idempotency_key,
        "created_at": distribution.created_at.timestamp_millis(),
    })
}

fn market_strategy_config_json(
    request: &CreateMarketStrategyRequest,
    status: &str,
    market_type: &str,
) -> Value {
    market_strategy_config_value(MarketStrategyConfigValue {
        pair_id: Some(request.pair_id),
        market_type,
        strategy_type: request.strategy_type.trim(),
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
        status,
    })
}

fn market_strategy_update_config_json(
    request: &UpdateMarketStrategyRequest,
    status: &str,
    market_type: &str,
) -> Value {
    market_strategy_config_value(MarketStrategyConfigValue {
        pair_id: None,
        market_type,
        strategy_type: request.strategy_type.trim(),
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
        status,
    })
}

struct MarketStrategyConfigValue<'a> {
    pair_id: Option<u64>,
    market_type: &'a str,
    strategy_type: &'a str,
    start_price: &'a BigDecimal,
    target_price: &'a BigDecimal,
    start_time: chrono::DateTime<chrono::Utc>,
    end_time: chrono::DateTime<chrono::Utc>,
    volatility: &'a BigDecimal,
    volume_min: &'a BigDecimal,
    volume_max: &'a BigDecimal,
    status: &'a str,
}

fn market_strategy_config_value(config: MarketStrategyConfigValue<'_>) -> Value {
    let mut value = json!({
        "market_type": config.market_type,
        "strategy_type": config.strategy_type,
        "start_price": config.start_price,
        "target_price": config.target_price,
        "start_time": config.start_time.timestamp_millis(),
        "end_time": config.end_time.timestamp_millis(),
        "volatility": config.volatility,
        "volume_min": config.volume_min,
        "volume_max": config.volume_max,
        "status": config.status,
    });
    if let Some(pair_id) = config.pair_id {
        value["pair_id"] = json!(pair_id);
    }
    value
}

fn market_strategy_audit_json(strategy: &AdminMarketStrategyResponse) -> Value {
    json!({
        "id": strategy.id,
        "pair_id": strategy.pair_id,
        "symbol": strategy.symbol,
        "market_type": strategy.market_type,
        "strategy_type": strategy.strategy_type,
        "start_price": strategy.start_price,
        "target_price": strategy.target_price,
        "start_time": strategy.start_time.timestamp_millis(),
        "end_time": strategy.end_time.timestamp_millis(),
        "volatility": strategy.volatility,
        "volume_min": strategy.volume_min,
        "volume_max": strategy.volume_max,
        "status": strategy.status,
        "run_status": strategy.run_status,
        "current_price": strategy.current_price,
        "last_generated_at": strategy.last_generated_at.map(|value| value.timestamp_millis()),
        "last_kline_open_time": strategy.last_kline_open_time.map(|value| value.timestamp_millis()),
        "recovery_status": strategy.recovery_status,
        "created_at": strategy.created_at.timestamp_millis(),
    })
}

fn agent_audit_json(agent: &AdminAgentResponse) -> Value {
    json!({
        "id": agent.id,
        "user_id": agent.user_id,
        "email": agent.email,
        "agent_code": agent.agent_code,
        "level": agent.level,
        "status": agent.status,
        "admin_user_id": agent.admin_user_id,
        "admin_username": agent.admin_username,
        "admin_status": agent.admin_status,
        "created_at": agent.created_at.timestamp_millis(),
    })
}

fn user_referral_audit_json(referral: &AdminUserReferralResponse) -> Value {
    json!({
        "user_id": referral.user_id,
        "direct_inviter_id": referral.direct_inviter_id,
        "direct_inviter_type": referral.direct_inviter_type,
        "root_agent_id": referral.root_agent_id,
        "depth": referral.depth,
        "path": referral.path,
        "created_at": referral.created_at.timestamp_millis(),
    })
}

fn agent_commission_audit_json(commission: &AdminAgentCommissionResponse) -> Value {
    json!({
        "id": commission.id,
        "agent_id": commission.agent_id,
        "user_id": commission.user_id,
        "source_type": commission.source_type,
        "source_id": commission.source_id,
        "source_amount": commission.source_amount,
        "commission_amount": commission.commission_amount,
        "status": commission.status,
        "created_at": commission.created_at.timestamp_millis(),
    })
}

fn agent_commission_rule_audit_json(rule: &AdminAgentCommissionRuleResponse) -> Value {
    json!({
        "id": rule.id,
        "agent_id": rule.agent_id,
        "product_type": rule.product_type,
        "commission_rate": rule.commission_rate,
        "status": rule.status,
        "created_at": rule.created_at.timestamp_millis(),
        "updated_at": rule.updated_at.timestamp_millis(),
    })
}

fn new_coin_convert_rule_audit_json(rule: &NewCoinConvertRuleResponse) -> Value {
    json!({
        "id": rule.id,
        "convert_pair_id": rule.convert_pair_id,
        "rate_source": rule.rate_source,
        "fixed_rate": rule.fixed_rate,
        "floating_rate_json": rule.floating_rate_json.as_ref().map(|value| &value.0),
        "status": rule.status,
        "created_by": rule.created_by,
    })
}

fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for admin convert routes".to_owned())
    })
}

fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(10_000)
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn required_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

fn map_duplicate_pair(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("convert pair already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_trading_pair(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("trading pair already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_asset(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("asset already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_user(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("user already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_distribution(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("new coin distribution has already been created".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_agent(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("agent already exists".to_owned())
    } else {
        AppError::Database(error)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Settings,
        modules::auth::{TokenScope, issue_token},
        state::AppState,
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode, header::AUTHORIZATION},
    };
    use secrecy::SecretString;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState::new(Settings {
            app_env: "test".to_owned(),
            app_host: "127.0.0.1".parse().unwrap(),
            app_port: 0,
            database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
            mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
            mongodb_database: "exchange_test".to_owned(),
            redis_url: SecretString::new("redis://localhost:6379".to_owned()),
            rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
            jwt_secret: SecretString::new("test-secret".to_owned()),
            credential_encryption_key: Some(SecretString::new(
                "0123456789abcdef0123456789abcdef".to_owned(),
            )),
            jwt_access_ttl_seconds: 900,
            jwt_refresh_ttl_seconds: 2_592_000,
            bitget_rest_base_url: "https://bitget.test".to_owned(),
            bitget_ws_url: "wss://bitget.test/ws".to_owned(),
            htx_rest_base_url: "https://htx.test".to_owned(),
            htx_ws_url: "wss://htx.test/ws".to_owned(),
            market_feed_symbols: Vec::new(),
            market_feed_intervals: Vec::new(),
            market_feed_providers: Vec::new(),
            market_feed_reconnect_seconds: 5,
            market_feed_rest_fallback_timeout_seconds: 3,
            event_inbox_retry_scan_seconds: 10,
            event_outbox_publisher_enabled: true,
            event_outbox_publisher_interval_seconds: 5,
            unlock_scanner_enabled: true,
            unlock_scanner_interval_seconds: 10,
            unlock_scanner_batch_limit: 100,
            kline_recovery_enabled: true,
            kline_recovery_interval_seconds: 30,
            kline_recovery_batch_limit: 100,
            seconds_contract_settlement_enabled: true,
            seconds_contract_settlement_interval_seconds: 5,
            seconds_contract_settlement_batch_limit: 100,
            earn_auto_redemption_enabled: true,
            earn_auto_redemption_interval_seconds: 60,
            earn_auto_redemption_batch_limit: 100,
            margin_liquidation_enabled: true,
            margin_liquidation_interval_seconds: 5,
            margin_liquidation_batch_limit: 100,
            margin_interest_enabled: true,
            margin_interest_interval_seconds: 60,
            margin_interest_batch_limit: 100,
        })
    }

    async fn post_agents(app: Router, token: Option<&str>) -> StatusCode {
        let mut request = Request::builder().method("POST").uri("/agents");
        if let Some(token) = token {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }

        app.oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn admin_routes_require_admin_scope() {
        let state = test_state();
        let user_token = issue_token(
            &state.settings,
            "user:1",
            TokenScope::User,
            state.settings.jwt_access_ttl_seconds,
        )
        .unwrap();
        let admin_token = issue_token(
            &state.settings,
            "admin:1",
            TokenScope::Admin,
            state.settings.jwt_access_ttl_seconds,
        )
        .unwrap();
        let app = routes().with_state(state);

        assert_eq!(
            post_agents(app.clone(), None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            post_agents(app.clone(), Some(&user_token)).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post_agents(app, Some(&admin_token)).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
    }
}
