//! 现货订单创建应用用例：校验、权威执行价、风控、建单冻结与提交后事件。

use crate::{
    error::{AppError, AppResult},
    modules::{
        risk::{RiskGuardInput, RiskScope, enforce_risk_control},
        spot::{
            MySqlSpotRepository, NewOrder, OrderSide, OrderType, SpotOrder, TradingPairRule,
            create_limit_order, create_market_order, create_stop_limit_order,
            infrastructure::{
                freeze_wallet_for_inserted_order_in_tx, insert_spot_order_in_tx,
                latest_spot_market_price, load_spot_pair_db_id, spot_order_reservation_in_tx,
                store_spot_order_idempotency_response_in_tx,
            },
            presentation::{CreateSpotOrderRequest, SpotOrderResponse, SpotTradeResponse},
            service::{
                SpotOrderRequestIdentity, ensure_market_price_within_reference,
                limit_order_reaches_execution_price, map_spot_error, normalize_idempotency_key,
                publish_spot_created_private_events_if_needed, spot_order_request_fingerprint,
                stop_limit_order_reaches_execution_price,
            },
        },
    },
};
use bigdecimal::BigDecimal;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};

use super::{
    idempotency::replay_spot_order_for_idempotency_key,
    triggering::{
        insert_triggered_buy_order_freeze_and_execute,
        insert_triggered_sell_order_freeze_and_execute,
    },
};

/// 创建现货订单并收口提交后的私有事件；调用前必须具备已认证用户、有效交易对及完整幂等参数。
/// 应用层先复核幂等重放，再取得服务端行情并执行风控，最后选择“仅冻结挂单”或“冻结后立即成交”的事务路径。
/// 订单、钱包冻结及可能发生的成交结算由下层事务原子提交；重复键只返回参数一致的原订单，绝不重复冻结。
/// 事件仅在数据库操作成功后发布，事件发布失败不会伪造第二笔订单，调用方可用同一幂等键重放查询既有结果。
pub(crate) async fn create_spot_order_with_events(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
    user_id: u64,
    mut request: CreateSpotOrderRequest,
) -> AppResult<SpotOrderResponse> {
    // 创建现货订单时同时处理幂等重放、撮合触发、下单提交与事件发布，避免路由层承担编排。
    request.idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    request.pair_id = request.pair_id.trim().to_ascii_uppercase();
    let request_fingerprint = spot_order_request_fingerprint(user_id, &request);
    let repository = MySqlSpotRepository::new(pool.clone());
    if let Some(existing) =
        replay_spot_order_for_idempotency_key(pool, user_id, &request, &request_fingerprint).await?
    {
        return Ok(existing);
    }

    let pair = repository
        .load_pair_rule_async(&request.pair_id)
        .await
        .map_err(map_spot_error)?;
    let triggered_execution_price =
        resolve_spot_order_execution_price(redis, &request, &pair.pair_id).await?;
    let new_order = build_create_spot_order(user_id, &request, &pair)?;
    enforce_spot_order_risk_control(
        pool,
        redis,
        user_id,
        &new_order,
        triggered_execution_price.as_ref(),
    )
    .await?;
    let (inserted, is_new_order, fill_event) =
        if let Some(execution_price) = triggered_execution_price.as_ref() {
            match new_order.side {
                OrderSide::Buy => {
                    insert_triggered_buy_order_freeze_and_execute(
                        pool,
                        new_order,
                        &request.idempotency_key,
                        &request_fingerprint,
                        request.price.as_ref(),
                        request.reference_price.as_ref(),
                        execution_price,
                    )
                    .await?
                }
                OrderSide::Sell => {
                    insert_triggered_sell_order_freeze_and_execute(
                        pool,
                        new_order,
                        &request.idempotency_key,
                        &request_fingerprint,
                        request.price.as_ref(),
                        request.reference_price.as_ref(),
                        execution_price,
                    )
                    .await?
                }
            }
        } else {
            let (order, is_new_order) = insert_order_and_freeze_wallet(
                pool,
                new_order,
                &request.idempotency_key,
                &request_fingerprint,
                request.price.as_ref(),
                request.reference_price.as_ref(),
            )
            .await?;
            (order, is_new_order, None)
        };
    let response = SpotOrderResponse::from(inserted);
    let fill_event = fill_event.map(|(counterparty_order, trade)| {
        (
            SpotOrderResponse::from(counterparty_order),
            SpotTradeResponse::from(trade),
        )
    });
    let fill_event = fill_event
        .as_ref()
        .map(|(counterparty_order, trade)| (counterparty_order, trade));
    publish_spot_created_private_events_if_needed(
        hub,
        user_id,
        &response,
        fill_event,
        is_new_order,
    )?;

    Ok(response)
}

/// 下单风控闸门必须在冻结与建单之前执行；市价单没有委托价时以服务端撮合价为准。
async fn enforce_spot_order_risk_control(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    new_order: &NewOrder,
    triggered_execution_price: Option<&BigDecimal>,
) -> AppResult<()> {
    let order_price = new_order
        .price
        .clone()
        .or_else(|| triggered_execution_price.cloned());
    let reference_price = match triggered_execution_price {
        Some(price) => Some(price.clone()),
        None => latest_spot_market_price(redis, &new_order.pair_id).await?,
    };

    enforce_risk_control(
        pool,
        redis,
        RiskGuardInput {
            user_id,
            operation: "spot.order.create",
            scopes: vec![
                RiskScope::new("user", user_id.to_string()),
                RiskScope::new("pair", new_order.pair_id.clone()),
            ],
            // 限额统一按计价币种口径折算，避免不同交易对的基础币数量不可比。
            amount: order_price
                .as_ref()
                .map(|price| price * &new_order.quantity),
            price: order_price,
            reference_price,
        },
    )
    .await
}

/// 将传输请求转换为领域建单命令；调用前交易对规则必须来自服务端仓储，用户编号必须来自认证主体。
/// 限价单使用委托价，市价单强制提供仅作滑点保护的参考价，止损限价单同时要求触发价和限价；数量及精度继续由领域规则校验。
/// 本函数不选择 Redis 执行价、不计算最终事务预留、不加锁或写库；校验失败无订单、钱包、流水、幂等和事件副作用。
pub(crate) fn build_create_spot_order(
    user_id: u64,
    request: &CreateSpotOrderRequest,
    pair: &TradingPairRule,
) -> AppResult<NewOrder> {
    // 建单规则集中在应用层入口，路由只负责传输协议，避免下单校验在多处漂移。
    match request.order_type {
        OrderType::Limit => create_limit_order(
            user_id.to_string(),
            request.side,
            request.price.clone().ok_or_else(|| {
                AppError::Validation("price is required for limit orders".to_owned())
            })?,
            request.quantity.clone(),
            pair,
        ),
        OrderType::Market => create_market_order(
            user_id.to_string(),
            request.side,
            request.quantity.clone(),
            request.reference_price.clone().ok_or_else(|| {
                AppError::Validation("reference_price is required for market orders".to_owned())
            })?,
            pair,
        ),
        OrderType::StopLimit => create_stop_limit_order(
            user_id.to_string(),
            request.side,
            request.trigger_price.clone().ok_or_else(|| {
                AppError::Validation("trigger_price is required for stop limit orders".to_owned())
            })?,
            request.price.clone().ok_or_else(|| {
                AppError::Validation("price is required for stop limit orders".to_owned())
            })?,
            request.quantity.clone(),
            pair,
        ),
    }
    .map_err(|error| AppError::Validation(format!("invalid spot order: {error:?}")))
}

/// 解析现货订单的权威执行价：市价单必须使用新鲜 Redis 行情，客户端参考价只参与滑点保护。
/// 限价/止盈止损限价仅在服务端最新价触发价格条件时返回执行价；行情缺失时保持挂单而不使用客户端价格兜底。
/// 本函数只读取行情并执行纯价格校验，不建单、不冻结资金，也不产生事件，调用方必须在后续事务前再次承担状态一致性。
pub(crate) async fn resolve_spot_order_execution_price(
    redis: Option<&ConnectionManager>,
    request: &CreateSpotOrderRequest,
    pair_symbol: &str,
) -> AppResult<Option<BigDecimal>> {
    match request.order_type {
        OrderType::Market => {
            let reference_price = request.reference_price.as_ref().ok_or_else(|| {
                crate::error::AppError::Validation(
                    "reference_price is required for market orders".to_owned(),
                )
            })?;
            let execution_price =
                resolve_market_execution_price(redis, pair_symbol, reference_price).await?;
            ensure_market_price_within_reference(request.side, &execution_price, reference_price)?;
            Ok(Some(execution_price))
        }
        OrderType::Limit => {
            let limit_price = request.price.as_ref().ok_or_else(|| {
                crate::error::AppError::Validation("price is required for limit orders".to_owned())
            })?;
            let Some(execution_price) = latest_spot_market_price(redis, pair_symbol).await? else {
                return Ok(None);
            };
            Ok(
                limit_order_reaches_execution_price(request.side, &execution_price, limit_price)
                    .then_some(execution_price),
            )
        }
        OrderType::StopLimit => {
            let trigger_price = request.trigger_price.as_ref().ok_or_else(|| {
                crate::error::AppError::Validation(
                    "trigger_price is required for stop limit orders".to_owned(),
                )
            })?;
            let limit_price = request.price.as_ref().ok_or_else(|| {
                crate::error::AppError::Validation(
                    "price is required for stop limit orders".to_owned(),
                )
            })?;
            let Some(execution_price) = latest_spot_market_price(redis, pair_symbol).await? else {
                return Ok(None);
            };
            Ok(stop_limit_order_reaches_execution_price(
                request.side,
                &execution_price,
                trigger_price,
                limit_price,
            )
            .then_some(execution_price))
        }
    }
}

async fn resolve_market_execution_price(
    redis: Option<&ConnectionManager>,
    pair_symbol: &str,
    _reference_price: &BigDecimal,
) -> AppResult<BigDecimal> {
    // 客户端参考价只用于滑点约束，绝不能在服务端行情缺失时充当成交价。
    latest_spot_market_price(redis, pair_symbol)
        .await?
        .ok_or_else(|| {
            AppError::Validation("fresh spot ticker is required for market order".to_owned())
        })
}
/// 原子插入未立即成交的现货订单并冻结钱包；调用前订单已通过交易对、价格、数量、风控和执行条件校验。
/// 事务计算预留：买单按限价/参考价预留报价资产 `price * quantity`，卖单预留基础资产数量；插单后锁钱包完成 available→frozen 与流水。
/// 幂等键已存在时返回原订单且不重复冻结；余额不足、参数冲突或任一数据库步骤失败会回滚订单和资金，本函数不发布事件。
pub(crate) async fn insert_order_and_freeze_wallet(
    pool: &Pool<MySql>,
    new_order: NewOrder,
    idempotency_key: &str,
    request_fingerprint: &str,
    request_price: Option<&BigDecimal>,
    reference_price: Option<&BigDecimal>,
) -> AppResult<(SpotOrder, bool)> {
    let pair_db_id = load_spot_pair_db_id(pool, &new_order.pair_id).await?;
    let mut tx = pool.begin().await?;
    let reservation = spot_order_reservation_in_tx(&mut tx, &new_order, reference_price).await?;
    let (order, is_new_order) = insert_spot_order_in_tx(
        &mut tx,
        new_order,
        pair_db_id,
        SpotOrderRequestIdentity {
            idempotency_key: Some(idempotency_key),
            request_fingerprint: Some(request_fingerprint),
            request_price,
            request_reference_price: reference_price,
        },
        &reservation,
    )
    .await?;
    if is_new_order {
        // 下单记录与钱包冻结必须同事务提交，避免订单可见但资金未锁定。
        freeze_wallet_for_inserted_order_in_tx(&mut tx, &order, &reservation).await?;
        let response = SpotOrderResponse::from(order.clone());
        store_spot_order_idempotency_response_in_tx(&mut tx, &order.id, &response).await?;
    }
    tx.commit().await?;
    Ok((order, is_new_order))
}
