//! new_coin bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    architecture::ApplicationLayer,
    error::{AppError, AppResult},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        new_coin::{
            LifecycleStatus,
            infrastructure::MySqlNewCoinReadRepository,
            presentation::{
                CreatePurchaseRequest, CreateSubscriptionRequest, ListQuery,
                NewCoinDistributionResponse, NewCoinDistributionsResponse,
                NewCoinOrderCreationResponse, NewCoinProjectResponse, NewCoinProjectsResponse,
                NewCoinPurchaseResponse, NewCoinPurchasesResponse, NewCoinSubscriptionResponse,
                NewCoinSubscriptionsResponse, NewCoinUnlockResponse, NewCoinUnlocksResponse,
                PayUnlockFeeRequest, PayUnlockFeeResponse, ReleaseUnlockResponse,
            },
            repository::{
                NewCoinOrderRepository, NewCoinPurchaseOrderWrite, NewCoinReadRepository,
                NewCoinSubscriptionOrderWrite, NewCoinUnlockFeeRepository,
                NewCoinUnlockReleaseRepository, UnlockFeePaymentWrite,
            },
            service::{
                ensure_idempotency_key, ensure_positive_amount,
                ensure_post_listing_purchase_enabled, ensure_unlock_fee_payment_matches,
                lifecycle_status, lock_positions_for_project, route_limit, user_id_from_subject,
            },
        },
    },
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use serde_json::json;
use sqlx::{MySql, Pool};

#[derive(Debug)]
pub struct ApplicationLayerMarker;

impl ApplicationLayer for ApplicationLayerMarker {}

pub(crate) async fn list_new_coin_projects(
    pool: Option<Pool<MySql>>,
    query: ListQuery,
) -> AppResult<NewCoinProjectsResponse> {
    let repository = new_coin_read_repository(pool)?;
    let projects = repository
        .list_active_projects(route_limit(query.limit))
        .await?
        .into_iter()
        .map(NewCoinProjectResponse::from)
        .collect();
    Ok(NewCoinProjectsResponse { projects })
}

pub(crate) async fn get_new_coin_project(
    pool: Option<Pool<MySql>>,
    symbol: &str,
) -> AppResult<NewCoinProjectResponse> {
    let repository = new_coin_read_repository(pool)?;
    repository
        .find_active_project_by_symbol(symbol)
        .await?
        .map(NewCoinProjectResponse::from)
        .ok_or(AppError::NotFound)
}

pub(crate) async fn list_new_coin_subscriptions(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: ListQuery,
) -> AppResult<NewCoinSubscriptionsResponse> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let subscriptions = repository
        .list_user_subscriptions(user_id, route_limit(query.limit))
        .await?
        .into_iter()
        .map(NewCoinSubscriptionResponse::from)
        .collect();
    Ok(NewCoinSubscriptionsResponse { subscriptions })
}

pub(crate) async fn list_new_coin_distributions(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: ListQuery,
) -> AppResult<NewCoinDistributionsResponse> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let distributions = repository
        .list_user_distributions(user_id, route_limit(query.limit))
        .await?
        .into_iter()
        .map(NewCoinDistributionResponse::from)
        .collect();
    Ok(NewCoinDistributionsResponse { distributions })
}

pub(crate) async fn list_new_coin_purchases(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: ListQuery,
) -> AppResult<NewCoinPurchasesResponse> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let purchases = repository
        .list_user_purchases(user_id, route_limit(query.limit))
        .await?
        .into_iter()
        .map(NewCoinPurchaseResponse::from)
        .collect();
    Ok(NewCoinPurchasesResponse { purchases })
}

pub(crate) async fn list_new_coin_unlocks(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: ListQuery,
) -> AppResult<NewCoinUnlocksResponse> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let unlocks = repository
        .list_user_unlocks(user_id, route_limit(query.limit))
        .await?
        .into_iter()
        .map(NewCoinUnlockResponse::from)
        .collect();
    Ok(NewCoinUnlocksResponse { unlocks })
}

pub(crate) async fn pay_new_coin_unlock_fee(
    pool: Option<Pool<MySql>>,
    subject: &str,
    unlock_idempotency_key: String,
    request: PayUnlockFeeRequest,
) -> AppResult<PayUnlockFeeResponse> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let expectation = repository
        .find_unlock_fee_expectation(&unlock_idempotency_key, user_id)
        .await?
        .ok_or(AppError::NotFound)?;
    ensure_unlock_fee_payment_matches(&expectation, request.payment_asset_id, &request.amount)?;
    let paid = repository
        .mark_unlock_fee_paid(UnlockFeePaymentWrite {
            unlock_idempotency_key: unlock_idempotency_key.clone(),
            user_id,
            payment_asset_id: request.payment_asset_id,
            amount: request.amount,
        })
        .await?;

    Ok(PayUnlockFeeResponse {
        unlock_idempotency_key,
        paid,
    })
}

pub(crate) async fn release_new_coin_unlock_with_events(
    pool: Option<Pool<MySql>>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    unlock_idempotency_key: String,
) -> AppResult<ReleaseUnlockResponse> {
    // 应用层统一处理解锁放行后的私有事件广播，路由层不再感知解锁结果格式。
    let user_id = user_id_from_subject(subject)?;
    let (response, released_outcome) =
        release_new_coin_unlock_with_internal(pool, subject, unlock_idempotency_key.clone())
            .await?;
    if let Some((asset_id, unlock_quantity)) = released_outcome {
        if let Some(hub) = event_broadcast_hub {
            hub.publish(EventBroadcastMessage::private_user(
                user_id,
                json!({
                    "type": "new_coin.unlock.released",
                    "unlock_idempotency_key": unlock_idempotency_key,
                    "asset_id": asset_id,
                    "unlock_quantity": unlock_quantity,
                    "released": true,
                })
                .to_string(),
            ));
        }
    }
    Ok(response)
}

async fn release_new_coin_unlock_with_internal(
    pool: Option<Pool<MySql>>,
    subject: &str,
    unlock_idempotency_key: String,
) -> AppResult<(ReleaseUnlockResponse, Option<(u64, BigDecimal)>)> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let outcome = repository
        .release_due_paid_unlock(&unlock_idempotency_key, user_id)
        .await?;
    let event_payload = if outcome.released {
        Some((outcome.asset_id, outcome.unlock_quantity))
    } else {
        None
    };

    Ok((
        ReleaseUnlockResponse {
            unlock_idempotency_key,
            released: true,
        },
        event_payload,
    ))
}

pub(crate) async fn create_new_coin_subscription_with_events(
    pool: Option<Pool<MySql>>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    symbol: String,
    request: CreateSubscriptionRequest,
) -> AppResult<NewCoinOrderCreationResponse> {
    // 应用层负责编排认购事件，仅在成功创建后发布统一事件体。
    let user_id = user_id_from_subject(subject)?;
    let (response, event_payload) =
        create_new_coin_subscription_with_internal(pool, subject, symbol, request).await?;
    if let Some(payload) = event_payload
        && let Some(hub) = event_broadcast_hub
    {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "new_coin.subscription.created",
                "idempotency_key": payload.idempotency_key,
                "project_id": payload.project_id,
                "asset_id": payload.asset_id,
                "quote_asset_id": payload.quote_asset_id,
                "quote_amount": payload.quote_amount,
                "quantity": payload.quantity,
                "status": payload.status,
                "lock_position_id": payload.lock_position_id,
            })
            .to_string(),
        ));
    }
    Ok(response)
}

struct NewCoinSubscriptionEventPayload {
    idempotency_key: String,
    project_id: u64,
    asset_id: u64,
    quote_asset_id: u64,
    quote_amount: BigDecimal,
    quantity: BigDecimal,
    status: String,
    lock_position_id: Option<u64>,
}

async fn create_new_coin_subscription_with_internal(
    pool: Option<Pool<MySql>>,
    subject: &str,
    symbol: String,
    request: CreateSubscriptionRequest,
) -> AppResult<(
    NewCoinOrderCreationResponse,
    Option<NewCoinSubscriptionEventPayload>,
)> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let project = repository
        .find_project_rule_by_symbol(&symbol)
        .await?
        .ok_or(AppError::NotFound)?;
    if lifecycle_status(&project.lifecycle_status)? != LifecycleStatus::Subscription {
        return Err(AppError::Validation(
            "new coin subscription is not open for this project".to_owned(),
        ));
    }
    ensure_positive_amount(&request.quote_amount, "quote_amount")?;
    ensure_positive_amount(&request.quantity, "quantity")?;
    ensure_idempotency_key(&request.idempotency_key)?;

    let idempotency_key = request.idempotency_key.clone();
    let quantity = request.quantity.clone();
    let quote_amount = request.quote_amount.clone();
    let quote_asset_id = request.quote_asset_id;
    let lock_positions = lock_positions_for_project(
        &project,
        user_id,
        project.asset_id,
        &idempotency_key,
        quantity.clone(),
        Utc::now(),
        "new_coin_subscription",
    )?;
    let lock_position_id = repository
        .create_subscription_order(NewCoinSubscriptionOrderWrite {
            user_id,
            project: project.clone(),
            quote_asset_id,
            quote_amount: quote_amount.clone(),
            quantity: quantity.clone(),
            idempotency_key: idempotency_key.clone(),
            lock_positions,
        })
        .await?;
    let status = if lock_position_id.is_some() {
        "allocated".to_owned()
    } else {
        "available".to_owned()
    };
    let response = NewCoinOrderCreationResponse {
        idempotency_key,
        status,
        lock_position_id,
    };
    let event_payload = NewCoinSubscriptionEventPayload {
        idempotency_key: response.idempotency_key.clone(),
        project_id: project.id,
        asset_id: project.asset_id,
        quote_asset_id,
        quote_amount,
        quantity,
        status: response.status.clone(),
        lock_position_id,
    };
    Ok((response, Some(event_payload)))
}

pub(crate) async fn create_new_coin_purchase_with_events(
    pool: Option<Pool<MySql>>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    symbol: String,
    request: CreatePurchaseRequest,
) -> AppResult<NewCoinOrderCreationResponse> {
    // 应用层统一认购后事件构建，避免路由层感知具体消息体。
    let user_id = user_id_from_subject(subject)?;
    let (response, event_payload) =
        create_new_coin_purchase_with_internal(pool, subject, symbol, request).await?;
    if let Some(payload) = event_payload
        && let Some(hub) = event_broadcast_hub
    {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "new_coin.purchase.created",
                "idempotency_key": payload.idempotency_key,
                "project_id": payload.project_id,
                "pair_id": payload.pair_id,
                "asset_id": payload.asset_id,
                "quote_asset_id": payload.quote_asset_id,
                "price": payload.price,
                "quantity": payload.quantity,
                "quote_amount": payload.quote_amount,
                "status": payload.status,
                "lock_position_id": payload.lock_position_id,
            })
            .to_string(),
        ));
    }
    Ok(response)
}

struct NewCoinPurchaseEventPayload {
    idempotency_key: String,
    project_id: u64,
    pair_id: u64,
    asset_id: u64,
    quote_asset_id: u64,
    price: BigDecimal,
    quantity: BigDecimal,
    quote_amount: BigDecimal,
    status: String,
    lock_position_id: Option<u64>,
}

async fn create_new_coin_purchase_with_internal(
    pool: Option<Pool<MySql>>,
    subject: &str,
    symbol: String,
    request: CreatePurchaseRequest,
) -> AppResult<(
    NewCoinOrderCreationResponse,
    Option<NewCoinPurchaseEventPayload>,
)> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let project = repository
        .find_project_rule_by_symbol(&symbol)
        .await?
        .ok_or(AppError::NotFound)?;
    if lifecycle_status(&project.lifecycle_status)? != LifecycleStatus::Listed {
        return Err(AppError::Validation(
            "post-listing new coin purchase is not open for this project".to_owned(),
        ));
    }
    // 上市后认购必须服从后台单独开关和绑定交易对，避免用户绕过后台配置直接下单。
    ensure_post_listing_purchase_enabled(&project, request.pair_id)?;
    ensure_positive_amount(&request.price, "price")?;
    ensure_positive_amount(&request.quantity, "quantity")?;
    ensure_idempotency_key(&request.idempotency_key)?;

    let pair = repository
        .find_pair_for_purchase(request.pair_id, project.asset_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let idempotency_key = request.idempotency_key.clone();
    let price = request.price.clone();
    let quantity = request.quantity.clone();
    let quote_amount = price.clone() * quantity.clone();
    let pair_id = request.pair_id;
    let lock_position_id = repository
        .create_purchase_order(NewCoinPurchaseOrderWrite {
            user_id,
            project: project.clone(),
            pair_id,
            price: price.clone(),
            quantity: quantity.clone(),
            quote_amount: quote_amount.clone(),
            idempotency_key: idempotency_key.clone(),
        })
        .await?;
    let status = if lock_position_id.is_some() {
        "locked".to_owned()
    } else {
        "available".to_owned()
    };
    let response = NewCoinOrderCreationResponse {
        idempotency_key,
        status,
        lock_position_id,
    };
    let event_payload = NewCoinPurchaseEventPayload {
        idempotency_key: response.idempotency_key.clone(),
        project_id: project.id,
        pair_id,
        asset_id: project.asset_id,
        quote_asset_id: pair.quote_asset_id,
        price,
        quantity,
        quote_amount,
        status: response.status.clone(),
        lock_position_id,
    };
    Ok((response, Some(event_payload)))
}

fn new_coin_read_repository(pool: Option<Pool<MySql>>) -> AppResult<MySqlNewCoinReadRepository> {
    Ok(MySqlNewCoinReadRepository::new(new_coin_mysql_pool(pool)?))
}

fn new_coin_mysql_pool(pool: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for new coin routes".to_owned())
    })
}
