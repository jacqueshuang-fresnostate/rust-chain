//! market bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    architecture::ApplicationLayer,
    error::{AppError, AppResult},
    modules::market::{
        KlineQuery, infrastructure,
        presentation::{
            DepthResponse, KlineQueryParams, KlineResponse, MarketsResponse, TickerResponse,
            TradesQueryParams, TradesResponse,
        },
        service::{
            fallback_market_symbol_is_listed, fallback_markets, route_limit, validate_market_symbol,
        },
    },
};
use mongodb::Database;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};

#[derive(Debug)]
pub struct ApplicationLayerMarker;

impl ApplicationLayer for ApplicationLayerMarker {}

pub(crate) async fn list_markets(mysql: Option<Pool<MySql>>) -> AppResult<MarketsResponse> {
    let Some(pool) = mysql else {
        return Ok(MarketsResponse {
            markets: fallback_markets(),
        });
    };

    let markets = infrastructure::list_active_markets(&pool).await?;
    Ok(MarketsResponse { markets })
}

pub(crate) async fn get_market_ticker(
    mysql: Option<Pool<MySql>>,
    redis: Option<ConnectionManager>,
    raw_symbol: &str,
) -> AppResult<TickerResponse> {
    let symbol = validate_market_symbol(raw_symbol)?;
    ensure_listed_market_symbol(mysql.as_ref(), symbol.as_str()).await?;
    let redis = redis.ok_or_else(|| {
        AppError::Internal("redis connection is not configured for market ticker routes".to_owned())
    })?;
    infrastructure::load_cached_ticker(redis, symbol.as_str()).await
}

pub(crate) async fn get_market_depth(
    mysql: Option<Pool<MySql>>,
    redis: Option<ConnectionManager>,
    raw_symbol: &str,
) -> AppResult<DepthResponse> {
    let symbol = validate_market_symbol(raw_symbol)?;
    ensure_listed_market_symbol(mysql.as_ref(), symbol.as_str()).await?;
    let redis = redis.ok_or_else(|| {
        AppError::Internal("redis connection is not configured for market depth routes".to_owned())
    })?;
    infrastructure::load_cached_depth(redis, symbol.as_str()).await
}

pub(crate) async fn list_market_trades(
    mysql: Option<Pool<MySql>>,
    raw_symbol: &str,
    query: TradesQueryParams,
) -> AppResult<TradesResponse> {
    let symbol = validate_market_symbol(raw_symbol)?;
    ensure_listed_market_symbol(mysql.as_ref(), symbol.as_str()).await?;
    let pool = mysql.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for market trade routes".to_owned())
    })?;
    let trades =
        infrastructure::list_recent_trades(&pool, symbol.as_str(), route_limit(query.limit))
            .await?;

    Ok(TradesResponse { trades })
}

pub(crate) async fn list_market_klines(
    mysql: Option<Pool<MySql>>,
    mongo: Option<Database>,
    raw_symbol: &str,
    query: KlineQueryParams,
) -> AppResult<Vec<KlineResponse>> {
    let symbol = validate_market_symbol(raw_symbol)?;
    ensure_listed_market_symbol(mysql.as_ref(), symbol.as_str()).await?;
    let query = KlineQuery::new(query.interval, query.start, query.end, query.limit)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let database = mongo.ok_or_else(|| {
        AppError::Internal("mongo database is not configured for market kline routes".to_owned())
    })?;

    infrastructure::list_klines(database, &symbol, query).await
}

async fn ensure_listed_market_symbol(pool: Option<&Pool<MySql>>, symbol: &str) -> AppResult<()> {
    let listed = if let Some(pool) = pool {
        infrastructure::market_symbol_is_listed(pool, symbol).await?
    } else {
        fallback_market_symbol_is_listed(symbol)
    };

    if !listed {
        return Err(AppError::Validation(
            "market symbol is not listed".to_owned(),
        ));
    }

    Ok(())
}
