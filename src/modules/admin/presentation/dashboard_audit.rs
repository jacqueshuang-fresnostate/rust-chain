//! 后台仪表盘聚合、强平记录与审计日志 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminMarginLiquidationQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) pair_id: Option<u64>,
    pub(crate) position_id: Option<u64>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminMarginLiquidationQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAuditLogsQuery {
    pub(crate) admin_id: Option<u64>,
    pub(crate) action: Option<String>,
    pub(crate) target_type: Option<String>,
    pub(crate) target_id: Option<String>,
    /// 审计发生时间下界（含），使用 Unix 毫秒，缺省时不限制最早时间。
    #[serde(default, with = "option_unix_millis")]
    pub(crate) created_from: Option<DateTime<Utc>>,
    /// 审计发生时间上界（含），使用 Unix 毫秒，缺省时不限制最晚时间。
    #[serde(default, with = "option_unix_millis")]
    pub(crate) created_to: Option<DateTime<Utc>>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminAuditLogsQuery {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminMarginLiquidationResponse {
    pub(crate) id: u64,
    pub(crate) position_id: u64,
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) margin_asset: u64,
    pub(crate) direction: String,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) notional_amount: BigDecimal,
    pub(crate) interest_amount: BigDecimal,
    pub(crate) entry_price: BigDecimal,
    pub(crate) mark_price: BigDecimal,
    pub(crate) maintenance_margin_rate: BigDecimal,
    pub(crate) equity: BigDecimal,
    pub(crate) maintenance_margin: BigDecimal,
    pub(crate) realized_pnl: BigDecimal,
    pub(crate) payout_amount: BigDecimal,
    pub(crate) reason: String,
    #[serde(with = "unix_millis")]
    pub(crate) liquidated_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminMarginLiquidationResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminMarginLiquidationsResponse {
    pub(crate) liquidations: Vec<AdminMarginLiquidationResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminMarginLiquidationsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAuditLogResponse {
    pub(crate) id: u64,
    pub(crate) admin_id: u64,
    pub(crate) action: String,
    pub(crate) target_type: String,
    pub(crate) target_id: String,
    pub(crate) before_json: Option<SqlxJson<Value>>,
    pub(crate) after_json: Option<SqlxJson<Value>>,
    pub(crate) reason: Option<String>,
    pub(crate) ip: Option<String>,
    pub(crate) request_id: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAuditLogResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAuditLogsResponse {
    pub(crate) logs: Vec<AdminAuditLogResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminAuditLogsResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDashboardResponse {
    #[serde(with = "unix_millis")]
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) environment: String,
    pub(crate) users: AdminDashboardUsersSummary,
    pub(crate) wallet: AdminDashboardWalletSummary,
    pub(crate) market: AdminDashboardMarketSummary,
    pub(crate) trading: AdminDashboardTradingSummary,
    pub(crate) products: AdminDashboardProductsSummary,
    pub(crate) risk: AdminDashboardRiskSummary,
    pub(crate) audit: AdminDashboardAuditSummary,
}

impl PresentationLayer for AdminDashboardResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardUsersSummary {
    pub(crate) total: i64,
    pub(crate) active: i64,
    pub(crate) new_24h: i64,
}

impl PresentationLayer for AdminDashboardUsersSummary {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardWalletSummary {
    pub(crate) active_assets: i64,
    pub(crate) wallet_accounts: i64,
    pub(crate) non_zero_accounts: i64,
    pub(crate) pending_unlocks: i64,
    pub(crate) pending_deposits: i64,
    pub(crate) pending_withdrawals: i64,
    pub(crate) custody_status: String,
}

impl PresentationLayer for AdminDashboardWalletSummary {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDashboardMarketSummary {
    pub(crate) active_pairs: i64,
    pub(crate) disabled_pairs: i64,
    pub(crate) external_pairs: i64,
    pub(crate) strategy_pairs: i64,
    pub(crate) feed_runtime_status: String,
    pub(crate) feed_needs_reload: bool,
    pub(crate) feed_symbols: Vec<String>,
    pub(crate) feed_providers: Vec<String>,
}

impl PresentationLayer for AdminDashboardMarketSummary {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardTradingSummary {
    pub(crate) spot_open_orders: i64,
    pub(crate) spot_trades_24h: i64,
    pub(crate) convert_pending_orders: i64,
    pub(crate) convert_completed_24h: i64,
}

impl PresentationLayer for AdminDashboardTradingSummary {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardProductsSummary {
    pub(crate) seconds_open_orders: i64,
    pub(crate) margin_open_positions: i64,
    pub(crate) margin_liquidated_24h: i64,
    pub(crate) earn_active_subscriptions: i64,
    pub(crate) earn_maturing_24h: i64,
}

impl PresentationLayer for AdminDashboardProductsSummary {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardRiskSummary {
    pub(crate) risk_events_24h: i64,
    pub(crate) blocked_events_24h: i64,
    pub(crate) pending_outbox_events: i64,
    pub(crate) retry_inbox_events: i64,
    pub(crate) dead_letter_inbox_events: i64,
}

impl PresentationLayer for AdminDashboardRiskSummary {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDashboardAuditSummary {
    pub(crate) admin_actions_24h: i64,
    pub(crate) latest_actions: Vec<AdminDashboardAuditAction>,
}

impl PresentationLayer for AdminDashboardAuditSummary {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardAuditAction {
    pub(crate) id: u64,
    pub(crate) admin_id: u64,
    pub(crate) action: String,
    pub(crate) target_type: String,
    pub(crate) target_id: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminDashboardAuditAction {}
