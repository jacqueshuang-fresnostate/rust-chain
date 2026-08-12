//! 市场查询持久化基础设施。
//!
//! MySQL 保存已上架交易对、用户自选和成交记录等权威业务数据，Mongo 保存历史 K 线；
//! Redis 查询仅返回 ingestion 已写入的实时快照。本模块不做 provider 协议解析或业务编排。

use super::cache::{market_depth_redis_key, market_ticker_redis_key};
use crate::{
    error::{AppError, AppResult},
    modules::market::{
        KlineQuery, ValidatedMarketSymbol,
        presentation::{
            DepthCachePayload, DepthResponse, KlineResponse, MarketFavoriteResponse,
            MarketResponse, TickerResponse, TradeResponse,
        },
        repository::{KlineDocumentRecord, SpotTradeRecord},
    },
};
use chrono::{DateTime, Utc};
use mongodb::{
    Database,
    bson::{DateTime as BsonDateTime, Document, doc},
};
use redis::AsyncCommands;
use sqlx::{MySql, Pool};

/// 为已验证交易对生成稳定 Mongo K 线集合名，恢复与实时 ingestion 必须使用同一命名入口。
pub fn kline_collection_name(symbol: &ValidatedMarketSymbol) -> String {
    format!("market_klines_{}", symbol.as_str())
}

pub(crate) async fn list_active_markets(pool: &Pool<MySql>) -> AppResult<Vec<MarketResponse>> {
    let markets = sqlx::query_as::<_, MarketResponse>(
        r#"SELECT pairs.id,
                  pairs.symbol,
                  pairs.logo_url,
                  base.logo_url AS base_logo_url,
                  quote.logo_url AS quote_logo_url,
                  base.symbol AS base_asset,
                  quote.symbol AS quote_asset,
                  pairs.price_precision,
                  pairs.qty_precision,
                  CAST(pairs.min_order_value AS CHAR) AS min_order_value,
                  pairs.status,
                  pairs.market_type
           FROM trading_pairs pairs
           INNER JOIN assets base ON base.id = pairs.base_asset
           INNER JOIN assets quote ON quote.id = pairs.quote_asset
           WHERE pairs.status = 'active'
           ORDER BY pairs.symbol ASC"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(markets)
}

pub(crate) async fn list_user_market_favorites(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<MarketFavoriteResponse>> {
    sqlx::query_as::<_, MarketFavoriteResponse>(
        r#"SELECT pairs.id AS market_id,
                  REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') AS symbol,
                  pairs.logo_url,
                  base.logo_url AS base_logo_url,
                  quote.logo_url AS quote_logo_url,
                  base.symbol AS base_asset,
                  quote.symbol AS quote_asset
           FROM user_market_favorites favorites
           INNER JOIN trading_pairs pairs ON pairs.id = favorites.trading_pair_id
           INNER JOIN assets base ON base.id = pairs.base_asset
           INNER JOIN assets quote ON quote.id = pairs.quote_asset
           WHERE favorites.user_id = ?
             AND pairs.status = 'active'
           ORDER BY favorites.created_at ASC, favorites.id ASC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn add_user_market_favorite(
    pool: &Pool<MySql>,
    user_id: u64,
    symbol: &str,
) -> AppResult<MarketFavoriteResponse> {
    let favorite = load_active_market_favorite(pool, symbol)
        .await?
        .ok_or_else(|| AppError::Validation("market symbol is not listed".to_owned()))?;

    sqlx::query(
        r#"INSERT INTO user_market_favorites (user_id, trading_pair_id)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE trading_pair_id = VALUES(trading_pair_id)"#,
    )
    .bind(user_id)
    .bind(favorite.market_id)
    .execute(pool)
    .await?;

    Ok(favorite)
}

pub(crate) async fn remove_user_market_favorite(
    pool: &Pool<MySql>,
    user_id: u64,
    symbol: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"DELETE favorites
           FROM user_market_favorites favorites
           INNER JOIN trading_pairs pairs ON pairs.id = favorites.trading_pair_id
           WHERE favorites.user_id = ?
             AND REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') = ?"#,
    )
    .bind(user_id)
    .bind(symbol)
    .execute(pool)
    .await?;

    Ok(())
}

async fn load_active_market_favorite(
    pool: &Pool<MySql>,
    symbol: &str,
) -> AppResult<Option<MarketFavoriteResponse>> {
    sqlx::query_as::<_, MarketFavoriteResponse>(
        r#"SELECT pairs.id AS market_id,
                  REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') AS symbol,
                  pairs.logo_url,
                  base.logo_url AS base_logo_url,
                  quote.logo_url AS quote_logo_url,
                  base.symbol AS base_asset,
                  quote.symbol AS quote_asset
           FROM trading_pairs pairs
           INNER JOIN assets base ON base.id = pairs.base_asset
           INNER JOIN assets quote ON quote.id = pairs.quote_asset
           WHERE pairs.status = 'active'
             AND REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') = ?
           LIMIT 1"#,
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn market_symbol_is_listed(pool: &Pool<MySql>, symbol: &str) -> AppResult<bool> {
    let listed = sqlx::query_as::<_, (i64,)>(
        r#"SELECT COUNT(*)
           FROM trading_pairs
           WHERE status = 'active'
             AND REPLACE(REPLACE(REPLACE(UPPER(symbol), '-', ''), '/', ''), '_', '') = ?"#,
    )
    .bind(symbol)
    .fetch_one(pool)
    .await?
    .0 > 0;

    Ok(listed)
}

pub(crate) async fn load_cached_ticker(
    redis: redis::aio::ConnectionManager,
    symbol: &str,
) -> AppResult<TickerResponse> {
    let mut connection = redis;
    let payload: Option<String> = connection.get(market_ticker_redis_key(symbol)).await?;
    let payload = payload.ok_or(AppError::NotFound)?;
    let ticker = serde_json::from_str::<TickerResponse>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached ticker payload: {error}")))?;

    Ok(ticker)
}

pub(crate) async fn load_cached_depth(
    redis: redis::aio::ConnectionManager,
    symbol: &str,
) -> AppResult<DepthResponse> {
    let mut connection = redis;
    let payload: Option<String> = connection.get(market_depth_redis_key(symbol)).await?;
    let payload = payload.ok_or(AppError::NotFound)?;
    let depth = serde_json::from_str::<DepthCachePayload>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached depth payload: {error}")))?;

    Ok(DepthResponse::from_cache(depth))
}

pub(crate) async fn list_recent_trades(
    pool: &Pool<MySql>,
    symbol: &str,
    limit: u32,
) -> AppResult<Vec<TradeResponse>> {
    let rows = sqlx::query_as::<_, SpotTradeRecord>(
        r#"SELECT trades.id,
                  pairs.symbol,
                  trades.price,
                  trades.quantity,
                  trades.created_at
           FROM spot_trades trades
           INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
           WHERE REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') = ?
           ORDER BY trades.created_at DESC, trades.id DESC
           LIMIT ?"#,
    )
    .bind(symbol)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(TradeResponse::from_record).collect())
}

pub(crate) async fn list_klines(
    database: Database,
    symbol: &ValidatedMarketSymbol,
    query: KlineQuery,
) -> AppResult<Vec<KlineResponse>> {
    let collection = database.collection::<KlineDocumentRecord>(&kline_collection_name(symbol));
    let mut filter = doc! { "interval": &query.interval };
    let time_filter = kline_time_filter(query.start, query.end);
    if !time_filter.is_empty() {
        filter.insert("open_time", time_filter);
    }
    let options = mongodb::options::FindOptions::builder()
        .sort(doc! { "open_time": 1 })
        .limit(i64::from(query.limit))
        .build();
    let mut cursor = collection.find(filter).with_options(options).await?;
    let mut rows = Vec::new();
    while cursor.advance().await? {
        let document = cursor.deserialize_current()?;
        rows.push(KlineResponse::from_document(symbol.as_str(), document));
    }

    Ok(rows)
}

fn kline_time_filter(start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> Document {
    let mut filter = Document::new();
    if let Some(start) = start {
        filter.insert("$gte", BsonDateTime::from_millis(start.timestamp_millis()));
    }
    if let Some(end) = end {
        filter.insert("$lte", BsonDateTime::from_millis(end.timestamp_millis()));
    }
    filter
}
