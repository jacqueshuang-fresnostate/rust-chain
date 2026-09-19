//! 资金异常工作台传输合同；金额保留十进制文本，调度时间使用 Unix 毫秒，不暴露租约 token。

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// 查询仅接受已登记任务及结果，分页范围由应用层校验。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FinancialRetriesQuery {
    pub task_kind: Option<String>,
    pub outcome: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// 调度快照独立于业务状态；过期租约仍不代表原资金事务已经结束。
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct FinancialRetrySchedule {
    pub version: String,
    pub task_kind: String,
    pub item_id: u64,
    pub attempt_count: Option<u64>,
    pub last_attempt_at: Option<i64>,
    pub next_attempt_at: Option<i64>,
    pub outcome: String,
    pub lease_status: String,
}

/// 来源关联缺失时显式返回 null；佣金来源 ID 保留字符串，不猜测它是某种订单主键。
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct FinancialRetryResponse {
    #[serde(flatten)]
    pub schedule: FinancialRetrySchedule,
    pub source_type: Option<String>,
    pub source_order_id: Option<String>,
    pub user_id: Option<u64>,
    pub amount: Option<String>,
    pub asset: Option<String>,
    pub incident: FinancialRetryIncident,
    pub failure_code: Option<String>,
    pub failure_at: Option<i64>,
    pub review_window_start: Option<i64>,
    pub review_window_end: Option<i64>,
}

/// 事件管理元数据与业务/调度独立；未分配、未设置期限均显式为 null，版本零表示尚未登记。
#[derive(Debug, Default, Serialize, ToSchema)]
pub(crate) struct FinancialRetryIncident {
    pub version: u64,
    pub owner_admin_id: Option<u64>,
    pub owner_name: Option<String>,
    pub due_at: Option<i64>,
    pub updated_by: Option<u64>,
    pub updated_at: Option<i64>,
}

/// 完整替换负责人和期限，字段必须显式提供；null 表示清空，不由服务器推定 SLA。
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateFinancialRetryIncidentRequest {
    pub expected_version: u64,
    #[serde(deserialize_with = "Option::deserialize")]
    #[schema(required = true)]
    pub owner_admin_id: Option<u64>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[schema(required = true)]
    pub due_at: Option<i64>,
    pub reason: String,
}

/// 分类计数与 total 使用相同筛选和读取快照，不以本页行数推断全量积压。
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct FinancialRetryCount {
    pub outcome: String,
    pub count: i64,
}

/// 只读分页结果；checked_at 为服务端租约判定时间。
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct FinancialRetriesResponse {
    pub retries: Vec<FinancialRetryResponse>,
    pub total: i64,
    pub counts: Vec<FinancialRetryCount>,
    pub limit: u32,
    pub offset: u32,
    pub checked_at: i64,
}

/// 重新排期仅接收原因，拒绝夹带金额、业务状态和任意排期参数。
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequeueFinancialRetryRequest {
    pub reason: String,
    pub expected_version: String,
}
