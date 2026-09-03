//! agent bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。
//! 代理端对外结构在此统一定义：时间列一律序列化为 Unix 毫秒时间戳，金额保持十进制不转浮点，
//! 多数响应体同时派生 sqlx 行映射，字段名与查询别名严格对应，改名会同时影响 SQL 与接口契约。

use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AgentMeResponse {
    pub(crate) agent_admin_id: u64,
    pub(crate) agent_id: u64,
    pub(crate) username: String,
    pub(crate) agent_code: String,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: u64,
    pub(crate) level: i32,
    pub(crate) path: String,
    pub(crate) agent_status: String,
    pub(crate) admin_status: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentUsersResponse {
    pub(crate) users: Vec<AgentTeamUserResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AgentTeamUserResponse {
    pub(crate) user_id: u64,
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) status: String,
    pub(crate) kyc_level: i32,
    pub(crate) owner_agent_id: u64,
    // 兼容旧客户端：该字段历史上表示直属归属代理，而不是总代理。
    pub(crate) root_agent_id: u64,
    pub(crate) owner_agent_code: String,
    pub(crate) owner_agent_level: i32,
    pub(crate) direct_inviter_id: Option<u64>,
    pub(crate) direct_inviter_type: Option<String>,
    pub(crate) depth: i32,
    #[serde(with = "unix_millis")]
    pub(crate) referred_at: DateTime<Utc>,
}

/// 代理端团队用户资产分页响应，现货与杠杆账户通过 `account_type` 显式区分。
#[derive(Debug, Serialize)]
pub(crate) struct AgentUserAssetsResponse {
    pub(crate) assets: Vec<AgentUserAssetResponse>,
    pub(crate) total: i64,
}

/// 代理端可见的单条钱包账户快照，金额保持数据库 Decimal 文本精度。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AgentUserAssetResponse {
    pub(crate) account_id: u64,
    pub(crate) account_type: String,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

/// 团队用户杠杆仓位分页响应，`total` 与当前状态筛选使用同一谓词。
#[derive(Debug, Serialize)]
pub(crate) struct AgentUserMarginPositionsResponse {
    pub(crate) positions: Vec<AgentUserMarginPositionResponse>,
    pub(crate) total: i64,
}

/// 代理端只读杠杆仓位快照，不包含任何需要行情回填才能计算的派生值。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AgentUserMarginPositionResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) margin_asset: u64,
    pub(crate) margin_asset_symbol: String,
    pub(crate) wallet_scope: String,
    pub(crate) margin_mode: String,
    pub(crate) direction: String,
    pub(crate) order_type: String,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) leverage: BigDecimal,
    pub(crate) notional_amount: BigDecimal,
    pub(crate) borrowed_amount: BigDecimal,
    pub(crate) interest_amount: BigDecimal,
    pub(crate) entry_price: Option<BigDecimal>,
    pub(crate) limit_price: Option<BigDecimal>,
    pub(crate) exit_price: Option<BigDecimal>,
    pub(crate) realized_pnl: Option<BigDecimal>,
    #[serde(with = "unix_millis")]
    pub(crate) opened_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) closed_at: Option<DateTime<Utc>>,
    pub(crate) status: String,
}

/// 团队用户秒合约订单分页响应，缺省查询同时包含进行中与全部终态。
#[derive(Debug, Serialize)]
pub(crate) struct AgentUserSecondsContractOrdersResponse {
    pub(crate) orders: Vec<AgentUserSecondsContractOrderResponse>,
    pub(crate) total: i64,
}

/// 代理端只读秒合约订单快照，本金、赔率和价格均以 Decimal 文本跨越 JSON 边界。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AgentUserSecondsContractOrderResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) stake_asset: u64,
    pub(crate) stake_asset_symbol: String,
    pub(crate) direction: String,
    pub(crate) stake_amount: BigDecimal,
    pub(crate) duration_seconds: u32,
    pub(crate) payout_rate: BigDecimal,
    pub(crate) entry_price: Option<BigDecimal>,
    pub(crate) settlement_price: Option<BigDecimal>,
    pub(crate) status: String,
    pub(crate) result: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) expires_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) settled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AgentListQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AgentUserMarginPositionsQuery {
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AgentUserSecondsContractOrdersQuery {
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateInviteCodeRequest {
    pub(crate) usage_limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateInviteCodeStatusRequest {
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangeAgentPasswordRequest {
    pub(crate) current_password: Option<String>,
    pub(crate) new_password: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentPasswordChangeResponse {
    pub(crate) changed: bool,
    pub(crate) requires_relogin: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentInviteCodesResponse {
    pub(crate) invite_codes: Vec<AgentInviteCodeResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AgentInviteCodeResponse {
    pub(crate) id: u64,
    pub(crate) owner_id: u64,
    pub(crate) code: String,
    pub(crate) usage_limit: Option<i32>,
    pub(crate) used_count: i32,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentTeamTreeResponse {
    pub(crate) root_agent_id: u64,
    pub(crate) agents: Vec<AgentSubAgentResponse>,
    pub(crate) nodes: Vec<AgentTeamTreeNodeResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentSubAgentsResponse {
    pub(crate) agents: Vec<AgentSubAgentResponse>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct AgentSubAgentResponse {
    pub(crate) id: u64,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: u64,
    pub(crate) agent_code: String,
    pub(crate) level: i32,
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) direct_user_count: i64,
    pub(crate) team_user_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AgentTeamTreeNodeResponse {
    pub(crate) user_id: u64,
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) status: String,
    pub(crate) direct_inviter_id: Option<u64>,
    pub(crate) direct_inviter_type: Option<String>,
    pub(crate) owner_agent_id: u64,
    pub(crate) owner_agent_code: String,
    pub(crate) owner_agent_level: i32,
    pub(crate) depth: i32,
    pub(crate) path: String,
    #[serde(with = "unix_millis")]
    pub(crate) referred_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentCommissionsResponse {
    pub(crate) agent_id: u64,
    pub(crate) total_records: u64,
    pub(crate) total_commission_amount: BigDecimal,
    pub(crate) commissions: Vec<AgentCommissionResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AgentCommissionResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) email: Option<String>,
    pub(crate) source_type: String,
    pub(crate) source_id: String,
    pub(crate) source_amount: BigDecimal,
    pub(crate) commission_rate: BigDecimal,
    pub(crate) commission_amount: BigDecimal,
    pub(crate) status: String,
    pub(crate) depth: i32,
    pub(crate) payout_ledger_id: Option<u64>,
    pub(crate) payout_asset_id: Option<u64>,
    pub(crate) payout_amount: Option<BigDecimal>,
    pub(crate) payout_balance_after: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) payout_created_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentDashboardResponse {
    pub(crate) agent_id: u64,
    pub(crate) team_user_count: i64,
    pub(crate) active_invite_code_count: i64,
    pub(crate) commission_record_count: i64,
    // 顶层金额只在全部佣金使用同一发放资产时保留，跨资产金额以 commission_assets 为准。
    pub(crate) pending_commission_amount: BigDecimal,
    pub(crate) settled_commission_amount: BigDecimal,
    pub(crate) total_commission_amount: BigDecimal,
    pub(crate) commission_assets: Vec<AgentDashboardAssetSummaryResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AgentDashboardAssetSummaryResponse {
    pub(crate) payout_asset_id: Option<u64>,
    pub(crate) commission_record_count: i64,
    pub(crate) pending_commission_amount: BigDecimal,
    pub(crate) settled_commission_amount: BigDecimal,
    pub(crate) total_commission_amount: BigDecimal,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentConvertStatsResponse {
    pub(crate) agent_id: u64,
    pub(crate) total_orders: i64,
    pub(crate) pending_orders: i64,
    pub(crate) completed_orders: i64,
    pub(crate) total_from_amount: BigDecimal,
    pub(crate) total_to_amount: BigDecimal,
}
