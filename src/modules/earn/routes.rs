use crate::{
    error::{AppError, AppResult},
    modules::{
        auth::{AdminAuth, UserAuth},
        events::EventBroadcastMessage,
    },
    state::AppState,
    time::unix_millis,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/earn/products", get(list_active_products))
        .route(
            "/earn/subscriptions",
            get(list_subscriptions).post(subscribe),
        )
        .route("/earn/subscriptions/:id/redeem", post(redeem_subscription))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/earn/products",
            get(list_admin_products).post(create_product),
        )
        .route("/earn/products/:id/status", patch(update_product_status))
        .route("/earn/subscriptions", get(list_admin_subscriptions))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminSubscriptionsQuery {
    limit: Option<u32>,
    user_id: Option<u64>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubscribeEarnRequest {
    product_id: u64,
    amount: BigDecimal,
    idempotency_key: String,
}

#[derive(Debug, Deserialize)]
struct CreateEarnProductRequest {
    asset_id: u64,
    name: String,
    term_days: u32,
    apr_rate: BigDecimal,
    min_subscribe: BigDecimal,
    max_subscribe: Option<BigDecimal>,
    status: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateEarnProductStatusRequest {
    status: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct EarnProductResponse {
    id: u64,
    asset_id: u64,
    asset_symbol: String,
    name: String,
    term_days: u32,
    apr_rate: BigDecimal,
    min_subscribe: BigDecimal,
    max_subscribe: Option<BigDecimal>,
    status: String,
}

#[derive(Debug, Serialize)]
struct EarnProductsResponse {
    products: Vec<EarnProductResponse>,
}

#[derive(Debug, Serialize)]
struct EarnSubscriptionsResponse {
    subscriptions: Vec<EarnSubscriptionResponse>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
struct EarnSubscriptionResponse {
    id: u64,
    user_id: u64,
    product_id: u64,
    asset_id: u64,
    amount: BigDecimal,
    apr_rate: BigDecimal,
    term_days: u32,
    status: String,
    idempotency_key: String,
    #[serde(with = "unix_millis")]
    matures_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct SubscribeEarnResponse {
    subscription: EarnSubscriptionResponse,
}

#[derive(Debug, Serialize)]
struct RedeemEarnResponse {
    subscription: EarnSubscriptionResponse,
    principal_amount: BigDecimal,
    yield_amount: BigDecimal,
    redeem_amount: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct EarnProductRuleRow {
    id: u64,
    asset_id: u64,
    term_days: u32,
    apr_rate: BigDecimal,
    min_subscribe: BigDecimal,
    max_subscribe: Option<BigDecimal>,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct EarnWalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

async fn list_active_products(
    UserAuth(_claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<EarnProductsResponse>> {
    list_products(
        mysql_pool(&state)?,
        Some("active"),
        route_limit(query.limit),
    )
    .await
}

async fn list_admin_products(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<EarnProductsResponse>> {
    list_products(mysql_pool(&state)?, None, route_limit(query.limit)).await
}

async fn list_subscriptions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<EarnSubscriptionsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let subscriptions = sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate, term_days,
                  status, idempotency_key, matures_at
           FROM earn_subscriptions
           WHERE user_id = ?
           ORDER BY created_at DESC, id DESC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;
    Ok(Json(EarnSubscriptionsResponse { subscriptions }))
}

async fn list_admin_subscriptions(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminSubscriptionsQuery>,
) -> AppResult<Json<EarnSubscriptionsResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate, term_days,
                  status, idempotency_key, matures_at
           FROM earn_subscriptions"#,
    );
    let mut has_filter = false;
    if let Some(user_id) = query.user_id {
        builder.push(" WHERE user_id = ");
        builder.push_bind(user_id);
        has_filter = true;
    }
    if let Some(status) = optional_string(query.status) {
        builder.push(if has_filter {
            " AND status = "
        } else {
            " WHERE status = "
        });
        builder.push_bind(status);
    }
    builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let subscriptions = builder
        .build_query_as::<EarnSubscriptionResponse>()
        .fetch_all(&mysql_pool(&state)?)
        .await?;
    Ok(Json(EarnSubscriptionsResponse { subscriptions }))
}

async fn create_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateEarnProductRequest>,
) -> AppResult<Json<EarnProductResponse>> {
    validate_create_product_request(&request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let status = normalized_product_status(request.status.as_deref().unwrap_or("active"))?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    ensure_asset_exists(&mut tx, request.asset_id).await?;
    let product_id = sqlx::query(
        r#"INSERT INTO earn_products
           (asset_id, name, term_days, apr_rate, min_subscribe, max_subscribe, status)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(request.asset_id)
    .bind(request.name.trim())
    .bind(request.term_days)
    .bind(&request.apr_rate)
    .bind(&request.min_subscribe)
    .bind(&request.max_subscribe)
    .bind(&status)
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    let product = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_product.create",
        product.id,
        None,
        Some(product_audit_json(&product)),
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(product))
}

async fn update_product_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateEarnProductStatusRequest>,
) -> AppResult<Json<EarnProductResponse>> {
    let status = normalized_product_status(&request.status)?;
    validate_optional_reason(request.reason.as_deref())?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_product_by_id(&mut tx, product_id).await?;
    sqlx::query("UPDATE earn_products SET status = ? WHERE id = ?")
        .bind(&status)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
    let after = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_product.update_status",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}

async fn list_products(
    pool: Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Json<EarnProductsResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.term_days, products.apr_rate,
                  products.min_subscribe, products.max_subscribe, products.status
           FROM earn_products products
           INNER JOIN assets ON assets.id = products.asset_id"#,
    );

    if let Some(status) = status {
        builder.push(" WHERE products.status = ");
        builder.push_bind(status);
    }

    builder.push(" ORDER BY products.id DESC LIMIT ");
    builder.push_bind(limit as i64);

    let products = builder
        .build_query_as::<EarnProductResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(EarnProductsResponse { products }))
}

async fn subscribe(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<SubscribeEarnRequest>,
) -> AppResult<Json<SubscribeEarnResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    validate_amount(&request.amount)?;
    let (subscription, is_new_subscription) = subscribe_in_tx(
        &mysql_pool(&state)?,
        user_id,
        request.product_id,
        request.amount,
        idempotency_key,
    )
    .await?;
    let response = SubscribeEarnResponse { subscription };
    if is_new_subscription && let Some(hub) = &state.event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "earn.subscription.created",
                "subscription_id": response.subscription.id,
                "product_id": response.subscription.product_id,
                "asset_id": response.subscription.asset_id,
                "amount": response.subscription.amount,
                "status": response.subscription.status,
            })
            .to_string(),
        ));
    }
    Ok(Json(response))
}

async fn redeem_subscription(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(subscription_id): Path<u64>,
) -> AppResult<Json<RedeemEarnResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let (response, is_new_redemption) =
        redeem_subscription_in_tx(&mysql_pool(&state)?, user_id, subscription_id).await?;
    if is_new_redemption && let Some(hub) = &state.event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "earn.subscription.redeemed",
                "subscription_id": response.subscription.id,
                "product_id": response.subscription.product_id,
                "asset_id": response.subscription.asset_id,
                "principal_amount": response.principal_amount,
                "yield_amount": response.yield_amount,
                "redeem_amount": response.redeem_amount,
                "status": response.subscription.status,
            })
            .to_string(),
        ));
    }
    Ok(Json(response))
}

async fn subscribe_in_tx(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    amount: BigDecimal,
    idempotency_key: String,
) -> AppResult<(EarnSubscriptionResponse, bool)> {
    if let Some(existing) =
        existing_subscription_for_idempotency_key_readonly(pool, user_id, &idempotency_key).await?
    {
        ensure_existing_subscription_matches_request(&existing, product_id, &amount)?;
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    let product = match lock_active_product(&mut tx, product_id).await {
        Ok(product) => product,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            if let Some(existing) = replay_existing_subscription_if_present(
                pool,
                user_id,
                product_id,
                &amount,
                &idempotency_key,
            )
            .await?
            {
                return Ok((existing, false));
            }
            return Err(AppError::NotFound);
        }
        Err(error) => return Err(error),
    };
    validate_product_amount(&amount, &product)?;
    let matures_at = earn_matures_at(product.term_days)?;
    let subscription_id = match sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status, idempotency_key, matures_at)
           VALUES (?, ?, ?, ?, ?, ?, 'subscribed', ?, ?)"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(product.asset_id)
    .bind(&amount)
    .bind(&product.apr_rate)
    .bind(product.term_days)
    .bind(&idempotency_key)
    .bind(matures_at)
    .execute(&mut *tx)
    .await
    {
        Ok(result) => result.last_insert_id(),
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            return replay_existing_subscription(pool, user_id, product_id, &amount, &idempotency_key)
                .await
                .map(|subscription| (subscription, false));
        }
        Err(error) => return Err(AppError::Database(error)),
    };

    let wallet = lock_wallet_row(&mut tx, user_id, product.asset_id).await?;
    if wallet.available < amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for earn subscription: requested {}, available {}, locked {}",
            amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();

    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(product.asset_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'earn_subscribe', ?, 'available', ?, ?, ?, ?, 'earn_subscription', ?)"#,
    )
    .bind(user_id)
    .bind(product.asset_id)
    .bind(-amount.clone())
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&wallet.locked)
    .bind(subscription_id.to_string())
    .execute(&mut *tx)
    .await?;

    let subscription = load_subscription_by_id(&mut tx, subscription_id).await?;
    tx.commit().await?;
    Ok((subscription, true))
}

async fn redeem_subscription_in_tx(
    pool: &Pool<MySql>,
    user_id: u64,
    subscription_id: u64,
) -> AppResult<(RedeemEarnResponse, bool)> {
    let mut tx = pool.begin().await?;
    let subscription = lock_subscription_by_id(&mut tx, user_id, subscription_id).await?;

    if subscription.status == "redeemed" {
        let (principal_amount, yield_amount, redeem_amount) =
            load_redeemed_amounts_from_ledger(&mut tx, &subscription).await?;
        tx.commit().await?;
        return Ok((
            RedeemEarnResponse {
                subscription,
                principal_amount,
                yield_amount,
                redeem_amount,
            },
            false,
        ));
    }
    if subscription.status != "subscribed" {
        return Err(AppError::Conflict(
            "earn subscription is not redeemable".to_owned(),
        ));
    }
    if subscription.matures_at > Utc::now() {
        return Err(AppError::Validation(
            "earn subscription has not matured".to_owned(),
        ));
    }

    let principal_amount = subscription.amount.clone();
    let yield_amount = earn_redeem_yield_amount(&subscription);
    let redeem_amount = principal_amount.clone() + yield_amount.clone();
    let wallet = lock_wallet_row(&mut tx, subscription.user_id, subscription.asset_id).await?;
    let available_after = wallet.available.clone() + redeem_amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(subscription.user_id)
        .bind(subscription.asset_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'earn_redeem', ?, 'available', ?, ?, ?, ?, 'earn_subscription', ?)"#,
    )
    .bind(subscription.user_id)
    .bind(subscription.asset_id)
    .bind(&redeem_amount)
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&wallet.locked)
    .bind(subscription.id.to_string())
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE earn_subscriptions SET status = 'redeemed', redeemed_at = CURRENT_TIMESTAMP(6) WHERE id = ?")
        .bind(subscription.id)
        .execute(&mut *tx)
        .await?;
    let redeemed_subscription = load_subscription_by_id(&mut tx, subscription.id).await?;
    tx.commit().await?;
    Ok((
        RedeemEarnResponse {
            subscription: redeemed_subscription,
            principal_amount,
            yield_amount,
            redeem_amount,
        },
        true,
    ))
}

async fn replay_existing_subscription(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<EarnSubscriptionResponse> {
    replay_existing_subscription_if_present(pool, user_id, product_id, amount, idempotency_key)
        .await?
        .ok_or_else(|| AppError::Conflict("earn idempotency key is being committed".to_owned()))
}

async fn replay_existing_subscription_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<EarnSubscriptionResponse>> {
    let mut tx = pool.begin().await?;
    let Some(existing) =
        existing_subscription_for_idempotency_key(&mut tx, user_id, idempotency_key).await?
    else {
        return Ok(None);
    };
    ensure_existing_subscription_matches_request(&existing, product_id, amount)?;
    tx.commit().await?;
    Ok(Some(existing))
}

async fn existing_subscription_for_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<EarnSubscriptionResponse>> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate, term_days,
                  status, idempotency_key, matures_at
           FROM earn_subscriptions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn existing_subscription_for_idempotency_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<EarnSubscriptionResponse>> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate, term_days,
                  status, idempotency_key, matures_at
           FROM earn_subscriptions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

async fn ensure_asset_exists(tx: &mut Transaction<'_, MySql>, asset_id: u64) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE id = ? LIMIT 1")
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

async fn load_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<EarnProductResponse> {
    sqlx::query_as::<_, EarnProductResponse>(
        r#"SELECT products.id, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.term_days, products.apr_rate,
                  products.min_subscribe, products.max_subscribe, products.status
           FROM earn_products products
           INNER JOIN assets ON assets.id = products.asset_id
           WHERE products.id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn lock_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<EarnProductResponse> {
    sqlx::query_as::<_, EarnProductResponse>(
        r#"SELECT products.id, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.term_days, products.apr_rate,
                  products.min_subscribe, products.max_subscribe, products.status
           FROM earn_products products
           INNER JOIN assets ON assets.id = products.asset_id
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn lock_active_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<EarnProductRuleRow> {
    let product = sqlx::query_as::<_, EarnProductRuleRow>(
        r#"SELECT id, asset_id, term_days, apr_rate, min_subscribe, max_subscribe, status
           FROM earn_products
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if product.status != "active" {
        return Err(AppError::NotFound);
    }
    Ok(product)
}

async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<EarnWalletRow> {
    sqlx::query_as::<_, EarnWalletRow>(
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
    .ok_or_else(|| AppError::Validation("wallet account is required for earn".to_owned()))
}

async fn load_subscription_by_id(
    tx: &mut Transaction<'_, MySql>,
    subscription_id: u64,
) -> AppResult<EarnSubscriptionResponse> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate, term_days,
                  status, idempotency_key, matures_at
           FROM earn_subscriptions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(subscription_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn lock_subscription_by_id(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    subscription_id: u64,
) -> AppResult<EarnSubscriptionResponse> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate, term_days,
                  status, idempotency_key, matures_at
           FROM earn_subscriptions
           WHERE id = ? AND user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(subscription_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn load_redeemed_amounts_from_ledger(
    tx: &mut Transaction<'_, MySql>,
    subscription: &EarnSubscriptionResponse,
) -> AppResult<(BigDecimal, BigDecimal, BigDecimal)> {
    let ref_id = subscription.id.to_string();
    let principal_amount = sqlx::query_scalar::<_, BigDecimal>(
        r#"SELECT -amount
           FROM wallet_ledger
           WHERE user_id = ?
             AND asset_id = ?
             AND change_type = 'earn_subscribe'
             AND ref_type = 'earn_subscription'
             AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(subscription.user_id)
    .bind(subscription.asset_id)
    .bind(&ref_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Internal("earn subscribe ledger is missing".to_owned()))?;

    let redeem_amount = sqlx::query_scalar::<_, BigDecimal>(
        r#"SELECT amount
           FROM wallet_ledger
           WHERE user_id = ?
             AND asset_id = ?
             AND change_type = 'earn_redeem'
             AND ref_type = 'earn_subscription'
             AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(subscription.user_id)
    .bind(subscription.asset_id)
    .bind(&ref_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Internal("earn redeem ledger is missing".to_owned()))?;

    let yield_amount = redeem_amount.clone() - principal_amount.clone();
    Ok((principal_amount, yield_amount, redeem_amount))
}

async fn insert_admin_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, 'earn_product', ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(optional_string(reason))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn product_audit_json(product: &EarnProductResponse) -> Value {
    json!({
        "id": product.id,
        "asset_id": product.asset_id,
        "asset_symbol": product.asset_symbol,
        "name": product.name,
        "term_days": product.term_days,
        "apr_rate": product.apr_rate,
        "min_subscribe": product.min_subscribe,
        "max_subscribe": product.max_subscribe,
        "status": product.status,
    })
}

fn validate_create_product_request(request: &CreateEarnProductRequest) -> AppResult<()> {
    if request.asset_id == 0 {
        return Err(AppError::Validation("asset_id is required".to_owned()));
    }
    let Some(name) = optional_string(Some(request.name.clone())) else {
        return Err(AppError::Validation(
            "earn product name is required".to_owned(),
        ));
    };
    if name.chars().count() > EARN_PRODUCT_NAME_MAX_LEN {
        return Err(AppError::Validation(
            "earn product name is too long".to_owned(),
        ));
    }
    validate_term_days(request.term_days)?;
    validate_apr_rate(&request.apr_rate)?;
    validate_amount(&request.min_subscribe)?;
    if let Some(max_subscribe) = &request.max_subscribe {
        validate_amount(max_subscribe)?;
        if max_subscribe < &request.min_subscribe {
            return Err(AppError::Validation(
                "earn product max_subscribe must be greater than or equal to min_subscribe"
                    .to_owned(),
            ));
        }
    }
    if let Some(status) = request.status.as_deref() {
        normalized_product_status(status)?;
    }
    validate_optional_reason(request.reason.as_deref())?;
    Ok(())
}

fn validate_optional_reason(reason: Option<&str>) -> AppResult<()> {
    if let Some(reason) = reason
        && reason.trim().chars().count() > EARN_AUDIT_REASON_MAX_LEN
    {
        return Err(AppError::Validation(
            "earn product reason is too long".to_owned(),
        ));
    }
    Ok(())
}

fn normalized_product_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "earn product status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "earn product status must be active or disabled".to_owned(),
        )),
    }
}

fn ensure_existing_subscription_matches_request(
    existing: &EarnSubscriptionResponse,
    product_id: u64,
    amount: &BigDecimal,
) -> AppResult<()> {
    if existing.product_id != product_id || existing.amount != *amount {
        return Err(AppError::Conflict(
            "earn idempotency key belongs to a different request".to_owned(),
        ));
    }
    Ok(())
}

fn earn_redeem_yield_amount(subscription: &EarnSubscriptionResponse) -> BigDecimal {
    let yearly_yield = subscription.amount.clone() * subscription.apr_rate.clone();
    (yearly_yield * BigDecimal::from(subscription.term_days) / BigDecimal::from(365)).with_scale(18)
}

fn validate_product_amount(amount: &BigDecimal, product: &EarnProductRuleRow) -> AppResult<()> {
    if amount < &product.min_subscribe {
        return Err(AppError::Validation(
            "earn subscription amount is below product minimum".to_owned(),
        ));
    }
    if let Some(max_subscribe) = &product.max_subscribe
        && amount > max_subscribe
    {
        return Err(AppError::Validation(
            "earn subscription amount exceeds product maximum".to_owned(),
        ));
    }
    Ok(())
}

const EARN_PRODUCT_MAX_TERM_DAYS: u32 = 3_650;
const EARN_PRODUCT_NAME_MAX_LEN: usize = 128;
const EARN_AUDIT_REASON_MAX_LEN: usize = 512;
const EARN_APR_MAX_SCALE: i64 = 8;
const EARN_APR_MAX_INTEGER_DIGITS: usize = 10;
const EARN_AMOUNT_MAX_SCALE: i64 = 18;
const EARN_AMOUNT_MAX_INTEGER_DIGITS: usize = 20;

fn earn_matures_at(term_days: u32) -> AppResult<chrono::DateTime<Utc>> {
    Utc::now()
        .checked_add_signed(chrono::TimeDelta::days(term_days as i64))
        .ok_or_else(|| {
            AppError::Validation("earn product term_days exceeds supported maximum".to_owned())
        })
}

fn validate_term_days(term_days: u32) -> AppResult<()> {
    if term_days == 0 {
        return Err(AppError::Validation(
            "earn product term_days must be positive".to_owned(),
        ));
    }
    if term_days > EARN_PRODUCT_MAX_TERM_DAYS {
        return Err(AppError::Validation(
            "earn product term_days exceeds supported maximum".to_owned(),
        ));
    }
    Ok(())
}

fn validate_apr_rate(apr_rate: &BigDecimal) -> AppResult<()> {
    if apr_rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "earn product apr_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        apr_rate,
        EARN_APR_MAX_SCALE,
        EARN_APR_MAX_INTEGER_DIGITS,
        "earn product apr_rate",
    )
}

fn validate_amount(amount: &BigDecimal) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "earn subscription amount must be positive".to_owned(),
        ));
    }

    validate_decimal_storage(
        amount,
        EARN_AMOUNT_MAX_SCALE,
        EARN_AMOUNT_MAX_INTEGER_DIGITS,
        "earn subscription amount",
    )
}

fn validate_decimal_storage(
    value: &BigDecimal,
    max_scale: i64,
    max_integer_digits: usize,
    label: &str,
) -> AppResult<()> {
    let (digits, scale) = value.as_bigint_and_exponent();
    if scale > max_scale {
        return Err(AppError::Validation(format!(
            "{label} supports at most {max_scale} decimal places"
        )));
    }

    let significant_digits = digits
        .to_str_radix(10)
        .trim_start_matches('-')
        .trim_start_matches('0')
        .len();
    let integer_digits = if scale >= 0 {
        significant_digits.saturating_sub(scale as usize)
    } else {
        significant_digits.saturating_add(scale.unsigned_abs() as usize)
    };
    if integer_digits > max_integer_digits {
        return Err(AppError::Validation(format!(
            "{label} exceeds decimal storage precision"
        )));
    }

    Ok(())
}

fn normalize_idempotency_key(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "idempotency_key is required for earn subscriptions".to_owned(),
        ));
    }
    if trimmed.len() > 255 {
        return Err(AppError::Validation(
            "idempotency_key is too long for earn subscriptions".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for earn routes".to_owned())
    })
}

fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
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

fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    let Some(database_error) = error.as_database_error() else {
        return false;
    };
    matches!(database_error.code().as_deref(), Some("1062"))
        || database_error.message().contains("Duplicate entry")
}
