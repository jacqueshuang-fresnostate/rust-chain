//! margin bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 用户侧 margin 动作的请求/响应结构集中放在这里，避免路由层继续定义传输 DTO。

use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer};
use sqlx::types::Json as SqlxJson;

/// margin 列表查询参数。
#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    pub(crate) limit: Option<u32>,
}

/// 用户仓位列表查询参数。
#[derive(Debug, Deserialize)]
pub(crate) struct ListPositionsQuery {
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

/// 管理后台资金费汇总查询参数。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminInterestSummaryQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) pair_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

/// 管理后台仓位列表查询参数。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminListPositionsQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) pair_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

/// 关闭/取消仓位的可选产品过滤参数。
#[derive(Debug, Deserialize)]
pub(crate) struct ProductActionRequest {
    pub(crate) product_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenMarginPositionRequest {
    pub(crate) product_id: u64,
    pub(crate) direction: String,
    // 当前逐仓引擎只会按服务端最新行情即时开仓；保留字段以明确拒绝旧端限价语义。
    pub(crate) order_type: Option<String>,
    pub(crate) price: Option<BigDecimal>,
    pub(crate) trigger_price: Option<BigDecimal>,
    pub(crate) margin_mode: Option<String>,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) leverage: BigDecimal,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenMarginPositionResponse {
    pub(crate) position: MarginPositionResponse,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TransferMarginFundsRequest {
    pub(crate) asset_id: Option<u64>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) amount: BigDecimal,
    pub(crate) idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateUserLeverageRequest {
    pub(crate) leverage: BigDecimal,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateUserMarginModeRequest {
    pub(crate) margin_mode: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateMarginProductRequest {
    pub(crate) pair_id: u64,
    pub(crate) margin_asset: u64,
    pub(crate) logo_url: Option<String>,
    pub(crate) margin_mode: Option<String>,
    pub(crate) margin_modes: Option<Vec<String>>,
    pub(crate) leverage_levels: Option<Vec<BigDecimal>>,
    pub(crate) max_leverage: BigDecimal,
    pub(crate) min_margin: BigDecimal,
    pub(crate) max_margin: Option<BigDecimal>,
    pub(crate) maintenance_margin_rate: BigDecimal,
    pub(crate) hourly_interest_rate: Option<BigDecimal>,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMarginProductRequest {
    pub(crate) pair_id: u64,
    pub(crate) margin_asset: u64,
    pub(crate) logo_url: Option<String>,
    pub(crate) margin_mode: Option<String>,
    pub(crate) margin_modes: Option<Vec<String>>,
    pub(crate) leverage_levels: Option<Vec<BigDecimal>>,
    pub(crate) max_leverage: BigDecimal,
    pub(crate) min_margin: BigDecimal,
    pub(crate) max_margin: Option<BigDecimal>,
    pub(crate) maintenance_margin_rate: BigDecimal,
    pub(crate) hourly_interest_rate: Option<BigDecimal>,
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMarginProductStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TransferMarginFundsResponse {
    pub(crate) transfer_id: String,
    pub(crate) spot_wallet: MarginWalletAccountSnapshot,
    pub(crate) margin_wallet: MarginWalletAccountSnapshot,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct MarginProductResponse {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) margin_asset: u64,
    pub(crate) margin_asset_symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) margin_mode: String,
    pub(crate) margin_modes: SqlxJson<Vec<String>>,
    pub(crate) leverage_levels: SqlxJson<Vec<String>>,
    pub(crate) max_leverage: BigDecimal,
    pub(crate) min_margin: BigDecimal,
    pub(crate) max_margin: Option<BigDecimal>,
    pub(crate) maintenance_margin_rate: BigDecimal,
    pub(crate) hourly_interest_rate: BigDecimal,
    pub(crate) status: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginProductsResponse {
    pub(crate) products: Vec<MarginProductResponse>,
    pub(crate) capabilities: MarginTradingCapabilitiesResponse,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginTradingCapabilitiesResponse {
    pub(crate) order_types: Vec<String>,
    pub(crate) margin_modes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginPositionsResponse {
    pub(crate) positions: Vec<MarginPositionResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginWalletsResponse {
    pub(crate) wallets: Vec<MarginWalletAccountResponse>,
    pub(crate) positions: Vec<MarginPositionResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct MarginWalletAccountResponse {
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) available: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) frozen: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) locked: BigDecimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct MarginWalletAccountSnapshot {
    pub(crate) asset_id: u64,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) available: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) frozen: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) locked: BigDecimal,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginUserSettingResponse {
    pub(crate) product_id: u64,
    pub(crate) margin_mode: Option<String>,
    pub(crate) leverage: Option<BigDecimal>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CloseMarginPositionResponse {
    pub(crate) position: MarginPositionResponse,
}

#[derive(Debug, Serialize)]
pub(crate) struct CloseAllMarginPositionsResponse {
    pub(crate) positions: Vec<MarginPositionResponse>,
    pub(crate) failures: Vec<MarginBatchActionFailure>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CancelMarginPositionResponse {
    pub(crate) position: MarginPositionResponse,
}

#[derive(Debug, Serialize)]
pub(crate) struct CancelAllMarginPositionsResponse {
    pub(crate) positions: Vec<MarginPositionResponse>,
    pub(crate) failures: Vec<MarginBatchActionFailure>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginBatchActionFailure {
    pub(crate) id: u64,
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct MarginPositionResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) margin_asset: u64,
    pub(crate) wallet_scope: String,
    pub(crate) margin_mode: String,
    pub(crate) direction: String,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) leverage: BigDecimal,
    pub(crate) notional_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) borrowed_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) interest_amount: BigDecimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) entry_price: Option<BigDecimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) exit_price: Option<BigDecimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) realized_pnl: Option<BigDecimal>,
    #[serde(
        default,
        with = "option_unix_millis",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) closed_at: Option<DateTime<Utc>>,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginPositionDetailResponse {
    pub(crate) position: MarginPositionResponse,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminMarginPositionsResponse {
    pub(crate) positions: Vec<AdminMarginPositionResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminInterestSummaryResponse {
    pub(crate) summaries: Vec<AdminInterestSummaryItem>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminInterestSummaryItem {
    pub(crate) margin_asset: u64,
    pub(crate) status: String,
    pub(crate) position_count: i64,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) borrowed_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) interest_amount: BigDecimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminMarginPositionResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) margin_asset: u64,
    pub(crate) wallet_scope: String,
    pub(crate) margin_mode: String,
    pub(crate) direction: String,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) leverage: BigDecimal,
    pub(crate) notional_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) borrowed_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) interest_amount: BigDecimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) entry_price: Option<BigDecimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) exit_price: Option<BigDecimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) realized_pnl: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) closed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) liquidated_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) liquidation_reason: Option<String>,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginRiskSnapshotResponse {
    pub(crate) risk: MarginRiskSnapshot,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginRiskSnapshot {
    pub(crate) position_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) margin_asset: u64,
    pub(crate) direction: String,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) notional_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) interest_amount: BigDecimal,
    pub(crate) entry_price: BigDecimal,
    pub(crate) mark_price: BigDecimal,
    pub(crate) maintenance_margin_rate: BigDecimal,
    pub(crate) realized_pnl: BigDecimal,
    pub(crate) equity: BigDecimal,
    pub(crate) maintenance_margin: BigDecimal,
    pub(crate) should_liquidate: bool,
    #[serde(with = "unix_millis")]
    pub(crate) observed_at: DateTime<Utc>,
}

fn serialize_decimal_amount<S>(amount: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{amount:.18}"))
}
