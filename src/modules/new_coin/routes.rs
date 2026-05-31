use crate::{
    error::{AppError, AppResult},
    modules::{
        auth::UserAuth,
        events::EventBroadcastMessage,
        new_coin::{
            LifecycleStatus, MySqlNewCoinRepository, UnlockFeePaymentUpdate, UnlockRule,
            apply_unlock_rule,
        },
    },
    state::AppState,
    time::{option_unix_millis, unix_millis},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{MySql, Pool, QueryBuilder, Transaction};
use std::cmp::Ordering;

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/new-coins", get(list_projects))
        .route("/new-coins/:symbol", get(project_detail))
        .route(
            "/new-coins/:symbol/subscriptions",
            post(create_subscription),
        )
        .route("/new-coins/subscriptions", get(list_subscriptions))
        .route("/new-coins/distributions", get(list_distributions))
        .route("/new-coins/:symbol/purchase", post(create_purchase))
        .route("/new-coins/purchases", get(list_purchases))
        .route("/new-coins/unlocks", get(list_unlocks))
        .route("/new-coins/unlocks/:id/pay-fee", post(pay_unlock_fee))
        .route("/new-coins/unlocks/:id/release", post(release_unlock))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    limit: Option<u32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinProjectResponse {
    id: u64,
    asset_id: u64,
    symbol: String,
    lifecycle_status: String,
    total_supply: BigDecimal,
    issue_price: BigDecimal,
    #[serde(default, with = "option_unix_millis")]
    listed_at: Option<chrono::DateTime<chrono::Utc>>,
    unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    relative_unlock_seconds: Option<u64>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    status: String,
}

#[derive(Debug, Serialize)]
struct NewCoinProjectsResponse {
    projects: Vec<NewCoinProjectResponse>,
}

#[derive(Debug, Deserialize)]
struct CreateSubscriptionRequest {
    quote_asset_id: u64,
    quote_amount: BigDecimal,
    quantity: BigDecimal,
    idempotency_key: String,
}

#[derive(Debug, Deserialize)]
struct CreatePurchaseRequest {
    pair_id: u64,
    price: BigDecimal,
    quantity: BigDecimal,
    idempotency_key: String,
}

#[derive(Debug, Serialize)]
struct NewCoinOrderCreationResponse {
    idempotency_key: String,
    status: String,
    lock_position_id: Option<u64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinSubscriptionResponse {
    id: u64,
    project_id: u64,
    user_id: u64,
    quote_asset: u64,
    quote_amount: BigDecimal,
    requested_quantity: BigDecimal,
    allocated_quantity: BigDecimal,
    status: String,
    idempotency_key: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct NewCoinSubscriptionsResponse {
    subscriptions: Vec<NewCoinSubscriptionResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinDistributionResponse {
    id: u64,
    project_id: u64,
    user_id: u64,
    subscription_id: Option<u64>,
    asset_id: u64,
    quantity: BigDecimal,
    lock_position_id: Option<u64>,
    status: String,
    idempotency_key: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct NewCoinDistributionsResponse {
    distributions: Vec<NewCoinDistributionResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinPurchaseResponse {
    id: u64,
    project_id: u64,
    user_id: u64,
    pair_id: u64,
    base_asset: u64,
    quote_asset: u64,
    price: BigDecimal,
    quantity: BigDecimal,
    quote_amount: BigDecimal,
    lock_position_id: Option<u64>,
    status: String,
    idempotency_key: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct NewCoinPurchasesResponse {
    purchases: Vec<NewCoinPurchaseResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NewCoinUnlockResponse {
    id: u64,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    unlock_quantity: BigDecimal,
    unlock_price: Option<BigDecimal>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    unlock_fee_amount: Option<BigDecimal>,
    fee_paid_status: String,
    status: String,
    idempotency_key: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct NewCoinUnlocksResponse {
    unlocks: Vec<NewCoinUnlockResponse>,
}

#[derive(Debug, Deserialize)]
struct PayUnlockFeeRequest {
    payment_asset_id: u64,
    amount: BigDecimal,
}

#[derive(Debug, Serialize)]
struct PayUnlockFeeResponse {
    unlock_idempotency_key: String,
    paid: bool,
}

#[derive(Debug, Serialize)]
struct ReleaseUnlockResponse {
    unlock_idempotency_key: String,
    released: bool,
}

#[derive(Debug)]
struct ReleaseUnlockOutcome {
    asset_id: u64,
    unlock_quantity: BigDecimal,
    released: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct UnlockFeeExpectation {
    unlock_fee_enabled: bool,
    unlock_fee_asset: Option<u64>,
    unlock_fee_amount: Option<BigDecimal>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct NewCoinProjectRuleRow {
    id: u64,
    asset_id: u64,
    lifecycle_status: String,
    issue_price: BigDecimal,
    listed_at: Option<chrono::DateTime<Utc>>,
    unlock_type: String,
    fixed_unlock_at: Option<chrono::DateTime<Utc>>,
    relative_unlock_seconds: Option<u64>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    post_listing_purchase_enabled: bool,
    post_listing_pair_id: Option<u64>,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinPairRow {
    base_asset_id: u64,
    quote_asset_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinWalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug)]
struct NewCoinLockPositionInsert {
    user_id: u64,
    asset_id: u64,
    unlock_type: String,
    unlock_at: chrono::DateTime<Utc>,
    amount: BigDecimal,
    merge_key: String,
    source_type: String,
    source_id: String,
}

#[derive(Debug, Clone, Copy)]
struct NewCoinLedgerMetadata<'a> {
    change_type: &'a str,
    ref_type: &'a str,
    ref_id: &'a str,
}

#[derive(Debug, sqlx::FromRow)]
struct ReleasableUnlockRow {
    unlock_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    unlock_quantity: BigDecimal,
}

async fn list_projects(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinProjectsResponse>> {
    let pool = mysql_pool(&state)?;
    let projects = sqlx::query_as::<_, NewCoinProjectResponse>(
        r#"SELECT id, asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
                  unlock_type, fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
                  unlock_fee_rate, unlock_fee_basis, unlock_fee_asset, status
           FROM new_coin_projects
           WHERE status = 'active'
           ORDER BY id DESC
           LIMIT ?"#,
    )
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&pool)
    .await?;

    Ok(Json(NewCoinProjectsResponse { projects }))
}

async fn project_detail(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let pool = mysql_pool(&state)?;
    let project = sqlx::query_as::<_, NewCoinProjectResponse>(
        r#"SELECT id, asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
                  unlock_type, fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
                  unlock_fee_rate, unlock_fee_basis, unlock_fee_asset, status
           FROM new_coin_projects
           WHERE symbol = ? AND status = 'active'
           LIMIT 1"#,
    )
    .bind(symbol)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(project))
}

async fn list_subscriptions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let rows = sqlx::query_as::<_, NewCoinSubscriptionResponse>(
        r#"SELECT id, project_id, user_id, quote_asset, quote_amount, requested_quantity,
                  allocated_quantity, status, idempotency_key, created_at
           FROM new_coin_subscriptions
           WHERE user_id = ?
           ORDER BY id DESC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;

    Ok(Json(NewCoinSubscriptionsResponse {
        subscriptions: rows,
    }))
}

async fn list_distributions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let rows = sqlx::query_as::<_, NewCoinDistributionResponse>(
        r#"SELECT id, project_id, user_id, subscription_id, asset_id, quantity,
                  lock_position_id, status, idempotency_key, created_at
           FROM new_coin_distributions
           WHERE user_id = ?
           ORDER BY id DESC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;

    Ok(Json(NewCoinDistributionsResponse {
        distributions: rows,
    }))
}

async fn list_purchases(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinPurchasesResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let rows = sqlx::query_as::<_, NewCoinPurchaseResponse>(
        r#"SELECT id, project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                  quote_amount, lock_position_id, status, idempotency_key, created_at
           FROM new_coin_purchase_orders
           WHERE user_id = ?
           ORDER BY id DESC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;

    Ok(Json(NewCoinPurchasesResponse { purchases: rows }))
}

async fn list_unlocks(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinUnlocksResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let rows = sqlx::query_as::<_, NewCoinUnlockResponse>(
        r#"SELECT id, user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
                  unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                  unlock_fee_amount, fee_paid_status, status, idempotency_key, created_at
           FROM asset_unlock_records
           WHERE user_id = ?
           ORDER BY id DESC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;

    Ok(Json(NewCoinUnlocksResponse { unlocks: rows }))
}

async fn pay_unlock_fee(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<PayUnlockFeeRequest>,
) -> AppResult<Json<PayUnlockFeeResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let repository = new_coin_repository(&state)?;
    let expectation = unlock_fee_expectation(repository.pool(), &id, user_id).await?;
    ensure_unlock_fee_payment_matches(&expectation, request.payment_asset_id, &request.amount)?;
    let paid = repository
        .mark_unlock_fee_paid(UnlockFeePaymentUpdate {
            unlock_idempotency_key: id.clone(),
            user_id,
            payment_asset_id: request.payment_asset_id,
            amount: request.amount,
        })
        .await
        .map_err(map_new_coin_repository_error)?;

    Ok(Json(PayUnlockFeeResponse {
        unlock_idempotency_key: id,
        paid,
    }))
}

async fn release_unlock(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ReleaseUnlockResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let outcome = release_unlock_transaction(&pool, &id, user_id).await?;
    if outcome.released
        && let Some(hub) = &state.event_broadcast_hub
    {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "new_coin.unlock.released",
                "unlock_idempotency_key": id,
                "asset_id": outcome.asset_id,
                "unlock_quantity": outcome.unlock_quantity,
                "released": true,
            })
            .to_string(),
        ));
    }

    Ok(Json(ReleaseUnlockResponse {
        unlock_idempotency_key: id,
        released: true,
    }))
}

async fn create_subscription(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(request): Json<CreateSubscriptionRequest>,
) -> AppResult<Json<NewCoinOrderCreationResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let project = project_rule_by_symbol(&pool, &symbol).await?;
    if lifecycle_status(&project.lifecycle_status)? != LifecycleStatus::Subscription {
        return Err(AppError::Validation(
            "new coin subscription is not open for this project".to_owned(),
        ));
    }
    ensure_positive_amount(&request.quote_amount, "quote_amount")?;
    ensure_positive_amount(&request.quantity, "quantity")?;
    ensure_idempotency_key(&request.idempotency_key)?;

    let lock_positions = lock_positions_for_project(
        &project,
        user_id,
        project.asset_id,
        &request.idempotency_key,
        request.quantity.clone(),
        Utc::now(),
        "new_coin_subscription",
    )?;
    let lock_position_id = create_subscription_transaction(
        &pool,
        user_id,
        &project,
        request.quote_asset_id,
        &request.quote_amount,
        &request.quantity,
        &request.idempotency_key,
        lock_positions,
    )
    .await?;
    let status = if lock_position_id.is_some() {
        "allocated".to_owned()
    } else {
        "available".to_owned()
    };
    if let Some(hub) = &state.event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "new_coin.subscription.created",
                "idempotency_key": request.idempotency_key,
                "project_id": project.id,
                "asset_id": project.asset_id,
                "quote_asset_id": request.quote_asset_id,
                "quote_amount": request.quote_amount,
                "quantity": request.quantity,
                "status": status,
                "lock_position_id": lock_position_id,
            })
            .to_string(),
        ));
    }

    Ok(Json(NewCoinOrderCreationResponse {
        idempotency_key: request.idempotency_key,
        status,
        lock_position_id,
    }))
}

async fn create_purchase(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(request): Json<CreatePurchaseRequest>,
) -> AppResult<Json<NewCoinOrderCreationResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let project = project_rule_by_symbol(&pool, &symbol).await?;
    if lifecycle_status(&project.lifecycle_status)? != LifecycleStatus::Listed {
        return Err(AppError::Validation(
            "post-listing new coin purchase is not open for this project".to_owned(),
        ));
    }
    // 上市后认购必须服从后台单独开关和绑定交易对，避免用户绕过后台配置直接下单。
    ensure_post_listing_purchase_enabled(&project, request.pair_id)?;
    ensure_positive_amount(&request.price, "price")?;
    ensure_positive_amount(&request.quantity, "quantity")?;
    ensure_idempotency_key(&request.idempotency_key)?;
    let pair = pair_for_purchase(&pool, request.pair_id, project.asset_id).await?;
    let quote_amount = request.price.clone() * request.quantity.clone();

    let lock_position_id = create_purchase_transaction(
        &pool,
        user_id,
        &project,
        &pair,
        request.pair_id,
        &request.price,
        &request.quantity,
        &quote_amount,
        &request.idempotency_key,
    )
    .await?;
    let status = if lock_position_id.is_some() {
        "locked".to_owned()
    } else {
        "available".to_owned()
    };
    if let Some(hub) = &state.event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "new_coin.purchase.created",
                "idempotency_key": request.idempotency_key,
                "project_id": project.id,
                "pair_id": request.pair_id,
                "asset_id": project.asset_id,
                "quote_asset_id": pair.quote_asset_id,
                "price": request.price,
                "quantity": request.quantity,
                "quote_amount": quote_amount,
                "status": status,
                "lock_position_id": lock_position_id,
            })
            .to_string(),
        ));
    }

    Ok(Json(NewCoinOrderCreationResponse {
        idempotency_key: request.idempotency_key,
        status,
        lock_position_id,
    }))
}

async fn project_rule_by_symbol(
    pool: &Pool<MySql>,
    symbol: &str,
) -> AppResult<NewCoinProjectRuleRow> {
    let sql = new_coin_project_rule_select_sql("symbol = ?", "LIMIT 1");
    sqlx::query_as::<_, NewCoinProjectRuleRow>(&sql)
        .bind(symbol)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

async fn pair_for_purchase(
    pool: &Pool<MySql>,
    pair_id: u64,
    project_asset_id: u64,
) -> AppResult<NewCoinPairRow> {
    let row = sqlx::query_as::<_, NewCoinPairRow>(new_coin_pair_select_sql(false))
        .bind(pair_id)
        .bind(project_asset_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(row)
}

async fn lock_purchase_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    requested_pair_id: u64,
) -> AppResult<NewCoinProjectRuleRow> {
    let sql = new_coin_project_rule_select_sql("id = ?", "LIMIT 1 FOR UPDATE");
    let project = sqlx::query_as::<_, NewCoinProjectRuleRow>(&sql)
        .bind(project_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    if lifecycle_status(&project.lifecycle_status)? != LifecycleStatus::Listed {
        return Err(AppError::Validation(
            "post-listing new coin purchase is not open for this project".to_owned(),
        ));
    }
    ensure_post_listing_purchase_enabled(&project, requested_pair_id)?;
    Ok(project)
}

async fn lock_pair_for_purchase_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    project_asset_id: u64,
) -> AppResult<NewCoinPairRow> {
    sqlx::query_as::<_, NewCoinPairRow>(new_coin_pair_select_sql(true))
        .bind(pair_id)
        .bind(project_asset_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

fn new_coin_project_rule_select_sql(predicate: &str, suffix: &str) -> String {
    format!(
        r#"SELECT id, asset_id, lifecycle_status, issue_price, listed_at, unlock_type,
                  fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
                  unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                  post_listing_purchase_enabled, post_listing_pair_id
           FROM new_coin_projects
           WHERE {predicate} AND status = 'active'
           {suffix}"#,
    )
}

fn new_coin_pair_select_sql(for_update: bool) -> &'static str {
    if for_update {
        r#"SELECT base_asset AS base_asset_id, quote_asset AS quote_asset_id
           FROM trading_pairs
           WHERE id = ? AND base_asset = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#
    } else {
        r#"SELECT base_asset AS base_asset_id, quote_asset AS quote_asset_id
           FROM trading_pairs
           WHERE id = ? AND base_asset = ? AND status = 'active'
           LIMIT 1"#
    }
}

fn ensure_post_listing_purchase_enabled(
    project: &NewCoinProjectRuleRow,
    requested_pair_id: u64,
) -> AppResult<()> {
    if !project.post_listing_purchase_enabled
        || project.post_listing_pair_id != Some(requested_pair_id)
    {
        return Err(AppError::Validation(
            "post-listing new coin purchase is not open for this project".to_owned(),
        ));
    }
    Ok(())
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for new coin routes".to_owned())
    })
}

fn new_coin_repository(state: &AppState) -> AppResult<MySqlNewCoinRepository> {
    Ok(MySqlNewCoinRepository::new(mysql_pool(state)?))
}

fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

#[allow(clippy::too_many_arguments)]
async fn create_subscription_transaction(
    pool: &Pool<MySql>,
    user_id: u64,
    project: &NewCoinProjectRuleRow,
    quote_asset_id: u64,
    quote_amount: &BigDecimal,
    quantity: &BigDecimal,
    idempotency_key: &str,
    lock_positions: Vec<NewCoinLockPositionInsert>,
) -> AppResult<Option<u64>> {
    let mut tx = pool.begin().await?;
    if idempotency_key_exists(&mut tx, "new_coin_subscriptions", idempotency_key).await? {
        return Err(AppError::Conflict(
            "new coin subscription has already been created".to_owned(),
        ));
    }
    sqlx::query(
        r#"INSERT INTO new_coin_subscriptions
           (project_id, user_id, quote_asset, quote_amount, requested_quantity,
            allocated_quantity, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, 0, 'pending', ?)"#,
    )
    .bind(project.id)
    .bind(user_id)
    .bind(quote_asset_id)
    .bind(quote_amount)
    .bind(quantity)
    .bind(idempotency_key)
    .execute(&mut *tx)
    .await?;

    let ledger = NewCoinLedgerMetadata {
        change_type: "new_coin_subscription_payment",
        ref_type: "new_coin_subscription",
        ref_id: idempotency_key,
    };
    debit_wallet_available(&mut tx, user_id, quote_asset_id, quote_amount, ledger).await?;
    let lock_position_id = apply_new_coin_allocation(
        &mut tx,
        user_id,
        project.asset_id,
        quantity,
        &lock_positions,
        &project.issue_price,
        quote_amount,
        project,
        NewCoinLedgerMetadata {
            change_type: "new_coin_subscription_lock",
            ref_type: "new_coin_subscription",
            ref_id: idempotency_key,
        },
    )
    .await?;
    sqlx::query(
        "UPDATE new_coin_subscriptions SET allocated_quantity = ?, status = 'allocated' WHERE idempotency_key = ?",
    )
    .bind(quantity)
    .bind(idempotency_key)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(lock_position_id)
}

#[allow(clippy::too_many_arguments)]
async fn create_purchase_transaction(
    pool: &Pool<MySql>,
    user_id: u64,
    project: &NewCoinProjectRuleRow,
    _pair: &NewCoinPairRow,
    pair_id: u64,
    price: &BigDecimal,
    quantity: &BigDecimal,
    quote_amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<u64>> {
    let mut tx = pool.begin().await?;
    // 下单事务内重新锁定项目和交易对，避免后台刚关闭认购或调整规则后用户仍按旧快照成交。
    let locked_project = lock_purchase_project_in_tx(&mut tx, project.id, pair_id).await?;
    let locked_pair =
        lock_pair_for_purchase_in_tx(&mut tx, pair_id, locked_project.asset_id).await?;
    let lock_positions = lock_positions_for_project(
        &locked_project,
        user_id,
        locked_project.asset_id,
        idempotency_key,
        quantity.clone(),
        Utc::now(),
        "new_coin_purchase",
    )?;
    if idempotency_key_exists(&mut tx, "new_coin_purchase_orders", idempotency_key).await? {
        return Err(AppError::Conflict(
            "new coin purchase has already been created".to_owned(),
        ));
    }
    sqlx::query(
        r#"INSERT INTO new_coin_purchase_orders
           (project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
            quote_amount, lock_position_id, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, 'pending', ?)"#,
    )
    .bind(locked_project.id)
    .bind(user_id)
    .bind(pair_id)
    .bind(locked_pair.base_asset_id)
    .bind(locked_pair.quote_asset_id)
    .bind(price)
    .bind(quantity)
    .bind(quote_amount)
    .bind(idempotency_key)
    .execute(&mut *tx)
    .await?;

    let ledger = NewCoinLedgerMetadata {
        change_type: "new_coin_purchase_payment",
        ref_type: "new_coin_purchase",
        ref_id: idempotency_key,
    };
    debit_wallet_available(
        &mut tx,
        user_id,
        locked_pair.quote_asset_id,
        quote_amount,
        ledger,
    )
    .await?;
    let lock_position_id = apply_new_coin_allocation(
        &mut tx,
        user_id,
        locked_project.asset_id,
        quantity,
        &lock_positions,
        price,
        quote_amount,
        &locked_project,
        NewCoinLedgerMetadata {
            change_type: "new_coin_purchase_lock",
            ref_type: "new_coin_purchase",
            ref_id: idempotency_key,
        },
    )
    .await?;
    sqlx::query(
        "UPDATE new_coin_purchase_orders SET lock_position_id = ?, status = 'locked' WHERE idempotency_key = ?",
    )
    .bind(lock_position_id)
    .bind(idempotency_key)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(lock_position_id)
}

async fn idempotency_key_exists(
    tx: &mut Transaction<'_, MySql>,
    table_name: &str,
    idempotency_key: &str,
) -> AppResult<bool> {
    let mut query = QueryBuilder::<MySql>::new("SELECT id FROM ");
    query
        .push(table_name)
        .push(" WHERE idempotency_key = ")
        .push_bind(idempotency_key)
        .push(" LIMIT 1 FOR UPDATE");
    let exists: Option<(u64,)> = query.build_query_as().fetch_optional(&mut **tx).await?;
    Ok(exists.is_some())
}

async fn unlock_fee_expectation(
    pool: &Pool<MySql>,
    id: &str,
    user_id: u64,
) -> AppResult<UnlockFeeExpectation> {
    sqlx::query_as::<_, UnlockFeeExpectation>(
        r#"SELECT unlock_fee_enabled, unlock_fee_asset, unlock_fee_amount
           FROM asset_unlock_records
           WHERE idempotency_key = ? AND user_id = ?
           LIMIT 1"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

async fn release_unlock_transaction(
    pool: &Pool<MySql>,
    id: &str,
    user_id: u64,
) -> AppResult<ReleaseUnlockOutcome> {
    let exists = sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM asset_unlock_records WHERE idempotency_key = ? AND user_id = ? LIMIT 1",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let mut tx = pool.begin().await?;
    let Some(row) = sqlx::query_as::<_, ReleasableUnlockRow>(
        r#"SELECT unlocks.id AS unlock_id, unlocks.asset_id, unlocks.lock_position_id,
                  unlocks.unlock_quantity
           FROM asset_unlock_records unlocks
           INNER JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
           WHERE unlocks.idempotency_key = ? AND unlocks.user_id = ?
             AND unlocks.status <> 'released'
             AND positions.status = 'active'
             AND positions.unlock_at <= CURRENT_TIMESTAMP(6)
             AND positions.remaining_amount >= unlocks.unlock_quantity
             AND (unlocks.unlock_fee_enabled = false OR unlocks.fee_paid_status = 'paid')
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        if let Some((asset_id, unlock_quantity)) = sqlx::query_as::<_, (u64, BigDecimal)>(
            r#"SELECT asset_id, unlock_quantity
               FROM asset_unlock_records
               WHERE idempotency_key = ? AND user_id = ? AND status = 'released'
               LIMIT 1"#,
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            tx.commit().await?;
            return Ok(ReleaseUnlockOutcome {
                asset_id,
                unlock_quantity,
                released: false,
            });
        }
        return Err(AppError::Validation(
            "unlock is not releasable until unlock time is reached and required fee is paid"
                .to_owned(),
        ));
    };

    let Some((available, frozen, locked)) = sqlx::query_as::<_, (BigDecimal, BigDecimal, BigDecimal)>(
        "SELECT available, frozen, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ? FOR UPDATE",
    )
    .bind(user_id)
    .bind(row.asset_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Err(AppError::Validation(
            "wallet account is required before unlock release".to_owned(),
        ));
    };

    if locked < row.unlock_quantity {
        return Err(AppError::Validation(
            "wallet locked balance is insufficient for unlock release".to_owned(),
        ));
    }

    let available_after = available + row.unlock_quantity.clone();
    let locked_after = locked - row.unlock_quantity.clone();

    let (remaining_before,) = sqlx::query_as::<_, (BigDecimal,)>(
        "SELECT remaining_amount FROM asset_lock_positions WHERE id = ? FOR UPDATE",
    )
    .bind(row.lock_position_id)
    .fetch_one(&mut *tx)
    .await?;
    let remaining_after = remaining_before - row.unlock_quantity.clone();
    let lock_status = if remaining_after == 0 {
        "released"
    } else {
        "active"
    };

    sqlx::query(
        r#"UPDATE asset_lock_positions
           SET released_amount = released_amount + ?,
               remaining_amount = ?,
               status = ?
           WHERE id = ? AND remaining_amount >= ?"#,
    )
    .bind(&row.unlock_quantity)
    .bind(&remaining_after)
    .bind(lock_status)
    .bind(row.lock_position_id)
    .bind(&row.unlock_quantity)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE asset_unlock_records SET status = 'released' WHERE id = ?")
        .bind(row.unlock_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, locked = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&locked_after)
    .bind(user_id)
    .bind(row.asset_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'new_coin_unlock_release', ?, 'locked', ?, ?, ?, ?, 'new_coin_unlock', ?),
                  (?, ?, 'new_coin_unlock_release', ?, 'available', ?, ?, ?, ?, 'new_coin_unlock', ?)"#,
    )
    .bind(user_id)
    .bind(row.asset_id)
    .bind(-row.unlock_quantity.clone())
    .bind(&locked_after)
    .bind(&available_after)
    .bind(&frozen)
    .bind(&locked_after)
    .bind(id)
    .bind(user_id)
    .bind(row.asset_id)
    .bind(&row.unlock_quantity)
    .bind(&available_after)
    .bind(&available_after)
    .bind(&frozen)
    .bind(&locked_after)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(ReleaseUnlockOutcome {
        asset_id: row.asset_id,
        unlock_quantity: row.unlock_quantity,
        released: true,
    })
}

fn lock_positions_for_project(
    project: &NewCoinProjectRuleRow,
    user_id: u64,
    asset_id: u64,
    source_id: &str,
    quantity: BigDecimal,
    source_time: chrono::DateTime<Utc>,
    source_type: &str,
) -> AppResult<Vec<NewCoinLockPositionInsert>> {
    let unlock_rule = unlock_rule_from_project(project)?;
    let application = apply_unlock_rule(
        &unlock_rule,
        vec![crate::modules::new_coin::UnlockSource {
            user_id: user_id.to_string(),
            asset_id: asset_id.to_string(),
            source_id: source_id.to_owned(),
            amount: quantity,
            source_time,
        }],
    )
    .map_err(|error| AppError::Validation(format!("invalid new coin unlock rule: {error:?}")))?;

    Ok(application
        .lock_positions
        .into_iter()
        .map(|position| NewCoinLockPositionInsert {
            user_id,
            asset_id,
            unlock_type: position.unlock_type,
            unlock_at: position.unlock_at,
            amount: position.remaining_amount,
            merge_key: position.merge_key,
            source_type: source_type.to_owned(),
            source_id: source_id.to_owned(),
        })
        .collect())
}

fn unlock_rule_from_project(project: &NewCoinProjectRuleRow) -> AppResult<UnlockRule> {
    match project.unlock_type.as_str() {
        "immediate_on_listing" => Ok(UnlockRule::ImmediateOnListing {
            listed_at: project.listed_at.ok_or_else(|| {
                AppError::Validation("listed_at is required for immediate unlock".to_owned())
            })?,
        }),
        "fixed_time" => Ok(UnlockRule::FixedTime {
            unlock_at: project.fixed_unlock_at.ok_or_else(|| {
                AppError::Validation("fixed_unlock_at is required for fixed unlock".to_owned())
            })?,
        }),
        "relative_period" => Ok(UnlockRule::RelativePeriod {
            seconds_after_source: project
                .relative_unlock_seconds
                .ok_or_else(|| {
                    AppError::Validation(
                        "relative_unlock_seconds is required for relative unlock".to_owned(),
                    )
                })?
                .try_into()
                .map_err(|_| {
                    AppError::Validation("relative unlock period is too large".to_owned())
                })?,
        }),
        _ => Err(AppError::Validation(
            "unsupported new coin unlock_type".to_owned(),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_new_coin_allocation(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    quantity: &BigDecimal,
    lock_positions: &[NewCoinLockPositionInsert],
    unlock_price: &BigDecimal,
    purchase_cost: &BigDecimal,
    project: &NewCoinProjectRuleRow,
    ledger: NewCoinLedgerMetadata<'_>,
) -> AppResult<Option<u64>> {
    if lock_positions.is_empty() {
        credit_wallet_available(
            tx,
            user_id,
            asset_id,
            quantity,
            ledger.change_type,
            ledger.ref_type,
            ledger.ref_id,
        )
        .await?;
        return Ok(None);
    }

    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let locked_after = wallet.locked.clone() + quantity.clone();
    sqlx::query("UPDATE wallet_accounts SET locked = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&locked_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_new_coin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        quantity.clone(),
        "locked",
        &locked_after,
        &wallet.available,
        &wallet.frozen,
        &locked_after,
        ledger.change_type,
        ledger.ref_type,
        ledger.ref_id,
    )
    .await?;

    let mut first_lock_position_id = None;
    for position in lock_positions {
        let position_id = upsert_lock_position(tx, position).await?;
        ensure_unlock_record(
            tx,
            user_id,
            asset_id,
            position_id,
            &position.amount,
            unlock_price,
            purchase_cost,
            project,
            &position.source_id,
        )
        .await?;
        if first_lock_position_id.is_none() {
            first_lock_position_id = Some(position_id);
        }
    }
    Ok(first_lock_position_id)
}

#[allow(clippy::too_many_arguments)]
async fn ensure_unlock_record(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    quantity: &BigDecimal,
    unlock_price: &BigDecimal,
    purchase_cost: &BigDecimal,
    project: &NewCoinProjectRuleRow,
    source_id: &str,
) -> AppResult<()> {
    let (fee_paid_status, unlock_fee_amount) =
        unlock_fee_fields(project, quantity, unlock_price, purchase_cost)?;
    sqlx::query(
        r#"INSERT INTO asset_unlock_records
           (user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
            unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
            unlock_fee_amount, fee_paid_status, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(lock_position_id)
    .bind(quantity)
    .bind(unlock_price)
    .bind(project.unlock_fee_enabled)
    .bind(&project.unlock_fee_rate)
    .bind(&project.unlock_fee_basis)
    .bind(project.unlock_fee_asset)
    .bind(&unlock_fee_amount)
    .bind(fee_paid_status)
    .bind(source_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn unlock_fee_fields(
    project: &NewCoinProjectRuleRow,
    quantity: &BigDecimal,
    unlock_price: &BigDecimal,
    purchase_cost: &BigDecimal,
) -> AppResult<(&'static str, Option<BigDecimal>)> {
    if !project.unlock_fee_enabled {
        return Ok(("not_required", None));
    }
    let fee_rate = project.unlock_fee_rate.clone().unwrap_or_default();
    if fee_rate <= BigDecimal::default() {
        return Ok(("not_required", Some(BigDecimal::default())));
    }
    if project.unlock_fee_asset.is_none() {
        return Err(AppError::Validation(
            "unlock_fee_asset is required when unlock fee is enabled".to_owned(),
        ));
    }
    let market_value = quantity.clone() * unlock_price.clone();
    let basis_amount = match project
        .unlock_fee_basis
        .as_deref()
        .unwrap_or("market_value")
    {
        "market_value" => market_value,
        "profit" => std::cmp::max(market_value - purchase_cost.clone(), BigDecimal::default()),
        _ => {
            return Err(AppError::Validation(
                "unsupported unlock_fee_basis".to_owned(),
            ));
        }
    };
    let fee_amount = basis_amount * fee_rate;
    let fee_paid_status = if fee_amount > BigDecimal::default() {
        "pending"
    } else {
        "not_required"
    };
    Ok((fee_paid_status, Some(fee_amount)))
}

async fn debit_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    ledger: NewCoinLedgerMetadata<'_>,
) -> AppResult<()> {
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for new coin order: requested {}, available {}, locked {}",
            amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_new_coin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        ledger.change_type,
        ledger.ref_type,
        ledger.ref_id,
    )
    .await
}

async fn credit_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let available_after = wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_new_coin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await
}

async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<NewCoinWalletRow> {
    sqlx::query_as::<_, NewCoinWalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("wallet account is required for new coin order".to_owned()))
}

async fn lock_or_create_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<NewCoinWalletRow> {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    lock_wallet_row(tx, user_id, asset_id).await
}

async fn upsert_lock_position(
    tx: &mut Transaction<'_, MySql>,
    position: &NewCoinLockPositionInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount,
            released_amount, remaining_amount, merge_key, status)
           VALUES (?, ?, ?, ?, 0, 0, 0, ?, 'active')
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(position.user_id)
    .bind(position.asset_id)
    .bind(&position.unlock_type)
    .bind(position.unlock_at.naive_utc())
    .bind(&position.merge_key)
    .execute(&mut **tx)
    .await?;

    let position_id = if result.last_insert_id() == 0 {
        sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM asset_lock_positions WHERE merge_key = ? LIMIT 1 FOR UPDATE",
        )
        .bind(&position.merge_key)
        .fetch_one(&mut **tx)
        .await?
        .0
    } else {
        result.last_insert_id()
    };

    let inserted = sqlx::query(
        r#"INSERT IGNORE INTO asset_lock_position_sources
           (lock_position_id, source_type, source_id, source_amount, source_time)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(position_id)
    .bind(&position.source_type)
    .bind(&position.source_id)
    .bind(&position.amount)
    .bind(position.unlock_at.naive_utc())
    .execute(&mut **tx)
    .await?;

    if inserted.rows_affected() > 0 {
        sqlx::query(
            r#"UPDATE asset_lock_positions
               SET locked_amount = locked_amount + ?,
                   remaining_amount = remaining_amount + ?,
                   status = 'active'
               WHERE id = ?"#,
        )
        .bind(&position.amount)
        .bind(&position.amount)
        .bind(position_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(position_id)
}

#[allow(clippy::too_many_arguments)]
async fn insert_new_coin_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn ensure_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &BigDecimal::default() {
        Err(AppError::Validation(format!("{field} must be positive")))
    } else {
        Ok(())
    }
}

fn ensure_idempotency_key(value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        Err(AppError::Validation(
            "idempotency_key must not be empty".to_owned(),
        ))
    } else {
        Ok(())
    }
}

fn lifecycle_status(value: &str) -> AppResult<LifecycleStatus> {
    match value {
        "preheat" => Ok(LifecycleStatus::Preheat),
        "subscription" => Ok(LifecycleStatus::Subscription),
        "distribution" => Ok(LifecycleStatus::Distribution),
        "listed" => Ok(LifecycleStatus::Listed),
        _ => Err(AppError::Validation(
            "unsupported lifecycle_status".to_owned(),
        )),
    }
}

fn ensure_unlock_fee_payment_matches(
    expectation: &UnlockFeeExpectation,
    payment_asset_id: u64,
    amount: &BigDecimal,
) -> AppResult<()> {
    if !expectation.unlock_fee_enabled {
        return Err(AppError::Validation(
            "unlock fee payment is not required for this unlock".to_owned(),
        ));
    }
    if expectation.unlock_fee_asset != Some(payment_asset_id) {
        return Err(AppError::Validation(
            "unlock fee payment asset does not match required asset".to_owned(),
        ));
    }
    let Some(expected_amount) = &expectation.unlock_fee_amount else {
        return Err(AppError::Validation(
            "unlock fee amount is not configured".to_owned(),
        ));
    };
    if amount <= &BigDecimal::default()
        || amount.normalized().cmp(&expected_amount.normalized()) != Ordering::Equal
    {
        return Err(AppError::Validation(
            "unlock fee payment amount does not match required amount".to_owned(),
        ));
    }
    Ok(())
}

fn map_new_coin_repository_error(
    error: crate::modules::new_coin::NewCoinRepositoryError,
) -> AppError {
    AppError::Internal(format!("{error:?}"))
}

#[allow(dead_code)]
fn filtered_query<'a>(base: &'a str, user_id: u64, limit: u32) -> QueryBuilder<'a, MySql> {
    let mut builder = QueryBuilder::<MySql>::new(base);
    builder.push_bind(user_id);
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(limit as i64);
    builder
}
