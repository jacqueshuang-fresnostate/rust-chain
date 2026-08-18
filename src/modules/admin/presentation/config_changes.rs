//! 高风险配置变更申请的管理端请求与响应合同。

use super::*;
use crate::modules::admin::repository::AdminConfigChangeRecord;

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAdminConfigChangeRequest {
    pub(crate) config_domain: String,
    pub(crate) target_type: String,
    pub(crate) target_id: String,
    pub(crate) action: String,
    pub(crate) base_revision: Option<u64>,
    pub(crate) before_json: Option<Value>,
    pub(crate) proposed_json: Value,
    pub(crate) reason: Option<String>,
    pub(crate) risk_level: Option<String>,
}

impl PresentationLayer for CreateAdminConfigChangeRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct ReviewAdminConfigChangeRequest {
    pub(crate) decision: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for ReviewAdminConfigChangeRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct ApplyAdminConfigChangeRequest {
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for ApplyAdminConfigChangeRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminConfigChangeQuery {
    pub(crate) config_domain: Option<String>,
    pub(crate) target_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) created_by: Option<u64>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminConfigChangeQuery {}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AdminConfigChangeResponse {
    pub(crate) id: u64,
    pub(crate) request_no: String,
    pub(crate) config_domain: String,
    pub(crate) target_type: String,
    pub(crate) target_id: String,
    pub(crate) action: String,
    pub(crate) base_revision: Option<u64>,
    pub(crate) before_json: Option<Value>,
    pub(crate) proposed_json: Value,
    pub(crate) reason: String,
    pub(crate) risk_level: String,
    pub(crate) status: String,
    pub(crate) created_by: u64,
    pub(crate) reviewed_by: Option<u64>,
    pub(crate) review_reason: Option<String>,
    pub(crate) applied_by: Option<u64>,
    #[serde(with = "option_unix_millis")]
    pub(crate) reviewed_at: Option<DateTime<Utc>>,
    #[serde(with = "option_unix_millis")]
    pub(crate) applied_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminConfigChangeResponse {}

impl From<AdminConfigChangeRecord> for AdminConfigChangeResponse {
    fn from(record: AdminConfigChangeRecord) -> Self {
        Self {
            id: record.id,
            request_no: record.request_no,
            config_domain: record.config_domain,
            target_type: record.target_type,
            target_id: record.target_id,
            action: record.action,
            base_revision: record.base_revision,
            before_json: record.before_json,
            proposed_json: record.proposed_json,
            reason: record.reason,
            risk_level: record.risk_level,
            status: record.status,
            created_by: record.created_by,
            reviewed_by: record.reviewed_by,
            review_reason: record.review_reason,
            applied_by: record.applied_by,
            reviewed_at: record.reviewed_at,
            applied_at: record.applied_at,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminConfigChangesResponse {
    pub(crate) requests: Vec<AdminConfigChangeResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminConfigChangesResponse {}
