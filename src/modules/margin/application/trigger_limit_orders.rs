//! 权威 ticker 驱动的杠杆限价挂单成交用例。
//!
//! 本模块只接收已经通过 Redis CAS 时序门禁的服务端 ticker，绝不接收 HTTP 请求中的客户价格。
//! 候选主键查询只做无锁初筛，每笔挂单都在独立事务中先锁仓位，再复核方向、限价、入场价和状态。
//! 只有从 `entry_price = NULL` 成功迁移到真实入场价的事务，才会重置计息起点、补建全仓账户、
//! 登记一次代理返佣并在提交后发布私有成交事件。撤单、重复 ticker 或多实例竞争只会有一方改动行。

use crate::{
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure::insert_agent_business_commission_in_tx,
            repository::AgentBusinessCommissionWrite, service::AGENT_COMMISSION_PRODUCT_MARGIN,
        },
        events::EventBroadcastHub,
        margin::{
            domain::margin_limit_order_is_triggered,
            infrastructure::{
                ensure_cross_margin_account, load_position_by_id,
                lock_pending_margin_limit_position_by_id, mark_margin_limit_position_filled,
                triggered_margin_limit_position_ids,
            },
            presentation::MarginPositionResponse,
            service::publish_margin_position_opened_event_if_needed,
        },
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool};

/// 以一笔服务端权威市场价扫描并尝试成交该交易对的杠杆限价挂单。
/// 市场价必须严格为正；候选查询失败会上抛，但单笔挂单失败只记录告警并继续，防止坏数据堵住同批其他用户。
/// 候选一次最多取 500 笔，每笔都重新开事务并锁行；本轮未取到的挂单会在后续 accepted ticker 上继续尝试。
/// 返回值只统计本次新成交且已提交的仓位，已撤、已成交、条件不再匹配的行都按幂等跳过。
pub async fn execute_triggered_margin_limit_orders_with_hub(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    hub: Option<&EventBroadcastHub>,
) -> AppResult<u32> {
    if market_price <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "margin market price must be positive".to_owned(),
        ));
    }

    let position_ids =
        triggered_margin_limit_position_ids(pool, pair_symbol, market_price, 500).await?;
    let mut filled_count = 0_u32;
    for position_id in position_ids {
        match execute_one_triggered_margin_limit_order(pool, position_id, market_price).await {
            Ok(Some(position)) => {
                filled_count += 1;
                publish_margin_position_opened_event_if_needed(
                    hub,
                    position.user_id,
                    &position,
                    true,
                );
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    position_id,
                    pair_symbol,
                    market_price = %market_price,
                    error = %error,
                    "杠杆限价挂单行情触发成交失败，跳过该仓位"
                );
            }
        }
    }
    Ok(filled_count)
}

/// 在独立事务中尝试把一笔 pending limit 迁移为已成交仓位。
/// 锁到行之后仍必须检查 `opened + limit + entry_price IS NULL`，因为无锁候选查询与真正取锁之间可能发生撤单或另一实例成交。
/// 域规则在锁内用同一 ticker 再判一次做多/做空边界；不满足时提交只读事务并返回 None，不留任何资金副作用。
/// 状态更新、全仓账户与佣金写入共享事务；任一环节失败均回滚，私有事件由上层在 commit 返回后才发布。
async fn execute_one_triggered_margin_limit_order(
    pool: &Pool<MySql>,
    position_id: u64,
    market_price: &BigDecimal,
) -> AppResult<Option<MarginPositionResponse>> {
    let mut tx = pool.begin().await?;
    let Some(position) = lock_pending_margin_limit_position_by_id(&mut tx, position_id).await?
    else {
        tx.commit().await?;
        return Ok(None);
    };
    if position.status != "opened"
        || position.order_type != "limit"
        || position.entry_price.is_some()
    {
        tx.commit().await?;
        return Ok(None);
    }
    let Some(limit_price) = position.limit_price.as_ref() else {
        return Err(AppError::Internal(
            "pending margin limit position is missing limit_price".to_owned(),
        ));
    };
    let is_triggered =
        margin_limit_order_is_triggered(&position.direction, limit_price, market_price)
            .map_err(|message| AppError::Internal(message.to_owned()))?;
    if !is_triggered {
        tx.commit().await?;
        return Ok(None);
    }
    if !mark_margin_limit_position_filled(&mut tx, position.id, market_price).await? {
        tx.rollback().await?;
        return Ok(None);
    }
    if position.margin_mode == "cross" {
        ensure_cross_margin_account(&mut tx, position.user_id, position.margin_asset).await?;
    }
    let commission_source_id = position.id.to_string();
    insert_agent_business_commission_in_tx(
        &mut tx,
        AgentBusinessCommissionWrite {
            user_id: position.user_id,
            product_type: AGENT_COMMISSION_PRODUCT_MARGIN,
            source_type: "margin_position",
            source_id: &commission_source_id,
            source_amount: &position.margin_amount,
            payout_asset_id: position.margin_asset,
        },
    )
    .await?;
    let filled_position = load_position_by_id(&mut tx, position.id).await?;
    tx.commit().await?;
    Ok(Some(filled_position))
}
