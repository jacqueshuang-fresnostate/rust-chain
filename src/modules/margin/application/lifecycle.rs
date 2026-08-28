//! 杠杆仓位的平仓与撤销生命周期用例。
//!
//! 平仓针对已成交仓位，按服务端标记价结算用户选择的剩余仓位份额并把对应权益写回钱包；
//! 1..99% 保留精确剩余敞口，100% 才迁移到终态。撤销只针对入场价为空的未成交仓位，
//! 把保证金原额退回。已成交全仓平仓按账户→仓位→钱包取锁；未成交撤单不进入全仓风险集合，仍按仓位→钱包处理。
//! 逐仓与全仓在平仓时资金口径不同：逐仓按非负返还额入账，亏损截零；
//! 全仓以有符号组合权益更新共享钱包，亏损真实扣减，扣穿则拒绝并交由账户级强平处理。
//! 所有用例都返回「是否产生首次结算执行」的布尔值，幂等或终态重放返回既有快照且不重复入账。
//! 批量版本逐笔独立开事务并即时发事件，单笔失败只进入 failures 列表，不回滚已成功的结算。

use super::support::is_duplicate_key_error;
use crate::{
    error::{AppError, AppResult},
    modules::{
        events::EventBroadcastHub,
        margin::{
            domain::{
                accumulate_margin_realized_pnl, allocate_margin_close_slice, margin_mark_pnl,
                margin_position_payout_amount,
            },
            infrastructure::{
                LockedMarginPositionRow, MarginCloseExecutionWrite,
                MarginPositionPartialCloseWrite, apply_cross_margin_position_settlement,
                bump_cross_margin_account_version, cached_margin_mark_price,
                credit_margin_position_amount, ensure_and_lock_cross_margin_account,
                insert_margin_close_execution, load_cancelable_position_ids,
                load_margin_close_execution_by_id, load_margin_close_execution_by_key_readonly,
                load_open_position_ids, load_position_by_id, load_user_position_by_id,
                lock_margin_close_execution_by_key, lock_user_position_by_id,
                mark_position_canceled, mark_position_closed, mark_position_partially_closed,
                require_active_cross_margin_account,
            },
            presentation::{
                CancelAllMarginPositionsResponse, CancelMarginPositionResponse,
                CloseAllMarginPositionsResponse, CloseMarginPositionRequest,
                CloseMarginPositionResponse, MarginBatchActionFailure,
                MarginPositionCloseExecutionResponse, MarginPositionResponse,
            },
            service::{
                publish_margin_position_canceled_event_if_needed,
                publish_margin_position_close_event_if_needed,
            },
        },
    },
};
use chrono::Utc;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};

/// 应用层归一化后的单仓平仓意图；无幂等键只可能是历史 100% 兼容请求。
struct NormalizedCloseRequest {
    percentage: u16,
    idempotency_key: Option<String>,
}

/// 主动平掉用户仓位；事务先锁定仓位，已非 opened 时重放当前终态且不再次结算。
/// 显式请求按加锁后的剩余仓位切出 1..=100% 并先占用用户级幂等键；部分执行缩减四类敞口，
/// 100% 才进入 closed。全仓以有符号切片权益更新共享钱包，逐仓按资金域返还非负切片权益。
/// 执行记录、余额、流水、仓位剩余值和全仓版本在同一事务提交；唯一键并发败方先回滚再重放。
pub(crate) async fn close_margin_position(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    position_id: u64,
    request: CloseMarginPositionRequest,
) -> AppResult<(CloseMarginPositionResponse, bool)> {
    let request = normalize_close_request(request)?;
    if let Some(idempotency_key) = request.idempotency_key.as_deref()
        && let Some(execution) =
            load_margin_close_execution_by_key_readonly(pool, user_id, idempotency_key).await?
    {
        return Ok((
            replay_close_execution(pool, user_id, position_id, request.percentage, execution)
                .await?,
            false,
        ));
    }

    let scope = load_user_position_by_id(pool, user_id, position_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut tx = pool.begin().await?;
    let cross_account = if scope.margin_mode == "cross" {
        Some(ensure_and_lock_cross_margin_account(&mut tx, user_id, scope.margin_asset).await?)
    } else {
        None
    };
    let Some(position) = lock_user_position_by_id(&mut tx, user_id, position_id).await? else {
        return Err(AppError::NotFound);
    };
    if let Some(idempotency_key) = request.idempotency_key.as_deref()
        && let Some(execution) =
            lock_margin_close_execution_by_key(&mut tx, user_id, idempotency_key).await?
    {
        ensure_close_execution_matches(&execution, position_id, request.percentage)?;
        let position = load_position_by_id(&mut tx, position.id).await?;
        tx.commit().await?;
        return Ok((
            CloseMarginPositionResponse {
                position,
                settlement_amount: Some(execution.settlement_amount.clone()),
                execution: Some(execution),
                replayed: true,
            },
            false,
        ));
    }
    if position.status != "opened" {
        if request.idempotency_key.is_some() {
            return Err(AppError::Validation(
                "only opened margin positions can be explicitly closed".to_owned(),
            ));
        }
        let position = load_position_by_id(&mut tx, position.id).await?;
        tx.commit().await?;
        return Ok((
            CloseMarginPositionResponse {
                position,
                execution: None,
                settlement_amount: None,
                replayed: true,
            },
            false,
        ));
    }
    let Some(entry_price) = position.entry_price.as_ref() else {
        return Err(AppError::Validation(
            "margin entry price is required to close position".to_owned(),
        ));
    };
    if position.margin_mode == "cross" {
        let account = cross_account.as_ref().ok_or_else(|| {
            AppError::Conflict("margin position account scope changed concurrently".to_owned())
        })?;
        require_active_cross_margin_account(account)?;
    }
    let mark_price = cached_margin_mark_price(redis, position.pair_id, &position.symbol).await?;
    let close_slice = allocate_margin_close_slice(
        &position.margin_amount,
        &position.notional_amount,
        &position.borrowed_amount,
        &position.interest_amount,
        request.percentage,
    )
    .map_err(|message| AppError::Validation(message.to_owned()))?;
    let realized_pnl = margin_mark_pnl(
        &position.direction,
        &close_slice.close_notional_amount,
        entry_price,
        &mark_price,
    )
    .map_err(|message| AppError::Validation(message.to_owned()))?;
    let cumulative_realized_pnl =
        accumulate_margin_realized_pnl(position.realized_pnl.as_ref(), &realized_pnl);
    let position_equity = (close_slice.close_margin_amount.clone() + realized_pnl.clone()
        - close_slice.close_interest_amount.clone())
    .with_scale(18);
    let payout_amount = margin_position_payout_amount(
        &close_slice.close_margin_amount,
        Some(&realized_pnl),
        &close_slice.close_interest_amount,
    );
    let settlement_amount = if position.margin_mode == "cross" {
        position_equity.clone()
    } else {
        payout_amount.clone()
    };
    let execution_id = if let Some(idempotency_key) = request.idempotency_key.as_deref() {
        match insert_margin_close_execution(
            &mut tx,
            MarginCloseExecutionWrite {
                user_id,
                position_id: position.id,
                idempotency_key,
                close_percentage: close_slice.close_percentage,
                close_margin_amount: &close_slice.close_margin_amount,
                close_notional_amount: &close_slice.close_notional_amount,
                close_borrowed_amount: &close_slice.close_borrowed_amount,
                close_interest_amount: &close_slice.close_interest_amount,
                exit_price: &mark_price,
                realized_pnl: &realized_pnl,
                settlement_amount: &settlement_amount,
                fully_closed: close_slice.fully_closed,
            },
        )
        .await
        {
            Ok(execution_id) => Some(execution_id),
            Err(error) if is_duplicate_key_error(&error) => {
                tx.rollback().await?;
                return Ok((
                    replay_close_execution_after_unique_conflict(
                        pool,
                        user_id,
                        position_id,
                        request.percentage,
                        idempotency_key,
                    )
                    .await?,
                    false,
                ));
            }
            Err(error) => return Err(error.into()),
        }
    } else {
        None
    };
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
    if close_slice.fully_closed {
        mark_position_closed(
            &mut tx,
            user_id,
            position.id,
            Utc::now(),
            &mark_price,
            &cumulative_realized_pnl,
        )
        .await?;
    } else {
        mark_position_partially_closed(
            &mut tx,
            user_id,
            position.id,
            MarginPositionPartialCloseWrite {
                remaining_margin_amount: &close_slice.remaining_margin_amount,
                remaining_notional_amount: &close_slice.remaining_notional_amount,
                remaining_borrowed_amount: &close_slice.remaining_borrowed_amount,
                remaining_interest_amount: &close_slice.remaining_interest_amount,
                cumulative_realized_pnl: &cumulative_realized_pnl,
            },
        )
        .await?;
    }
    if let Some(account) = cross_account {
        bump_cross_margin_account_version(&mut tx, user_id, position.margin_asset, account.version)
            .await?;
    }
    let position = load_position_by_id(&mut tx, position.id).await?;
    let execution = match execution_id {
        Some(execution_id) => Some(load_margin_close_execution_by_id(&mut tx, execution_id).await?),
        None => None,
    };
    tx.commit().await?;
    Ok((
        CloseMarginPositionResponse {
            position,
            execution,
            settlement_amount: Some(settlement_amount),
            replayed: false,
        },
        true,
    ))
}

/// 调用单仓平仓事务，并仅在首次成功关闭后发布用户私有仓位事件。
/// 终态重放返回现有仓位且不重复入账或发事件；行情、结算或持久化失败不广播。
pub(crate) async fn close_margin_position_with_events(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position_id: u64,
    request: CloseMarginPositionRequest,
) -> AppResult<CloseMarginPositionResponse> {
    let (response, is_new_close) =
        close_margin_position(pool, redis, user_id, position_id, request).await?;
    publish_margin_position_close_event_if_needed(hub, user_id, &response, is_new_close);
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
        match close_margin_position_with_events(
            pool,
            redis,
            hub,
            user_id,
            position_id,
            CloseMarginPositionRequest::default(),
        )
        .await
        {
            Ok(response) => positions.push(response.position),
            Err(error) => failures.push(margin_batch_action_failure(position_id, error)),
        }
    }

    Ok(CloseAllMarginPositionsResponse {
        positions,
        failures,
    })
}

/// 把传输层可选字段归一成历史全平或显式幂等意图；所有非法值在事务与行情读取前失败。
fn normalize_close_request(
    request: CloseMarginPositionRequest,
) -> AppResult<NormalizedCloseRequest> {
    let is_explicit = request.percentage.is_some() || request.idempotency_key.is_some();
    if !is_explicit {
        return Ok(NormalizedCloseRequest {
            percentage: 100,
            idempotency_key: None,
        });
    }
    let requested_percentage = request.percentage.unwrap_or(100);
    if !(1..=100).contains(&requested_percentage) {
        return Err(AppError::Validation(
            "margin close percentage must be between 1 and 100".to_owned(),
        ));
    }
    let percentage = u16::try_from(requested_percentage).map_err(|_| {
        AppError::Validation("margin close percentage must be between 1 and 100".to_owned())
    })?;
    let idempotency_key = request
        .idempotency_key
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("margin close idempotency_key is required".to_owned())
        })?;
    if idempotency_key.len() > 128 {
        return Err(AppError::Validation(
            "margin close idempotency_key is too long".to_owned(),
        ));
    }
    Ok(NormalizedCloseRequest {
        percentage,
        idempotency_key: Some(idempotency_key),
    })
}

/// 核对既有执行是否属于同一仓位和同一比例；同键异参必须在返回任何成功结果前冲突。
fn ensure_close_execution_matches(
    execution: &MarginPositionCloseExecutionResponse,
    position_id: u64,
    percentage: u16,
) -> AppResult<()> {
    if execution.position_id != position_id || execution.close_percentage != percentage {
        return Err(AppError::Conflict(
            "margin close idempotency key belongs to a different request".to_owned(),
        ));
    }
    Ok(())
}

/// 用只读执行记录与当前权威仓位组装幂等重放响应，不读取行情也不触碰任何钱包。
async fn replay_close_execution(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
    percentage: u16,
    execution: MarginPositionCloseExecutionResponse,
) -> AppResult<CloseMarginPositionResponse> {
    ensure_close_execution_matches(&execution, position_id, percentage)?;
    let position = load_user_position_by_id(pool, user_id, position_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(CloseMarginPositionResponse {
        position,
        settlement_amount: Some(execution.settlement_amount.clone()),
        execution: Some(execution),
        replayed: true,
    })
}

/// 唯一键并发败方回滚后等待并取回胜方执行；读不到表示对方仍在提交，返回可重试冲突。
async fn replay_close_execution_after_unique_conflict(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
    percentage: u16,
    idempotency_key: &str,
) -> AppResult<CloseMarginPositionResponse> {
    let execution = load_margin_close_execution_by_key_readonly(pool, user_id, idempotency_key)
        .await?
        .ok_or_else(|| {
            AppError::Conflict("margin close idempotency key is being committed".to_owned())
        })?;
    replay_close_execution(pool, user_id, position_id, percentage, execution).await
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

/// 把单笔批量操作的失败折叠成结构化条目，附上稳定错误码和人类可读消息供前端逐条展示。
/// 错误码从 `AppError` 变体映射成固定字符串常量，`AppError::Api` 直接沿用其自带的业务码。
/// 该映射确保批量接口即便部分失败也能返回 200，调用方据 failures 判断哪些仓位没被处理。
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

/// 判定一笔已加锁的仓位能否被撤销，必须同时满足状态为 opened 且入场价为空两个条件。
/// 入场价非空意味着仓位已按行情成交，只能走平仓结算，此时返回带指引的参数错误而非静默转平仓。
/// 该判定在锁定仓位之后执行，因此不会被并发成交或并发平仓插到中间造成误判。
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
