use super::support::validate_positive_decimal;
use crate::{
    error::{AppError, AppResult},
    modules::{
        events::EventBroadcastHub,
        margin::{
            domain::margin_position_payout_amount,
            infrastructure::{
                LockedMarginPositionRow, apply_cross_margin_position_settlement,
                cached_margin_mark_price, credit_margin_position_amount,
                load_cancelable_position_ids, load_open_position_ids, load_position_by_id,
                lock_user_position_by_id, mark_position_canceled, mark_position_closed,
            },
            presentation::{
                CancelAllMarginPositionsResponse, CancelMarginPositionResponse,
                CloseAllMarginPositionsResponse, CloseMarginPositionResponse,
                MarginBatchActionFailure, MarginPositionResponse,
            },
            service::{
                publish_margin_position_canceled_event_if_needed,
                publish_margin_position_closed_event_if_needed,
            },
        },
    },
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};
/// 主动平掉用户仓位；事务先锁定仓位，已非 opened 时重放当前终态且不再次结算。
/// opened 仓位必须有入场价并取得服务端新鲜标记价，再计算已实现盈亏、利息后权益和返还额。
/// 全仓仅以有符号权益更新原杠杆钱包，逐仓按 `wallet_scope` 返还非负金额；余额、流水和仓位终态同事务提交。
/// 成功提交后仅事件包装层对首次平仓发布通知；失败或终态重放均不得重复入账。
pub(crate) async fn close_margin_position(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    position_id: u64,
) -> AppResult<(MarginPositionResponse, bool)> {
    let mut tx = pool.begin().await?;
    let Some(position) = lock_user_position_by_id(&mut tx, user_id, position_id).await? else {
        return Err(AppError::NotFound);
    };
    if position.status != "opened" {
        let position = load_position_by_id(&mut tx, position.id).await?;
        tx.commit().await?;
        return Ok((position, false));
    }
    let Some(entry_price) = position.entry_price.as_ref() else {
        return Err(AppError::Validation(
            "margin entry price is required to close position".to_owned(),
        ));
    };
    let mark_price = cached_margin_mark_price(redis, position.pair_id, &position.symbol).await?;
    let realized_pnl = margin_realized_pnl(
        &position.direction,
        &position.notional_amount,
        entry_price,
        &mark_price,
    )?;
    let position_equity = (position.margin_amount.clone() + realized_pnl.clone()
        - position.interest_amount.clone())
    .with_scale(18);
    let payout_amount = margin_position_payout_amount(
        &position.margin_amount,
        Some(&realized_pnl),
        &position.interest_amount,
    );
    if position.margin_mode == "cross" {
        if position.wallet_scope != "margin" {
            return Err(AppError::Validation(
                "cross margin position must use margin wallet scope".to_owned(),
            ));
        }
        // 全仓亏损必须真实扣减共享钱包，不能按单仓把负权益截断为零。
        apply_cross_margin_position_settlement(
            &mut tx,
            user_id,
            position.margin_asset,
            &position_equity,
            position.id,
        )
        .await?;
    } else {
        // 逐仓平仓仍按单仓非负权益返还原资金账户。
        credit_margin_position_amount(
            &mut tx,
            user_id,
            position.margin_asset,
            &position.wallet_scope,
            &payout_amount,
            "margin_position_close",
            position.id,
        )
        .await?;
    }
    mark_position_closed(
        &mut tx,
        user_id,
        position.id,
        Utc::now(),
        &mark_price,
        &realized_pnl,
    )
    .await?;
    let position = load_position_by_id(&mut tx, position.id).await?;
    tx.commit().await?;
    Ok((position, true))
}

/// 调用单仓平仓事务，并仅在首次成功关闭后发布用户私有仓位事件。
/// 终态重放返回现有仓位且不重复入账或发事件；行情、结算或持久化失败不广播。
pub(crate) async fn close_margin_position_with_events(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position_id: u64,
) -> AppResult<CloseMarginPositionResponse> {
    let (position, is_new_close) = close_margin_position(pool, redis, user_id, position_id).await?;
    let response = CloseMarginPositionResponse { position };
    publish_margin_position_closed_event_if_needed(hub, user_id, &response.position, is_new_close);
    Ok(response)
}

/// 按主键顺序枚举用户可平仓仓位并逐笔复用单仓平仓与事件流程。
/// 每笔使用独立事务；单笔失败进入 failures 后继续，已成功结算不会被后续错误回滚。
pub(crate) async fn close_all_margin_positions_with_events(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    product_id: Option<u64>,
) -> AppResult<CloseAllMarginPositionsResponse> {
    let position_ids = load_open_position_ids(pool, user_id, product_id).await?;
    let mut positions = Vec::with_capacity(position_ids.len());
    let mut failures = Vec::new();
    for position_id in position_ids {
        // 每笔平仓独立提交后立刻发事件；后续失败不能吞掉前面已成功交易的通知。
        match close_margin_position_with_events(pool, redis, hub, user_id, position_id).await {
            Ok(response) => positions.push(response.position),
            Err(error) => failures.push(margin_batch_action_failure(position_id, error)),
        }
    }

    Ok(CloseAllMarginPositionsResponse {
        positions,
        failures,
    })
}

/// 在事务内锁定用户未成交仓位，把保证金原路退回记录的 wallet_scope 并标记 canceled。
/// 重复取消返回既有终态且不再次入账；退款、流水或状态更新任一步失败均整体回滚。
pub(crate) async fn cancel_margin_position(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<(MarginPositionResponse, bool)> {
    let mut tx = pool.begin().await?;
    let Some(position) = lock_user_position_by_id(&mut tx, user_id, position_id).await? else {
        return Err(AppError::NotFound);
    };
    if position.status == "canceled" {
        let position = load_position_by_id(&mut tx, position.id).await?;
        tx.commit().await?;
        return Ok((position, false));
    }
    validate_cancelable_position(&position)?;
    // 撤单只允许未成交仓位，保证金原路返还并与状态更新保持事务一致。
    credit_margin_position_amount(
        &mut tx,
        user_id,
        position.margin_asset,
        &position.wallet_scope,
        &position.margin_amount,
        "margin_position_cancel",
        position.id,
    )
    .await?;
    mark_position_canceled(&mut tx, user_id, position.id, Utc::now()).await?;
    let position = load_position_by_id(&mut tx, position.id).await?;
    tx.commit().await?;
    Ok((position, true))
}

/// 调用单仓取消事务，并仅在首次成功取消后发布用户私有仓位事件。
/// 终态重放不重复退款或发事件；资金与状态事务失败时不产生广播副作用。
pub(crate) async fn cancel_margin_position_with_events(
    pool: &Pool<MySql>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position_id: u64,
) -> AppResult<CancelMarginPositionResponse> {
    let (position, is_new_cancel) = cancel_margin_position(pool, user_id, position_id).await?;
    let response = CancelMarginPositionResponse { position };
    publish_margin_position_canceled_event_if_needed(
        hub,
        user_id,
        &response.position,
        is_new_cancel,
    );
    Ok(response)
}

/// 枚举用户可取消仓位并逐笔执行退款、状态提交及私有事件发布。
/// 每笔事务相互独立，失败项被汇总后继续处理，重放不会重复返还保证金。
pub(crate) async fn cancel_all_margin_positions_with_events(
    pool: &Pool<MySql>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    product_id: Option<u64>,
) -> AppResult<CancelAllMarginPositionsResponse> {
    let position_ids = load_cancelable_position_ids(pool, user_id, product_id).await?;
    let mut positions = Vec::with_capacity(position_ids.len());
    let mut failures = Vec::new();
    for position_id in position_ids {
        // 撤单与事件同样逐笔收口，保留前序成功结果及其私有事件。
        match cancel_margin_position_with_events(pool, hub, user_id, position_id).await {
            Ok(response) => positions.push(response.position),
            Err(error) => failures.push(margin_batch_action_failure(position_id, error)),
        }
    }

    Ok(CancelAllMarginPositionsResponse {
        positions,
        failures,
    })
}

fn margin_batch_action_failure(id: u64, error: AppError) -> MarginBatchActionFailure {
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
    MarginBatchActionFailure {
        id,
        code,
        message: error.to_string(),
    }
}

fn validate_cancelable_position(position: &LockedMarginPositionRow) -> AppResult<()> {
    if position.status != "opened" {
        return Err(AppError::Validation(
            "only opened margin positions can be canceled".to_owned(),
        ));
    }
    if position.entry_price.is_some() {
        return Err(AppError::Validation(
            "filled margin positions cannot be canceled; close the position instead".to_owned(),
        ));
    }
    Ok(())
}

fn margin_realized_pnl(
    direction: &str,
    notional_amount: &BigDecimal,
    entry_price: &BigDecimal,
    mark_price: &BigDecimal,
) -> AppResult<BigDecimal> {
    validate_positive_decimal(entry_price, "entry price")?;
    validate_positive_decimal(mark_price, "mark price")?;
    let price_delta = match direction {
        "long" => mark_price.clone() - entry_price.clone(),
        "short" => entry_price.clone() - mark_price.clone(),
        _ => {
            return Err(AppError::Validation(
                "margin direction must be long or short".to_owned(),
            ));
        }
    };
    Ok((notional_amount.clone() * price_delta / entry_price.clone()).with_scale(18))
}
