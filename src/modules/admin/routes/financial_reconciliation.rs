//! 实时报表只读；人工采集/跟进仅写侧车记录，权限由 AdminAuth 按精确方法路径校验。

use super::*;
use crate::modules::admin::{
    application::financial_reconciliation::{get_financial_reconciliation, snapshots},
    presentation::financial_reconciliation::{
        FinancialReconciliationQuery, FinancialReconciliationResponse, snapshots::*,
    },
};

/// 不提供修复、补账、资金操作或业务状态变更命令。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/financial-reconciliation", get(read))
        .route(
            "/financial-reconciliation/snapshots",
            get(history).post(capture),
        )
        .route("/financial-reconciliation/snapshots/:id", get(detail))
        .route(
            "/financial-reconciliation/snapshots/:id/follow-ups",
            get(followups).post(append_followup),
        )
}

async fn capture(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CaptureRequest>,
) -> AppResult<Json<SnapshotDetail>> {
    Ok(Json(
        snapshots::capture(
            state.mysql.clone(),
            admin_id_from_subject(&claims.sub)?,
            request,
        )
        .await?,
    ))
}

async fn history(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<SnapshotQuery>,
) -> AppResult<Json<SnapshotHistory>> {
    Ok(Json(snapshots::history(state.mysql.clone(), query).await?))
}

async fn detail(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> AppResult<Json<SnapshotDetail>> {
    Ok(Json(snapshots::detail(state.mysql.clone(), id).await?))
}

async fn followups(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Query(query): Query<FollowupQuery>,
) -> AppResult<Json<FollowupHistory>> {
    Ok(Json(
        snapshots::followups(state.mysql.clone(), id, query).await?,
    ))
}

async fn append_followup(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(request): Json<FollowupRequest>,
) -> AppResult<Json<FollowupRecord>> {
    Ok(Json(
        snapshots::append_followup(
            state.mysql.clone(),
            admin_id_from_subject(&claims.sub)?,
            id,
            request,
        )
        .await?,
    ))
}

async fn read(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<FinancialReconciliationQuery>,
) -> AppResult<Json<FinancialReconciliationResponse>> {
    Ok(Json(
        get_financial_reconciliation(state.mysql.clone(), query).await?,
    ))
}
