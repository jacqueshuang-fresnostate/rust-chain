//! agent bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

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

#[derive(Debug, Deserialize)]
pub(crate) struct CreateInviteCodeRequest {
    pub(crate) usage_limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateInviteCodeStatusRequest {
    pub(crate) status: String,
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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AgentDashboardResponse {
    pub(crate) agent_id: u64,
    pub(crate) team_user_count: i64,
    pub(crate) active_invite_code_count: i64,
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
