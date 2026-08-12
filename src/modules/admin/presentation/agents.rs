//! 后台代理层级、代理用户与佣金规则 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAgentRequest {
    pub(crate) user_id: u64,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) agent_code: String,
    pub(crate) admin_username: String,
    pub(crate) admin_password: Option<String>,
    pub(crate) admin_password_hash: Option<String>,
    pub(crate) level: Option<i32>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAgentRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAgentStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAgentStatusRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResetAgentPasswordRequest {
    pub(crate) password: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for ResetAgentPasswordRequest {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentPasswordResetResponse {
    pub(crate) agent_id: u64,
    pub(crate) admin_user_id: u64,
    pub(crate) admin_username: String,
    pub(crate) requires_relogin: bool,
}

impl PresentationLayer for AdminAgentPasswordResetResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AssignUserAgentRequest {
    pub(crate) agent_id: u64,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for AssignUserAgentRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAgentUsersQuery {
    /// 用于限制返回团队成员数量，保持接口分页行为一致。
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminAgentUsersQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAgentQuery {
    pub(crate) agent_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) level: Option<i32>,
    pub(crate) agent_code: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminAgentQuery {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAgentResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) email: Option<String>,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) parent_agent_code: Option<String>,
    pub(crate) root_agent_id: u64,
    pub(crate) root_agent_code: String,
    pub(crate) agent_code: String,
    pub(crate) level: i32,
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) direct_user_count: i64,
    pub(crate) team_user_count: i64,
    pub(crate) child_agent_count: i64,
    pub(crate) admin_user_id: Option<u64>,
    pub(crate) admin_username: Option<String>,
    pub(crate) admin_status: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAgentResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAgentUserResponse {
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
    pub(crate) path: String,
    #[serde(with = "unix_millis")]
    pub(crate) referred_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAgentUserResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminUserReferralResponse {
    pub(crate) user_id: u64,
    pub(crate) direct_inviter_id: Option<u64>,
    pub(crate) direct_inviter_type: Option<String>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) depth: i32,
    pub(crate) path: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminUserReferralResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentsResponse {
    pub(crate) agents: Vec<AdminAgentResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminAgentsResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentUsersResponse {
    pub(crate) users: Vec<AdminAgentUserResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminAgentUsersResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAgentCommissionStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAgentCommissionStatusRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAgentCommissionRuleRequest {
    pub(crate) agent_id: u64,
    pub(crate) product_type: String,
    pub(crate) commission_rate: BigDecimal,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAgentCommissionRuleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAgentCommissionRuleRequest {
    pub(crate) commission_rate: Option<BigDecimal>,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAgentCommissionRuleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAgentCommissionQuery {
    pub(crate) agent_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminAgentCommissionQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAgentCommissionRuleQuery {
    pub(crate) agent_id: Option<u64>,
    pub(crate) product_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminAgentCommissionRuleQuery {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAgentCommissionResponse {
    pub(crate) id: u64,
    pub(crate) agent_id: u64,
    pub(crate) user_id: u64,
    pub(crate) source_type: String,
    pub(crate) source_id: String,
    pub(crate) source_amount: BigDecimal,
    pub(crate) payout_asset_id: Option<u64>,
    pub(crate) commission_rate: BigDecimal,
    pub(crate) commission_amount: BigDecimal,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAgentCommissionResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAgentCommissionRuleResponse {
    pub(crate) id: u64,
    pub(crate) agent_id: u64,
    pub(crate) product_type: String,
    pub(crate) commission_rate: BigDecimal,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAgentCommissionRuleResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentCommissionsResponse {
    pub(crate) commissions: Vec<AdminAgentCommissionResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminAgentCommissionsResponse {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BatchUpdateAgentCommissionStatusRequest {
    pub(crate) ids: Vec<u64>,
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for BatchUpdateAgentCommissionStatusRequest {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentCommissionBatchStatusItemResponse {
    pub(crate) id: u64,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
}

impl PresentationLayer for AdminAgentCommissionBatchStatusItemResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentCommissionBatchStatusResponse {
    pub(crate) results: Vec<AdminAgentCommissionBatchStatusItemResponse>,
}

impl PresentationLayer for AdminAgentCommissionBatchStatusResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentCommissionRulesResponse {
    pub(crate) rules: Vec<AdminAgentCommissionRuleResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminAgentCommissionRulesResponse {}
