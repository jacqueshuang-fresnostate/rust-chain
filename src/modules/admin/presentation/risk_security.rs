//! 后台风控规则、风险事件与安全策略 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminRiskRuleQuery {
    pub(crate) rule_type: Option<String>,
    pub(crate) target_type: Option<String>,
    pub(crate) enabled: Option<bool>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminRiskRuleQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminRiskEventQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) decision: Option<String>,
    pub(crate) risk_level: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminRiskEventQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateRiskRuleRequest {
    pub(crate) rule_type: String,
    pub(crate) target_type: String,
    pub(crate) target_id: Option<String>,
    pub(crate) config_json: Value,
    pub(crate) enabled: Option<bool>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateRiskRuleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateRiskRuleStatusRequest {
    pub(crate) enabled: bool,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateRiskRuleStatusRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct RiskRuleResponse {
    pub(crate) id: u64,
    pub(crate) rule_type: String,
    pub(crate) target_type: String,
    pub(crate) target_id: Option<String>,
    pub(crate) config_json: SqlxJson<Value>,
    pub(crate) enabled: bool,
    pub(crate) created_by: Option<u64>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for RiskRuleResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct RiskEventResponse {
    pub(crate) id: u64,
    pub(crate) user_id: Option<u64>,
    pub(crate) actor_type: String,
    pub(crate) actor_id: Option<u64>,
    pub(crate) event_type: String,
    pub(crate) risk_level: String,
    pub(crate) decision: String,
    pub(crate) reason: Option<String>,
    pub(crate) payload_json: SqlxJson<Value>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for RiskEventResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct RiskRulesResponse {
    pub(crate) rules: Vec<RiskRuleResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for RiskRulesResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct RiskEventsResponse {
    pub(crate) events: Vec<RiskEventResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for RiskEventsResponse {}
