//! seconds_contract bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。

use crate::architecture::RepositoryLayer;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct SecondsContractProductRow {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) stake_asset: u64,
    pub(crate) stake_asset_symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) duration_seconds: u32,
    pub(crate) payout_rate: BigDecimal,
    pub(crate) min_stake: BigDecimal,
    pub(crate) max_stake: Option<BigDecimal>,
    pub(crate) status: String,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct SecondsContractProductRuleRow {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) stake_asset: u64,
    pub(crate) stake_asset_precision: i32,
    pub(crate) duration_seconds: u32,
    pub(crate) payout_rate: BigDecimal,
    pub(crate) min_stake: BigDecimal,
    pub(crate) max_stake: Option<BigDecimal>,
    pub(crate) status: String,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct SecondsContractWalletRow {
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
}

#[derive(Debug, Clone)]
pub(crate) struct SecondsContractAdminOrderFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct SecondsContractProductWrite {
    pub(crate) pair_id: u64,
    pub(crate) stake_asset: u64,
    pub(crate) logo_url: Option<String>,
    pub(crate) duration_seconds: u32,
    pub(crate) payout_rate: BigDecimal,
    pub(crate) min_stake: BigDecimal,
    pub(crate) max_stake: Option<BigDecimal>,
    pub(crate) status: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SecondsContractOrderInsert {
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) stake_asset: u64,
    pub(crate) direction: String,
    pub(crate) stake_amount: BigDecimal,
    pub(crate) duration_seconds: u32,
    pub(crate) payout_rate: BigDecimal,
    pub(crate) entry_price: BigDecimal,
    pub(crate) idempotency_key: String,
    pub(crate) expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct SecondsContractWalletLedgerWrite {
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) change_type: &'static str,
    pub(crate) amount: BigDecimal,
    pub(crate) available_after: BigDecimal,
    pub(crate) frozen_after: BigDecimal,
    pub(crate) locked_after: BigDecimal,
    pub(crate) ref_id: String,
}
