//! 人工采集与追加跟进传输合同；不接受客户端财务证据，不提供日结或资金修复命令。

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// 采集只接受资产、原因和原始幂等键；证据必须由服务器同一快照重新读取。
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CaptureRequest {
    pub asset_id: u64,
    pub reason: String,
    pub idempotency_key: String,
}

/// 资产过滤与有界分页；历史读取不重新计算保存证据。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SnapshotQuery {
    pub asset_id: Option<u64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// 跟进历史与采集历史独立分页，最新版本不会从当前页条数推断。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FollowupQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// 每次跟进明确提交完整负责人/期限和读取版本；null 为显式清空，遗漏字段拒绝。
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FollowupRequest {
    pub expected_version: u64,
    #[serde(deserialize_with = "required_nullable")]
    #[schema(required = true)]
    pub owner_admin_id: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    #[schema(required = true)]
    pub due_at: Option<i64>,
    pub notes: String,
    pub reason: String,
    pub idempotency_key: String,
}

/// 身份及时间均为采集时存储值，后续资产改名或管理员变更不改写历史。
#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub(crate) struct SnapshotSummary {
    pub id: u64,
    pub asset_id: u64,
    pub asset_symbol: String,
    pub precision_scale: i32,
    pub schema_version: u32,
    pub captured_at: i64,
    pub admin_id: u64,
    pub reason: String,
}

/// JSON 原样重放；摘要用于证据一致性检测，不表示外部认证或储备证明。
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct SnapshotDetail {
    #[serde(flatten)]
    pub summary: SnapshotSummary,
    pub idempotency_key: String,
    pub report_hash: String,
    #[schema(value_type = super::FinancialReconciliationReport)]
    pub report: Value,
}

/// 同一只读快照内的历史页及真实总数。
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct SnapshotHistory {
    pub snapshots: Vec<SnapshotSummary>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}

/// 追加式跟进回执；最新版本描述当前分配，旧版本和备注始终保留，不声明问题已解决。
#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub(crate) struct FollowupRecord {
    pub id: u64,
    pub snapshot_id: u64,
    pub version: u64,
    pub owner_admin_id: Option<u64>,
    pub due_at: Option<i64>,
    pub notes: String,
    pub admin_id: u64,
    pub reason: String,
    pub recorded_at: i64,
    pub idempotency_key: String,
}

/// 版本从所有已提交跟进获取；空历史显式版本 0，不虚构负责人或处理期限。
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct FollowupHistory {
    pub snapshot_id: u64,
    pub latest_version: u64,
    pub records: Vec<FollowupRecord>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}
