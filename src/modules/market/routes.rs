use crate::{
    error::{AppError, AppResult},
    infra::mongo::kline_collection_name,
    modules::market::{
        KlineQuery, ValidatedMarketSymbol, market_depth_redis_key, market_ticker_redis_key,
    },
    state::AppState,
    time::{option_unix_millis, unix_millis},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use mongodb::bson::{DateTime as BsonDateTime, Document, doc, oid::ObjectId};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/markets", get(list_markets))
        .route("/markets/:symbol/ticker", get(get_ticker))
        .route("/markets/:symbol/klines", get(list_klines))
        .route("/markets/:symbol/depth", get(get_depth))
        .route("/markets/:symbol/trades", get(list_trades))
}

#[derive(Debug, Serialize)]
struct MarketsResponse {
    markets: Vec<MarketResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct MarketResponse {
    id: u64,
    symbol: String,
    base_asset: String,
    quote_asset: String,
    price_precision: i32,
    qty_precision: i32,
    min_order_value: String,
    status: String,
    market_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TickerResponse {
    symbol: String,
    last_price: String,
    volume_24h: String,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct KlineQueryParams {
    interval: String,
    #[serde(default, with = "option_unix_millis")]
    start: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    end: Option<DateTime<Utc>>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct KlineResponse {
    symbol: String,
    interval: String,
    #[serde(with = "unix_millis")]
    open_time: DateTime<Utc>,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
}

#[derive(Debug, Deserialize)]
struct TradesQueryParams {
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct DepthResponse {
    symbol: String,
    bids: Vec<DepthLevelResponse>,
    asks: Vec<DepthLevelResponse>,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct DepthLevelResponse {
    price: String,
    amount: String,
}

#[derive(Debug, Deserialize)]
struct DepthCachePayload {
    symbol: String,
    bids: Vec<DepthCacheLevel>,
    asks: Vec<DepthCacheLevel>,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct DepthCacheLevel {
    price: BigDecimal,
    quantity: BigDecimal,
}

#[derive(Debug, Serialize)]
struct TradesResponse {
    trades: Vec<TradeResponse>,
}

#[derive(Debug, Serialize)]
struct TradeResponse {
    id: String,
    symbol: String,
    price: String,
    amount: String,
    direction: String,
    #[serde(with = "unix_millis")]
    time: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SpotTradeRow {
    id: u64,
    symbol: String,
    price: BigDecimal,
    quantity: BigDecimal,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct KlineDocument {
    #[serde(rename = "_id")]
    _id: Option<ObjectId>,
    interval: String,
    open_time: BsonDateTime,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
}

async fn list_markets(State(state): State<AppState>) -> AppResult<Json<MarketsResponse>> {
    let Some(pool) = state.mysql.as_ref() else {
        return Ok(Json(MarketsResponse {
            markets: vec![
                MarketResponse::fallback("BTCUSDT", "BTC", "USDT", "external"),
                MarketResponse::fallback("NEWUSDT", "NEW", "USDT", "strategy"),
            ],
        }));
    };
    let markets = sqlx::query_as::<_, MarketResponse>(
        r#"SELECT pairs.id,
                  pairs.symbol,
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

    Ok(Json(MarketsResponse { markets }))
}

async fn get_ticker(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<TickerResponse>> {
    let symbol = ValidatedMarketSymbol::from_raw(&symbol)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    ensure_listed_market_symbol(&state, symbol.as_str()).await?;
    let mut connection = state.redis.clone().ok_or_else(|| {
        AppError::Internal("redis connection is not configured for market ticker routes".to_owned())
    })?;
    let payload: Option<String> = connection
        .get(market_ticker_redis_key(symbol.as_str()))
        .await?;
    let payload = payload.ok_or(AppError::NotFound)?;
    let ticker = serde_json::from_str::<TickerResponse>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached ticker payload: {error}")))?;

    Ok(Json(ticker))
}

async fn get_depth(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<DepthResponse>> {
    let symbol = ValidatedMarketSymbol::from_raw(&symbol)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    ensure_listed_market_symbol(&state, symbol.as_str()).await?;
    let mut connection = state.redis.clone().ok_or_else(|| {
        AppError::Internal("redis connection is not configured for market depth routes".to_owned())
    })?;
    let payload: Option<String> = connection
        .get(market_depth_redis_key(symbol.as_str()))
        .await?;
    let payload = payload.ok_or(AppError::NotFound)?;
    let depth = serde_json::from_str::<DepthCachePayload>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached depth payload: {error}")))?;

    Ok(Json(DepthResponse::from_cache(depth)))
}

async fn list_trades(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<TradesQueryParams>,
) -> AppResult<Json<TradesResponse>> {
    let symbol = ValidatedMarketSymbol::from_raw(&symbol)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    ensure_listed_market_symbol(&state, symbol.as_str()).await?;
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for market trade routes".to_owned())
    })?;
    let rows = sqlx::query_as::<_, SpotTradeRow>(
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
    .bind(symbol.as_str())
    .bind(i64::from(route_limit(query.limit)))
    .fetch_all(pool)
    .await?;

    Ok(Json(TradesResponse {
        trades: rows.into_iter().map(TradeResponse::from).collect(),
    }))
}

async fn list_klines(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<KlineQueryParams>,
) -> AppResult<Json<Vec<KlineResponse>>> {
    let symbol = ValidatedMarketSymbol::from_raw(&symbol)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    ensure_listed_market_symbol(&state, symbol.as_str()).await?;
    let query = KlineQuery::new(query.interval, query.start, query.end, query.limit)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let database = state.mongo.clone().ok_or_else(|| {
        AppError::Internal("mongo database is not configured for market kline routes".to_owned())
    })?;
    let collection = database.collection::<KlineDocument>(&kline_collection_name(&symbol));
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

    Ok(Json(rows))
}

async fn ensure_listed_market_symbol(state: &AppState, symbol: &str) -> AppResult<()> {
    let listed = if let Some(pool) = state.mysql.as_ref() {
        sqlx::query_as::<_, (i64,)>(
            r#"SELECT COUNT(*)
               FROM trading_pairs
               WHERE status = 'active'
                 AND REPLACE(REPLACE(REPLACE(UPPER(symbol), '-', ''), '/', ''), '_', '') = ?"#,
        )
        .bind(symbol)
        .fetch_one(pool)
        .await?
        .0 > 0
    } else {
        matches!(symbol, "BTCUSDT" | "NEWUSDT")
    };

    if !listed {
        return Err(AppError::Validation(
            "market symbol is not listed".to_owned(),
        ));
    }

    Ok(())
}

fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
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

impl DepthResponse {
    fn from_cache(depth: DepthCachePayload) -> Self {
        Self {
            symbol: depth.symbol,
            bids: depth
                .bids
                .into_iter()
                .map(DepthLevelResponse::from)
                .collect(),
            asks: depth
                .asks
                .into_iter()
                .map(DepthLevelResponse::from)
                .collect(),
            observed_at: depth.observed_at,
        }
    }
}

impl From<DepthCacheLevel> for DepthLevelResponse {
    fn from(level: DepthCacheLevel) -> Self {
        Self {
            price: level.price.to_string(),
            amount: level.quantity.to_string(),
        }
    }
}

impl From<SpotTradeRow> for TradeResponse {
    fn from(row: SpotTradeRow) -> Self {
        Self {
            id: row.id.to_string(),
            symbol: ValidatedMarketSymbol::from_raw(&row.symbol)
                .map(|symbol| symbol.as_str().to_owned())
                .unwrap_or(row.symbol),
            price: row.price.to_string(),
            amount: row.quantity.to_string(),
            direction: "BUY".to_owned(),
            time: row.created_at,
        }
    }
}

impl MarketResponse {
    fn fallback(symbol: &str, base_asset: &str, quote_asset: &str, market_type: &str) -> Self {
        Self {
            id: 0,
            symbol: symbol.to_owned(),
            base_asset: base_asset.to_owned(),
            quote_asset: quote_asset.to_owned(),
            price_precision: 8,
            qty_precision: 8,
            min_order_value: "1".to_owned(),
            status: "active".to_owned(),
            market_type: market_type.to_owned(),
        }
    }
}

impl KlineResponse {
    fn from_document(symbol: &str, document: KlineDocument) -> Self {
        Self {
            symbol: symbol.to_owned(),
            interval: document.interval,
            open_time: DateTime::<Utc>::from(document.open_time.to_system_time()),
            open: document.open,
            high: document.high,
            low: document.low,
            close: document.close,
            volume: document.volume,
        }
    }
}
