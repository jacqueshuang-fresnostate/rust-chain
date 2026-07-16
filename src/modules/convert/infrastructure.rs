//! convert bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure::insert_agent_business_commission_in_tx,
            repository::AgentBusinessCommissionWrite, service::AGENT_COMMISSION_PRODUCT_CONVERT,
        },
        convert::{
            ConvertConfirmationInsert, ConvertQuoteCacheEntry, ConvertQuoteInsert,
            ConvertQuoteInsertResult, ConvertRepositoryError, QuoteId,
            presentation::{ConvertOrderResponse, ConvertPairResponse},
            repository::{
                ConvertPairRule, ConvertPairRuleDbRecord, ConvertSettlementOrderRecord,
                ConvertSettlementWalletRecord, WalletBalanceRecord,
            },
            service::{convert_pair_rule_from_record, ensure_asset_precision_scale},
        },
        market::market_ticker_redis_key,
        wallet::truncate_amount_to_asset_precision,
    },
};
use bigdecimal::BigDecimal;
use redis::AsyncCommands;
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction};
use std::str::FromStr;

#[derive(Debug)]
pub struct InfrastructureLayerMarker;

impl InfrastructureLayer for InfrastructureLayerMarker {}

#[derive(Clone)]
pub struct RedisConvertQuoteCache {
    manager: redis::aio::ConnectionManager,
}

impl RedisConvertQuoteCache {
    pub fn new(manager: redis::aio::ConnectionManager) -> Self {
        Self { manager }
    }

    pub fn manager(&self) -> &redis::aio::ConnectionManager {
        &self.manager
    }

    pub async fn save_quote_ttl(
        &self,
        entry: ConvertQuoteCacheEntry,
    ) -> Result<(), ConvertRepositoryError> {
        let payload = serde_json::to_string(&entry)?;
        let mut connection = self.manager.clone();
        let _: () = connection
            .set_ex(&entry.redis_key, payload, entry.ttl_seconds as u64)
            .await?;
        Ok(())
    }

    pub async fn get_quote_ttl(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<ConvertQuoteCacheEntry>, ConvertRepositoryError> {
        let mut connection = self.manager.clone();
        let payload: Option<String> = connection.get(quote_redis_key(quote_id)).await?;
        payload
            .map(|value| serde_json::from_str::<ConvertQuoteCacheEntry>(&value))
            .transpose()
            .map_err(Into::into)
    }
}

fn quote_redis_key(quote_id: &QuoteId) -> String {
    format!("convert:quote:{}", quote_id.0)
}

#[derive(Debug, Clone)]
pub struct MySqlConvertRepository {
    pool: Pool<MySql>,
}

impl MySqlConvertRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    pub async fn insert_quote(
        &self,
        quote: ConvertQuoteInsert,
    ) -> Result<ConvertQuoteInsertResult, ConvertRepositoryError> {
        // 以 quote_id 幂等落库，重复提交只返回已有记录，避免重复开仓。
        let insert_result = sqlx::query(
            r#"INSERT INTO convert_quotes
               (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount,
                to_amount, rate, spread_rate, fee_rate, fee_amount, expires_at, status)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'quoted')
               ON DUPLICATE KEY UPDATE quote_id = quote_id"#,
        )
        .bind(quote.quote_id.0.to_string())
        .bind(quote.convert_pair_id)
        .bind(quote.user_id)
        .bind(quote.from_asset_id)
        .bind(quote.to_asset_id)
        .bind(quote.from_amount)
        .bind(quote.to_amount)
        .bind(quote.rate)
        .bind(quote.spread_rate)
        .bind(quote.fee_rate)
        .bind(quote.fee_amount)
        .bind(quote.expires_at.naive_utc())
        .execute(&self.pool)
        .await?;

        let quote_row_id = if insert_result.last_insert_id() == 0 {
            self.quote_row_id(&quote.quote_id).await?
        } else {
            insert_result.last_insert_id()
        };

        Ok(ConvertQuoteInsertResult {
            quote_row_id,
            inserted: insert_result.rows_affected() == 1,
        })
    }

    pub async fn insert_order_for_quote(
        &self,
        quote_id: &QuoteId,
    ) -> Result<ConvertConfirmationInsert, ConvertRepositoryError> {
        let result = sqlx::query(
            r#"INSERT INTO convert_orders
               (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount,
                to_amount, rate, fee_rate, fee_amount, status)
               SELECT quotes.quote_id, quotes.convert_pair_id, quotes.user_id, quotes.from_asset,
                      quotes.to_asset, quotes.from_amount, quotes.to_amount, quotes.rate,
                      quotes.fee_rate, quotes.fee_amount, 'pending'
               FROM convert_quotes quotes
               WHERE quotes.quote_id = ?
               ON DUPLICATE KEY UPDATE quote_id = convert_orders.quote_id"#,
        )
        .bind(quote_id.0.to_string())
        .execute(&self.pool)
        .await?;

        if result.last_insert_id() == 0 {
            Ok(ConvertConfirmationInsert::Duplicate)
        } else {
            Ok(ConvertConfirmationInsert::Inserted)
        }
    }

    async fn quote_row_id(&self, quote_id: &QuoteId) -> Result<u64, ConvertRepositoryError> {
        let row =
            sqlx::query_as::<_, (u64,)>("SELECT id FROM convert_quotes WHERE quote_id = ? LIMIT 1")
                .bind(quote_id.0.to_string())
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }
}

pub(crate) async fn list_convert_pairs(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<Vec<ConvertPairResponse>> {
    let pairs = sqlx::query_as::<_, ConvertPairResponse>(
        r#"SELECT pairs.id,
                  pairs.from_asset AS from_asset_id,
                  from_assets.symbol AS from_asset_symbol,
                  pairs.to_asset AS to_asset_id,
                  to_assets.symbol AS to_asset_symbol,
                  pairs.pricing_mode, pairs.spread_rate, pairs.fee_rate, pairs.min_amount,
                  pairs.max_amount, pairs.target_min_amount, pairs.target_max_amount,
                  pairs.enabled
           FROM convert_pairs pairs
           INNER JOIN assets from_assets ON from_assets.id = pairs.from_asset
           INNER JOIN assets to_assets ON to_assets.id = pairs.to_asset
           WHERE pairs.enabled = true
           ORDER BY pairs.id DESC
           LIMIT ?"#,
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    Ok(pairs)
}

pub(crate) async fn list_convert_orders(
    pool: &Pool<MySql>,
    user_id: u64,
    status: Option<String>,
    limit: u32,
) -> AppResult<Vec<ConvertOrderResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, quote_id, convert_pair_id, from_asset AS from_asset_id,
                  to_asset AS to_asset_id, from_amount, to_amount, rate,
                  fee_rate, fee_amount, status, created_at
           FROM convert_orders
           WHERE user_id = "#,
    );
    builder.push_bind(user_id);

    if let Some(status) = status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }

    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(limit as i64);

    let orders = builder
        .build_query_as::<ConvertOrderResponse>()
        .fetch_all(pool)
        .await?;

    Ok(orders)
}

pub(crate) async fn load_pair_rule(
    pool: &Pool<MySql>,
    from_asset_id: u64,
    to_asset_id: u64,
) -> AppResult<ConvertPairRule> {
    let row = sqlx::query_as::<_, ConvertPairRuleDbRecord>(
        r#"SELECT pairs.id, pairs.from_asset AS from_asset_id, pairs.to_asset AS to_asset_id,
                  pairs.pricing_mode, pairs.spread_rate, pairs.fee_rate,
                  pairs.min_amount, pairs.max_amount,
                  pairs.target_min_amount, pairs.target_max_amount,
                  rules.fixed_rate,
                  market_pairs.symbol AS market_pair_symbol,
                  market_pairs.base_asset AS market_base_asset_id,
                  market_pairs.quote_asset AS market_quote_asset_id
           FROM convert_pairs pairs
           LEFT JOIN new_coin_convert_rules rules
             ON rules.convert_pair_id = pairs.id AND rules.status = 'active' AND rules.rate_source = 'fixed'
           LEFT JOIN trading_pairs market_pairs
             ON ((market_pairs.base_asset = pairs.from_asset AND market_pairs.quote_asset = pairs.to_asset)
                 OR (market_pairs.base_asset = pairs.to_asset AND market_pairs.quote_asset = pairs.from_asset))
            AND market_pairs.status = 'active'
           WHERE ((pairs.from_asset = ? AND pairs.to_asset = ?)
                  OR (pairs.from_asset = ? AND pairs.to_asset = ?))
             AND pairs.enabled = true
           ORDER BY CASE WHEN pairs.from_asset = ? AND pairs.to_asset = ? THEN 0 ELSE 1 END,
                    pairs.id DESC
           LIMIT 1"#,
    )
    .bind(from_asset_id)
    .bind(to_asset_id)
    .bind(to_asset_id)
    .bind(from_asset_id)
    .bind(from_asset_id)
    .bind(to_asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    convert_pair_rule_from_record(row, from_asset_id, to_asset_id)
}

pub(crate) async fn load_wallet_balance(
    pool: &Pool<MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<WalletBalanceRecord> {
    let row = sqlx::query_as::<_, WalletBalanceRecord>(
        "SELECT available, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.unwrap_or_else(|| WalletBalanceRecord {
        available: BigDecimal::from(0),
        locked: BigDecimal::from(0),
    }))
}

pub(crate) async fn latest_market_price(
    redis: Option<redis::aio::ConnectionManager>,
    pair_symbol: &str,
) -> AppResult<Option<BigDecimal>> {
    let Some(mut connection) = redis else {
        return Ok(None);
    };
    let payload: Option<String> = connection
        .get(market_ticker_redis_key(pair_symbol))
        .await
        .map_err(AppError::from)?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    let value = serde_json::from_str::<Value>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached ticker payload: {error}")))?;
    let last_price = value
        .get("last_price")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Internal("cached ticker is missing last_price".to_owned()))?;
    let price = BigDecimal::from_str(last_price)
        .map_err(|_| AppError::Internal("cached ticker last_price is invalid".to_owned()))?;
    if price <= BigDecimal::from(0) {
        return Err(AppError::Validation(
            "convert market price must be positive".to_owned(),
        ));
    }
    Ok(Some(price))
}

pub(crate) async fn load_asset_precision_scale<'e, E>(executor: E, asset_id: u64) -> AppResult<i32>
where
    E: sqlx::Executor<'e, Database = MySql>,
{
    let (precision_scale,): (i32,) =
        sqlx::query_as("SELECT precision_scale FROM assets WHERE id = ? LIMIT 1")
            .bind(asset_id)
            .fetch_optional(executor)
            .await?
            .ok_or(AppError::NotFound)?;
    ensure_asset_precision_scale(precision_scale)?;
    Ok(precision_scale)
}

pub(crate) async fn quote_exists_for_user(
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

pub(crate) async fn confirm_and_settle_convert_quote(
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
            to_amount, rate, fee_rate, fee_amount, status)
           SELECT quotes.quote_id, quotes.convert_pair_id, quotes.user_id, quotes.from_asset,
                  quotes.to_asset, quotes.from_amount, quotes.to_amount, quotes.rate,
                  quotes.fee_rate, quotes.fee_amount, 'pending'
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
    let order = sqlx::query_as::<_, ConvertSettlementOrderRecord>(
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
    let to_precision_scale = load_asset_precision_scale(&mut **tx, order.to_asset_id).await?;

    let from_available_after = from_wallet.available.clone() - order.from_amount.clone();
    let raw_to_available_after = to_wallet.available.clone() + order.to_amount.clone();
    let to_available_after =
        truncate_amount_to_asset_precision(&raw_to_available_after, to_precision_scale);

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

    insert_agent_business_commission_in_tx(
        tx,
        AgentBusinessCommissionWrite {
            user_id,
            product_type: AGENT_COMMISSION_PRODUCT_CONVERT,
            source_type: "convert_order",
            source_id: quote_id,
            source_amount: &order.from_amount,
            payout_asset_id: order.from_asset_id,
        },
    )
    .await?;

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

async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<ConvertSettlementWalletRecord> {
    sqlx::query_as::<_, ConvertSettlementWalletRecord>(
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
