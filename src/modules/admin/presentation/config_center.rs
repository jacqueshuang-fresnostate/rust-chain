//! 后台配置中心查询与响应 DTO。

use crate::{
    architecture::PresentationLayer,
    modules::admin::service::{
        AdminConfigCenterItem, AdminConfigCenterSummary, AdminConfigCenterView,
    },
    time::option_unix_millis,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 配置中心搜索参数；空白归一、长度和枚举合法性由服务层统一处理。
#[derive(Debug, Default, Deserialize)]
pub(crate) struct AdminConfigCenterQuery {
    pub(crate) query: Option<String>,
    pub(crate) group: Option<String>,
    pub(crate) status: Option<String>,
}

impl PresentationLayer for AdminConfigCenterQuery {}

/// 单个配置域的稳定传输合同，时间统一为 Unix 毫秒且所有错误均为服务层生成的安全摘要。
#[derive(Debug, Serialize)]
pub(crate) struct AdminConfigCenterItemResponse {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) group: String,
    pub(crate) group_name: String,
    pub(crate) config_path: String,
    pub(crate) operation_path: Option<String>,
    pub(crate) configured_count: u64,
    pub(crate) config_status: String,
    pub(crate) published_version: Option<u64>,
    pub(crate) applied_version: Option<u64>,
    pub(crate) runtime_status: String,
    #[serde(with = "option_unix_millis")]
    pub(crate) last_modified_at: Option<DateTime<Utc>>,
    #[serde(with = "option_unix_millis")]
    pub(crate) last_applied_at: Option<DateTime<Utc>>,
    #[serde(with = "option_unix_millis")]
    pub(crate) last_tested_at: Option<DateTime<Utc>>,
    pub(crate) last_error_summary: Option<String>,
}

impl From<AdminConfigCenterItem> for AdminConfigCenterItemResponse {
    /// 将内部枚举映射为稳定代码并复制静态目录文本；转换不会重新读取数据库或接触原始错误。
    fn from(item: AdminConfigCenterItem) -> Self {
        Self {
            code: item.code.to_owned(),
            name: item.name.to_owned(),
            group: item.group.to_owned(),
            group_name: item.group_name.to_owned(),
            config_path: item.config_path.to_owned(),
            operation_path: item.operation_path.map(str::to_owned),
            configured_count: item.configured_count,
            config_status: item.config_status.as_code().to_owned(),
            published_version: item.published_version,
            applied_version: item.applied_version,
            runtime_status: item.runtime_status.as_code().to_owned(),
            last_modified_at: item.last_modified_at,
            last_applied_at: item.last_applied_at,
            last_tested_at: item.last_tested_at,
            last_error_summary: item.last_error_summary,
        }
    }
}

impl PresentationLayer for AdminConfigCenterItemResponse {}

/// 搜索和分组范围内的四态计数，供前端直接渲染筛选摘要而无需遍历或猜测状态。
#[derive(Debug, Serialize)]
pub(crate) struct AdminConfigCenterSummaryResponse {
    pub(crate) total: u64,
    pub(crate) unconfigured: u64,
    pub(crate) pending_apply: u64,
    pub(crate) runtime_error: u64,
    pub(crate) normal: u64,
}

impl From<AdminConfigCenterSummary> for AdminConfigCenterSummaryResponse {
    /// 保持服务层已计算的分面计数，不因最终状态过滤而二次改写。
    fn from(summary: AdminConfigCenterSummary) -> Self {
        Self {
            total: summary.total,
            unconfigured: summary.unconfigured,
            pending_apply: summary.pending_apply,
            runtime_error: summary.runtime_error,
            normal: summary.normal,
        }
    }
}

impl PresentationLayer for AdminConfigCenterSummaryResponse {}

/// 配置中心列表响应；`total` 表示当前 items 数量，summary 保留状态筛选前的完整分面。
#[derive(Debug, Serialize)]
pub(crate) struct AdminConfigCenterResponse {
    pub(crate) items: Vec<AdminConfigCenterItemResponse>,
    pub(crate) total: u64,
    pub(crate) summary: AdminConfigCenterSummaryResponse,
}

impl From<AdminConfigCenterView> for AdminConfigCenterResponse {
    /// 把纯查询视图转换为 JSON DTO，保持后端目录顺序和已判定状态不变。
    fn from(view: AdminConfigCenterView) -> Self {
        Self {
            items: view.items.into_iter().map(Into::into).collect(),
            total: view.total,
            summary: view.summary.into(),
        }
    }
}

impl PresentationLayer for AdminConfigCenterResponse {}
