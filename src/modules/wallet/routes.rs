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
    extract::{Path, Query, State},
    routing::{get, post},
};

use super::{
    application::{
        admin_id_from_subject, approve_withdrawal as approve_withdrawal_use_case,
        broadcast_withdrawal as broadcast_withdrawal_use_case, build_wallet_ledger_filter,
        confirm_withdrawal as confirm_withdrawal_use_case,
        create_withdrawal_request as create_withdrawal_request_use_case,
        fail_withdrawal as fail_withdrawal_use_case,
        get_or_assign_deposit_address as get_or_assign_deposit_address_use_case,
        get_return_history as get_return_history_use_case,
        get_today_return as get_today_return_use_case,
        list_admin_deposits as list_admin_deposits_use_case,
        list_admin_withdrawals as list_admin_withdrawals_use_case,
        list_deposit_assets as list_deposit_assets_use_case,
        list_deposit_networks_by_query as list_deposit_networks_use_case,
        list_user_withdrawals as list_user_withdrawals_use_case,
        list_wallet_accounts as list_wallet_accounts_use_case,
        list_wallet_ledger as list_wallet_ledger_use_case,
        list_withdraw_assets as list_withdraw_assets_use_case, mysql_pool,
        normalize_deposit_networks_query_asset, observe_deposit as observe_deposit_use_case,
        reject_withdrawal as reject_withdrawal_use_case,
        reverse_deposit as reverse_deposit_use_case, validate_return_history_days,
    },
    presentation::{
        AdminWalletListQuery, AdminWalletWithdrawalsResponse, BroadcastWithdrawalRequest,
        ConfirmWithdrawalRequest, CreateWithdrawalRequest, DepositAddressRequest,
        DepositAddressResponse, DepositAssetsResponse, DepositNetworksQuery,
        DepositNetworksResponse, FailWithdrawalRequest, ObserveDepositRequest, ReturnHistoryQuery,
        ReturnHistoryResponse, ReverseDepositRequest, ReviewWithdrawalRequest, TodayReturnResponse,
        WalletAccountsResponse, WalletDepositEventResponse, WalletDepositsResponse,
        WalletLedgerQuery, WalletLedgerResponse, WalletWithdrawalQuery, WalletWithdrawalResponse,
        WalletWithdrawalsResponse, WithdrawalRequestResponse,
    },
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/wallet/accounts", get(list_accounts))
        .route("/wallet/today-return", get(get_today_return))
        .route("/wallet/return-history", get(get_return_history))
        .route("/wallet/ledger", get(list_ledger))
        .route("/wallet/deposit-assets", get(list_deposit_assets))
        .route("/wallet/deposit-networks", get(list_deposit_networks))
        .route("/wallet/withdraw-assets", get(list_withdraw_assets))
        .route(
            "/wallet/deposit-address",
            post(get_or_assign_deposit_address),
        )
        .route(
            "/wallet/withdrawals",
            get(list_user_withdrawals).post(create_withdrawal_request),
        )
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/wallet/withdrawals", get(list_admin_withdrawals))
        .route("/wallet/withdrawals/:id/approve", post(approve_withdrawal))
        .route("/wallet/withdrawals/:id/reject", post(reject_withdrawal))
        .route(
            "/wallet/withdrawals/:id/broadcast",
            post(broadcast_withdrawal),
        )
        .route("/wallet/withdrawals/:id/confirm", post(confirm_withdrawal))
        .route("/wallet/withdrawals/:id/fail", post(fail_withdrawal))
        .route("/wallet/deposits", get(list_admin_deposits))
        .route("/wallet/deposits/observe", post(observe_deposit))
        .route("/wallet/deposits/:id/reverse", post(reverse_deposit))
}

async fn get_or_assign_deposit_address(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<DepositAddressRequest>,
) -> AppResult<Json<DepositAddressResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let address = get_or_assign_deposit_address_use_case(&pool, user_id, request).await?;

    Ok(Json(address))
}

async fn list_deposit_assets(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<DepositAssetsResponse>> {
    user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let assets = list_deposit_assets_use_case(&pool).await?;

    Ok(Json(DepositAssetsResponse { assets }))
}

async fn list_deposit_networks(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<DepositNetworksQuery>,
) -> AppResult<Json<DepositNetworksResponse>> {
    user_id_from_subject(&claims.sub)?;
    let _ = normalize_deposit_networks_query_asset(&query)?;
    let pool = mysql_pool(&state)?;
    let networks = list_deposit_networks_use_case(&pool, &query).await?;

    Ok(Json(DepositNetworksResponse { networks }))
}

async fn list_withdraw_assets(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<DepositAssetsResponse>> {
    user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let assets = list_withdraw_assets_use_case(&pool).await?;

    Ok(Json(DepositAssetsResponse { assets }))
}

async fn list_accounts(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<WalletAccountsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let accounts = list_wallet_accounts_use_case(&pool, user_id).await?;

    Ok(Json(WalletAccountsResponse { accounts }))
}

async fn get_today_return(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<TodayReturnResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = get_today_return_use_case(&pool, state.redis.as_ref(), user_id).await?;

    Ok(Json(response))
}

async fn get_return_history(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ReturnHistoryQuery>,
) -> AppResult<Json<ReturnHistoryResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let period_days = validate_return_history_days(query.days)?;
    let pool = mysql_pool(&state)?;
    let response = get_return_history_use_case(
        &pool,
        state.mongo.as_ref(),
        state.redis.as_ref(),
        user_id,
        period_days,
    )
    .await?;

    Ok(Json(response))
}

async fn list_ledger(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<WalletLedgerQuery>,
) -> AppResult<Json<WalletLedgerResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let filter = build_wallet_ledger_filter(query)?;
    let pool = mysql_pool(&state)?;
    let ledger = list_wallet_ledger_use_case(&pool, user_id, filter).await?;

    Ok(Json(ledger))
}

async fn create_withdrawal_request(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateWithdrawalRequest>,
) -> AppResult<Json<WithdrawalRequestResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let withdrawal =
        create_withdrawal_request_use_case(&pool, state.settings.as_ref(), user_id, request)
            .await?;
    Ok(Json(withdrawal))
}

async fn list_user_withdrawals(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<WalletWithdrawalQuery>,
) -> AppResult<Json<WalletWithdrawalsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let withdrawals = list_user_withdrawals_use_case(&mysql_pool(&state)?, user_id, query).await?;
    Ok(Json(WalletWithdrawalsResponse { withdrawals }))
}

async fn list_admin_withdrawals(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminWalletListQuery>,
) -> AppResult<Json<AdminWalletWithdrawalsResponse>> {
    Ok(Json(
        list_admin_withdrawals_use_case(&mysql_pool(&state)?, query).await?,
    ))
}

async fn approve_withdrawal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(withdrawal_id): Path<u64>,
    Json(request): Json<ReviewWithdrawalRequest>,
) -> AppResult<Json<WalletWithdrawalResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        approve_withdrawal_use_case(&mysql_pool(&state)?, admin_id, withdrawal_id, request).await?,
    ))
}

async fn reject_withdrawal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(withdrawal_id): Path<u64>,
    Json(request): Json<ReviewWithdrawalRequest>,
) -> AppResult<Json<WalletWithdrawalResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        reject_withdrawal_use_case(&mysql_pool(&state)?, admin_id, withdrawal_id, request).await?,
    ))
}

async fn broadcast_withdrawal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(withdrawal_id): Path<u64>,
    Json(request): Json<BroadcastWithdrawalRequest>,
) -> AppResult<Json<WalletWithdrawalResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        broadcast_withdrawal_use_case(&mysql_pool(&state)?, admin_id, withdrawal_id, request)
            .await?,
    ))
}

async fn confirm_withdrawal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(withdrawal_id): Path<u64>,
    Json(request): Json<ConfirmWithdrawalRequest>,
) -> AppResult<Json<WalletWithdrawalResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        confirm_withdrawal_use_case(&mysql_pool(&state)?, admin_id, withdrawal_id, request).await?,
    ))
}

async fn fail_withdrawal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(withdrawal_id): Path<u64>,
    Json(request): Json<FailWithdrawalRequest>,
) -> AppResult<Json<WalletWithdrawalResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        fail_withdrawal_use_case(&mysql_pool(&state)?, admin_id, withdrawal_id, request).await?,
    ))
}

async fn list_admin_deposits(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminWalletListQuery>,
) -> AppResult<Json<WalletDepositsResponse>> {
    Ok(Json(
        list_admin_deposits_use_case(&mysql_pool(&state)?, query).await?,
    ))
}

async fn observe_deposit(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<ObserveDepositRequest>,
) -> AppResult<Json<WalletDepositEventResponse>> {
    Ok(Json(
        observe_deposit_use_case(&mysql_pool(&state)?, request).await?,
    ))
}

async fn reverse_deposit(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(deposit_id): Path<u64>,
    Json(request): Json<ReverseDepositRequest>,
) -> AppResult<Json<WalletDepositEventResponse>> {
    Ok(Json(
        reverse_deposit_use_case(&mysql_pool(&state)?, deposit_id, request).await?,
    ))
}
#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_wallet_routes_tests.rs"]
mod tests;
