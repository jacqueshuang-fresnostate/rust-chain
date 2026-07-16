use crate::{
    error::AppResult, modules::auth::UserAuth, modules::user::service::user_id_from_subject,
    state::AppState,
};
use axum::{Json, Router, extract::Query, extract::State, routing::get, routing::post};

use super::{
    application::{
        build_wallet_ledger_filter,
        create_withdrawal_request as create_withdrawal_request_use_case,
        get_or_assign_deposit_address as get_or_assign_deposit_address_use_case,
        list_deposit_assets as list_deposit_assets_use_case,
        list_deposit_networks_by_query as list_deposit_networks_use_case,
        list_wallet_accounts as list_wallet_accounts_use_case,
        list_wallet_ledger as list_wallet_ledger_use_case,
        list_withdraw_assets as list_withdraw_assets_use_case, mysql_pool,
        normalize_deposit_networks_query_asset,
    },
    presentation::{
        CreateWithdrawalRequest, DepositAddressRequest, DepositAddressResponse,
        DepositAssetsResponse, DepositNetworksQuery, DepositNetworksResponse,
        WalletAccountsResponse, WalletLedgerQuery, WalletLedgerResponse, WithdrawalRequestResponse,
    },
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/wallet/accounts", get(list_accounts))
        .route("/wallet/ledger", get(list_ledger))
        .route("/wallet/deposit-assets", get(list_deposit_assets))
        .route("/wallet/deposit-networks", get(list_deposit_networks))
        .route("/wallet/withdraw-assets", get(list_withdraw_assets))
        .route(
            "/wallet/deposit-address",
            post(get_or_assign_deposit_address),
        )
        .route("/wallet/withdrawals", post(create_withdrawal_request))
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

async fn list_ledger(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<WalletLedgerQuery>,
) -> AppResult<Json<WalletLedgerResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let filter = build_wallet_ledger_filter(query)?;
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
#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_wallet_routes_tests.rs"]
mod tests;
