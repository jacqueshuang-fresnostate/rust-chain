//! earn bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。

use crate::architecture::RepositoryLayer;
use bigdecimal::BigDecimal;
use serde_json::Value;

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct EarnProductRuleRow {
    pub(crate) id: u64,
    pub(crate) asset_id: u64,
    pub(crate) term_days: u32,
    pub(crate) apr_rate: BigDecimal,
    pub(crate) redemption_fee_rate: BigDecimal,
    pub(crate) maturity_profit_fee_rate: BigDecimal,
    pub(crate) early_redeem_fee_basis: String,
    pub(crate) early_redeem_fee_rate: BigDecimal,
    pub(crate) min_subscribe: BigDecimal,
    pub(crate) max_subscribe: Option<BigDecimal>,
    pub(crate) status: String,
}

#[derive(Debug, Clone)]
pub(crate) struct EarnProductFeeConfig {
    pub(crate) redemption_fee_rate: BigDecimal,
    pub(crate) maturity_profit_fee_rate: BigDecimal,
    pub(crate) early_redeem_fee_basis: String,
    pub(crate) early_redeem_fee_rate: BigDecimal,
}

#[derive(Debug, Clone)]
pub(crate) struct EarnCategoryWrite {
    pub(crate) code: String,
    pub(crate) name_json: Value,
    pub(crate) sort_order: i32,
    pub(crate) status: String,
}

#[derive(Debug, Clone)]
pub(crate) struct EarnProductWrite {
    pub(crate) asset_id: u64,
    pub(crate) name: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) introduction_json: Value,
    pub(crate) term_days: u32,
    pub(crate) apr_rate: BigDecimal,
    pub(crate) redemption_fee_rate: BigDecimal,
    pub(crate) maturity_profit_fee_rate: BigDecimal,
    pub(crate) early_redeem_fee_basis: String,
    pub(crate) early_redeem_fee_rate: BigDecimal,
    pub(crate) min_subscribe: BigDecimal,
    pub(crate) max_subscribe: Option<BigDecimal>,
    pub(crate) status: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct EarnWalletRow {
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
}
