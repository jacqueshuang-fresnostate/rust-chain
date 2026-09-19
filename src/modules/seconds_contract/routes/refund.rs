//! 本金退款策略和人工操作的薄路由；身份来自 AdminAuth，权限由后台请求中间件精确映射。

use super::*;
use crate::modules::seconds_contract::{
    application::refund as use_cases,
    presentation::refund::{
        PrincipalRefundReceipt, RefundContext, RefundPolicy, RefundPrincipalRequest,
        UpdateRefundPolicy,
    },
};

/// 产品策略读写与单笔退款分开授权；GET 只展示快照，不执行资格修复或资金操作。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/seconds-contracts/products/:id/refund-policy",
            get(get_policy).patch(update_policy),
        )
        .route(
            "/seconds-contracts/orders/:id/principal-refund",
            get(get_context).post(refund_principal),
        )
}

async fn get_policy(
    AdminAuth(_): AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> AppResult<Json<RefundPolicy>> {
    Ok(Json(use_cases::get_policy(&mysql_pool(&state)?, id).await?))
}

async fn update_policy(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(request): Json<UpdateRefundPolicy>,
) -> AppResult<Json<RefundPolicy>> {
    Ok(Json(
        use_cases::update_policy(
            &mysql_pool(&state)?,
            admin_id_from_subject(&claims.sub)?,
            id,
            request,
        )
        .await?,
    ))
}

async fn get_context(
    AdminAuth(_): AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> AppResult<Json<RefundContext>> {
    Ok(Json(
        use_cases::get_refund_context(&mysql_pool(&state)?, id).await?,
    ))
}

async fn refund_principal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(request): Json<RefundPrincipalRequest>,
) -> AppResult<Json<PrincipalRefundReceipt>> {
    Ok(Json(
        use_cases::refund_principal_with_events(
            &mysql_pool(&state)?,
            admin_id_from_subject(&claims.sub)?,
            id,
            request,
            state.event_broadcast_hub.as_ref(),
        )
        .await?,
    ))
}
