//! prediction 模块表现层。
//!
//! 负责 HTTP 请求/响应 DTO 与查询模型。

use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json as SqlxJson;

use super::{
    repository::{
        PredictionAssetConfigRow, PredictionMarketRow, PredictionOrderRow, PredictionSettingsRow,
        PredictionStakeAssetRow, PredictionSyncLogRow,
    },
    service,
};

#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminMarketQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) display_status: Option<String>,
    pub(crate) settlement_status: Option<String>,
    pub(crate) keyword: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OrdersQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) status: Option<String>,
    pub(crate) market_id: Option<u64>,
    pub(crate) email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SavePredictionSettingsRequest {
    pub(crate) sync_enabled: bool,
    pub(crate) sync_interval_seconds: u32,
    pub(crate) sync_tags: Vec<String>,
    pub(crate) allowed_asset_ids: Vec<u64>,
    pub(crate) default_fee_rate: BigDecimal,
    pub(crate) default_settlement_mode: String,
    pub(crate) default_invalid_refund_policy: String,
    pub(crate) quote_ttl_seconds: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpsertPredictionAssetConfigRequest {
    pub(crate) asset_id: u64,
    pub(crate) enabled: bool,
    pub(crate) max_payout_amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdatePredictionAssetConfigRequest {
    pub(crate) enabled: bool,
    pub(crate) max_payout_amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdatePredictionMarketRequest {
    pub(crate) display_status: String,
    pub(crate) settlement_mode_override: Option<String>,
    pub(crate) allowed_asset_ids_override: Option<Vec<u64>>,
    pub(crate) payout_cap_overrides: Option<Value>,
    pub(crate) fee_rate_override: Option<BigDecimal>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreatePredictionQuoteRequest {
    pub(crate) market_id: u64,
    pub(crate) outcome: String,
    pub(crate) asset_id: u64,
    pub(crate) stake_amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreatePredictionOrderRequest {
    pub(crate) quote_id: String,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettlePredictionMarketRequest {
    pub(crate) result: String,
    pub(crate) invalid_refund_policy: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionSettingsResponse {
    pub(crate) sync_enabled: bool,
    pub(crate) sync_interval_seconds: u32,
    pub(crate) sync_tags: Vec<String>,
    pub(crate) allowed_asset_ids: Vec<u64>,
    pub(crate) default_fee_rate: BigDecimal,
    pub(crate) default_settlement_mode: String,
    pub(crate) default_invalid_refund_policy: String,
    pub(crate) quote_ttl_seconds: u32,
    pub(crate) last_sync_status: Option<String>,
    pub(crate) last_sync_error: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_sync_started_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_sync_finished_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_successful_sync_at: Option<DateTime<Utc>>,
    pub(crate) last_sync_imported_count: u32,
    pub(crate) last_sync_updated_count: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionStakeAssetResponse {
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) max_payout_amount: BigDecimal,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionUserConfigResponse {
    pub(crate) allowed_assets: Vec<PredictionStakeAssetResponse>,
    pub(crate) default_fee_rate: BigDecimal,
    pub(crate) quote_ttl_seconds: u32,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct PredictionMarketResponse {
    pub(crate) id: u64,
    pub(crate) source: String,
    pub(crate) external_event_id: Option<String>,
    pub(crate) external_market_id: String,
    pub(crate) slug: Option<String>,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) image_url: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) tags_json: SqlxJson<Value>,
    pub(crate) outcome_yes_label: String,
    pub(crate) outcome_no_label: String,
    pub(crate) yes_price: BigDecimal,
    pub(crate) no_price: BigDecimal,
    pub(crate) volume: Option<BigDecimal>,
    pub(crate) liquidity: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) end_at: Option<DateTime<Utc>>,
    pub(crate) source_status: String,
    pub(crate) display_status: String,
    pub(crate) external_resolution: Option<String>,
    pub(crate) local_resolution: Option<String>,
    pub(crate) settlement_status: String,
    pub(crate) settlement_mode_override: Option<String>,
    pub(crate) allowed_asset_ids_override_json: Option<SqlxJson<Value>>,
    pub(crate) payout_cap_overrides_json: Option<SqlxJson<Value>>,
    pub(crate) fee_rate_override: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_synced_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionMarketsResponse {
    pub(crate) markets: Vec<PredictionMarketResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionAssetConfigsResponse {
    pub(crate) configs: Vec<PredictionAssetConfigResponse>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct PredictionAssetConfigResponse {
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) enabled: bool,
    pub(crate) max_payout_amount: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct PredictionQuoteResponse {
    pub(crate) quote_id: String,
    pub(crate) market_id: u64,
    pub(crate) outcome: String,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) stake_amount: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) accepted_price: BigDecimal,
    pub(crate) shares: BigDecimal,
    pub(crate) theoretical_payout: BigDecimal,
    pub(crate) effective_payout_cap: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct PredictionOrderResponse {
    pub(crate) id: u64,
    pub(crate) order_no: Option<String>,
    pub(crate) user_id: u64,
    pub(crate) user_email: Option<String>,
    pub(crate) market_id: u64,
    pub(crate) market_title: String,
    pub(crate) outcome: String,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) stake_amount: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) accepted_price: BigDecimal,
    pub(crate) shares: BigDecimal,
    pub(crate) theoretical_payout: BigDecimal,
    pub(crate) effective_payout_cap: BigDecimal,
    pub(crate) status: String,
    pub(crate) result: Option<String>,
    pub(crate) payout_amount: BigDecimal,
    pub(crate) refund_amount: BigDecimal,
    pub(crate) fee_refund_amount: BigDecimal,
    pub(crate) invalid_refund_policy_used: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) settled_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionOrdersResponse {
    pub(crate) orders: Vec<PredictionOrderResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionOrderActionResponse {
    pub(crate) order: PredictionOrderResponse,
    pub(crate) changed: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionSettlementResponse {
    pub(crate) market: PredictionMarketResponse,
    pub(crate) settled_orders: u32,
    pub(crate) changed: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionSyncResponse {
    pub(crate) imported_count: u32,
    pub(crate) updated_count: u32,
    pub(crate) status: String,
    pub(crate) error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct PredictionSyncLogResponse {
    pub(crate) id: u64,
    pub(crate) trigger_type: String,
    pub(crate) status: String,
    pub(crate) imported_count: u32,
    pub(crate) updated_count: u32,
    pub(crate) error_message: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) started_at: DateTime<Utc>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionSyncLogsResponse {
    pub(crate) logs: Vec<PredictionSyncLogResponse>,
}

impl From<PredictionSettingsRow> for PredictionSettingsResponse {
    fn from(row: PredictionSettingsRow) -> Self {
        Self {
            sync_enabled: row.sync_enabled,
            sync_interval_seconds: row.sync_interval_seconds,
            sync_tags: service::json_string_array(&row.sync_tags_json),
            allowed_asset_ids: service::json_u64_array(&row.allowed_asset_ids_json),
            default_fee_rate: row.default_fee_rate,
            default_settlement_mode: row.default_settlement_mode,
            default_invalid_refund_policy: row.default_invalid_refund_policy,
            quote_ttl_seconds: row.quote_ttl_seconds,
            last_sync_status: row.last_sync_status,
            last_sync_error: row.last_sync_error,
            last_sync_started_at: row.last_sync_started_at,
            last_sync_finished_at: row.last_sync_finished_at,
            last_successful_sync_at: row.last_successful_sync_at,
            last_sync_imported_count: row.last_sync_imported_count,
            last_sync_updated_count: row.last_sync_updated_count,
        }
    }
}

impl From<PredictionStakeAssetRow> for PredictionStakeAssetResponse {
    fn from(row: PredictionStakeAssetRow) -> Self {
        Self {
            asset_id: row.asset_id,
            asset_symbol: row.asset_symbol,
            max_payout_amount: row.max_payout_amount,
        }
    }
}

impl From<PredictionMarketRow> for PredictionMarketResponse {
    fn from(row: PredictionMarketRow) -> Self {
        Self {
            id: row.id,
            source: row.source,
            external_event_id: row.external_event_id,
            external_market_id: row.external_market_id,
            slug: row.slug,
            title: row.title,
            description: row.description,
            image_url: row.image_url,
            category: row.category,
            tags_json: SqlxJson(row.tags_json),
            outcome_yes_label: row.outcome_yes_label,
            outcome_no_label: row.outcome_no_label,
            yes_price: row.yes_price,
            no_price: row.no_price,
            volume: row.volume,
            liquidity: row.liquidity,
            end_at: row.end_at,
            source_status: row.source_status,
            display_status: row.display_status,
            external_resolution: row.external_resolution,
            local_resolution: row.local_resolution,
            settlement_status: row.settlement_status,
            settlement_mode_override: row.settlement_mode_override,
            allowed_asset_ids_override_json: row.allowed_asset_ids_override_json.map(SqlxJson),
            payout_cap_overrides_json: row.payout_cap_overrides_json.map(SqlxJson),
            fee_rate_override: row.fee_rate_override,
            last_synced_at: row.last_synced_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<PredictionAssetConfigRow> for PredictionAssetConfigResponse {
    fn from(row: PredictionAssetConfigRow) -> Self {
        Self {
            asset_id: row.asset_id,
            asset_symbol: row.asset_symbol,
            enabled: row.enabled,
            max_payout_amount: row.max_payout_amount,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<PredictionOrderRow> for PredictionOrderResponse {
    fn from(row: PredictionOrderRow) -> Self {
        Self {
            id: row.id,
            order_no: row.order_no,
            user_id: row.user_id,
            user_email: row.user_email,
            market_id: row.market_id,
            market_title: row.market_title,
            outcome: row.outcome,
            asset_id: row.asset_id,
            asset_symbol: row.asset_symbol,
            stake_amount: row.stake_amount,
            fee_amount: row.fee_amount,
            accepted_price: row.accepted_price,
            shares: row.shares,
            theoretical_payout: row.theoretical_payout,
            effective_payout_cap: row.effective_payout_cap,
            status: row.status,
            result: row.result,
            payout_amount: row.payout_amount,
            refund_amount: row.refund_amount,
            fee_refund_amount: row.fee_refund_amount,
            invalid_refund_policy_used: row.invalid_refund_policy_used,
            settled_at: row.settled_at,
            created_at: row.created_at,
        }
    }
}

impl From<PredictionSyncLogRow> for PredictionSyncLogResponse {
    fn from(row: PredictionSyncLogRow) -> Self {
        Self {
            id: row.id,
            trigger_type: row.trigger_type,
            status: row.status,
            imported_count: row.imported_count,
            updated_count: row.updated_count,
            error_message: row.error_message,
            started_at: row.started_at,
            finished_at: row.finished_at,
        }
    }
}
