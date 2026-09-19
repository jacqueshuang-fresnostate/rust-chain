//! 资金重试只读工作台与带审计重新排期契约；直接复用传输 DTO，避免金额与时间文档漂移。

use super::*;
pub(super) use crate::modules::admin::presentation::{
    FinancialRetriesResponse, FinancialRetryCount, FinancialRetryIncident, FinancialRetryResponse,
    FinancialRetrySchedule, RequeueFinancialRetryRequest, UpdateFinancialRetryIncidentRequest,
};

/// 仅查询已持久化的任务；total 和分类计数均应用当前筛选，不表示全部待处理业务订单。
#[utoipa::path(
    get,
    path = "/admin/api/v1/governance/financial-retries",
    tag = "admin-financial-retries",
    summary = "只读查询资金任务异常",
    params(
        ("task_kind" = Option<String>, Query, description = "earn / loan / commission / seconds"),
        ("outcome" = Option<String>, Query, description = "ready / running / waiting_balance / waiting_source / failed / manual_review"),
        ("limit" = Option<u32>, Query, description = "1–100，默认 50"),
        ("offset" = Option<u32>, Query, description = "0–100000，默认 0")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "只读调度快照，需要 governance.financial.read", body = FinancialRetriesResponse),
        (status = 400, description = "筛选或分页无效", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "权限不足", body = ErrorResponse),
        (status = 500, description = "读取失败，不返回零积压", body = ErrorResponse)
    )
)]
fn list_admin_financial_retries() {}

/// 重新排期不直接付款、退款或修改订单；锁内校验 expected_version，有效租约拒绝，过期 token 被撤销。
#[utoipa::path(
    post,
    path = "/admin/api/v1/governance/financial-retries/{kind}/{id}/requeue",
    tag = "admin-financial-retries",
    summary = "审计后重新排期资金任务",
    params(
        ("kind" = String, Path, description = "earn / loan / commission"),
        ("id" = u64, Path, description = "调度条目的正整数 item_id")
    ),
    request_body = RequeueFinancialRetryRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "仅更新调度，需要 governance.financial.operate", body = FinancialRetrySchedule),
        (status = 400, description = "原因为空、超过 500 字或参数无效", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "权限不足", body = ErrorResponse),
        (status = 404, description = "调度记录已不存在", body = ErrorResponse),
        (status = 409, description = "租约仍有效或调度版本已变化；刷新后重新确认", body = ErrorResponse),
        (status = 500, description = "更新或审计失败，整体回滚", body = ErrorResponse)
    )
)]
fn requeue_admin_financial_retry() {}

/// 只更新负责人和显式期限并审计；秒合约复核仅可登记跟进信息，不能付款、退款或判输赢。
#[utoipa::path(
    patch,
    path = "/admin/api/v1/governance/financial-retries/{kind}/{id}/incident",
    tag = "admin-financial-retries",
    summary = "审计更新资金异常负责人及期限",
    params(
        ("kind" = String, Path, description = "earn / loan / commission / seconds"),
        ("id" = u64, Path, description = "正整数异常记录 ID")
    ),
    request_body = UpdateFinancialRetryIncidentRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "仅元数据更新，需要 governance.financial.operate", body = FinancialRetryIncident),
        (status = 400, description = "原因、期限或负责人无效", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "权限不足", body = ErrorResponse),
        (status = 404, description = "异常已不存在或已离开复核", body = ErrorResponse),
        (status = 409, description = "元数据版本已变化", body = ErrorResponse),
        (status = 500, description = "更新或审计失败，整体回滚", body = ErrorResponse)
    )
)]
fn update_admin_financial_incident() {}
