//! spot bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 现货订单只读查询先进入应用层，路由只负责鉴权和传输协议。

use crate::{
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure::insert_agent_business_commission_in_tx,
            repository::AgentBusinessCommissionWrite, service::AGENT_COMMISSION_PRODUCT_SPOT,
        },
        spot::MySqlSpotRepository,
        spot::{
            NewOrder, OrderSide, OrderStatus, OrderType, SpotOrder, SpotTrade, TradingPairRule,
            apply_fill, create_limit_order, create_market_order, create_stop_limit_order,
            infrastructure::{
                SpotLedgerMetadata, SpotOrderListFilter, SpotTradeListFilter,
                SqlxSpotOrderCancelRepository, apply_spot_wallet_settlement_leg,
                credit_spot_liquidity_wallet_in_tx, ensure_spot_liquidity_user_in_tx,
                ensure_wallet_account_in_tx, freeze_wallet_for_inserted_order_in_tx,
                insert_spot_liquidity_buy_order_in_tx, insert_spot_liquidity_sell_order_in_tx,
                insert_spot_order_in_tx, insert_spot_trade, is_duplicate_key_error,
                latest_spot_market_price, list_spot_orders, list_spot_trades,
                list_user_cancellable_spot_order_ids, load_existing_spot_trade_by_idempotency_key,
                load_spot_order_by_id, load_spot_order_by_idempotency_key, load_spot_pair_db_id,
                lock_spot_fill_orders_in_order, lock_spot_fill_wallet_rows_in_order,
                lock_spot_order_by_db_id, pair_assets_in_tx,
                release_buy_order_surplus_reservation_after_fill,
                remaining_spot_fill_reservation_before_trade_in_tx, save_spot_order_fill_state,
                spot_order_reservation_in_tx, triggered_limit_buy_order_ids,
                triggered_limit_sell_order_ids, triggered_stop_limit_buy_order_ids,
                triggered_stop_limit_sell_order_ids,
            },
            presentation::{
                AdminCancelSpotOrderRequest, AdminSpotOrdersQuery, AdminSpotTradesQuery,
                CancelAllSpotOrdersQuery, CreateSpotOrderRequest, FillSpotOrdersRequest,
                SpotBatchActionFailure, SpotCancelAllResponse, SpotCancelResponse,
                SpotFillResponse, SpotOrderResponse, SpotOrdersQuery, SpotOrdersResponse,
                SpotTradeResponse, SpotTradesQuery, SpotTradesResponse,
            },
            repository::{
                SpotAdminCancelCommand, SpotOrderCancelRepository, SpotUserCancelCommand,
            },
            service::{
                SpotOrderIdempotencyCheck, SpotOrderReservation,
                ensure_existing_spot_trade_matches_request, ensure_fill_orders_match,
                ensure_fill_price_matches_limits, ensure_market_price_within_reference,
                ensure_spot_fill_within_order_reservation, ensure_spot_order_idempotency_matches,
                is_triggerable_limit_buy_order, is_triggerable_limit_sell_order,
                is_triggerable_stop_limit_buy_order, is_triggerable_stop_limit_sell_order,
                limit_order_reaches_execution_price, map_spot_error, market_buy_reservation_price,
                normalize_idempotency_key, publish_spot_cancel_private_event_by_order_if_needed,
                publish_spot_cancel_private_event_if_needed,
                publish_spot_created_private_events_if_needed,
                publish_spot_fill_private_events_if_needed,
                stop_limit_order_reaches_execution_price,
            },
            spot_reservation_amount,
        },
    },
    state::AppState,
};
use bigdecimal::BigDecimal;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool, Transaction};

/// 统一从应用状态中获取数据库连接池。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for spot routes".to_owned())
    })
}

pub(crate) async fn list_user_spot_orders(
    pool: &Pool<MySql>,
    user_id: u64,
    query: SpotOrdersQuery,
) -> AppResult<SpotOrdersResponse> {
    let orders = list_spot_orders(
        pool,
        SpotOrderListFilter {
            user_id: Some(user_id),
            pair_id: optional_query_string(query.pair_id),
            status: optional_query_string(query.status),
            email: None,
            include_internal: true,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(SpotOrdersResponse { orders })
}

pub(crate) async fn list_admin_spot_orders(
    pool: &Pool<MySql>,
    query: AdminSpotOrdersQuery,
) -> AppResult<SpotOrdersResponse> {
    let orders = list_spot_orders(
        pool,
        SpotOrderListFilter {
            user_id: query.user_id,
            pair_id: optional_query_string(query.pair_id),
            status: optional_query_string(query.status),
            email: optional_query_string(query.email),
            include_internal: query.include_internal == Some(true),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(SpotOrdersResponse { orders })
}

pub(crate) async fn list_user_spot_trades(
    pool: &Pool<MySql>,
    user_id: u64,
    query: SpotTradesQuery,
) -> AppResult<SpotTradesResponse> {
    let trades = list_spot_trades(
        pool,
        SpotTradeListFilter {
            pair_id: optional_query_string(Some(query.pair_id)),
            user_id: Some(user_id),
            email: None,
            include_internal: true,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(SpotTradesResponse { trades })
}

pub(crate) async fn get_admin_spot_order(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<SpotOrderResponse> {
    load_spot_order_by_id(pool, order_id).await
}

pub(crate) async fn replay_spot_order_for_idempotency_key(
    pool: &Pool<MySql>,
    user_id: u64,
    request: &CreateSpotOrderRequest,
) -> AppResult<Option<SpotOrderResponse>> {
    let Some(idempotency_key) = normalize_idempotency_key(request.idempotency_key.as_deref())
    else {
        return Ok(None);
    };
    let existing = load_spot_order_by_idempotency_key(pool, idempotency_key).await?;

    match existing {
        Some(order) if order.user_id == user_id => {
            let expected = spot_order_idempotency_check_for_request(request);
            ensure_spot_order_idempotency_matches(&order, &expected)?;
            Ok(Some(order.into()))
        }
        Some(_) => Err(crate::error::AppError::Conflict(
            "spot order idempotency key belongs to another user".to_owned(),
        )),
        None => Ok(None),
    }
}

pub(crate) async fn create_spot_order_with_events(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
    user_id: u64,
    request: CreateSpotOrderRequest,
) -> AppResult<SpotOrderResponse> {
    // 创建现货订单时同时处理幂等重放、撮合触发、下单提交与事件发布，避免路由层承担编排。
    let repository = MySqlSpotRepository::new(pool.clone());
    if let Some(existing) = replay_spot_order_for_idempotency_key(pool, user_id, &request).await? {
        return Ok(existing);
    }

    let pair = repository
        .load_pair_rule_async(&request.pair_id)
        .await
        .map_err(map_spot_error)?;
    let triggered_execution_price =
        resolve_spot_order_execution_price(redis, &request, &pair.pair_id).await?;
    let new_order = build_create_spot_order(user_id, &request, &pair)?;
    let (inserted, is_new_order, fill_event) =
        if let Some(execution_price) = triggered_execution_price.as_ref() {
            match new_order.side {
                OrderSide::Buy => {
                    insert_triggered_buy_order_freeze_and_execute(
                        pool,
                        new_order,
                        request.idempotency_key.as_deref(),
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
                        request.idempotency_key.as_deref(),
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
                request.idempotency_key.as_deref(),
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

pub(crate) async fn cancel_user_spot_order(
    pool: &Pool<MySql>,
    order_id: u64,
    user_id: u64,
) -> AppResult<SpotCancelResponse> {
    let repository = SqlxSpotOrderCancelRepository::new(pool.clone());
    let result = repository
        .cancel_user_order(SpotUserCancelCommand { order_id, user_id })
        .await?;
    Ok(SpotCancelResponse {
        order: result.order.into(),
        cancelled: result.cancelled,
    })
}

pub(crate) async fn cancel_user_spot_order_with_events(
    pool: &Pool<MySql>,
    order_id: u64,
    user_id: u64,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SpotCancelResponse> {
    // 取消订单和事件发布作为一个用例返回，路由层只做参数透传。
    let response = cancel_user_spot_order(pool, order_id, user_id).await?;
    publish_spot_cancel_private_event_if_needed(hub, user_id, &response.order, response.cancelled);
    Ok(response)
}

pub(crate) async fn cancel_all_user_spot_orders_with_events(
    pool: &Pool<MySql>,
    user_id: u64,
    query: CancelAllSpotOrdersQuery,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SpotCancelAllResponse> {
    let order_ids =
        list_user_cancellable_spot_order_ids(pool, user_id, optional_query_string(query.pair_id))
            .await?;
    let mut orders = Vec::with_capacity(order_ids.len());
    let mut failures = Vec::new();
    for order_id in order_ids {
        // 批量撤单逐笔复用单单事务，重试时已撤订单不会再次解冻或重复发事件。
        match cancel_user_spot_order_with_events(pool, order_id, user_id, hub).await {
            Ok(response) if response.cancelled => orders.push(response.order),
            Ok(_) => {}
            Err(error) => failures.push(spot_batch_action_failure(order_id, error)),
        }
    }
    Ok(SpotCancelAllResponse { orders, failures })
}

fn spot_batch_action_failure(id: u64, error: AppError) -> SpotBatchActionFailure {
    let code = match &error {
        AppError::Config(_) => "CONFIG_ERROR",
        AppError::Database(_) => "DATABASE_ERROR",
        AppError::Mongo(_) => "MONGO_ERROR",
        AppError::Redis(_) => "REDIS_ERROR",
        AppError::RabbitMq(_) => "RABBITMQ_ERROR",
        AppError::Unauthorized => "UNAUTHORIZED",
        AppError::Forbidden => "FORBIDDEN",
        AppError::Validation(_) => "VALIDATION_ERROR",
        AppError::NotFound => "NOT_FOUND",
        AppError::Conflict(_) => "CONFLICT",
        AppError::Internal(_) => "INTERNAL_ERROR",
        AppError::Api { code, .. } => *code,
    };
    SpotBatchActionFailure {
        id: id.to_string(),
        code,
        message: error.to_string(),
    }
}

pub(crate) async fn cancel_admin_spot_order(
    pool: &Pool<MySql>,
    order_id: u64,
    admin_id: u64,
    reason: String,
) -> AppResult<SpotCancelResponse> {
    let repository = SqlxSpotOrderCancelRepository::new(pool.clone());
    let result = repository
        .cancel_admin_order(SpotAdminCancelCommand {
            order_id,
            admin_id,
            reason,
        })
        .await?;
    Ok(SpotCancelResponse {
        order: result.order.into(),
        cancelled: result.cancelled,
    })
}

pub(crate) async fn cancel_admin_spot_order_with_events(
    pool: &Pool<MySql>,
    order_id: u64,
    admin_id: u64,
    reason: String,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SpotCancelResponse> {
    // 管理员撤单需要 admin 审计上下文，事件发布与交易结果同一事务边界外执行。
    let response = cancel_admin_spot_order(pool, order_id, admin_id, reason).await?;
    publish_spot_cancel_private_event_by_order_if_needed(hub, &response.order, response.cancelled)?;
    Ok(response)
}

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

pub(crate) async fn list_admin_spot_trades(
    pool: &Pool<MySql>,
    query: AdminSpotTradesQuery,
) -> AppResult<SpotTradesResponse> {
    let trades = list_spot_trades(
        pool,
        SpotTradeListFilter {
            pair_id: optional_query_string(query.pair_id),
            user_id: query.user_id,
            email: optional_query_string(query.email),
            include_internal: query.include_internal == Some(true),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(SpotTradesResponse { trades })
}

/// 执行触发订单撮合，返回所有成交明细用于上层事件发布。
pub(crate) async fn execute_triggered_spot_limit_orders(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
) -> AppResult<Vec<(SpotOrder, SpotOrder, SpotTrade, &'static str)>> {
    if market_price <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "market price must be positive".to_owned(),
        ));
    }

    let mut fills = Vec::new();

    let order_ids = triggered_limit_buy_order_ids(pool, pair_symbol, market_price, 20).await?;
    for order_id in order_ids {
        if let Some((order, counterparty_order, trade)) =
            execute_triggered_limit_buy_order(pool, order_id, market_price).await?
        {
            fills.push((order, counterparty_order, trade, "buy"));
        }
    }

    let order_ids = triggered_limit_sell_order_ids(pool, pair_symbol, market_price, 20).await?;
    for order_id in order_ids {
        if let Some((order, counterparty_order, trade)) =
            execute_triggered_limit_sell_order(pool, order_id, market_price).await?
        {
            fills.push((order, counterparty_order, trade, "sell"));
        }
    }

    let order_ids = triggered_stop_limit_buy_order_ids(pool, pair_symbol, market_price, 20).await?;
    for order_id in order_ids {
        if let Some((order, counterparty_order, trade)) =
            execute_triggered_stop_limit_buy_order(pool, order_id, market_price).await?
        {
            fills.push((order, counterparty_order, trade, "buy"));
        }
    }

    let order_ids =
        triggered_stop_limit_sell_order_ids(pool, pair_symbol, market_price, 20).await?;
    for order_id in order_ids {
        if let Some((order, counterparty_order, trade)) =
            execute_triggered_stop_limit_sell_order(pool, order_id, market_price).await?
        {
            fills.push((order, counterparty_order, trade, "sell"));
        }
    }

    Ok(fills)
}

/// 在外部触发价格驱动撮合时复用的应用服务入口。
/// 该入口由市场行情轮询与测试调用，用于复用撮合与事件广播流程。
pub async fn execute_triggered_spot_limit_orders_with_hub(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<u32> {
    let fills = execute_triggered_spot_limit_orders(pool, pair_symbol, market_price).await?;
    let mut filled_count = 0_u32;

    for (order, counterparty_order, trade, side) in fills {
        filled_count += 1;
        if let Some(hub) = hub {
            let (buy_order, sell_order) = match side {
                "sell" => (counterparty_order, order),
                _ => (order, counterparty_order),
            };
            publish_spot_fill_private_events_if_needed(
                Some(hub),
                &crate::modules::spot::presentation::SpotFillResponse {
                    buy_order: SpotOrderResponse::from(buy_order),
                    sell_order: SpotOrderResponse::from(sell_order),
                    trade: SpotTradeResponse::from(trade),
                },
                true,
            )?;
        }
    }

    Ok(filled_count)
}

pub(crate) async fn settle_spot_fill(
    pool: &Pool<MySql>,
    buy_order_id: &str,
    sell_order_id: &str,
    price: &BigDecimal,
    quantity: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<(SpotOrder, SpotOrder, SpotTrade, bool)> {
    let mut tx = pool.begin().await?;
    let (mut buy_order, mut sell_order) =
        lock_spot_fill_orders_in_order(&mut tx, buy_order_id, sell_order_id).await?;
    ensure_fill_orders_match(&buy_order, &sell_order)?;
    if let Some(trade) =
        load_existing_spot_trade_by_idempotency_key(&mut tx, idempotency_key).await?
    {
        ensure_existing_spot_trade_matches_request(
            &trade,
            &buy_order.id,
            &sell_order.id,
            price,
            quantity,
        )?;
        tx.commit().await?;
        return Ok((buy_order, sell_order, trade, false));
    }
    ensure_fill_price_matches_limits(&buy_order, &sell_order, price)?;
    let assets = pair_assets_in_tx(&mut tx, &buy_order.pair_id).await?;
    let base_asset_id = assets.base_asset_id;
    let quote_asset_id = assets.quote_asset_id;
    let buyer_id = buy_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    let seller_id = sell_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    let fill_quote_amount = price.clone() * quantity.clone();
    // 成交幂等键先占位再锁钱包，避免重复键事务和钱包结算互相等待造成死锁或 500。
    let trade = match insert_spot_trade(
        &mut tx,
        &buy_order,
        &sell_order,
        price,
        quantity,
        idempotency_key,
    )
    .await
    {
        Ok(trade) => trade,
        Err(AppError::Database(error)) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            return replay_existing_spot_fill(
                pool,
                buy_order_id,
                sell_order_id,
                price,
                quantity,
                idempotency_key,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    lock_spot_fill_wallet_rows_in_order(
        &mut tx,
        buyer_id,
        seller_id,
        base_asset_id,
        quote_asset_id,
    )
    .await?;
    let buy_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(&mut tx, &buy_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        OrderSide::Buy,
    )?;
    let sell_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(&mut tx, &sell_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &sell_order_remaining_reservation,
        quantity,
        OrderSide::Sell,
    )?;
    apply_fill(&mut buy_order, quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot buy fill: {error:?}")))?;
    apply_fill(&mut sell_order, quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot sell fill: {error:?}")))?;
    let ref_id = format!("{}:{}", buy_order.id, sell_order.id);
    let ledger = SpotLedgerMetadata {
        change_type: "spot_trade_settlement",
        ref_type: "spot_trade",
        ref_id: &ref_id,
    };

    apply_spot_wallet_settlement_leg(
        &mut tx,
        buyer_id,
        quote_asset_id,
        &fill_quote_amount,
        false,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(&mut tx, buyer_id, base_asset_id, quantity, true, ledger)
        .await?;
    apply_spot_wallet_settlement_leg(&mut tx, seller_id, base_asset_id, quantity, false, ledger)
        .await?;
    apply_spot_wallet_settlement_leg(
        &mut tx,
        seller_id,
        quote_asset_id,
        &fill_quote_amount,
        true,
        ledger,
    )
    .await?;
    release_buy_order_surplus_reservation_after_fill(
        &mut tx,
        buyer_id,
        &buy_order,
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        &ref_id,
    )
    .await?;

    insert_spot_fill_commissions_in_tx(
        &mut tx,
        &trade,
        buyer_id,
        seller_id,
        base_asset_id,
        quote_asset_id,
        &fill_quote_amount,
    )
    .await?;

    save_spot_order_fill_state(&mut tx, &buy_order).await?;
    save_spot_order_fill_state(&mut tx, &sell_order).await?;
    tx.commit().await?;
    Ok((buy_order, sell_order, trade, true))
}

pub(crate) async fn fill_spot_orders_with_events(
    pool: &Pool<MySql>,
    buy_order_id: &str,
    sell_order_id: &str,
    price: &BigDecimal,
    quantity: &BigDecimal,
    idempotency_key: &str,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SpotFillResponse> {
    // 成交处理后的事件发布收口到应用层，路由只返回统一的填单响应。
    let (buy_order, sell_order, trade, is_new_trade) = settle_spot_fill(
        pool,
        buy_order_id,
        sell_order_id,
        price,
        quantity,
        idempotency_key,
    )
    .await?;
    let response = SpotFillResponse {
        buy_order: buy_order.into(),
        sell_order: sell_order.into(),
        trade: trade.into(),
    };

    publish_spot_fill_private_events_if_needed(hub, &response, is_new_trade)?;
    Ok(response)
}

pub(crate) async fn fill_spot_orders_with_events_with_request(
    pool: &Pool<MySql>,
    request: FillSpotOrdersRequest,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SpotFillResponse> {
    // 成交请求的参数边界（幂等键与数量、价格正数）放在应用服务层，避免路由重复校验。
    let request = validate_fill_spot_order_request(request)?;

    fill_spot_orders_with_events(
        pool,
        &request.buy_order_id,
        &request.sell_order_id,
        &request.price,
        &request.quantity,
        &request.idempotency_key,
        hub,
    )
    .await
}

pub(crate) fn validate_admin_cancel_spot_order_request(
    request: AdminCancelSpotOrderRequest,
) -> AppResult<String> {
    request
        .reason
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("reason is required".to_owned()))
}

fn validate_fill_spot_order_request(
    mut request: FillSpotOrdersRequest,
) -> AppResult<FillSpotOrdersRequest> {
    validate_positive_amount(&request.price, "price")?;
    validate_positive_amount(&request.quantity, "quantity")?;
    request.idempotency_key = request.idempotency_key.trim().to_owned();
    if request.idempotency_key.is_empty() {
        return Err(AppError::Validation(
            "idempotency_key is required".to_owned(),
        ));
    }
    Ok(request)
}

fn validate_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        Err(AppError::Validation(format!("{field} must be positive")))
    } else {
        Ok(())
    }
}

async fn replay_existing_spot_fill(
    pool: &Pool<MySql>,
    buy_order_id: &str,
    sell_order_id: &str,
    price: &BigDecimal,
    quantity: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<(SpotOrder, SpotOrder, SpotTrade, bool)> {
    let mut tx = pool.begin().await?;
    let (buy_order, sell_order) =
        lock_spot_fill_orders_in_order(&mut tx, buy_order_id, sell_order_id).await?;
    let trade = load_existing_spot_trade_by_idempotency_key(&mut tx, idempotency_key)
        .await?
        .ok_or_else(|| {
            AppError::Conflict("spot fill idempotency key is being committed".to_owned())
        })?;
    ensure_existing_spot_trade_matches_request(
        &trade,
        &buy_order.id,
        &sell_order.id,
        price,
        quantity,
    )?;
    tx.commit().await?;
    Ok((buy_order, sell_order, trade, false))
}

pub(crate) async fn insert_order_and_freeze_wallet(
    pool: &Pool<MySql>,
    new_order: NewOrder,
    idempotency_key: Option<&str>,
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
        normalize_idempotency_key(idempotency_key),
        request_price,
        reference_price,
        &reservation,
    )
    .await?;
    if is_new_order {
        // 下单记录与钱包冻结必须同事务提交，避免订单可见但资金未锁定。
        freeze_wallet_for_inserted_order_in_tx(&mut tx, &order, &reservation).await?;
    }
    tx.commit().await?;
    Ok((order, is_new_order))
}

pub(crate) async fn insert_triggered_buy_order_freeze_and_execute(
    pool: &Pool<MySql>,
    new_order: NewOrder,
    idempotency_key: Option<&str>,
    request_price: Option<&BigDecimal>,
    request_reference_price: Option<&BigDecimal>,
    execution_price: &BigDecimal,
) -> AppResult<(SpotOrder, bool, Option<(SpotOrder, SpotTrade)>)> {
    if let Some(request_reference_price) = request_reference_price {
        ensure_market_price_within_reference(
            OrderSide::Buy,
            execution_price,
            request_reference_price,
        )?;
    } else {
        ensure_limit_buy_price_reached(&new_order, execution_price)?;
    }
    let pair_db_id = load_spot_pair_db_id(pool, &new_order.pair_id).await?;
    let mut tx = pool.begin().await?;
    let reservation_reference_price =
        market_buy_reservation_price(request_reference_price, execution_price);
    let reservation =
        spot_order_reservation_in_tx(&mut tx, &new_order, reservation_reference_price).await?;
    let (order, is_new_order) = insert_spot_order_in_tx(
        &mut tx,
        new_order,
        pair_db_id,
        normalize_idempotency_key(idempotency_key),
        request_price,
        request_reference_price,
        &reservation,
    )
    .await?;
    if !is_new_order {
        tx.commit().await?;
        return Ok((order, false, None));
    }
    // 即时触发成交会同时冻结用户订单、生成系统对手单并结算，必须共享事务。
    freeze_wallet_for_inserted_order_in_tx(&mut tx, &order, &reservation).await?;
    let (order, counterparty_order, trade) =
        execute_triggered_buy_order_in_tx(&mut tx, order, execution_price).await?;
    tx.commit().await?;
    Ok((order, true, Some((counterparty_order, trade))))
}

pub(crate) async fn insert_triggered_sell_order_freeze_and_execute(
    pool: &Pool<MySql>,
    new_order: NewOrder,
    idempotency_key: Option<&str>,
    request_price: Option<&BigDecimal>,
    request_reference_price: Option<&BigDecimal>,
    execution_price: &BigDecimal,
) -> AppResult<(SpotOrder, bool, Option<(SpotOrder, SpotTrade)>)> {
    if let Some(request_reference_price) = request_reference_price {
        ensure_market_price_within_reference(
            OrderSide::Sell,
            execution_price,
            request_reference_price,
        )?;
    } else {
        ensure_limit_sell_price_reached(&new_order, execution_price)?;
    }
    let pair_db_id = load_spot_pair_db_id(pool, &new_order.pair_id).await?;
    let mut tx = pool.begin().await?;
    let reservation =
        spot_order_reservation_in_tx(&mut tx, &new_order, request_reference_price).await?;
    let (order, is_new_order) = insert_spot_order_in_tx(
        &mut tx,
        new_order,
        pair_db_id,
        normalize_idempotency_key(idempotency_key),
        request_price,
        request_reference_price,
        &reservation,
    )
    .await?;
    if !is_new_order {
        tx.commit().await?;
        return Ok((order, false, None));
    }
    // 卖单即时触发同样需要订单冻结、系统买单和成交结算原子提交。
    freeze_wallet_for_inserted_order_in_tx(&mut tx, &order, &reservation).await?;
    let (order, counterparty_order, trade) =
        execute_triggered_sell_order_in_tx(&mut tx, order, execution_price).await?;
    tx.commit().await?;
    Ok((order, true, Some((counterparty_order, trade))))
}

pub(crate) async fn execute_triggered_limit_buy_order(
    pool: &Pool<MySql>,
    order_id: u64,
    market_price: &BigDecimal,
) -> AppResult<Option<(SpotOrder, SpotOrder, SpotTrade)>> {
    let mut tx = pool.begin().await?;
    let order = lock_spot_order_by_db_id(&mut tx, order_id).await?;
    if !is_triggerable_limit_buy_order(&order, market_price) {
        tx.commit().await?;
        return Ok(None);
    }
    let result = execute_triggered_buy_order_in_tx(&mut tx, order, market_price).await?;
    tx.commit().await?;
    Ok(Some(result))
}

pub(crate) async fn execute_triggered_limit_sell_order(
    pool: &Pool<MySql>,
    order_id: u64,
    market_price: &BigDecimal,
) -> AppResult<Option<(SpotOrder, SpotOrder, SpotTrade)>> {
    let mut tx = pool.begin().await?;
    let order = lock_spot_order_by_db_id(&mut tx, order_id).await?;
    if !is_triggerable_limit_sell_order(&order, market_price) {
        tx.commit().await?;
        return Ok(None);
    }
    let result = execute_triggered_sell_order_in_tx(&mut tx, order, market_price).await?;
    tx.commit().await?;
    Ok(Some(result))
}

pub(crate) async fn execute_triggered_stop_limit_buy_order(
    pool: &Pool<MySql>,
    order_id: u64,
    market_price: &BigDecimal,
) -> AppResult<Option<(SpotOrder, SpotOrder, SpotTrade)>> {
    let mut tx = pool.begin().await?;
    let order = lock_spot_order_by_db_id(&mut tx, order_id).await?;
    if !is_triggerable_stop_limit_buy_order(&order, market_price) {
        tx.commit().await?;
        return Ok(None);
    }
    let result = execute_triggered_buy_order_in_tx(&mut tx, order, market_price).await?;
    tx.commit().await?;
    Ok(Some(result))
}

pub(crate) async fn execute_triggered_stop_limit_sell_order(
    pool: &Pool<MySql>,
    order_id: u64,
    market_price: &BigDecimal,
) -> AppResult<Option<(SpotOrder, SpotOrder, SpotTrade)>> {
    let mut tx = pool.begin().await?;
    let order = lock_spot_order_by_db_id(&mut tx, order_id).await?;
    if !is_triggerable_stop_limit_sell_order(&order, market_price) {
        tx.commit().await?;
        return Ok(None);
    }
    let result = execute_triggered_sell_order_in_tx(&mut tx, order, market_price).await?;
    tx.commit().await?;
    Ok(Some(result))
}

async fn execute_triggered_buy_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    mut buy_order: SpotOrder,
    execution_price: &BigDecimal,
) -> AppResult<(SpotOrder, SpotOrder, SpotTrade)> {
    if buy_order.side != OrderSide::Buy {
        return Err(AppError::Internal(
            "triggered spot execution requires a buy order".to_owned(),
        ));
    }
    if let Some(limit_price) = buy_order.price.as_ref()
        && execution_price > limit_price
    {
        return Err(AppError::Validation(
            "market price is above buy limit".to_owned(),
        ));
    }
    let fill_quantity = buy_order.quantity.clone() - buy_order.filled_quantity.clone();
    if fill_quantity <= BigDecimal::from(0) {
        return Err(AppError::Validation(
            "spot buy order has no remaining quantity".to_owned(),
        ));
    }
    let assets = pair_assets_in_tx(tx, &buy_order.pair_id).await?;
    let buyer_id = buy_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    ensure_wallet_account_in_tx(tx, buyer_id, assets.base_asset_id).await?;
    let liquidity_user_id = ensure_spot_liquidity_user_in_tx(tx).await?;
    credit_spot_liquidity_wallet_in_tx(
        tx,
        liquidity_user_id,
        assets.base_asset_id,
        &fill_quantity,
        "spot_system_liquidity_credit",
        "spot_order",
        &buy_order.id,
    )
    .await?;
    ensure_wallet_account_in_tx(tx, liquidity_user_id, assets.quote_asset_id).await?;

    let mut sell_order = insert_spot_liquidity_sell_order_in_tx(
        tx,
        liquidity_user_id,
        &buy_order,
        execution_price,
        &fill_quantity,
    )
    .await?;
    let sell_reservation = SpotOrderReservation {
        asset_id: assets.base_asset_id,
        amount: fill_quantity.clone(),
    };
    freeze_wallet_for_inserted_order_in_tx(tx, &sell_order, &sell_reservation).await?;

    buy_order.status = OrderStatus::Open;
    sell_order.status = OrderStatus::Open;
    let trade_idempotency_key = format!("spot_triggered_buy:{}", buy_order.id);
    let trade = insert_spot_trade(
        tx,
        &buy_order,
        &sell_order,
        execution_price,
        &fill_quantity,
        &trade_idempotency_key,
    )
    .await?;
    lock_spot_fill_wallet_rows_in_order(
        tx,
        buyer_id,
        liquidity_user_id,
        assets.base_asset_id,
        assets.quote_asset_id,
    )
    .await?;

    let fill_quote_amount = execution_price.clone() * fill_quantity.clone();
    let buy_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(tx, &buy_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        OrderSide::Buy,
    )?;
    let sell_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(tx, &sell_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &sell_order_remaining_reservation,
        &trade.quantity,
        OrderSide::Sell,
    )?;

    apply_fill(&mut buy_order, trade.quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot buy fill: {error:?}")))?;
    apply_fill(&mut sell_order, trade.quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot sell fill: {error:?}")))?;
    let ref_id = format!("{}:{}", buy_order.id, sell_order.id);
    let ledger = SpotLedgerMetadata {
        change_type: "spot_trade_settlement",
        ref_type: "spot_trade",
        ref_id: &ref_id,
    };
    apply_spot_wallet_settlement_leg(
        tx,
        buyer_id,
        assets.quote_asset_id,
        &fill_quote_amount,
        false,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        buyer_id,
        assets.base_asset_id,
        &trade.quantity,
        true,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        liquidity_user_id,
        assets.base_asset_id,
        &trade.quantity,
        false,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        liquidity_user_id,
        assets.quote_asset_id,
        &fill_quote_amount,
        true,
        ledger,
    )
    .await?;
    release_buy_order_surplus_reservation_after_fill(
        tx,
        buyer_id,
        &buy_order,
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        &ref_id,
    )
    .await?;
    insert_spot_fill_commissions_in_tx(
        tx,
        &trade,
        buyer_id,
        liquidity_user_id,
        assets.base_asset_id,
        assets.quote_asset_id,
        &fill_quote_amount,
    )
    .await?;
    save_spot_order_fill_state(tx, &buy_order).await?;
    save_spot_order_fill_state(tx, &sell_order).await?;
    Ok((buy_order, sell_order, trade))
}

async fn execute_triggered_sell_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    mut sell_order: SpotOrder,
    execution_price: &BigDecimal,
) -> AppResult<(SpotOrder, SpotOrder, SpotTrade)> {
    if sell_order.side != OrderSide::Sell {
        return Err(AppError::Internal(
            "triggered spot execution requires a sell order".to_owned(),
        ));
    }
    if let Some(limit_price) = sell_order.price.as_ref()
        && execution_price < limit_price
    {
        return Err(AppError::Validation(
            "market price is below sell limit".to_owned(),
        ));
    }
    let fill_quantity = sell_order.quantity.clone() - sell_order.filled_quantity.clone();
    if fill_quantity <= BigDecimal::from(0) {
        return Err(AppError::Validation(
            "spot sell order has no remaining quantity".to_owned(),
        ));
    }
    let assets = pair_assets_in_tx(tx, &sell_order.pair_id).await?;
    let seller_id = sell_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    ensure_wallet_account_in_tx(tx, seller_id, assets.quote_asset_id).await?;
    let liquidity_user_id = ensure_spot_liquidity_user_in_tx(tx).await?;
    let fill_quote_amount = execution_price.clone() * fill_quantity.clone();
    credit_spot_liquidity_wallet_in_tx(
        tx,
        liquidity_user_id,
        assets.quote_asset_id,
        &fill_quote_amount,
        "spot_system_liquidity_credit",
        "spot_order",
        &sell_order.id,
    )
    .await?;
    ensure_wallet_account_in_tx(tx, liquidity_user_id, assets.base_asset_id).await?;

    let mut buy_order = insert_spot_liquidity_buy_order_in_tx(
        tx,
        liquidity_user_id,
        &sell_order,
        execution_price,
        &fill_quantity,
    )
    .await?;
    let buy_reservation = SpotOrderReservation {
        asset_id: assets.quote_asset_id,
        amount: fill_quote_amount.clone(),
    };
    freeze_wallet_for_inserted_order_in_tx(tx, &buy_order, &buy_reservation).await?;

    buy_order.status = OrderStatus::Open;
    sell_order.status = OrderStatus::Open;
    let trade_idempotency_key = format!("spot_triggered_sell:{}", sell_order.id);
    let trade = insert_spot_trade(
        tx,
        &buy_order,
        &sell_order,
        execution_price,
        &fill_quantity,
        &trade_idempotency_key,
    )
    .await?;
    lock_spot_fill_wallet_rows_in_order(
        tx,
        liquidity_user_id,
        seller_id,
        assets.base_asset_id,
        assets.quote_asset_id,
    )
    .await?;

    let buy_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(tx, &buy_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        OrderSide::Buy,
    )?;
    let sell_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(tx, &sell_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &sell_order_remaining_reservation,
        &trade.quantity,
        OrderSide::Sell,
    )?;

    apply_fill(&mut buy_order, trade.quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot buy fill: {error:?}")))?;
    apply_fill(&mut sell_order, trade.quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot sell fill: {error:?}")))?;
    let ref_id = format!("{}:{}", buy_order.id, sell_order.id);
    let ledger = SpotLedgerMetadata {
        change_type: "spot_trade_settlement",
        ref_type: "spot_trade",
        ref_id: &ref_id,
    };
    apply_spot_wallet_settlement_leg(
        tx,
        liquidity_user_id,
        assets.quote_asset_id,
        &fill_quote_amount,
        false,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        liquidity_user_id,
        assets.base_asset_id,
        &trade.quantity,
        true,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        seller_id,
        assets.base_asset_id,
        &trade.quantity,
        false,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        seller_id,
        assets.quote_asset_id,
        &fill_quote_amount,
        true,
        ledger,
    )
    .await?;
    release_buy_order_surplus_reservation_after_fill(
        tx,
        liquidity_user_id,
        &buy_order,
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        &ref_id,
    )
    .await?;
    insert_spot_fill_commissions_in_tx(
        tx,
        &trade,
        liquidity_user_id,
        seller_id,
        assets.base_asset_id,
        assets.quote_asset_id,
        &fill_quote_amount,
    )
    .await?;
    save_spot_order_fill_state(tx, &buy_order).await?;
    save_spot_order_fill_state(tx, &sell_order).await?;
    Ok((sell_order, buy_order, trade))
}

#[allow(clippy::too_many_arguments)]
async fn insert_spot_fill_commissions_in_tx(
    tx: &mut Transaction<'_, MySql>,
    trade: &SpotTrade,
    buyer_id: u64,
    seller_id: u64,
    base_asset_id: u64,
    quote_asset_id: u64,
    fill_quote_amount: &BigDecimal,
) -> AppResult<()> {
    // 买卖双方的返佣基数和结算资产不同，来源类型分开后同代理撮合也不会撞幂等键。
    insert_agent_business_commission_in_tx(
        tx,
        AgentBusinessCommissionWrite {
            user_id: buyer_id,
            product_type: AGENT_COMMISSION_PRODUCT_SPOT,
            source_type: "spot_trade_buy",
            source_id: &trade.id,
            source_amount: fill_quote_amount,
            payout_asset_id: quote_asset_id,
        },
    )
    .await?;
    insert_agent_business_commission_in_tx(
        tx,
        AgentBusinessCommissionWrite {
            user_id: seller_id,
            product_type: AGENT_COMMISSION_PRODUCT_SPOT,
            source_type: "spot_trade_sell",
            source_id: &trade.id,
            source_amount: &trade.quantity,
            payout_asset_id: base_asset_id,
        },
    )
    .await
}

fn ensure_limit_buy_price_reached(order: &NewOrder, execution_price: &BigDecimal) -> AppResult<()> {
    if order.side != OrderSide::Buy
        || !matches!(order.order_type, OrderType::Limit | OrderType::StopLimit)
    {
        return Err(AppError::Internal(
            "price trigger requires a buy limit order".to_owned(),
        ));
    }
    let limit_price = order
        .price
        .as_ref()
        .ok_or_else(|| AppError::Validation("price is required for limit orders".to_owned()))?;
    if execution_price > limit_price {
        return Err(AppError::Validation(
            "market price is above buy limit".to_owned(),
        ));
    }
    if order.order_type == OrderType::StopLimit {
        let trigger_price = order.trigger_price.as_ref().ok_or_else(|| {
            AppError::Validation("trigger_price is required for stop limit orders".to_owned())
        })?;
        if execution_price > trigger_price {
            return Err(AppError::Validation(
                "market price is above buy trigger".to_owned(),
            ));
        }
    }
    Ok(())
}

fn ensure_limit_sell_price_reached(
    order: &NewOrder,
    execution_price: &BigDecimal,
) -> AppResult<()> {
    if order.side != OrderSide::Sell
        || !matches!(order.order_type, OrderType::Limit | OrderType::StopLimit)
    {
        return Err(AppError::Internal(
            "price trigger requires a sell limit order".to_owned(),
        ));
    }
    let limit_price = order
        .price
        .as_ref()
        .ok_or_else(|| AppError::Validation("price is required for limit orders".to_owned()))?;
    if execution_price < limit_price {
        return Err(AppError::Validation(
            "market price is below sell limit".to_owned(),
        ));
    }
    if order.order_type == OrderType::StopLimit {
        let trigger_price = order.trigger_price.as_ref().ok_or_else(|| {
            AppError::Validation("trigger_price is required for stop limit orders".to_owned())
        })?;
        if execution_price < trigger_price {
            return Err(AppError::Validation(
                "market price is below sell trigger".to_owned(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

fn optional_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn spot_order_idempotency_check_for_request(
    request: &CreateSpotOrderRequest,
) -> SpotOrderIdempotencyCheck {
    let expected_reservation_price = match request.order_type {
        OrderType::Limit => request.price.as_ref(),
        OrderType::Market => request.reference_price.as_ref(),
        OrderType::StopLimit => request.price.as_ref(),
    };
    SpotOrderIdempotencyCheck {
        pair_id: request.pair_id.clone(),
        side: request.side,
        order_type: request.order_type,
        price: match request.order_type {
            OrderType::Limit | OrderType::StopLimit => request.price.clone(),
            OrderType::Market => None,
        },
        trigger_price: request.trigger_price.clone(),
        quantity: request.quantity.clone(),
        reserved_amount: expected_reservation_price
            .map(|price| spot_reservation_amount(request.side, price, &request.quantity)),
        request_reference_price: match request.order_type {
            OrderType::Limit | OrderType::StopLimit => None,
            OrderType::Market => request.reference_price.clone(),
        },
        request_price: request.price.clone(),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_spot_application_tests.rs"]
mod tests;
