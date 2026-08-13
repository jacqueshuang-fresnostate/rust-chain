//! market bounded context 的 HTTP 路由层。
//!
//! 本文件只做 axum 提取器到应用用例的薄转发：从 `AppState` 取出 MySQL、Redis、Mongo 句柄，
//! 把路径参数与查询参数原样交给 application 层，再把用例返回的 DTO 序列化为 JSON。
//! 交易对规范化、上架校验、缓存读取与错误分类都发生在应用层，本文件不含业务判断，也不广播实时事件。

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

/// 注册行情读接口与用户自选读写接口：`/markets` 系列为公开查询，`/user/market-favorites` 需要用户令牌。
/// 单个自选交易对同时挂 PUT 与 DELETE，分别对应幂等新增与删除，其余端点一律只接受 GET。
/// 本函数只声明路由表并把状态类型固定为 `AppState`，不做鉴权、参数校验或依赖可用性检查。
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

/// 返回全部处于 active 状态的交易对元数据，未配置 MySQL 时由应用层给出内置兜底目录而不是报错。
/// 该端点无需登录，也不合并 Redis 实时价格，响应只描述交易对配置本身。
async fn list_markets_handler(State(state): State<AppState>) -> AppResult<Json<MarketsResponse>> {
    Ok(Json(list_markets(state.mysql.clone()).await?))
}

/// 先把访问令牌的 `sub` 解析为用户 ID，再返回该用户仍处于上架状态的自选交易对。
/// 令牌主体无法解析为用户 ID 时在查库之前就失败；已下架的交易对不会出现在结果中。
async fn list_favorites(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarketFavoritesResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        list_user_market_favorites(state.mysql.clone(), user_id).await?,
    ))
}

/// 把路径中的交易对加入当前登录用户的自选，并回传含资产 Logo 的自选详情。
/// 交易对未上架时返回校验错误；应用层按用户与交易对的唯一键 upsert，重复 PUT 只保留一条记录。
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

/// 删除当前登录用户对该交易对的自选，成功统一返回 204 且不带响应体。
/// 记录本就不存在时同样视为成功，因此重复 DELETE 不会报错；删除范围限定在本用户，不影响他人收藏。
async fn remove_favorite(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<StatusCode> {
    let user_id = user_id_from_subject(&claims.sub)?;
    remove_user_market_favorite(state.mysql.clone(), user_id, &symbol).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 返回该交易对的最新价与 24 小时统计，数据取自行情摄取写入的 Redis 快照。
/// Redis 未配置或快照缺失一律失败，不会退化成静态价格；响应原样透出 `observed_at`，新鲜度判断留给调用方。
async fn get_ticker(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<TickerResponse>> {
    Ok(Json(
        get_market_ticker(state.mysql.clone(), state.redis.clone(), &symbol).await?,
    ))
}

/// 返回该交易对最近一次落缓存的盘口快照，买卖档位保持摄取写入时的顺序。
/// 缓存键缺失按 NotFound 处理，载荷损坏归类为内部错误；本接口不重排档位，也不回源第三方 HTTP 补档。
async fn get_depth(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<DepthResponse>> {
    Ok(Json(
        get_market_depth(state.mysql.clone(), state.redis.clone(), &symbol).await?,
    ))
}

/// 按成交时间倒序返回该交易对的平台现货成交，条数由 `limit` 收敛到 1 至 100，缺省 50。
/// 数据取自 MySQL 成交表而非外部逐笔流，因此只反映本平台撮合结果，不含供应商行情中的第三方成交。
async fn list_trades(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<TradesQueryParams>,
) -> AppResult<Json<TradesResponse>> {
    Ok(Json(
        list_market_trades(state.mysql.clone(), &symbol, query).await?,
    ))
}

/// 按开盘时间升序返回历史 K 线，周期与可选的起止时间来自查询参数，单次最多 100 根。
/// 周期不在支持白名单内返回校验错误；蜡烛读自该交易对独立的 Mongo 集合，缺口不会被合成或补齐。
async fn list_klines(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<KlineQueryParams>,
) -> AppResult<Json<Vec<KlineResponse>>> {
    Ok(Json(
        list_market_klines(state.mysql.clone(), state.mongo.clone(), &symbol, query).await?,
    ))
}
