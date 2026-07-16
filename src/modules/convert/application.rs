//! convert bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    architecture::ApplicationLayer,
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

#[derive(Debug)]
pub struct ApplicationLayerMarker;

impl ApplicationLayer for ApplicationLayerMarker {}

pub(crate) async fn list_convert_pairs(
    mysql: Option<Pool<MySql>>,
    query: ListQuery,
) -> AppResult<ConvertPairsResponse> {
    let pool = mysql_pool(mysql)?;
    let pairs = infrastructure::list_convert_pairs(&pool, route_limit(query.limit)).await?;
    Ok(ConvertPairsResponse { pairs })
}

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
