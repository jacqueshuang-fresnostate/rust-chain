//! prediction bounded context repository layer.
//!
//! 仓储层仅定义持久化边界对象和业务层需要的契约。
//! SQLx QueryBuilder/具体 SQL 语句应放在 infrastructure.rs 中。

use crate::architecture::RepositoryLayer;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct PredictionSettingsRow {
    pub(crate) sync_enabled: bool,
    pub(crate) sync_interval_seconds: u32,
    pub(crate) sync_tags_json: Value,
    pub(crate) allowed_asset_ids_json: Value,
    pub(crate) default_fee_rate: BigDecimal,
    pub(crate) default_settlement_mode: String,
    pub(crate) default_invalid_refund_policy: String,
    pub(crate) quote_ttl_seconds: u32,
    pub(crate) last_sync_status: Option<String>,
    pub(crate) last_sync_error: Option<String>,
    pub(crate) last_sync_started_at: Option<DateTime<Utc>>,
    pub(crate) last_sync_finished_at: Option<DateTime<Utc>>,
    pub(crate) last_successful_sync_at: Option<DateTime<Utc>>,
    pub(crate) last_sync_imported_count: u32,
    pub(crate) last_sync_updated_count: u32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct PredictionMarketRow {
    pub(crate) id: u64,
    pub(crate) source: String,
    pub(crate) external_event_id: Option<String>,
    pub(crate) external_market_id: String,
    pub(crate) slug: Option<String>,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) image_url: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) tags_json: Value,
    pub(crate) outcome_yes_label: String,
    pub(crate) outcome_no_label: String,
    pub(crate) yes_price: BigDecimal,
    pub(crate) no_price: BigDecimal,
    pub(crate) volume: Option<BigDecimal>,
    pub(crate) liquidity: Option<BigDecimal>,
    pub(crate) end_at: Option<DateTime<Utc>>,
    pub(crate) source_status: String,
    pub(crate) display_status: String,
    pub(crate) local_resolution: Option<String>,
    pub(crate) settlement_status: String,
    pub(crate) settlement_mode_override: Option<String>,
    pub(crate) allowed_asset_ids_override_json: Option<Value>,
    pub(crate) payout_cap_overrides_json: Option<Value>,
    pub(crate) fee_rate_override: Option<BigDecimal>,
    pub(crate) last_synced_at: Option<DateTime<Utc>>,
    pub(crate) external_resolution: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct PredictionAssetConfigRow {
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) enabled: bool,
    pub(crate) max_payout_amount: BigDecimal,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct PredictionStakeAssetRow {
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) max_payout_amount: BigDecimal,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct PredictionOrderRow {
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
    pub(crate) settled_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct PredictionSyncLogRow {
    pub(crate) id: u64,
    pub(crate) trigger_type: String,
    pub(crate) status: String,
    pub(crate) imported_count: u32,
    pub(crate) updated_count: u32,
    pub(crate) error_message: Option<String>,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct PredictionAssetMetaRow {
    pub(crate) symbol: String,
    pub(crate) status: String,
    pub(crate) precision_scale: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct PredictionQuoteLockRow {
    pub(crate) quote_id: String,
    pub(crate) user_id: u64,
    pub(crate) market_id: u64,
    pub(crate) outcome: String,
    pub(crate) asset_id: u64,
    pub(crate) stake_amount: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) accepted_price: BigDecimal,
    pub(crate) shares: BigDecimal,
    pub(crate) theoretical_payout: BigDecimal,
    pub(crate) effective_payout_cap: BigDecimal,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct PredictionWalletRow {
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct PredictionOrderSettlementRow {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) outcome: String,
    pub(crate) stake_amount: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) theoretical_payout: BigDecimal,
    pub(crate) effective_payout_cap: BigDecimal,
    pub(crate) status: String,
}
