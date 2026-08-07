use super::{
    application::{
        add_user_market_favorite, get_market_depth, get_market_ticker, list_market_klines,
        list_market_trades, list_markets, list_user_market_favorites, remove_user_market_favorite,
    },
    presentation::{
        DepthResponse, KlineQueryParams, KlineResponse, MarketFavoriteMutationResponse,
        MarketFavoritesResponse, MarketsResponse, TickerResponse, TradesQueryParams,
        TradesResponse,
    },
};
use crate::{
    error::AppResult,
    modules::{auth::UserAuth, user::service::user_id_from_subject},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, put},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/markets", get(list_markets_handler))
        .route("/markets/:symbol/ticker", get(get_ticker))
        .route("/markets/:symbol/klines", get(list_klines))
        .route("/markets/:symbol/depth", get(get_depth))
        .route("/markets/:symbol/trades", get(list_trades))
        .route("/user/market-favorites", get(list_favorites))
        .route(
            "/user/market-favorites/:symbol",
            put(add_favorite).delete(remove_favorite),
        )
}

async fn list_markets_handler(State(state): State<AppState>) -> AppResult<Json<MarketsResponse>> {
    Ok(Json(list_markets(state.mysql.clone()).await?))
}

async fn list_favorites(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarketFavoritesResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        list_user_market_favorites(state.mysql.clone(), user_id).await?,
    ))
}

async fn add_favorite(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<MarketFavoriteMutationResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        add_user_market_favorite(state.mysql.clone(), user_id, &symbol).await?,
    ))
}

async fn remove_favorite(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<StatusCode> {
    let user_id = user_id_from_subject(&claims.sub)?;
    remove_user_market_favorite(state.mysql.clone(), user_id, &symbol).await?;
    Ok(StatusCode::NO_CONTENT)
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
