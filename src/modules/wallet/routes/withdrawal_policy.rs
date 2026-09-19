//! 提现策略与地址薄 HTTP 入口，仅做鉴权、参数提取及用例转发。

use super::super::{
    application::{admin_id_from_subject, mysql_pool, withdrawal_policy as application},
    presentation::withdrawal_policy::{
        RegisterWithdrawalAddressRequest, SaveWithdrawalPolicyRequest, WithdrawalAddressResponse,
        WithdrawalPolicyResponse,
    },
};
use crate::{
    error::AppResult,
    modules::{
        auth::{AdminAuth, UserAuth},
        user::service::user_id_from_subject,
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};

/// 策略入口使用独立路径，由后台权限表映射至安全配置权限。
pub(super) fn admin_routes() -> Router<AppState> {
    Router::new().route(
        "/wallet/withdrawal-policies/:asset_id",
        get(load_policy).patch(save_policy),
    )
}

/// 地址薄只允许当前用户登记和读取，不开放管理员回填时间的旁路。
pub(super) fn user_routes() -> Router<AppState> {
    Router::new().route(
        "/wallet/withdrawal-addresses",
        get(list_addresses).post(register_address),
    )
}

async fn load_policy(
    AdminAuth(_): AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
) -> AppResult<Json<WithdrawalPolicyResponse>> {
    Ok(Json(
        application::load_policy(&mysql_pool(&state)?, asset_id).await?,
    ))
}

async fn save_policy(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
    Json(request): Json<SaveWithdrawalPolicyRequest>,
) -> AppResult<Json<WithdrawalPolicyResponse>> {
    Ok(Json(
        application::save_policy(
            &mysql_pool(&state)?,
            admin_id_from_subject(&claims.sub)?,
            asset_id,
            request,
        )
        .await?,
    ))
}

async fn register_address(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<RegisterWithdrawalAddressRequest>,
) -> AppResult<Json<WithdrawalAddressResponse>> {
    Ok(Json(
        application::register_address(
            &mysql_pool(&state)?,
            &state.settings,
            user_id_from_subject(&claims.sub)?,
            request,
        )
        .await?,
    ))
}

async fn list_addresses(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<WithdrawalAddressResponse>>> {
    Ok(Json(
        application::list_addresses(&mysql_pool(&state)?, user_id_from_subject(&claims.sub)?)
            .await?,
    ))
}
