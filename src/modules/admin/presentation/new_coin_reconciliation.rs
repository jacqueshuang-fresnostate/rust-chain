//! 新币派发与退款批次对账响应 DTO。
//!
//! 对账是只读的项目级批次视图：把发行供给、申购、派发、钱包流水和人工退款放在同一份
//! 快照中，前端可以据此定位差额而不需要自行拼接多个分页接口。金额继续使用
//! `BigDecimal`，时间沿用后台统一的 Unix 毫秒格式。

use super::*;

/// 新币项目派发/退款批次的完整对账结果。
///
/// `*_delta` 字段均按“左侧事实减右侧事实”计算；数值为零表示该项守恒。
/// `status` 为 `balanced` 或 `attention`，后者表示至少存在一项需要人工核查的异常。
#[derive(Debug, Serialize)]
pub(crate) struct NewCoinReconciliationResponse {
    pub(crate) project_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) reserved_supply: BigDecimal,
    pub(crate) allocated_supply: BigDecimal,
    pub(crate) remaining_supply: BigDecimal,
    pub(crate) supply_delta: BigDecimal,
    pub(crate) subscription_count: i64,
    pub(crate) pending_manual_count: i64,
    pub(crate) requested_quantity: BigDecimal,
    pub(crate) subscription_allocated_quantity: BigDecimal,
    pub(crate) distribution_quantity: BigDecimal,
    pub(crate) linked_distribution_quantity: BigDecimal,
    pub(crate) unlinked_distribution_quantity: BigDecimal,
    pub(crate) distribution_ledger_quantity: BigDecimal,
    pub(crate) invalid_subscription_link_count: i64,
    pub(crate) subscription_distribution_delta: BigDecimal,
    pub(crate) distribution_ledger_delta: BigDecimal,
    pub(crate) manual_quote_amount: BigDecimal,
    pub(crate) manual_frozen_quote_amount: BigDecimal,
    pub(crate) manual_settled_quote_amount: BigDecimal,
    pub(crate) manual_refunded_quote_amount: BigDecimal,
    pub(crate) manual_quote_delta: BigDecimal,
    pub(crate) anomaly_count: i64,
    pub(crate) anomalies: Vec<String>,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) checked_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinReconciliationResponse {}
