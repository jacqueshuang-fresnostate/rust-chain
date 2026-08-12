//! convert bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    error::{AppError, AppResult},
    modules::{
        convert::{
            ConvertQuoteCacheEntry, ConvertQuoteInsert, MySqlConvertRepository, QuoteId,
            RedisConvertQuoteCache, infrastructure,
            presentation::{
                ConfirmConvertQuoteRequest, ConfirmConvertQuoteResponse, ConvertOrdersQuery,
                ConvertOrdersResponse, ConvertPairsResponse, ConvertQuoteResponse,
                CreateConvertQuoteRequest, ListQuery,
            },
            repository::ConvertPairRule,
            service::{
                QUOTE_TTL_SECONDS, convert_market_pricing_source, convert_quote_amounts,
                ensure_convert_amount_precision, ensure_sufficient_convert_balance,
                map_convert_repository_error, optional_query_string, parse_quote_id,
                resolve_fixed_convert_rate, resolve_market_convert_rate, route_limit,
                user_id_from_subject, validate_quote_amount,
            },
        },
        events::{EventBroadcastHub, EventBroadcastMessage},
    },
};
use chrono::{TimeDelta, Utc};
use redis::aio::ConnectionManager;
use serde_json::json;
use sqlx::{MySql, Pool};
use uuid::Uuid;

/// 编排闪兑交易对读取与响应组装；该只读用例不创建资金事务，也不改变闪兑业务状态。
/// 返回启用闪兑对、限额与数据库资产 Logo，不从符号推导图片或汇率。
pub(crate) async fn list_convert_pairs(
    mysql: Option<Pool<MySql>>,
    query: ListQuery,
) -> AppResult<ConvertPairsResponse> {
    let pool = mysql_pool(mysql)?;
    let pairs = infrastructure::list_convert_pairs(&pool, route_limit(query.limit)).await?;
    Ok(ConvertPairsResponse { pairs })
}

/// 编排闪兑订单读取与响应组装；该只读用例不创建资金事务，也不改变闪兑业务状态。
/// 按认证用户和可选状态读取闪兑订单，失败不返回其他用户数据。
pub(crate) async fn list_convert_orders(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: ConvertOrdersQuery,
) -> AppResult<ConvertOrdersResponse> {
    let user_id = user_id_from_subject(subject)?;
    let pool = mysql_pool(mysql)?;
    let orders = infrastructure::list_convert_orders(
        &pool,
        user_id,
        optional_query_string(query.status),
        route_limit(query.limit),
    )
    .await?;

    Ok(ConvertOrdersResponse { orders })
}

/// 为当前用户按启用换币规则生成限时报价，并同时落库及写入 Redis 缓存。
/// 调用方须提供用户身份、可用 MySQL/Redis；金额、资产精度、交易对限额及源钱包余额会先校验。
/// 汇率取固定配置或当前市场源，目标额和费用按各资产精度截断，返回值必须与持久化快照一致。
/// MySQL 写入与 Redis 缓存不在同一事务；数据库成功后缓存失败会报错并可能留下不可确认的报价行。
/// 每次调用生成新的报价编号，不提供请求幂等；确认环节仍以归属、缓存存在性和过期时间为准。
pub(crate) async fn create_convert_quote(
    mysql: Option<Pool<MySql>>,
    redis: Option<ConnectionManager>,
    subject: &str,
    request: CreateConvertQuoteRequest,
) -> AppResult<ConvertQuoteResponse> {
    let user_id = user_id_from_subject(subject)?;
    let pool = mysql_pool(mysql)?;
    let redis = RedisConvertQuoteCache::new(redis_manager(redis)?);
    let pair =
        infrastructure::load_pair_rule(&pool, request.from_asset_id, request.to_asset_id).await?;
    let from_precision_scale =
        infrastructure::load_asset_precision_scale(&pool, pair.from_asset_id).await?;
    let to_precision_scale =
        infrastructure::load_asset_precision_scale(&pool, pair.to_asset_id).await?;
    validate_quote_amount(&request.from_amount, &pair)?;
    ensure_convert_amount_precision(&request.from_amount, from_precision_scale, "from_amount")?;
    let balance =
        infrastructure::load_wallet_balance(&pool, user_id, request.from_asset_id).await?;
    ensure_sufficient_convert_balance(&request.from_amount, &balance)?;

    let rate = resolve_convert_quote_rate(redis.manager().clone().into(), &pair).await?;
    let amounts = convert_quote_amounts(
        &request.from_amount,
        &pair,
        &rate,
        from_precision_scale,
        to_precision_scale,
    )?;
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
            to_amount: amounts.to_amount.clone(),
            rate: rate.clone(),
            spread_rate: pair.spread_rate.clone(),
            fee_rate: pair.fee_rate.clone(),
            fee_amount: amounts.fee_amount.clone(),
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
            to_amount: amounts.to_amount.clone(),
            fee_rate: pair.fee_rate.clone(),
            fee_amount: amounts.fee_amount.clone(),
            expires_at,
            redis_key: format!("convert:quote:{}", quote_id.0),
            ttl_seconds: QUOTE_TTL_SECONDS,
        })
        .await
        .map_err(map_convert_repository_error)?;

    Ok(ConvertQuoteResponse {
        quote_id: quote_id.0.to_string(),
        convert_pair_id: pair.id,
        from_asset_id: pair.from_asset_id,
        to_asset_id: pair.to_asset_id,
        from_amount: request.from_amount,
        to_amount: amounts.to_amount,
        rate,
        spread_rate: pair.spread_rate,
        fee_rate: pair.fee_rate,
        fee_amount: amounts.fee_amount,
        expires_at,
    })
}

/// 确认闪兑报价的归属与有效期，并在单一数据库事务内完成双资产余额及流水结算。
/// Redis 快照须存在、归属当前用户且未到期，MySQL 也须存在同一用户报价；报价阶段没有预冻结资金。
/// 结算事务从源 available 扣完整 from_amount、向目标 available 加 to_amount，frozen/locked 不变，并写两条 quote_id 流水。
/// 相同报价重放由订单唯一键拒绝二次入账；缓存缺失/过期发生在事务前，确认期余额不足则回滚 pending 订单及全部资金写入。
pub(crate) async fn confirm_convert_quote(
    mysql: Option<Pool<MySql>>,
    redis: Option<ConnectionManager>,
    subject: &str,
    request: ConfirmConvertQuoteRequest,
) -> AppResult<ConfirmConvertQuoteResponse> {
    let user_id = user_id_from_subject(subject)?;
    let quote_id = parse_quote_id(&request.quote_id)?;
    let redis = RedisConvertQuoteCache::new(redis_manager(redis)?);
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

    let pool = mysql_pool(mysql)?;
    if !infrastructure::quote_exists_for_user(&pool, &quote_id, user_id).await? {
        return Err(AppError::NotFound);
    }
    infrastructure::confirm_and_settle_convert_quote(&pool, &quote_id, user_id).await?;
    Ok(ConfirmConvertQuoteResponse {
        quote_id: request.quote_id,
        confirmed: true,
    })
}

/// 编排闪兑确认并在数据库结算成功后发布用户私有完成事件，广播内容只引用已提交的报价结果。
/// 结算失败时不广播；广播发生在事务提交后，未配置或内存广播无人接收都不回滚已完成交易。
pub(crate) async fn confirm_convert_quote_with_events(
    mysql: Option<Pool<MySql>>,
    redis: Option<ConnectionManager>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    request: ConfirmConvertQuoteRequest,
) -> AppResult<ConfirmConvertQuoteResponse> {
    // 应用层负责事务完成后的事件编排：路由层只负责参数透传，不处理消息推送细节。
    let response = confirm_convert_quote(mysql, redis, subject, request).await?;
    let user_id = user_id_from_subject(subject)?;
    let quote_id = response.quote_id.clone();
    if let Some(hub) = event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "convert.confirmed",
                "quote_id": quote_id,
                "status": "completed",
            })
            .to_string(),
        ));
    }
    Ok(response)
}

async fn resolve_convert_quote_rate(
    redis: Option<ConnectionManager>,
    pair: &ConvertPairRule,
) -> AppResult<bigdecimal::BigDecimal> {
    match pair.pricing_mode.as_str() {
        "fixed" => resolve_fixed_convert_rate(pair),
        "market" => {
            let (symbol, market_base_asset_id, market_quote_asset_id) =
                convert_market_pricing_source(pair)?;
            let market_price = infrastructure::latest_market_price(redis, symbol)
                .await?
                .ok_or_else(|| {
                    AppError::Validation(
                        "convert market pricing requires cached market price".to_owned(),
                    )
                })?;
            resolve_market_convert_rate(
                pair,
                market_price,
                market_base_asset_id,
                market_quote_asset_id,
            )
        }
        _ => Err(AppError::Validation(
            "unsupported convert pricing_mode".to_owned(),
        )),
    }
}

fn mysql_pool(pool: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for convert routes".to_owned())
    })
}

fn redis_manager(redis: Option<ConnectionManager>) -> AppResult<ConnectionManager> {
    redis.ok_or_else(|| {
        AppError::Internal("redis connection is not configured for convert routes".to_owned())
    })
}
