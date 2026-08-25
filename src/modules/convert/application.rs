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
            repository::{ConvertAuthoritativeQuoteRecord, ConvertPairRule},
            service::{
                QUOTE_TTL_SECONDS, convert_market_pricing_source, convert_quote_amounts,
                convert_quote_fingerprint, ensure_convert_amount_precision,
                ensure_sufficient_convert_balance, map_convert_repository_error,
                normalize_convert_rate_for_storage, optional_query_string, parse_quote_id,
                resolve_fixed_convert_rate, resolve_market_convert_rate, route_limit,
                user_id_from_subject, validate_convert_market_price_snapshot,
                validate_quote_amount,
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

#[derive(Debug, Clone)]
struct ResolvedConvertPrice {
    rate: bigdecimal::BigDecimal,
    source: String,
    symbol: Option<String>,
    observed_at: chrono::DateTime<Utc>,
    version: String,
}

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

/// 为当前用户按启用换币规则生成限时报价，先写 MySQL 权威快照，再尽力写 Redis 展示缓存。
/// 金额、资产精度、交易对限额及源钱包余额会先校验；市场汇率只读 append-only MySQL 历史。
/// 缓存失败不影响已落库报价的可确认性，确认环节只信 MySQL owner、指纹、过期与消费状态。
pub(crate) async fn create_convert_quote(
    mysql: Option<Pool<MySql>>,
    redis: Option<ConnectionManager>,
    subject: &str,
    request: CreateConvertQuoteRequest,
) -> AppResult<ConvertQuoteResponse> {
    let user_id = user_id_from_subject(subject)?;
    let pool = mysql_pool(mysql)?;
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

    let database_now = infrastructure::database_now(&pool).await?;
    let mut pricing = resolve_convert_quote_rate(&pool, &pair, database_now).await?;
    pricing.rate = normalize_convert_rate_for_storage(&pricing.rate)?;
    let amounts = convert_quote_amounts(
        &request.from_amount,
        &pair,
        &pricing.rate,
        from_precision_scale,
        to_precision_scale,
    )?;
    let quote_id = QuoteId(Uuid::now_v7());
    let expires_at = database_now + TimeDelta::seconds(QUOTE_TTL_SECONDS);
    let quote_id_value = quote_id.0.to_string();
    let mut authoritative = ConvertAuthoritativeQuoteRecord {
        quote_id: quote_id_value.clone(),
        convert_pair_id: pair.id,
        user_id,
        from_asset_id: pair.from_asset_id,
        to_asset_id: pair.to_asset_id,
        from_amount: request.from_amount.clone(),
        to_amount: amounts.to_amount.clone(),
        rate: pricing.rate.clone(),
        spread_rate: pair.spread_rate.clone(),
        fee_rate: pair.fee_rate.clone(),
        fee_amount: amounts.fee_amount.clone(),
        request_fingerprint: String::new(),
        price_source: pricing.source.clone(),
        price_symbol: pricing.symbol.clone(),
        price_observed_at: pricing.observed_at,
        price_version: pricing.version.clone(),
        expires_at,
        status: "quoted".to_owned(),
        consumed_at: None,
    };
    authoritative.request_fingerprint = convert_quote_fingerprint(&authoritative);
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
            rate: pricing.rate.clone(),
            spread_rate: pair.spread_rate.clone(),
            fee_rate: pair.fee_rate.clone(),
            fee_amount: amounts.fee_amount.clone(),
            request_fingerprint: authoritative.request_fingerprint.clone(),
            price_source: pricing.source.clone(),
            price_symbol: pricing.symbol.clone(),
            price_observed_at: pricing.observed_at,
            price_version: pricing.version.clone(),
            expires_at,
        })
        .await
        .map_err(map_convert_repository_error)?;
    if let Some(redis) = redis {
        let cache = RedisConvertQuoteCache::new(redis);
        if let Err(error) = cache
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
        {
            tracing::warn!(%error, quote_id = %quote_id.0, "闪兑 Redis quote 缓存写入失败，MySQL 权威报价仍可确认");
        }
    }

    Ok(ConvertQuoteResponse {
        quote_id: quote_id.0.to_string(),
        convert_pair_id: pair.id,
        from_asset_id: pair.from_asset_id,
        to_asset_id: pair.to_asset_id,
        from_amount: request.from_amount,
        to_amount: amounts.to_amount,
        rate: pricing.rate,
        spread_rate: pair.spread_rate,
        fee_rate: pair.fee_rate,
        fee_amount: amounts.fee_amount,
        price_source: pricing.source,
        price_symbol: pricing.symbol,
        price_observed_at: pricing.observed_at,
        price_version: pricing.version,
        expires_at,
    })
}

/// 确认 MySQL 权威报价，并在单一数据库事务内完成双资产余额、流水与 quote 一次消费。
/// 事务先锁 quote 验证 owner、指纹、数据库时间过期和消费状态，再按 `(user_id, asset_id)` 稳定顺序锁钱包。
/// Redis 参数仅为兼容既有调用签名而保留，缓存缺失、过期或被篡改都不参与资金判断。
pub(crate) async fn confirm_convert_quote(
    mysql: Option<Pool<MySql>>,
    _redis: Option<ConnectionManager>,
    subject: &str,
    request: ConfirmConvertQuoteRequest,
) -> AppResult<ConfirmConvertQuoteResponse> {
    let user_id = user_id_from_subject(subject)?;
    let quote_id = parse_quote_id(&request.quote_id)?;
    let pool = mysql_pool(mysql)?;
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

/// 按交易对的 pricing_mode 选择服务端权威汇率来源：fixed 读配置固定汇率，market 读 MySQL 行情历史。
/// market 分支要求交易对已关联活动 trading_pair，且最新历史快照非未来、未陈旧并有完整证据。
/// 返回的是尚未叠加价差的原始汇率，价差与手续费在 `convert_quote_amounts` 中统一折算。
/// 未识别的 pricing_mode 返回参数错误；本函数只读配置和缓存，不写库、不冻结资金。
async fn resolve_convert_quote_rate(
    pool: &Pool<MySql>,
    pair: &ConvertPairRule,
    database_now: chrono::DateTime<Utc>,
) -> AppResult<ResolvedConvertPrice> {
    match pair.pricing_mode.as_str() {
        "fixed" => Ok(ResolvedConvertPrice {
            rate: resolve_fixed_convert_rate(pair)?,
            source: "fixed".to_owned(),
            symbol: None,
            observed_at: pair.pricing_updated_at,
            version: format!(
                "convert_pair:{}:{}",
                pair.id,
                pair.pricing_updated_at.timestamp_micros()
            ),
        }),
        "market" => {
            let (symbol, market_base_asset_id, market_quote_asset_id) =
                convert_market_pricing_source(pair)?;
            let market_price = infrastructure::latest_market_price(pool, symbol)
                .await?
                .ok_or_else(|| {
                    AppError::Validation(
                        "convert market pricing requires authoritative price history".to_owned(),
                    )
                })?;
            validate_convert_market_price_snapshot(&market_price, symbol, database_now, 60)?;
            let rate = resolve_market_convert_rate(
                pair,
                market_price.price.clone(),
                market_base_asset_id,
                market_quote_asset_id,
            )?;
            Ok(ResolvedConvertPrice {
                rate,
                source: market_price.source,
                symbol: Some(market_price.symbol),
                observed_at: market_price.observed_at,
                version: market_price.source_version,
            })
        }
        _ => Err(AppError::Validation(
            "unsupported convert pricing_mode".to_owned(),
        )),
    }
}

/// 把可选的 MySQL 池解包为必需依赖，缺失时按内部错误处理而不是静默返回空结果。
/// 闪兑的报价落库与结算事务都不能降级运行，因此未配置数据库属于部署故障而非业务校验失败。
fn mysql_pool(pool: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for convert routes".to_owned())
    })
}
