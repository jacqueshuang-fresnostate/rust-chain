use crate::{
    error::{AppError, AppResult},
    modules::{
        auth::UserAuth,
        convert::{
            ConvertQuoteCacheEntry, ConvertQuoteInsert, MySqlConvertRepository, QuoteId,
            RedisConvertQuoteCache,
        },
        events::EventBroadcastMessage,
    },
    state::AppState,
    time::unix_millis,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{get, post},
};
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{MySql, Pool, QueryBuilder, Transaction};
use uuid::Uuid;

const QUOTE_TTL_SECONDS: i64 = 30;

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/convert/pairs", get(list_pairs))
        .route("/convert/quote", post(create_quote))
        .route("/convert/confirm", post(confirm_quote))
        .route("/convert/orders", get(list_orders))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ConvertOrdersQuery {
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CreateConvertQuoteRequest {
    from_asset_id: u64,
    to_asset_id: u64,
    from_amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
struct ConfirmConvertQuoteRequest {
    quote_id: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ConvertPairResponse {
    id: u64,
    from_asset_id: u64,
    to_asset_id: u64,
    pricing_mode: String,
    spread_rate: BigDecimal,
    min_amount: BigDecimal,
    max_amount: Option<BigDecimal>,
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct ConvertPairsResponse {
    pairs: Vec<ConvertPairResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ConvertOrderResponse {
    id: u64,
    quote_id: String,
    convert_pair_id: u64,
    from_asset_id: u64,
    to_asset_id: u64,
    from_amount: BigDecimal,
    to_amount: BigDecimal,
    rate: BigDecimal,
    status: String,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ConvertOrdersResponse {
    orders: Vec<ConvertOrderResponse>,
}

#[derive(Debug, Serialize)]
struct ConvertQuoteResponse {
    quote_id: String,
    convert_pair_id: u64,
    from_asset_id: u64,
    to_asset_id: u64,
    from_amount: BigDecimal,
    to_amount: BigDecimal,
    rate: BigDecimal,
    spread_rate: BigDecimal,
    #[serde(with = "unix_millis")]
    expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ConfirmConvertQuoteResponse {
    quote_id: String,
    confirmed: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct ConvertPairRuleRow {
    id: u64,
    from_asset_id: u64,
    to_asset_id: u64,
    pricing_mode: String,
    spread_rate: BigDecimal,
    min_amount: BigDecimal,
    max_amount: Option<BigDecimal>,
    fixed_rate: Option<BigDecimal>,
}

#[derive(Debug, sqlx::FromRow)]
struct WalletBalanceRow {
    available: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct ConvertSettlementOrderRow {
    from_asset_id: u64,
    to_asset_id: u64,
    from_amount: BigDecimal,
    to_amount: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct ConvertAgentCommissionRuleRow {
    agent_id: u64,
    commission_rate: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct ConvertSettlementWalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

async fn list_pairs(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<ConvertPairsResponse>> {
    let pairs = sqlx::query_as::<_, ConvertPairResponse>(
        r#"SELECT id, from_asset AS from_asset_id, to_asset AS to_asset_id, pricing_mode,
                  spread_rate, min_amount, max_amount, enabled
           FROM convert_pairs
           WHERE enabled = true
           ORDER BY id DESC
           LIMIT ?"#,
    )
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;

    Ok(Json(ConvertPairsResponse { pairs }))
}

async fn create_quote(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateConvertQuoteRequest>,
) -> AppResult<Json<ConvertQuoteResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let redis = RedisConvertQuoteCache::new(redis_manager(&state)?);
    let pair = load_pair_rule(&pool, request.from_asset_id, request.to_asset_id).await?;
    validate_quote_amount(&request.from_amount, &pair)?;
    let balance = load_wallet_balance(&pool, user_id, request.from_asset_id).await?;
    if balance.available < request.from_amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for convert: requested {}, available {}, locked {}",
            request.from_amount, balance.available, balance.locked
        )));
    }

    let rate = pair.fixed_rate.clone().ok_or_else(|| {
        AppError::Validation("convert quote requires active fixed pricing rule".to_owned())
    })?;
    let effective_rate = rate.clone() * (BigDecimal::from(1) - pair.spread_rate.clone());
    let to_amount = request.from_amount.clone() * effective_rate;
    let quote_id = QuoteId(Uuid::now_v7());
    let expires_at = Utc::now() + TimeDelta::seconds(QUOTE_TTL_SECONDS);
    let repository = MySqlConvertRepository::new(pool);

    repository
        .insert_quote(ConvertQuoteInsert {
            quote_id: quote_id.clone(),
            convert_pair_id: pair.id,
            user_id,
            from_asset_id: pair.from_asset_id,
            to_asset_id: pair.to_asset_id,
            from_amount: request.from_amount.clone(),
            to_amount: to_amount.clone(),
            rate: rate.clone(),
            spread_rate: pair.spread_rate.clone(),
            expires_at,
        })
        .await
        .map_err(map_convert_repository_error)?;
    redis
        .save_quote_ttl(ConvertQuoteCacheEntry {
            quote_id: quote_id.clone(),
            user_id: user_id.to_string(),
            from_asset: pair.from_asset_id.to_string(),
            to_asset: pair.to_asset_id.to_string(),
            from_amount: request.from_amount.clone(),
            to_amount: to_amount.clone(),
            expires_at,
            redis_key: format!("convert:quote:{}", quote_id.0),
            ttl_seconds: QUOTE_TTL_SECONDS,
        })
        .await
        .map_err(map_convert_repository_error)?;

    Ok(Json(ConvertQuoteResponse {
        quote_id: quote_id.0.to_string(),
        convert_pair_id: pair.id,
        from_asset_id: pair.from_asset_id,
        to_asset_id: pair.to_asset_id,
        from_amount: request.from_amount,
        to_amount,
        rate,
        spread_rate: pair.spread_rate,
        expires_at,
    }))
}

async fn confirm_quote(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ConfirmConvertQuoteRequest>,
) -> AppResult<Json<ConfirmConvertQuoteResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let quote_id = parse_quote_id(&request.quote_id)?;
    let redis = RedisConvertQuoteCache::new(redis_manager(&state)?);
    let entry = redis
        .get_quote_ttl(&quote_id)
        .await
        .map_err(map_convert_repository_error)?
        .ok_or(AppError::NotFound)?;

    if entry.user_id != user_id.to_string() {
        return Err(AppError::NotFound);
    }
    if Utc::now() >= entry.expires_at {
        return Err(AppError::Validation("convert quote is expired".to_owned()));
    }

    let pool = mysql_pool(&state)?;
    if !quote_exists_for_user(&pool, &quote_id, user_id).await? {
        return Err(AppError::NotFound);
    }
    confirm_and_settle_convert_quote(&pool, &quote_id, user_id).await?;

    let confirmed_quote_id = request.quote_id.clone();
    if let Some(hub) = &state.event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "convert.confirmed",
                "quote_id": confirmed_quote_id,
                "status": "completed",
            })
            .to_string(),
        ));
    }

    Ok(Json(ConfirmConvertQuoteResponse {
        quote_id: request.quote_id,
        confirmed: true,
    }))
}

async fn list_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ConvertOrdersQuery>,
) -> AppResult<Json<ConvertOrdersResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, quote_id, convert_pair_id, from_asset AS from_asset_id,
                  to_asset AS to_asset_id, from_amount, to_amount, rate, status, created_at
           FROM convert_orders
           WHERE user_id = "#,
    );
    builder.push_bind(user_id);

    if let Some(status) = optional_query_string(query.status) {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }

    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let orders = builder
        .build_query_as::<ConvertOrderResponse>()
        .fetch_all(&pool)
        .await?;

    Ok(Json(ConvertOrdersResponse { orders }))
}

async fn load_pair_rule(
    pool: &Pool<MySql>,
    from_asset_id: u64,
    to_asset_id: u64,
) -> AppResult<ConvertPairRuleRow> {
    sqlx::query_as::<_, ConvertPairRuleRow>(
        r#"SELECT pairs.id, pairs.from_asset AS from_asset_id, pairs.to_asset AS to_asset_id,
                  pairs.pricing_mode, pairs.spread_rate, pairs.min_amount, pairs.max_amount,
                  rules.fixed_rate
           FROM convert_pairs pairs
           LEFT JOIN new_coin_convert_rules rules
             ON rules.convert_pair_id = pairs.id AND rules.status = 'active' AND rules.rate_source = 'fixed'
           WHERE pairs.from_asset = ? AND pairs.to_asset = ? AND pairs.enabled = true
           LIMIT 1"#,
    )
    .bind(from_asset_id)
    .bind(to_asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

async fn load_wallet_balance(
    pool: &Pool<MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<WalletBalanceRow> {
    let row = sqlx::query_as::<_, WalletBalanceRow>(
        "SELECT available, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.unwrap_or_else(|| WalletBalanceRow {
        available: BigDecimal::from(0),
        locked: BigDecimal::from(0),
    }))
}

async fn confirm_and_settle_convert_quote(
    pool: &Pool<MySql>,
    quote_id: &QuoteId,
    user_id: u64,
) -> AppResult<()> {
    let quote_id_value = quote_id.0.to_string();
    let mut tx = pool.begin().await?;
    let inserted = insert_order_for_quote_in_tx(&mut tx, &quote_id_value).await?;
    if !inserted {
        return Err(AppError::Conflict(
            "convert quote has already been confirmed".to_owned(),
        ));
    }
    settle_convert_order_in_tx(&mut tx, &quote_id_value, user_id).await?;
    tx.commit().await?;
    Ok(())
}

async fn insert_order_for_quote_in_tx(
    tx: &mut Transaction<'_, MySql>,
    quote_id: &str,
) -> AppResult<bool> {
    // 同一事务内先锁定并插入订单，再完成钱包结算；任意一步失败都会整体回滚，避免留下不可恢复的 pending 订单。
    let result = sqlx::query(
        r#"INSERT INTO convert_orders
           (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount,
            to_amount, rate, status)
           SELECT quotes.quote_id, quotes.convert_pair_id, quotes.user_id, quotes.from_asset,
                  quotes.to_asset, quotes.from_amount, quotes.to_amount, quotes.rate, 'pending'
           FROM convert_quotes quotes
           WHERE quotes.quote_id = ?
           ON DUPLICATE KEY UPDATE quote_id = convert_orders.quote_id"#,
    )
    .bind(quote_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.last_insert_id() != 0)
}

async fn settle_convert_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    quote_id: &str,
    user_id: u64,
) -> AppResult<()> {
    let order = sqlx::query_as::<_, ConvertSettlementOrderRow>(
        r#"SELECT from_asset AS from_asset_id, to_asset AS to_asset_id, from_amount, to_amount
           FROM convert_orders
           WHERE quote_id = ? AND user_id = ? AND status = 'pending'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(quote_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let from_wallet = lock_wallet_row(tx, user_id, order.from_asset_id).await?;
    if from_wallet.available < order.from_amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for convert settlement: requested {}, available {}, locked {}",
            order.from_amount, from_wallet.available, from_wallet.locked
        )));
    }
    let to_wallet = lock_wallet_row(tx, user_id, order.to_asset_id).await?;

    let from_available_after = from_wallet.available.clone() - order.from_amount.clone();
    let to_available_after = to_wallet.available.clone() + order.to_amount.clone();

    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&from_available_after)
        .bind(user_id)
        .bind(order.from_asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&to_available_after)
        .bind(user_id)
        .bind(order.to_asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "UPDATE convert_orders SET status = 'completed' WHERE quote_id = ? AND user_id = ?",
    )
    .bind(quote_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    insert_convert_agent_commission_in_tx(tx, user_id, quote_id, &order.from_amount).await?;

    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'convert_settlement', ?, 'available', ?, ?, ?, ?, 'convert_order', ?),
                  (?, ?, 'convert_settlement', ?, 'available', ?, ?, ?, ?, 'convert_order', ?)"#,
    )
    .bind(user_id)
    .bind(order.from_asset_id)
    .bind(-order.from_amount.clone())
    .bind(&from_available_after)
    .bind(&from_available_after)
    .bind(&from_wallet.frozen)
    .bind(&from_wallet.locked)
    .bind(quote_id)
    .bind(user_id)
    .bind(order.to_asset_id)
    .bind(&order.to_amount)
    .bind(&to_available_after)
    .bind(&to_available_after)
    .bind(&to_wallet.frozen)
    .bind(&to_wallet.locked)
    .bind(quote_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn insert_convert_agent_commission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    source_id: &str,
    source_amount: &BigDecimal,
) -> AppResult<()> {
    let Some(rule) = sqlx::query_as::<_, ConvertAgentCommissionRuleRow>(
        r#"SELECT referrals.root_agent_id AS agent_id, rules.commission_rate
           FROM user_referrals referrals
           INNER JOIN agent_commission_rules rules
             ON rules.agent_id = referrals.root_agent_id
            AND rules.product_type = 'convert'
            AND rules.status = 'active'
           WHERE referrals.user_id = ? AND referrals.root_agent_id IS NOT NULL
           ORDER BY rules.id DESC
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(());
    };

    let commission_amount = (source_amount.clone() * rule.commission_rate).with_scale(18);
    sqlx::query(
        r#"INSERT INTO agent_commission_records
           (agent_id, user_id, source_type, source_id, source_amount, commission_amount, status)
           VALUES (?, ?, 'convert_order', ?, ?, ?, 'pending')
           ON DUPLICATE KEY UPDATE id = agent_commission_records.id"#,
    )
    .bind(rule.agent_id)
    .bind(user_id)
    .bind(source_id)
    .bind(source_amount)
    .bind(commission_amount)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<ConvertSettlementWalletRow> {
    sqlx::query_as::<_, ConvertSettlementWalletRow>(
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
    .ok_or_else(|| {
        AppError::Validation("wallet account is required for convert settlement".to_owned())
    })
}

async fn quote_exists_for_user(
    pool: &Pool<MySql>,
    quote_id: &QuoteId,
    user_id: u64,
) -> AppResult<bool> {
    let row = sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM convert_quotes WHERE quote_id = ? AND user_id = ? LIMIT 1",
    )
    .bind(quote_id.0.to_string())
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

fn validate_quote_amount(amount: &BigDecimal, pair: &ConvertPairRuleRow) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "convert amount must be positive".to_owned(),
        ));
    }
    if amount < &pair.min_amount {
        return Err(AppError::Validation(
            "convert amount is below pair minimum".to_owned(),
        ));
    }
    if let Some(max_amount) = &pair.max_amount
        && amount > max_amount
    {
        return Err(AppError::Validation(
            "convert amount exceeds pair maximum".to_owned(),
        ));
    }
    if pair.pricing_mode != "fixed" {
        return Err(AppError::Validation(
            "only fixed convert pricing is supported by this route".to_owned(),
        ));
    }
    Ok(())
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for convert routes".to_owned())
    })
}

fn redis_manager(state: &AppState) -> AppResult<redis::aio::ConnectionManager> {
    state.redis.clone().ok_or_else(|| {
        AppError::Internal("redis connection is not configured for convert routes".to_owned())
    })
}

fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

fn parse_quote_id(value: &str) -> AppResult<QuoteId> {
    Uuid::parse_str(value)
        .map(QuoteId)
        .map_err(|_| AppError::Validation("invalid quote_id".to_owned()))
}

fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

fn optional_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn map_convert_repository_error(
    error: crate::modules::convert::ConvertRepositoryError,
) -> AppError {
    AppError::Internal(format!("{error:?}"))
}
