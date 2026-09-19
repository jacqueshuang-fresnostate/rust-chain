//! 平台对账、人工采集和追加跟进文档；不提供自动日结、补记历史或资金修复接口。

use crate::{
    error::ErrorResponse,
    modules::admin::presentation::financial_reconciliation::{snapshots::*, *},
};
use utoipa::OpenApi;

#[utoipa::path(
    get, path="/admin/api/v1/financial-reconciliation", tag="financial-reconciliation",
    summary="只读单资产当前对账（历史覆盖不完整）",
    params(("asset_id"=Option<u64>,Query),("limit"=Option<u32>,Query),("offset"=Option<u32>,Query)),
    security(("bearerAuth"=[])),
    responses(
        (status=200,description="governance.financial.read；READ ONLY 一致快照，明细上限 100，附真实总数",body=FinancialReconciliationResponse),
        (status=400,body=ErrorResponse),(status=401,body=ErrorResponse),(status=403,body=ErrorResponse),
        (status=404,body=ErrorResponse),(status=500,body=ErrorResponse)
    )
)]
fn current_report() {}

#[utoipa::path(
    post, path="/admin/api/v1/financial-reconciliation/snapshots", tag="financial-reconciliation",
    summary="人工采集不可变单资产证据（不是日终结账）",
    request_body=CaptureRequest, security(("bearerAuth"=[])),
    responses(
        (status=200,description="governance.financial.operate；重新读取并与审计原子保存；相同原始键/请求稳定重放",body=SnapshotDetail),
        (status=400,body=ErrorResponse),(status=401,body=ErrorResponse),(status=403,body=ErrorResponse),
        (status=404,body=ErrorResponse),(status=409,description="原始键或请求与已存记录不符",body=ErrorResponse),
        (status=500,description="采集或审计失败整体回滚",body=ErrorResponse)
    )
)]
fn capture() {}

#[utoipa::path(
    get, path="/admin/api/v1/financial-reconciliation/snapshots", tag="financial-reconciliation",
    summary="只读人工采集历史",
    params(("asset_id"=Option<u64>,Query),("limit"=Option<u32>,Query),("offset"=Option<u32>,Query)),
    security(("bearerAuth"=[])),
    responses(
        (status=200,description="governance.financial.read；按 ID 倒序，limit 1–100 默认 20，offset 0–100000",body=SnapshotHistory),
        (status=400,body=ErrorResponse),(status=401,body=ErrorResponse),(status=403,body=ErrorResponse),(status=500,body=ErrorResponse)
    )
)]
fn history() {}

#[utoipa::path(
    get, path="/admin/api/v1/financial-reconciliation/snapshots/{id}", tag="financial-reconciliation",
    summary="回放原始采集 JSON，不重新计算历史",
    params(("id"=u64,Path,description="正整数采集 ID")), security(("bearerAuth"=[])),
    responses(
        (status=200,description="governance.financial.read；保留 partial、明细上限及全量计数",body=SnapshotDetail),
        (status=401,body=ErrorResponse),(status=403,body=ErrorResponse),(status=404,body=ErrorResponse),(status=500,body=ErrorResponse)
    )
)]
fn detail() {}

#[utoipa::path(
    get, path="/admin/api/v1/financial-reconciliation/snapshots/{id}/follow-ups", tag="financial-reconciliation",
    summary="只读追加跟进历史和最新版本",
    params(("id"=u64,Path),("limit"=Option<u32>,Query),("offset"=Option<u32>,Query)),
    security(("bearerAuth"=[])),
    responses(
        (status=200,description="governance.financial.read；空历史版本 0，不推定负责人或已解决",body=FollowupHistory),
        (status=400,body=ErrorResponse),(status=401,body=ErrorResponse),(status=403,body=ErrorResponse),
        (status=404,body=ErrorResponse),(status=500,body=ErrorResponse)
    )
)]
fn followups() {}

#[utoipa::path(
    post, path="/admin/api/v1/financial-reconciliation/snapshots/{id}/follow-ups", tag="financial-reconciliation",
    summary="原子审计追加负责人、期限和备注",
    params(("id"=u64,Path)), request_body=FollowupRequest, security(("bearerAuth"=[])),
    responses(
        (status=200,description="governance.financial.operate；expected_version 乐观锁，原键稳定重放，不变更任何资金",body=FollowupRecord),
        (status=400,body=ErrorResponse),(status=401,body=ErrorResponse),(status=403,body=ErrorResponse),
        (status=404,body=ErrorResponse),(status=409,description="版本或幂等请求冲突",body=ErrorResponse),
        (status=500,description="追加或审计失败整体回滚",body=ErrorResponse)
    )
)]
fn append_followup() {}

/// 聚合真实传输 DTO 和六个方法；主文档合并后两个 OpenAPI 地址保持同一份合同。
#[derive(OpenApi)]
#[openapi(paths(current_report, capture, history, detail, followups, append_followup))]
pub(super) struct ReconciliationApiDoc;
