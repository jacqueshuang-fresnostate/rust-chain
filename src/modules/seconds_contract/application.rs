//! seconds_contract bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use super::{
    infrastructure,
    presentation::{
        AdminOrdersQuery, CreateSecondsContractProductRequest, DeleteSecondsContractProductRequest,
        OpenSecondsContractOrderRequest, OpenSecondsContractOrderResponse,
        SecondsContractOrderResponse, SecondsContractOrdersResponse,
        SecondsContractProductResponse, SecondsContractProductsResponse,
        SettleSecondsContractOrderRequest, SettleSecondsContractOrderResponse,
        UpdateSecondsContractProductRequest, UpdateSecondsContractProductStatusRequest,
    },
    repository::{
        SecondsContractAdminOrderFilter, SecondsContractOrderInsert, SecondsContractProductWrite,
        SecondsContractWalletLedgerWrite,
    },
    service::{
        NormalizedSecondsContractProductCycle, ensure_existing_order_matches_request,
        ensure_existing_settlement_matches, is_duplicate_key_error, normalize_direction,
        normalize_idempotency_key, normalize_settlement_result, normalized_product_status,
        optional_image_url, optional_string, order_audit_json, product_audit_json,
        publish_seconds_contract_order_opened_event_if_needed,
        publish_seconds_contract_order_settled_event_if_needed, required_reason, route_limit,
        settlement_payout_amount, validate_create_product_request, validate_product_stake,
        validate_stake_amount, validate_update_product_request,
    },
};
use crate::{
    error::{AppError, AppResult},
    modules::agent::{
        infrastructure::insert_agent_business_commission_in_tx,
        repository::AgentBusinessCommissionWrite,
        service::AGENT_COMMISSION_PRODUCT_SECONDS_CONTRACT,
    },
    state::AppState,
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};

/// 统一从应用状态中获取数据库连接池。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for seconds contract routes".to_owned())
    })
}

pub(crate) async fn list_active_products(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<SecondsContractProductsResponse> {
    let products = infrastructure::list_products(pool, Some("active"), limit).await?;
    Ok(SecondsContractProductsResponse { products })
}

pub(crate) async fn list_admin_products(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<SecondsContractProductsResponse> {
    let products = infrastructure::list_products(pool, None, limit).await?;
    Ok(SecondsContractProductsResponse { products })
}

pub(crate) async fn get_admin_product(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<SecondsContractProductResponse> {
    infrastructure::load_product_by_id_from_pool(pool, product_id).await
}

pub(crate) async fn create_product(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    request: CreateSecondsContractProductRequest,
) -> AppResult<SecondsContractProductResponse> {
    let cycles = validate_create_product_request(&request)?;
    let reason = required_reason(request.reason.clone())?;
    let status = normalized_product_status(request.status.as_deref().unwrap_or("active"))?;
    let logo_url = optional_image_url(
        request.logo_url.clone(),
        "seconds contract product logo_url",
    )?;
    let default_cycle = default_product_cycle(&cycles)?;
    let write = product_write_from_cycle(
        request.pair_id,
        request.stake_asset,
        logo_url,
        status,
        default_cycle,
    );

    let pool = require_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    // 产品主表、周期配置和后台审计必须同事务提交，避免配置生效后缺少可追溯记录。
    infrastructure::ensure_pair_exists(&mut tx, write.pair_id).await?;
    infrastructure::ensure_asset_exists(&mut tx, write.stake_asset).await?;
    let product_id = infrastructure::insert_product(&mut tx, &write).await?;
    infrastructure::insert_product_cycles(&mut tx, product_id, &cycles).await?;
    let product = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_product.create",
        "seconds_contract_product",
        product.id,
        None,
        Some(product_audit_json(&product)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(product)
}

pub(crate) async fn update_product(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    product_id: u64,
    request: UpdateSecondsContractProductRequest,
) -> AppResult<SecondsContractProductResponse> {
    let cycles = validate_update_product_request(&request)?;
    let reason = required_reason(request.reason.clone())?;
    let status = normalized_product_status(&request.status)?;
    let logo_url = optional_image_url(
        request.logo_url.clone(),
        "seconds contract product logo_url",
    )?;
    let default_cycle = default_product_cycle(&cycles)?;
    let write = product_write_from_cycle(
        request.pair_id,
        request.stake_asset,
        logo_url,
        status,
        default_cycle,
    );

    let pool = require_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    // 编辑产品时先锁定旧快照，再写入新快照和审计，确保审计 before/after 对应同一次变更。
    let before = infrastructure::lock_product_by_id(&mut tx, product_id).await?;
    infrastructure::ensure_pair_exists(&mut tx, write.pair_id).await?;
    infrastructure::ensure_asset_exists(&mut tx, write.stake_asset).await?;
    infrastructure::update_product(&mut tx, product_id, &write).await?;
    infrastructure::replace_product_cycles(&mut tx, product_id, &cycles).await?;
    let after = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_product.update",
        "seconds_contract_product",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_product_status(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    product_id: u64,
    request: UpdateSecondsContractProductStatusRequest,
) -> AppResult<SecondsContractProductResponse> {
    let status = normalized_product_status(&request.status)?;
    let reason = required_reason(request.reason.clone())?;
    let pool = require_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    // 状态变更同样保留 before/after 审计，便于追踪产品下架或恢复的责任人和原因。
    let before = infrastructure::lock_product_by_id(&mut tx, product_id).await?;
    infrastructure::update_product_status(&mut tx, product_id, &status).await?;
    let after = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_product.update_status",
        "seconds_contract_product",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn delete_product(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    product_id: u64,
    request: DeleteSecondsContractProductRequest,
) -> AppResult<()> {
    let reason = required_reason(request.reason.clone())?;
    let pool = require_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    // 删除前锁定产品并确认已禁用、无订单，避免仍可交易的秒合约配置被物理删除。
    let before = infrastructure::lock_product_by_id(&mut tx, product_id).await?;
    if before.status != "disabled" {
        return Err(AppError::Validation(
            "seconds contract product must be disabled before deletion".to_owned(),
        ));
    }
    infrastructure::ensure_product_has_no_orders(&mut tx, product_id).await?;
    infrastructure::delete_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_product.delete",
        "seconds_contract_product",
        product_id,
        Some(product_audit_json(&before)),
        None,
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn open_order(
    pool: Option<&Pool<MySql>>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    request: OpenSecondsContractOrderRequest,
) -> AppResult<(OpenSecondsContractOrderResponse, bool)> {
    let idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    let direction = normalize_direction(&request.direction)?;
    validate_stake_amount(&request.stake_amount)?;
    let pool = require_mysql_pool(pool)?;

    if let Some(existing) =
        infrastructure::existing_order_for_idempotency_key_readonly(pool, user_id, &idempotency_key)
            .await?
    {
        ensure_existing_order_matches_request(
            &existing,
            request.product_id,
            request.duration_seconds,
            &direction,
            &request.stake_amount,
        )?;
        return Ok((OpenSecondsContractOrderResponse { order: existing }, false));
    }

    let mut tx = pool.begin().await?;
    let product = match infrastructure::lock_active_product(
        &mut tx,
        request.product_id,
        request.duration_seconds,
    )
    .await
    {
        Ok(product) => product,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            if let Some(existing) = replay_existing_order_if_present(
                pool,
                user_id,
                request.product_id,
                request.duration_seconds,
                &direction,
                &request.stake_amount,
                &idempotency_key,
            )
            .await?
            {
                return Ok((OpenSecondsContractOrderResponse { order: existing }, false));
            }
            return Err(AppError::NotFound);
        }
        Err(error) => return Err(error),
    };
    validate_product_stake(&request.stake_amount, &product)?;
    let entry_price =
        infrastructure::cached_entry_price(redis, product.pair_id, product.symbol.as_str()).await?;
    let expires_at = Utc::now() + chrono::TimeDelta::seconds(product.duration_seconds as i64);
    let order = SecondsContractOrderInsert {
        user_id,
        product_id: product.id,
        pair_id: product.pair_id,
        stake_asset: product.stake_asset,
        direction,
        stake_amount: request.stake_amount.clone(),
        duration_seconds: product.duration_seconds,
        payout_rate: product.payout_rate.clone(),
        entry_price,
        idempotency_key,
        expires_at,
    };

    // 先占用用户幂等键，再锁钱包扣款；并发同 key 请求只会有一个进入扣款路径。
    let order_id = match infrastructure::insert_open_order(&mut tx, &order).await {
        Ok(order_id) => order_id,
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            return replay_existing_order(
                pool,
                user_id,
                request.product_id,
                request.duration_seconds,
                &order.direction,
                &request.stake_amount,
                &order.idempotency_key,
            )
            .await
            .map(|order| (OpenSecondsContractOrderResponse { order }, false));
        }
        Err(error) => return Err(AppError::Database(error)),
    };

    let wallet = infrastructure::lock_wallet_row(&mut tx, user_id, product.stake_asset).await?;
    if wallet.available < request.stake_amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for seconds contract: requested {}, available {}, locked {}",
            request.stake_amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - request.stake_amount.clone();
    // 订单、钱包扣款和流水必须同事务提交，避免出现已开仓但余额/流水不一致。
    infrastructure::update_wallet_available(
        &mut tx,
        user_id,
        product.stake_asset,
        &available_after,
    )
    .await?;
    infrastructure::insert_wallet_ledger(
        &mut tx,
        SecondsContractWalletLedgerWrite {
            user_id,
            asset_id: product.stake_asset,
            change_type: "seconds_contract_open",
            amount: -request.stake_amount.clone(),
            available_after: available_after.clone(),
            frozen_after: wallet.frozen,
            locked_after: wallet.locked,
            ref_id: order_id.to_string(),
        },
    )
    .await?;

    let commission_source_id = order_id.to_string();
    insert_agent_business_commission_in_tx(
        &mut tx,
        AgentBusinessCommissionWrite {
            user_id,
            product_type: AGENT_COMMISSION_PRODUCT_SECONDS_CONTRACT,
            source_type: "seconds_contract_order",
            source_id: &commission_source_id,
            source_amount: &request.stake_amount,
            payout_asset_id: product.stake_asset,
        },
    )
    .await?;

    let order = infrastructure::load_order_by_id(&mut tx, order_id).await?;
    tx.commit().await?;
    Ok((OpenSecondsContractOrderResponse { order }, true))
}

pub(crate) async fn open_order_with_events(
    pool: Option<&Pool<MySql>>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    request: OpenSecondsContractOrderRequest,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<OpenSecondsContractOrderResponse> {
    // 秒合约开仓保持原有幂等和钱包结算逻辑不变，在应用层统一触发开仓事件。
    let (response, is_new_order) = open_order(pool, redis, user_id, request).await?;
    publish_seconds_contract_order_opened_event_if_needed(hub, user_id, &response, is_new_order);
    Ok(response)
}

pub(crate) async fn settle_order(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    order_id: u64,
    request: SettleSecondsContractOrderRequest,
) -> AppResult<(SettleSecondsContractOrderResponse, bool)> {
    let result = normalize_settlement_result(&request.result)?;
    let reason = required_reason(request.reason.clone())?;
    let pool = require_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let order = infrastructure::lock_order_by_id(&mut tx, order_id).await?;
    let stake_asset_precision =
        infrastructure::load_asset_precision_scale(&mut tx, order.stake_asset).await?;
    if order.status == "settled" {
        ensure_existing_settlement_matches(&order, &result)?;
        let payout_amount = settlement_payout_amount(&order, &result, stake_asset_precision);
        tx.commit().await?;
        return Ok((
            SettleSecondsContractOrderResponse {
                order,
                payout_amount,
            },
            false,
        ));
    }
    if order.status != "opened" {
        return Err(AppError::Conflict(
            "seconds contract order is not open for settlement".to_owned(),
        ));
    }

    let before_json = Some(order_audit_json(&order, BigDecimal::from(0)));
    let payout_amount = settlement_payout_amount(&order, &result, stake_asset_precision);

    if payout_amount > 0 {
        let wallet =
            infrastructure::lock_wallet_row(&mut tx, order.user_id, order.stake_asset).await?;
        let available_after = wallet.available.clone() + payout_amount.clone();
        // 派奖入账和流水写入必须与订单结算状态同事务完成，避免重复派奖或遗漏审计。
        infrastructure::update_wallet_available(
            &mut tx,
            order.user_id,
            order.stake_asset,
            &available_after,
        )
        .await?;
        infrastructure::insert_wallet_ledger(
            &mut tx,
            SecondsContractWalletLedgerWrite {
                user_id: order.user_id,
                asset_id: order.stake_asset,
                change_type: "seconds_contract_settle_win",
                amount: payout_amount.clone(),
                available_after: available_after.clone(),
                frozen_after: wallet.frozen,
                locked_after: wallet.locked,
                ref_id: order.id.to_string(),
            },
        )
        .await?;
    }

    infrastructure::mark_order_settled(&mut tx, order.id, &result).await?;
    let settled_order = infrastructure::load_order_by_id(&mut tx, order.id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_order.settle",
        "seconds_contract_order",
        order.id,
        before_json,
        Some(order_audit_json(&settled_order, payout_amount.clone())),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok((
        SettleSecondsContractOrderResponse {
            order: settled_order,
            payout_amount,
        },
        true,
    ))
}

pub(crate) async fn settle_order_with_events(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    order_id: u64,
    request: SettleSecondsContractOrderRequest,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SettleSecondsContractOrderResponse> {
    // 秒合约结算统一返回响应对象，同时在应用层根据是否为新结算推送事件。
    let (response, is_new_settlement) = settle_order(pool, admin_id, order_id, request).await?;
    publish_seconds_contract_order_settled_event_if_needed(
        hub,
        response.order.user_id,
        &response,
        is_new_settlement,
    );
    Ok(response)
}

pub(crate) async fn list_user_orders(
    pool: &Pool<MySql>,
    user_id: u64,
    limit: u32,
) -> AppResult<SecondsContractOrdersResponse> {
    let orders = infrastructure::list_user_orders(pool, user_id, limit).await?;
    Ok(SecondsContractOrdersResponse { orders })
}

pub(crate) async fn list_admin_orders(
    pool: &Pool<MySql>,
    query: AdminOrdersQuery,
) -> AppResult<SecondsContractOrdersResponse> {
    let filter = SecondsContractAdminOrderFilter {
        user_id: query.user_id,
        email: optional_string(query.email),
        status: optional_string(query.status),
        limit: route_limit(query.limit),
    };
    let orders = infrastructure::list_admin_orders(pool, filter).await?;
    Ok(SecondsContractOrdersResponse { orders })
}

pub(crate) async fn get_admin_order(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    infrastructure::load_order_by_id_from_pool(pool, order_id).await
}

async fn replay_existing_order(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    duration_seconds: Option<u32>,
    direction: &str,
    stake_amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<SecondsContractOrderResponse> {
    replay_existing_order_if_present(
        pool,
        user_id,
        product_id,
        duration_seconds,
        direction,
        stake_amount,
        idempotency_key,
    )
    .await?
    .ok_or_else(|| {
        AppError::Conflict("seconds contract idempotency key is being committed".to_owned())
    })
}

async fn replay_existing_order_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    duration_seconds: Option<u32>,
    direction: &str,
    stake_amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<SecondsContractOrderResponse>> {
    let mut tx = pool.begin().await?;
    let Some(existing) =
        infrastructure::existing_order_for_idempotency_key(&mut tx, user_id, idempotency_key)
            .await?
    else {
        return Ok(None);
    };
    ensure_existing_order_matches_request(
        &existing,
        product_id,
        duration_seconds,
        direction,
        stake_amount,
    )?;
    tx.commit().await?;
    Ok(Some(existing))
}

fn default_product_cycle(
    cycles: &[NormalizedSecondsContractProductCycle],
) -> AppResult<&NormalizedSecondsContractProductCycle> {
    cycles
        .first()
        .ok_or_else(|| AppError::Validation("seconds contract cycles must not be empty".to_owned()))
}

fn product_write_from_cycle(
    pair_id: u64,
    stake_asset: u64,
    logo_url: Option<String>,
    status: String,
    cycle: &NormalizedSecondsContractProductCycle,
) -> SecondsContractProductWrite {
    SecondsContractProductWrite {
        pair_id,
        stake_asset,
        logo_url,
        duration_seconds: cycle.duration_seconds,
        payout_rate: cycle.payout_rate.clone(),
        min_stake: cycle.min_stake.clone(),
        max_stake: cycle.max_stake.clone(),
        status,
    }
}

fn require_mysql_pool(pool: Option<&Pool<MySql>>) -> AppResult<&Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for seconds contract routes".to_owned())
    })
}
