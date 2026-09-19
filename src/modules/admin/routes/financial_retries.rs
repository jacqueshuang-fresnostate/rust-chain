//! 资金异常入口；AdminAuth 校验真实路由权限，调用者不能指定审计操作者。

use super::*;
use crate::modules::admin::{
    application::{
        get_admin_financial_retries, requeue_admin_financial_retry, update_admin_financial_incident,
    },
    presentation::{
        FinancialRetriesQuery, FinancialRetriesResponse, FinancialRetryIncident,
        FinancialRetrySchedule, RequeueFinancialRetryRequest, UpdateFinancialRetryIncidentRequest,
    },
};

/// 注册只读查询和独立重新排期命令；后者不暴露任何资金执行参数。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/governance/financial-retries", get(list))
        .route(
            "/governance/financial-retries/:kind/:id/requeue",
            post(requeue),
        )
        .route(
            "/governance/financial-retries/:kind/:id/incident",
            patch(update_incident),
        )
}

async fn update_incident(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, u64)>,
    Json(request): Json<UpdateFinancialRetryIncidentRequest>,
) -> AppResult<Json<FinancialRetryIncident>> {
    Ok(Json(
        update_admin_financial_incident(
            state.mysql.clone(),
            admin_id_from_subject(&claims.sub)?,
            kind,
            id,
            request,
        )
        .await?,
    ))
}

async fn list(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<FinancialRetriesQuery>,
) -> AppResult<Json<FinancialRetriesResponse>> {
    Ok(Json(
        get_admin_financial_retries(state.mysql.clone(), query).await?,
    ))
}

async fn requeue(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, u64)>,
    Json(request): Json<RequeueFinancialRetryRequest>,
) -> AppResult<Json<FinancialRetrySchedule>> {
    Ok(Json(
        requeue_admin_financial_retry(
            state.mysql.clone(),
            admin_id_from_subject(&claims.sub)?,
            kind,
            id,
            request,
        )
        .await?,
    ))
}
