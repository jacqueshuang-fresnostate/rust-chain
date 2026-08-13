//! market bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 行情用例遵循同一条链路：先规范化交易对，再确认已上架，最后按数据种类分派到 MySQL、Redis 或 Mongo。
//! 最新价与盘口只读行情摄取写入的 Redis 快照，历史 K 线只读 Mongo，成交与用户自选只读写 MySQL。
//! 本层不发布 WebSocket 事件，也不做跨存储事务；依赖缺失时按内部错误失败，绝不伪造价格。

use crate::{
    error::{AppError, AppResult},
    modules::market::{
        KlineQuery, infrastructure,
        presentation::{
            DepthResponse, KlineQueryParams, KlineResponse, MarketFavoriteMutationResponse,
            MarketFavoritesResponse, MarketsResponse, TickerResponse, TradesQueryParams,
            TradesResponse,
        },
        service::{
            fallback_market_symbol_is_listed, fallback_markets, route_limit, validate_market_symbol,
        },
    },
};
use mongodb::Database;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};

/// 返回 MySQL 中启用的交易对元数据；未配置 MySQL 时返回内置公开交易对目录。
/// 本用例不读取或合并 Redis 行情，也不创建资金事务或改变市场状态。
pub(crate) async fn list_markets(mysql: Option<Pool<MySql>>) -> AppResult<MarketsResponse> {
    let Some(pool) = mysql else {
        return Ok(MarketsResponse {
            markets: fallback_markets(),
        });
    };

    let markets = infrastructure::list_active_markets(&pool).await?;
    Ok(MarketsResponse { markets })
}

/// 按认证用户读取仍启用的收藏交易对，禁止暴露其他用户或下架记录。
/// 未配置 MySQL 时直接按内部错误失败，自选没有兜底目录可用；结果按收藏创建时间稳定排序。
/// 纯只读操作，不会清理指向已下架交易对的历史收藏，也不读取任何行情缓存。
pub(crate) async fn list_user_market_favorites(
    mysql: Option<Pool<MySql>>,
    user_id: u64,
) -> AppResult<MarketFavoritesResponse> {
    let pool = required_mysql_pool(mysql)?;
    let favorites = infrastructure::list_user_market_favorites(&pool, user_id).await?;
    Ok(MarketFavoritesResponse { favorites })
}

/// 规范交易对并为认证用户新增自选；MySQL 唯一键使重复添加保持单条记录，未知或下架交易对返回校验错误。
/// 本用例不改钱包或行情缓存，数据库写入失败直接返回。
pub(crate) async fn add_user_market_favorite(
    mysql: Option<Pool<MySql>>,
    user_id: u64,
    raw_symbol: &str,
) -> AppResult<MarketFavoriteMutationResponse> {
    let symbol = validate_market_symbol(raw_symbol)?;
    let pool = required_mysql_pool(mysql)?;
    let favorite =
        infrastructure::add_user_market_favorite(&pool, user_id, symbol.as_str()).await?;
    Ok(MarketFavoriteMutationResponse { favorite })
}

/// 规范交易对并删除认证用户自己的自选；记录不存在时仍成功，不会影响其他用户的同一交易对收藏。
pub(crate) async fn remove_user_market_favorite(
    mysql: Option<Pool<MySql>>,
    user_id: u64,
    raw_symbol: &str,
) -> AppResult<()> {
    let symbol = validate_market_symbol(raw_symbol)?;
    let pool = required_mysql_pool(mysql)?;
    infrastructure::remove_user_market_favorite(&pool, user_id, symbol.as_str()).await
}

/// 返回公开市场的权威 ticker：先验证交易对已上架，再读取行情 ingestion 写入的 Redis 快照。
/// Redis 未配置或缓存缺失/损坏必须返回错误，不使用客户端价格或静态市场信息伪造最新价。
/// 响应原样保留快照的 `observed_at`；本路由不按时间戳判断新鲜度，资金链路需自行执行陈旧价格检查。
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

/// 校验交易对已上架后读取 ingestion 写入的 Redis 盘口 JSON；缓存缺失返回 NotFound，损坏或 Redis 故障返回错误。
/// 本接口不排序、补档或执行新鲜度判断，也不回退到第三方 HTTP。
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

/// 校验交易对已上架后从 MySQL 读取现货成交，按成交时间与主键倒序返回 1～100 条。
/// 上架校验优先查 `trading_pairs`，MySQL 缺席时退回内置兜底目录，但真正取成交仍要求连接池存在，否则返回内部错误。
/// 条数缺省 50 并夹紧到 1 至 100；返回的是本平台撮合成交，不含供应商逐笔流，也不读取行情缓存。
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

/// 校验交易对及周期后，从该交易对的 Mongo 集合按开盘时间升序读取最多 100 根 K 线。
/// `start`/`end` 使用闭区间过滤；Mongo 未配置、查询或反序列化失败时返回错误，不合成蜡烛。
/// 条数缺省 100 并夹紧到 1 至 100；周期不在支持白名单内返回校验错误，起止时间可同时省略表示不限范围。
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

/// 在读取任何行情之前确认交易对已上架：有 MySQL 时查 active 交易对，否则退回内置兜底目录判断。
/// 未命中一律返回 `AppError::Validation`，因此未知或已下架交易对不会继续走到 Redis、Mongo 查询。
/// 数据库查询错误按原错误上抛，不会被误判成“未上架”；本函数只读，也不缓存判定结果。
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

/// 取出用例必需的 MySQL 连接池，缺失时返回内部错误而不是静默降级到兜底数据。
/// 供自选读写这类必须落库的用例使用；公开只读列表另有兜底路径，不应调用本函数。
fn required_mysql_pool(mysql: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    mysql.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for market routes".to_owned())
    })
}
