//! seconds_contract bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use super::{
    presentation::{
        CachedTickerPayload, SecondsContractOrderResponse, SecondsContractProductCycleResponse,
        SecondsContractProductResponse,
    },
    repository::{
        SecondsContractAdminOrderFilter, SecondsContractOrderInsert, SecondsContractProductRow,
        SecondsContractProductRuleRow, SecondsContractProductWrite,
        SecondsContractWalletLedgerWrite, SecondsContractWalletRow,
    },
    service::{NormalizedSecondsContractProductCycle, optional_string},
};
use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::market::market_ticker_redis_key,
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use redis::{AsyncCommands, aio::ConnectionManager};
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

#[derive(Debug)]
pub struct InfrastructureLayerMarker;

impl InfrastructureLayer for InfrastructureLayerMarker {}

pub(crate) async fn list_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<SecondsContractProductResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.logo_url,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           INNER JOIN assets pair_base_assets ON pair_base_assets.id = pairs.base_asset
           INNER JOIN assets pair_quote_assets ON pair_quote_assets.id = pairs.quote_asset"#,
    );

    if let Some(status) = status {
        builder.push(" WHERE products.status = ");
        builder.push_bind(status);
        if status == "active" {
            builder.push(
                " AND pairs.status = 'active' AND assets.status = 'active' AND pair_base_assets.status = 'active' AND pair_quote_assets.status = 'active'",
            );
        }
    }

    builder.push(" ORDER BY products.id DESC LIMIT ");
    builder.push_bind(limit as i64);

    let product_rows = builder
        .build_query_as::<SecondsContractProductRow>()
        .fetch_all(pool)
        .await?;
    attach_product_cycles_from_pool(pool, product_rows).await
}

pub(crate) async fn load_product_by_id_from_pool(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<SecondsContractProductResponse> {
    let product = sqlx::query_as::<_, SecondsContractProductRow>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.logo_url,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           WHERE products.id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let cycles = load_product_cycles_from_pool(pool, product_id).await?;
    Ok(product_response_from_row(product, cycles))
}

pub(crate) async fn ensure_pair_exists(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM trading_pairs WHERE id = ? LIMIT 1")
        .bind(pair_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub(crate) async fn ensure_asset_exists(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE id = ? LIMIT 1")
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub(crate) async fn ensure_product_has_no_orders(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<()> {
    let order_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM seconds_contract_orders WHERE product_id = ?",
    )
    .bind(product_id)
    .fetch_one(&mut **tx)
    .await?;
    if order_count > 0 {
        return Err(AppError::Validation(
            "seconds contract product with orders cannot be deleted".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) async fn insert_product(
    tx: &mut Transaction<'_, MySql>,
    write: &SecondsContractProductWrite,
) -> AppResult<u64> {
    let product_id = sqlx::query(
        r#"INSERT INTO seconds_contract_products
           (pair_id, stake_asset, logo_url, duration_seconds, payout_rate, min_stake, max_stake, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(write.pair_id)
    .bind(write.stake_asset)
    .bind(&write.logo_url)
    .bind(write.duration_seconds)
    .bind(&write.payout_rate)
    .bind(&write.min_stake)
    .bind(&write.max_stake)
    .bind(&write.status)
    .execute(&mut **tx)
    .await?
    .last_insert_id();
    Ok(product_id)
}

pub(crate) async fn update_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    write: &SecondsContractProductWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE seconds_contract_products
           SET pair_id = ?, stake_asset = ?, logo_url = ?, duration_seconds = ?, payout_rate = ?,
               min_stake = ?, max_stake = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(write.pair_id)
    .bind(write.stake_asset)
    .bind(&write.logo_url)
    .bind(write.duration_seconds)
    .bind(&write.payout_rate)
    .bind(&write.min_stake)
    .bind(&write.max_stake)
    .bind(&write.status)
    .bind(product_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_product_status(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE seconds_contract_products SET status = ? WHERE id = ?")
        .bind(status)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn delete_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM seconds_contract_products WHERE id = ?")
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn load_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<SecondsContractProductResponse> {
    let product = sqlx::query_as::<_, SecondsContractProductRow>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.logo_url,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           WHERE products.id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let cycles = load_product_cycles(tx, product_id).await?;
    Ok(product_response_from_row(product, cycles))
}

pub(crate) async fn lock_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<SecondsContractProductResponse> {
    let product = sqlx::query_as::<_, SecondsContractProductRow>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.logo_url,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let cycles = load_product_cycles(tx, product_id).await?;
    Ok(product_response_from_row(product, cycles))
}

pub(crate) async fn insert_product_cycles(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    cycles: &[NormalizedSecondsContractProductCycle],
) -> AppResult<()> {
    for (index, cycle) in cycles.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO seconds_contract_product_cycles
               (product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order)
               VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(product_id)
        .bind(cycle.duration_seconds)
        .bind(&cycle.payout_rate)
        .bind(&cycle.min_stake)
        .bind(&cycle.max_stake)
        .bind(index as u32)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub(crate) async fn replace_product_cycles(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    cycles: &[NormalizedSecondsContractProductCycle],
) -> AppResult<()> {
    sqlx::query("DELETE FROM seconds_contract_product_cycles WHERE product_id = ?")
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    insert_product_cycles(tx, product_id, cycles).await
}

pub(crate) async fn list_user_orders(
    pool: &Pool<MySql>,
    user_id: u64,
    limit: u32,
) -> AppResult<Vec<SecondsContractOrderResponse>> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  NULL AS email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
                  orders.direction, orders.stake_amount, orders.duration_seconds,
                  orders.payout_rate, orders.entry_price, orders.settlement_price, orders.status, orders.result,
                  orders.idempotency_key, orders.expires_at, orders.created_at
           FROM seconds_contract_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset
           WHERE orders.user_id = ?
           ORDER BY orders.created_at DESC, orders.id DESC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(limit as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn list_admin_orders(
    pool: &Pool<MySql>,
    filter: SecondsContractAdminOrderFilter,
) -> AppResult<Vec<SecondsContractOrderResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  users.email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
                  orders.direction, orders.stake_amount, orders.duration_seconds,
                  orders.payout_rate, orders.entry_price, orders.settlement_price, orders.status, orders.result,
                  orders.idempotency_key, orders.expires_at, orders.created_at
           FROM seconds_contract_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset"#,
    );
    let mut has_filter = false;
    if let Some(user_id) = filter.user_id {
        builder.push(" WHERE orders.user_id = ");
        builder.push_bind(user_id);
        has_filter = true;
    }
    if let Some(email) = filter.email {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("users.email = ");
        builder.push_bind(email);
        has_filter = true;
    }
    if let Some(status) = filter.status {
        builder.push(if has_filter {
            " AND orders.status = "
        } else {
            " WHERE orders.status = "
        });
        builder.push_bind(status);
    }
    builder.push(" ORDER BY orders.created_at DESC, orders.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    builder
        .build_query_as::<SecondsContractOrderResponse>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

pub(crate) async fn load_order_by_id_from_pool(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(seconds_contract_order_by_id_sql())
        .bind(order_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn existing_order_for_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<SecondsContractOrderResponse>> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  NULL AS email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
                  orders.direction, orders.stake_amount, orders.duration_seconds,
                  orders.payout_rate, orders.entry_price, orders.settlement_price, orders.status, orders.result,
                  orders.idempotency_key, orders.expires_at, orders.created_at
           FROM seconds_contract_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset
           WHERE orders.user_id = ? AND orders.idempotency_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn existing_order_for_idempotency_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<SecondsContractOrderResponse>> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  NULL AS email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
                  orders.direction, orders.stake_amount, orders.duration_seconds,
                  orders.payout_rate, orders.entry_price, orders.settlement_price, orders.status, orders.result,
                  orders.idempotency_key, orders.expires_at, orders.created_at
           FROM seconds_contract_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset
           WHERE orders.user_id = ? AND orders.idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn cached_entry_price(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<BigDecimal> {
    let redis = redis.ok_or_else(|| {
        AppError::Validation(
            "fresh cached ticker is required to open seconds contract orders".to_owned(),
        )
    })?;
    let ticker = cached_ticker_price(redis, symbol).await?;
    if ticker.last_price <= 0 {
        return Err(AppError::Validation(format!(
            "seconds contract entry price must be positive for pair {pair_id}"
        )));
    }
    if ticker.observed_at < Utc::now() - chrono::TimeDelta::seconds(60) {
        return Err(AppError::Validation(format!(
            "seconds contract entry ticker is stale for pair {pair_id}"
        )));
    }
    Ok(ticker.last_price)
}

pub(crate) async fn lock_active_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    duration_seconds: Option<u32>,
) -> AppResult<SecondsContractProductRuleRow> {
    let product = sqlx::query_as::<_, SecondsContractProductRuleRow>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.precision_scale AS stake_asset_precision,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           INNER JOIN assets pair_base_assets ON pair_base_assets.id = pairs.base_asset
           INNER JOIN assets pair_quote_assets ON pair_quote_assets.id = pairs.quote_asset
           WHERE products.id = ? AND products.status = 'active'
             AND pairs.status = 'active' AND assets.status = 'active'
             AND pair_base_assets.status = 'active' AND pair_quote_assets.status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let requested_duration = duration_seconds.unwrap_or(product.duration_seconds);
    if requested_duration == 0 {
        return Err(AppError::Validation(
            "seconds contract duration_seconds must be positive".to_owned(),
        ));
    }

    let cycle = sqlx::query_as::<_, SecondsContractProductCycleResponse>(
        r#"SELECT id, product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order
           FROM seconds_contract_product_cycles
           WHERE product_id = ? AND duration_seconds = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .bind(requested_duration)
    .fetch_optional(&mut **tx)
    .await?;
    let (duration_seconds, payout_rate, min_stake, max_stake) = if let Some(cycle) = cycle {
        (
            cycle.duration_seconds,
            cycle.payout_rate,
            cycle.min_stake,
            cycle.max_stake,
        )
    } else if duration_seconds.is_none() && requested_duration == product.duration_seconds {
        (
            product.duration_seconds,
            product.payout_rate.clone(),
            product.min_stake.clone(),
            product.max_stake.clone(),
        )
    } else {
        return Err(AppError::NotFound);
    };

    Ok(SecondsContractProductRuleRow {
        id: product.id,
        pair_id: product.pair_id,
        symbol: product.symbol,
        stake_asset: product.stake_asset,
        stake_asset_precision: product.stake_asset_precision,
        duration_seconds,
        payout_rate,
        min_stake,
        max_stake,
        status: product.status,
    })
}

pub(crate) async fn load_asset_precision_scale(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<i32> {
    sqlx::query_scalar::<_, i32>("SELECT precision_scale FROM assets WHERE id = ? LIMIT 1")
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_open_order(
    tx: &mut Transaction<'_, MySql>,
    order: &SecondsContractOrderInsert,
) -> Result<u64, sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            duration_seconds, payout_rate, entry_price, status, idempotency_key, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'opened', ?, ?)"#,
    )
    .bind(order.user_id)
    .bind(order.product_id)
    .bind(order.pair_id)
    .bind(order.stake_asset)
    .bind(&order.direction)
    .bind(&order.stake_amount)
    .bind(order.duration_seconds)
    .bind(&order.payout_rate)
    .bind(&order.entry_price)
    .bind(&order.idempotency_key)
    .bind(order.expires_at)
    .execute(&mut **tx)
    .await
    .map(|result| result.last_insert_id())
}

pub(crate) async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<SecondsContractWalletRow> {
    sqlx::query_as::<_, SecondsContractWalletRow>(
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
        AppError::Validation("wallet account is required for seconds contract".to_owned())
    })
}

pub(crate) async fn update_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    available_after: &BigDecimal,
) -> AppResult<()> {
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn insert_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    entry: SecondsContractWalletLedgerWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, 'available', ?, ?, ?, ?, 'seconds_contract_order', ?)"#,
    )
    .bind(entry.user_id)
    .bind(entry.asset_id)
    .bind(entry.change_type)
    .bind(&entry.amount)
    .bind(&entry.available_after)
    .bind(&entry.available_after)
    .bind(&entry.frozen_after)
    .bind(&entry.locked_after)
    .bind(entry.ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_order_by_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(seconds_contract_order_by_id_sql())
        .bind(order_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_order_by_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  users.email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
                  orders.direction, orders.stake_amount, orders.duration_seconds,
                  orders.payout_rate, orders.entry_price, orders.settlement_price, orders.status, orders.result,
                  orders.idempotency_key, orders.expires_at, orders.created_at
           FROM seconds_contract_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset
           WHERE orders.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn mark_order_settled(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
    result: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE seconds_contract_orders SET status = 'settled', result = ?, settled_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(result)
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn insert_admin_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_type: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(optional_string(reason))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn cached_ticker_price(
    redis: &ConnectionManager,
    symbol: &str,
) -> AppResult<CachedTickerPayload> {
    let mut connection = redis.clone();
    let payload: Option<String> = connection.get(market_ticker_redis_key(symbol)).await?;
    let payload = payload.ok_or_else(|| {
        AppError::Validation("cached ticker is required to open seconds contract orders".to_owned())
    })?;
    serde_json::from_str::<CachedTickerPayload>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached ticker payload: {error}")))
}

async fn attach_product_cycles_from_pool(
    pool: &Pool<MySql>,
    product_rows: Vec<SecondsContractProductRow>,
) -> AppResult<Vec<SecondsContractProductResponse>> {
    if product_rows.is_empty() {
        return Ok(Vec::new());
    }
    let product_ids = product_rows
        .iter()
        .map(|product| product.id)
        .collect::<Vec<_>>();
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order
           FROM seconds_contract_product_cycles
           WHERE product_id IN ("#,
    );
    let mut separated = builder.separated(", ");
    for product_id in &product_ids {
        separated.push_bind(product_id);
    }
    separated.push_unseparated(") ORDER BY product_id, sort_order, duration_seconds, id");
    let cycle_rows = builder
        .build_query_as::<SecondsContractProductCycleResponse>()
        .fetch_all(pool)
        .await?;

    Ok(product_rows
        .into_iter()
        .map(|product| {
            let cycles = cycle_rows
                .iter()
                .filter(|cycle| cycle.product_id == product.id)
                .cloned()
                .collect::<Vec<_>>();
            product_response_from_row(product, cycles)
        })
        .collect())
}

async fn load_product_cycles_from_pool(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<Vec<SecondsContractProductCycleResponse>> {
    sqlx::query_as::<_, SecondsContractProductCycleResponse>(
        r#"SELECT id, product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order
           FROM seconds_contract_product_cycles
           WHERE product_id = ?
           ORDER BY sort_order, duration_seconds, id"#,
    )
    .bind(product_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn load_product_cycles(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<Vec<SecondsContractProductCycleResponse>> {
    sqlx::query_as::<_, SecondsContractProductCycleResponse>(
        r#"SELECT id, product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order
           FROM seconds_contract_product_cycles
           WHERE product_id = ?
           ORDER BY sort_order, duration_seconds, id"#,
    )
    .bind(product_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}

fn product_response_from_row(
    product: SecondsContractProductRow,
    cycles: Vec<SecondsContractProductCycleResponse>,
) -> SecondsContractProductResponse {
    let cycles = if cycles.is_empty() {
        vec![SecondsContractProductCycleResponse {
            id: 0,
            product_id: product.id,
            duration_seconds: product.duration_seconds,
            payout_rate: product.payout_rate.clone(),
            min_stake: product.min_stake.clone(),
            max_stake: product.max_stake.clone(),
            sort_order: 0,
        }]
    } else {
        cycles
    };
    let default_cycle = cycles.first();
    SecondsContractProductResponse {
        id: product.id,
        pair_id: product.pair_id,
        symbol: product.symbol,
        stake_asset: product.stake_asset,
        stake_asset_symbol: product.stake_asset_symbol,
        logo_url: product.logo_url,
        duration_seconds: default_cycle
            .map(|cycle| cycle.duration_seconds)
            .unwrap_or(product.duration_seconds),
        payout_rate: default_cycle
            .map(|cycle| cycle.payout_rate.clone())
            .unwrap_or(product.payout_rate),
        min_stake: default_cycle
            .map(|cycle| cycle.min_stake.clone())
            .unwrap_or(product.min_stake),
        max_stake: default_cycle
            .map(|cycle| cycle.max_stake.clone())
            .unwrap_or(product.max_stake),
        cycles,
        status: product.status,
    }
}

fn seconds_contract_order_by_id_sql() -> &'static str {
    r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
              users.email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
              orders.direction, orders.stake_amount, orders.duration_seconds,
              orders.payout_rate, orders.entry_price, orders.settlement_price, orders.status, orders.result,
              orders.idempotency_key, orders.expires_at, orders.created_at
       FROM seconds_contract_orders orders
       INNER JOIN users ON users.id = orders.user_id
       INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
       INNER JOIN assets ON assets.id = orders.stake_asset
       WHERE orders.id = ?
       LIMIT 1"#
}
