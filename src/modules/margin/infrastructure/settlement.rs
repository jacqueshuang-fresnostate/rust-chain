//! 杠杆仓位的资金结算与终态迁移适配器。
//!
//! 覆盖开仓抵押扣减、逐仓返还入账、全仓单仓与账户级权益结算，以及平仓和撤销的状态原子迁移。
//! 所有函数都在调用方给定的事务上执行，自己不 begin 也不 commit，
//! 因此「余额更新、流水写入、仓位终态」三件事能否原子落地完全由应用层的事务边界保证。
//! 资金只在 available 桶内流动，frozen 与 locked 原样带入流水快照，杠杆不使用冻结语义。
//! 逐仓按非负金额单向返还，亏损截零；全仓用有符号权益直接加减共享余额，扣穿则拒绝或记为坏账。
//! 每笔余额变更都配一条同额流水，流水中的 after 快照必须与本次更新后的余额完全一致。

use super::ledger::{
    insert_margin_wallet_ledger, insert_spot_wallet_ledger, lock_existing_margin_wallet_row,
    lock_margin_wallet_row, lock_spot_wallet_row,
};
use crate::{
    error::{AppError, AppResult},
    modules::margin::presentation::MarginPositionResponse,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Transaction};
/// 在开仓事务内按模式选择保证金或现货钱包，锁行后扣减抵押并写对应流水。
/// 全仓必须使用保证金钱包；逐仓优先既有保证金余额再回退现货，余额不足或任一步失败整体回滚。
///
/// 返回值是实际扣款的资金域字符串 `margin` 或 `spot`，调用方必须把它写回仓位的 `wallet_scope`，
/// 后续平仓、撤销和强平据此原路返还，否则会把钱退错账户。
/// 全仓走 `lock_margin_wallet_row`，账户不存在时会先补一行零余额再加锁，余额不足直接报校验错误。
/// 逐仓先用 `lock_existing_margin_wallet_row` 尝试杠杆钱包，只有该账户存在且余额足够才走杠杆；
/// 否则回退到现货钱包，此时现货账户必须已存在，余额不足同样报错并附上可用和锁定数额。
/// 三条分支都只动 available 一桶，frozen 与 locked 原样写进流水快照，杠杆开仓不使用冻结。
/// 流水的 `balance_after` 与 `available_after` 都填扣减后的可用余额，引用类型固定为 `margin_position`。
/// 本函数只扣款不建仓，调用方必须已先插入仓位占用幂等键，避免同键并发重复扣抵押。
pub(crate) async fn debit_margin_position_open_collateral(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    position_id: u64,
    margin_mode: &str,
) -> AppResult<String> {
    if margin_mode == "cross" {
        let margin_wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
        if margin_wallet.available < *amount {
            return Err(AppError::Validation(format!(
                "insufficient margin wallet balance for cross position: requested {}, available {}",
                amount, margin_wallet.available
            )));
        }
        let available_after = margin_wallet.available.clone() - amount.clone();
        sqlx::query(
            "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
        insert_margin_wallet_ledger(
            tx,
            user_id,
            asset_id,
            "margin_position_open",
            &(-amount.clone()),
            &available_after,
            &available_after,
            &margin_wallet.frozen,
            &margin_wallet.locked,
            "margin_position",
            &position_id.to_string(),
        )
        .await?;
        return Ok("margin".to_owned());
    }

    if let Some(margin_wallet) = lock_existing_margin_wallet_row(tx, user_id, asset_id).await?
        && margin_wallet.available >= *amount
    {
        let available_after = margin_wallet.available.clone() - amount.clone();
        sqlx::query(
            "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
        insert_margin_wallet_ledger(
            tx,
            user_id,
            asset_id,
            "margin_position_open",
            &(-amount.clone()),
            &available_after,
            &available_after,
            &margin_wallet.frozen,
            &margin_wallet.locked,
            "margin_position",
            &position_id.to_string(),
        )
        .await?;
        return Ok("margin".to_owned());
    }

    let wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for margin position: requested {}, available {}, locked {}",
            amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_position_open",
        &(-amount.clone()),
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        "margin_position",
        &position_id.to_string(),
    )
    .await?;
    Ok("spot".to_owned())
}

/// 在调用方事务内按主键回读仓位完整快照，用于把结算后的落库结果原样返回给客户端。
/// 不带用户条件也不加 FOR UPDATE，因为调用方此前已用 `lock_user_position_by_id` 锁住该行并校验过归属。
/// 记录缺失返回 NotFound；处在结算事务中出现这种情况说明同事务写入未生效，属于异常。
/// 只读一行，不改变已有锁序，也不产生任何资金状态变化。
pub(crate) async fn load_position_by_id(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
) -> AppResult<MarginPositionResponse> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, order_type, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price, limit_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在仓位结算事务内按已记录的 `wallet_scope` 返还正金额；零或负金额不产生余额与流水变更。
/// `margin` 锁杠杆钱包，`spot` 锁现货钱包，未知资金域直接报错，禁止静默回退到任一账户。
/// 可用余额增量、余额快照与指定结算流水必须保持同额并由调用方连同仓位终态一起提交。
/// 本函数不独立去重或提交；调用方必须先锁仓位并确认首次结算，提交后没有外部副作用。
///
/// 金额为零或负时直接返回 Ok 且不做任何写入，因此逐仓亏损截零的平仓不会留下空流水。
/// `change_type` 由调用方给定，平仓传 `margin_position_close`、撤销传 `margin_position_cancel`，
/// 强平路径另有自己的类型，用于在流水里区分同一仓位不同阶段的入账原因。
/// 两个资金域分支的差别只在锁哪张钱包表和写哪张流水表，入账口径完全一致：只加 available，
/// frozen 与 locked 原样带入快照，`balance_after` 与 `available_after` 都填加款后的可用余额。
pub(crate) async fn credit_margin_position_amount(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    wallet_scope: &str,
    amount: &BigDecimal,
    change_type: &str,
    position_id: u64,
) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Ok(());
    }
    match wallet_scope {
        "margin" => {
            let wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
            let available_after = wallet.available.clone() + amount.clone();
            sqlx::query(
                "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
            )
            .bind(&available_after)
            .bind(user_id)
            .bind(asset_id)
            .execute(&mut **tx)
            .await?;
            insert_margin_wallet_ledger(
                tx,
                user_id,
                asset_id,
                change_type,
                amount,
                &available_after,
                &available_after,
                &wallet.frozen,
                &wallet.locked,
                "margin_position",
                &position_id.to_string(),
            )
            .await
        }
        "spot" => {
            let wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
            let available_after = wallet.available.clone() + amount.clone();
            sqlx::query(
                "UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
            )
            .bind(&available_after)
            .bind(user_id)
            .bind(asset_id)
            .execute(&mut **tx)
            .await?;
            insert_spot_wallet_ledger(
                tx,
                user_id,
                asset_id,
                change_type,
                amount,
                &available_after,
                &available_after,
                &wallet.frozen,
                &wallet.locked,
                "margin_position",
                &position_id.to_string(),
            )
            .await
        }
        _ => Err(AppError::Validation(
            "margin position wallet_scope must be spot or margin".to_owned(),
        )),
    }
}

/// 全仓单仓主动平仓使用有符号权益更新共享钱包，余额不足时必须交给账户级强平处理。
/// 调用方须先锁定账户与仓位；任一余额或流水写入失败必须回滚，禁止留下半结算状态。
///
/// `amount` 是该仓位的有符号权益，即保证金加已实现盈亏再减利息，可正可负，不做非负截断。
/// 与逐仓返还的本质区别在于亏损会真实扣减共享余额，而不是把负权益当成零。
/// 扣减后可用余额为负时返回校验错误并提示需要账户级强平，绝不把共享钱包写成负数；
/// 这条分支是主动平仓与强平的分界：单仓平不动的穿仓只能由强平 worker 统一处置。
/// 权益恰为零时提前返回，不产生余额更新也不写流水，避免留下无意义的零额记录。
/// 流水类型固定为 `margin_cross_position_close`，引用到具体仓位主键以便逐笔追溯。
pub(crate) async fn apply_cross_margin_position_settlement(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    position_id: u64,
) -> AppResult<()> {
    let wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
    let available_after = (wallet.available.clone() + amount.clone()).with_scale(18);
    if available_after < 0 {
        return Err(AppError::Validation(
            "cross margin position loss exceeds shared available equity; account liquidation is required"
                .to_owned(),
        ));
    }
    if amount == &BigDecimal::from(0) {
        return Ok(());
    }
    sqlx::query(
        "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_margin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_cross_position_close",
        amount,
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        "margin_position",
        &position_id.to_string(),
    )
    .await
}

#[derive(Debug)]
/// 全仓账户级结算结果，同时返回归零后的可用余额与极端跳空坏账。
pub(crate) struct CrossMarginAccountSettlement {
    /// 结算后的杠杆钱包可用余额，恒为非负；穿仓时被钳到零而不是写成负数。
    pub(crate) available_after: BigDecimal,
    /// 穿仓缺口，等于被钳掉的那部分负余额，正常情况下为零，非零表示平台承担了坏账。
    pub(crate) bad_debt: BigDecimal,
}

/// 全仓强平只在共享钱包上结算一次；极端跳空超过可用余额的部分单独记为坏账。
/// 账户、仓位、钱包和流水必须同事务提交；失败时不关闭部分仓位，也不产生外部结算事件。
///
/// `portfolio_equity` 是该账户下全部被强平仓位的组合权益之和，有符号，通常为负。
/// 与单仓结算的关键差别是这里不拒绝穿仓：可用余额加权益为负时钳到零，负的部分作为 `bad_debt` 返回，
/// 由调用方登记为平台坏账，因此强平永远能推进，不会因为余额不足而卡住。
/// 实际写库用的是钳位后余额与原余额的差值，保证流水金额与余额变化严格对应而非等于原始权益。
/// 差值为零时跳过更新和流水，避免账户本就为零、权益也为零时留下空记录。
/// 流水类型固定为 `margin_cross_account_liquidate`，引用类型为账户级而非单个仓位。
pub(crate) async fn apply_cross_margin_account_settlement(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    portfolio_equity: &BigDecimal,
    reference_id: &str,
) -> AppResult<CrossMarginAccountSettlement> {
    let wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
    let raw_available_after = (wallet.available.clone() + portfolio_equity.clone()).with_scale(18);
    let (available_after, bad_debt) = if raw_available_after < 0 {
        (
            BigDecimal::from(0).with_scale(18),
            (-raw_available_after).with_scale(18),
        )
    } else {
        (raw_available_after, BigDecimal::from(0).with_scale(18))
    };
    let applied_delta = (available_after.clone() - wallet.available.clone()).with_scale(18);
    if applied_delta != 0 {
        sqlx::query(
            "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
        insert_margin_wallet_ledger(
            tx,
            user_id,
            asset_id,
            "margin_cross_account_liquidate",
            &applied_delta,
            &available_after,
            &available_after,
            &wallet.frozen,
            &wallet.locked,
            "margin_cross_account",
            reference_id,
        )
        .await?;
    }
    Ok(CrossMarginAccountSettlement {
        available_after,
        bad_debt,
    })
}

/// 在结算事务内以 opened 条件原子写入关闭时间、退出价和已实现盈亏。
/// 受影响行不是一行即返回并发冲突，调用方必须回滚钱包结算与流水。
///
/// WHERE 同时约束仓位主键、用户标识和 status = 'opened'，把状态检查与状态迁移合成一条语句，
/// 因此即使并发的强平 worker 抢先关闭了同一仓位，这里也只会影响零行而不会重复结算。
/// 顺带把 `next_liquidation_attempt_at` 清空，让强平调度不再挑选这条已终结的仓位。
/// 关闭时间以 UTC 朴素时间写入，与库中其余时间列的存储口径一致。
pub(crate) async fn mark_position_closed(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    position_id: u64,
    closed_at: DateTime<Utc>,
    exit_price: &BigDecimal,
    realized_pnl: &BigDecimal,
) -> AppResult<()> {
    let update_position = sqlx::query(
        r#"UPDATE margin_positions
           SET status = 'closed', closed_at = ?, exit_price = ?, realized_pnl = ?,
               next_liquidation_attempt_at = NULL
           WHERE id = ? AND user_id = ? AND status = 'opened'"#,
    )
    .bind(closed_at.naive_utc())
    .bind(exit_price)
    .bind(realized_pnl)
    .bind(position_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    if update_position.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "margin position close status changed concurrently".to_owned(),
        ));
    }
    Ok(())
}

/// 在取消事务内仅把未成交 opened 仓位原子迁移为 canceled 并记录关闭时间。
/// 状态条件不再满足时返回冲突，调用方必须回滚保证金退款及流水。
///
/// 比平仓多一个 `entry_price IS NULL` 条件，这是撤销与平仓的分水岭：一旦仓位已按行情成交，
/// 该条件不成立、影响零行并返回冲突，从数据库层面兜住「已成交仓位被误撤并原额退款」的风险。
/// 不写退出价和已实现盈亏，因为未成交仓位不存在成交结果，只记录关闭时间。
/// 同样清空 `next_liquidation_attempt_at`，把已终结的仓位移出强平调度范围。
pub(crate) async fn mark_position_canceled(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    position_id: u64,
    closed_at: DateTime<Utc>,
) -> AppResult<()> {
    let update_position = sqlx::query(
        r#"UPDATE margin_positions
           SET status = 'canceled', closed_at = ?, next_liquidation_attempt_at = NULL
           WHERE id = ? AND user_id = ? AND status = 'opened' AND entry_price IS NULL"#,
    )
    .bind(closed_at.naive_utc())
    .bind(position_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    if update_position.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "margin position cancel status changed concurrently".to_owned(),
        ));
    }
    Ok(())
}
