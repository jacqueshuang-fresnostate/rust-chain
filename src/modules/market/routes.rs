use super::{
    application::{
        get_market_depth, get_market_ticker, list_market_klines, list_market_trades, list_markets,
    },
    presentation::{
        DepthResponse, KlineQueryParams, KlineResponse, MarketsResponse, TickerResponse,
        TradesQueryParams, TradesResponse,
    },
};
use crate::{error::AppResult, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/markets", get(list_markets_handler))
        .route("/markets/:symbol/ticker", get(get_ticker))
        .route("/markets/:symbol/klines", get(list_klines))
        .route("/markets/:symbol/depth", get(get_depth))
        .route("/markets/:symbol/trades", get(list_trades))
}

async fn list_markets_handler(State(state): State<AppState>) -> AppResult<Json<MarketsResponse>> {
    Ok(Json(list_markets(state.mysql.clone()).await?))
}

async fn get_ticker(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<TickerResponse>> {
    Ok(Json(
        get_market_ticker(state.mysql.clone(), state.redis.clone(), &symbol).await?,
    ))
}

async fn get_depth(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<DepthResponse>> {
    Ok(Json(
        get_market_depth(state.mysql.clone(), state.redis.clone(), &symbol).await?,
    ))
}

async fn list_trades(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<TradesQueryParams>,
) -> AppResult<Json<TradesResponse>> {
    Ok(Json(
        list_market_trades(state.mysql.clone(), &symbol, query).await?,
    ))
}

async fn list_klines(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<KlineQueryParams>,
) -> AppResult<Json<Vec<KlineResponse>>> {
    Ok(Json(
        list_market_klines(state.mysql.clone(), state.mongo.clone(), &symbol, query).await?,
    ))
}
