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

/// 管理后台产品列表查询参数。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminMarginProductsQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

/// 管理后台资金费汇总查询参数。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminInterestSummaryQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) pair_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

/// 管理后台仓位列表查询参数。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminListPositionsQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) pair_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

/// 关闭/取消仓位的可选产品过滤参数。
#[derive(Debug, Deserialize)]
pub(crate) struct ProductActionRequest {
    pub(crate) product_id: Option<u64>,
}

/// 单仓主动平仓请求；空对象保持历史 100% 全平语义，显式比例必须同时携带幂等键。
#[derive(Debug, Default, Deserialize)]
pub(crate) struct CloseMarginPositionRequest {
    /// 作用于事务内当前剩余仓位的整数百分比，合法区间由应用层统一校验为 1..=100。
    pub(crate) percentage: Option<i64>,
    /// 显式部分/全平意图的用户级幂等键；历史空请求不要求该字段。
    pub(crate) idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenMarginPositionRequest {
    pub(crate) product_id: u64,
    pub(crate) direction: String,
    // 缺省为 market 以兼容历史 PC 调用；limit 必须同时携带正数 price。
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
    pub(crate) price_precision: i32,
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
pub(crate) struct AdminMarginProductsResponse {
    pub(crate) products: Vec<MarginProductResponse>,
    pub(crate) capabilities: MarginTradingCapabilitiesResponse,
    pub(crate) total: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginTradingCapabilitiesResponse {
    pub(crate) order_types: Vec<String>,
    pub(crate) margin_modes: Vec<String>,
    /// 是否已实现止盈止损委托；当前仅声明能力，不允许客户端自行假设支持。
    pub(crate) take_profit_stop_loss: bool,
    /// 是否已实现策略委托；false 时客户端必须展示不可用状态而不是模拟订单。
    pub(crate) strategy_orders: bool,
    /// 是否支持按当前用户或产品批量平掉已成交仓位。
    pub(crate) bulk_close: bool,
    /// 是否提供按最新服务端行情计算的单仓风险快照。
    pub(crate) position_risk: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginPositionsResponse {
    pub(crate) positions: Vec<MarginPositionResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarginWalletsResponse {
    pub(crate) wallets: Vec<MarginWalletAccountResponse>,
    pub(crate) positions: Vec<MarginPositionResponse>,
    pub(crate) cross_accounts: Vec<MarginCrossAccountResponse>,
}

/// 用户当前保证金资产对应的全仓风险快照。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct MarginCrossAccountResponse {
    pub(crate) margin_asset: u64,
    pub(crate) status: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) equity: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) unrealized_pnl: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) interest_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) maintenance_margin: BigDecimal,
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) margin_ratio: Option<BigDecimal>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct MarginWalletAccountResponse {
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) margin_transfer_enabled: bool,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) available: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) frozen: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) locked: BigDecimal,
    /// 当前服务端风险快照允许从 margin 转回 spot 的上限，不能由 available 替代。
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) max_transferable_to_spot: BigDecimal,
    /// 上限为零时的稳定拒绝原因；无拒绝时为 null。
    pub(crate) transfer_to_spot_block_reason: Option<String>,
    /// 计算上限时读取的全仓账户版本，提交时会在锁内再次核对。
    pub(crate) cross_account_version: Option<u64>,
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) transfer_risk_equity: Option<BigDecimal>,
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) transfer_risk_maintenance_margin: Option<BigDecimal>,
    #[serde(
        default,
        with = "option_unix_millis",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) transfer_risk_observed_at: Option<DateTime<Utc>>,
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
    /// 显式比例请求对应的不可变结算执行；历史空请求保持 null。
    pub(crate) execution: Option<MarginPositionCloseExecutionResponse>,
    /// 本次执行真实应用到钱包的增量；重放沿用原执行值，历史终态重放为 null。
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) settlement_amount: Option<BigDecimal>,
    /// 是否命中既有幂等执行；true 时本次请求没有产生资金或仓位写入。
    pub(crate) replayed: bool,
}

/// 一次显式部分或全量平仓的不可变审计结果，全部金额来自同一事务内的服务端计算。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct MarginPositionCloseExecutionResponse {
    pub(crate) id: u64,
    pub(crate) position_id: u64,
    pub(crate) idempotency_key: String,
    pub(crate) close_percentage: u16,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) close_margin_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) close_notional_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) close_borrowed_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) close_interest_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) exit_price: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) realized_pnl: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) settlement_amount: BigDecimal,
    pub(crate) fully_closed: bool,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
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
    pub(crate) order_type: String,
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
    pub(crate) limit_price: Option<BigDecimal>,
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
    pub(crate) total: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminInterestSummaryResponse {
    pub(crate) summaries: Vec<AdminInterestSummaryItem>,
    pub(crate) total: i64,
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
    pub(crate) order_type: String,
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
    pub(crate) limit_price: Option<BigDecimal>,
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
    /// 按标记价计算的浮动盈亏；这是页面展示应使用的准确业务名称。
    pub(crate) unrealized_pnl: BigDecimal,
    /// 兼容旧客户端的历史字段名，值与 `unrealized_pnl` 完全相同。
    pub(crate) realized_pnl: BigDecimal,
    pub(crate) equity: BigDecimal,
    pub(crate) maintenance_margin: BigDecimal,
    /// 名义价值除以入场价得到的基础资产持仓数量。
    pub(crate) position_quantity: BigDecimal,
    /// 浮动盈亏除以投入保证金；分母无效时返回 null。
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) return_rate: Option<BigDecimal>,
    /// 当前权益除以维持保证金；维持保证金为零时返回 null。
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) margin_ratio: Option<BigDecimal>,
    /// 逐仓模式下按当前产品维持保证金率估算的强平价格；全仓返回 null。
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) estimated_liquidation_price: Option<BigDecimal>,
    /// 标记价到预估强平价的绝对距离占标记价比例；无独立强平价时返回 null。
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) liquidation_distance_rate: Option<BigDecimal>,
    pub(crate) should_liquidate: bool,
    #[serde(with = "unix_millis")]
    pub(crate) observed_at: DateTime<Utc>,
    /// 全仓模式的权威账户级风险快照；逐仓响应不序列化该字段以保持旧结构。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cross_account_risk: Option<MarginCrossAccountRisk>,
}

/// 按 `(user_id, margin_asset)` 实时重算的全仓账户风险，所有小数在 HTTP 边界保持十八位字符串。
#[derive(Debug, Serialize)]
pub(crate) struct MarginCrossAccountRisk {
    /// 账户共享的保证金资产主键。
    pub(crate) margin_asset: u64,
    /// 当前持仓卡所属 pair，只它的共享标记价在条件求根中变化。
    pub(crate) reference_pair_id: u64,
    /// 稳定假设值，明确其他 pair 保持当前标记价。
    pub(crate) price_assumption: &'static str,
    /// 杠杆钱包 available、所有仓位保证金、浮盈和利息汇总后的账户权益。
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) equity: BigDecimal,
    /// 账户全部已成交全仓仓位的静态名义维持保证金之和。
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) maintenance_margin: BigDecimal,
    /// 账户权益减维持保证金，是条件强平价公式中的 Buffer。
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) liquidation_buffer: BigDecimal,
    /// 账户权益除以维持保证金；分母为零时是 null。
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) margin_ratio: Option<BigDecimal>,
    /// 所有账户仓位按各自唯一 pair 当前标记价计算的浮动盈亏之和。
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) unrealized_pnl: BigDecimal,
    /// 所有账户仓位已计提未结算利息之和。
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) interest_amount: BigDecimal,
    /// 当前权益是否已不高于维持保证金。
    pub(crate) should_liquidate: bool,
    /// 参考 pair 按 long 正、short 负汇总的基础资产数量。
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) net_quantity: BigDecimal,
    /// 参考 pair 不抵消方向的基础资产总数量。
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) gross_quantity: BigDecimal,
    /// 条件价是否可稳定求解的 snake_case 状态码。
    pub(crate) estimate_status: &'static str,
    /// 按 `P*=P0-Buffer/D` 求根并保守圆整的正条件强平价，不可解时为 null。
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) conditional_liquidation_price: Option<BigDecimal>,
    /// 条件价与参考 pair 当前标记价的绝对距离比例。
    #[serde(default, serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) conditional_liquidation_distance_rate: Option<BigDecimal>,
    /// 该完整账户快照中最早的行情观测时间。
    #[serde(with = "unix_millis")]
    pub(crate) marks_observed_at_min: DateTime<Utc>,
    /// 该完整账户快照中最晚的行情观测时间。
    #[serde(with = "unix_millis")]
    pub(crate) marks_observed_at_max: DateTime<Utc>,
}

fn serialize_decimal_amount<S>(amount: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{amount:.18}"))
}

fn serialize_optional_decimal_amount<S>(
    amount: &Option<BigDecimal>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match amount {
        Some(value) => serializer.serialize_some(&format!("{value:.18}")),
        None => serializer.serialize_none(),
    }
}
