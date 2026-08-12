//! convert bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。

use bigdecimal::BigDecimal;
use chrono::Utc;

use crate::modules::convert::domain::{
    ConvertConfirmationInsert, ConvertQuoteCacheEntry, ConvertQuoteConfirmationRecord,
    ConvertRepositoryError, QuoteId,
};

pub trait ConvertQuoteRepository {
    /// 保存限时报价缓存快照，TTL 与 expires_at 必须表达同一过期边界。
    /// 实现失败返回仓储错误；不得把缺少缓存的报价伪装成可确认状态。
    fn save_quote_ttl(
        &mut self,
        entry: ConvertQuoteCacheEntry,
    ) -> Result<(), ConvertRepositoryError>;

    /// 按报价标识读取限时缓存；未命中返回空，存储或反序列化失败必须区分为错误。
    fn get_quote_ttl(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<ConvertQuoteCacheEntry>, ConvertRepositoryError>;
}

pub trait ConvertOrderRepository {
    /// 写入报价确认记录并以唯一键区分首次确认和重复确认。
    /// 实现必须让重放返回 Duplicate，禁止同一报价生成第二份订单或资金副作用。
    fn insert_quote_confirmation(
        &mut self,
        record: ConvertQuoteConfirmationRecord,
    ) -> Result<ConvertConfirmationInsert, ConvertRepositoryError>;
}

#[derive(Debug, Clone)]
pub struct ConvertQuoteInsert {
    pub quote_id: QuoteId,
    pub convert_pair_id: u64,
    pub user_id: u64,
    pub from_asset_id: u64,
    pub to_asset_id: u64,
    pub from_amount: BigDecimal,
    pub to_amount: BigDecimal,
    pub rate: BigDecimal,
    pub spread_rate: BigDecimal,
    pub fee_rate: BigDecimal,
    pub fee_amount: BigDecimal,
    pub expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertQuoteInsertResult {
    pub quote_row_id: u64,
    pub inserted: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ConvertPairRuleDbRecord {
    pub(crate) id: u64,
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) target_min_amount: BigDecimal,
    pub(crate) target_max_amount: Option<BigDecimal>,
    pub(crate) fixed_rate: Option<BigDecimal>,
    pub(crate) market_pair_symbol: Option<String>,
    pub(crate) market_base_asset_id: Option<u64>,
    pub(crate) market_quote_asset_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct ConvertPairRule {
    pub(crate) id: u64,
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) fixed_rate: Option<BigDecimal>,
    pub(crate) market_pair_symbol: Option<String>,
    pub(crate) market_base_asset_id: Option<u64>,
    pub(crate) market_quote_asset_id: Option<u64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct WalletBalanceRecord {
    pub(crate) available: BigDecimal,
    pub(crate) locked: BigDecimal,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ConvertSettlementOrderRecord {
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) from_amount: BigDecimal,
    pub(crate) to_amount: BigDecimal,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ConvertSettlementWalletRecord {
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
}
