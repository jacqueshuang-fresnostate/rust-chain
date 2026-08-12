//! 现货钱包冻结、释放与成交资金腿 SQL。
//!
//! 本模块不拥有事务：订单创建、撤单或成交应用用例必须传入同一 MySQL 事务并负责提交/回滚。
//! 冻结/释放均保持 `available + frozen + locked` 不变并写 available/frozen 镜像流水；
//! 成交四条资金腿只允许从 frozen 借记、向 available 贷记，余额更新与每条流水必须原子提交。

use super::common::SYSTEM_SPOT_LIQUIDITY_EMAIL;
use crate::{
    error::{AppError, AppResult},
    modules::{
        auth::hash_password,
        spot::{
            OrderStatus, SpotOrder,
            service::{
                SpotOrderReservation as CreateSpotOrderReservation, spot_fill_wallet_lock_keys,
            },
        },
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Transaction};

#[derive(Debug, sqlx::FromRow)]
pub(super) struct SpotWalletRow {
    pub(super) available: BigDecimal,
    pub(super) frozen: BigDecimal,
    pub(super) locked: BigDecimal,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SpotLedgerMetadata<'a> {
    pub(crate) change_type: &'a str,
    pub(crate) ref_type: &'a str,
    pub(crate) ref_id: &'a str,
}

pub(super) async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<SpotWalletRow> {
    sqlx::query_as::<_, SpotWalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("wallet account is required for spot order".to_owned()))
}

/// 在调用方事务内把指定金额从 available 等额转入 frozen，并为两个余额桶各写一条镜像账本。
/// 调用方必须先确定资产精度、正数金额和业务幂等边界；函数以钱包行锁校验可用余额，任一写入失败都应回滚整笔订单事务。
/// `available + frozen + locked` 总额不得因冻结改变，账本快照必须与更新后的钱包三桶一致；本函数不自行提交或发布事件。
pub(crate) async fn apply_spot_wallet_freeze(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for spot order: requested {}, available {}, locked {}",
            amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    let frozen_after = wallet.frozen.clone() + amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await
}

pub(crate) async fn freeze_wallet_for_inserted_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    reservation: &CreateSpotOrderReservation,
) -> AppResult<()> {
    let user_id = order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    apply_spot_wallet_freeze(
        tx,
        user_id,
        reservation.asset_id,
        &reservation.amount,
        "spot_freeze",
        "spot_order",
        &order.id,
    )
    .await
}

pub(crate) async fn lock_spot_fill_wallet_rows_in_order(
    tx: &mut Transaction<'_, MySql>,
    buyer_id: u64,
    seller_id: u64,
    base_asset_id: u64,
    quote_asset_id: u64,
) -> AppResult<()> {
    // 成交结算会触达买卖双方的 base/quote 钱包，先按固定顺序锁行，避免交叉方向成交互相等待。
    for (user_id, asset_id) in
        spot_fill_wallet_lock_keys(buyer_id, seller_id, base_asset_id, quote_asset_id)
    {
        lock_wallet_row(tx, user_id, asset_id).await?;
    }
    Ok(())
}

/// 在现货成交事务内执行单条资金腿：贷记进入 available，借记只能从 frozen 扣除，并追加对应余额桶账本。
/// 调用方必须事先按稳定顺序锁齐买卖双方 base/quote 钱包，并保证金额已满足订单剩余保留额及资产精度。
/// frozen 不足会中止整笔成交；钱包更新与账本写入必须由调用方事务一起提交，函数本身不负责幂等占位或事件发布。
pub(crate) async fn apply_spot_wallet_settlement_leg(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    credit_available: bool,
    ledger: SpotLedgerMetadata<'_>,
) -> AppResult<()> {
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    let (amount_change, available_after, frozen_after, balance_type, balance_after) =
        if credit_available {
            let available_after = wallet.available.clone() + amount.clone();
            (
                amount.clone(),
                available_after.clone(),
                wallet.frozen.clone(),
                "available",
                available_after,
            )
        } else {
            if wallet.frozen < *amount {
                return Err(AppError::Validation(format!(
                    "insufficient frozen balance for spot fill: requested {}, frozen {}",
                    amount, wallet.frozen
                )));
            }
            let frozen_after = wallet.frozen.clone() - amount.clone();
            (
                -amount.clone(),
                wallet.available.clone(),
                frozen_after.clone(),
                "frozen",
                frozen_after,
            )
        };
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount_change,
        balance_type,
        &balance_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        ledger.change_type,
        ledger.ref_type,
        ledger.ref_id,
    )
    .await
}

pub(crate) async fn ensure_spot_liquidity_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<u64> {
    // 内部做市账户不允许使用固定可猜密码；随机哈希只在首次插入时持久化。
    let disabled_password_hash = hash_password(&uuid::Uuid::now_v7().to_string())?;
    let result = sqlx::query(
        r#"INSERT INTO users (email, password_hash, status)
           VALUES (?, ?, 'active')
           ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(id)"#,
    )
    .bind(SYSTEM_SPOT_LIQUIDITY_EMAIL)
    .bind(disabled_password_hash)
    .execute(&mut **tx)
    .await?;
    let user_id = result.last_insert_id();
    if user_id > 0 {
        return Ok(user_id);
    }
    let (user_id,): (u64,) = sqlx::query_as("SELECT id FROM users WHERE email = ? LIMIT 1")
        .bind(SYSTEM_SPOT_LIQUIDITY_EMAIL)
        .fetch_one(&mut **tx)
        .await?;
    Ok(user_id)
}

pub(crate) async fn ensure_wallet_account_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 系统做市成交只能消费后台预充值库存，禁止在成交路径自动增加资产。
pub(crate) async fn ensure_spot_liquidity_inventory_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    required_amount: &BigDecimal,
) -> AppResult<()> {
    ensure_wallet_account_in_tx(tx, user_id, asset_id).await?;
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *required_amount {
        return Err(AppError::Conflict(format!(
            "insufficient system spot liquidity inventory: required {}, available {}",
            required_amount, wallet.available
        )));
    }
    Ok(())
}

pub(crate) async fn release_buy_order_surplus_reservation_after_fill(
    tx: &mut Transaction<'_, MySql>,
    buyer_id: u64,
    buy_order: &SpotOrder,
    reservation_before_fill: &CreateSpotOrderReservation,
    fill_quote_amount: &BigDecimal,
    ref_id: &str,
) -> AppResult<()> {
    let surplus_amount = reservation_before_fill.amount.clone() - fill_quote_amount.clone();
    if surplus_amount <= 0 || buy_order.status == OrderStatus::PartiallyFilled {
        return Ok(());
    }
    // 非继续挂单的买单成交后释放剩余订单级预留，避免市价单全成后价差长期冻结。
    apply_spot_wallet_unfreeze(
        tx,
        buyer_id,
        reservation_before_fill.asset_id,
        &surplus_amount,
        "spot_price_improvement_release",
        "spot_trade",
        ref_id,
    )
    .await
}

/// 在调用方事务内把 frozen 等额退回 available，并为两个余额桶写镜像流水。
/// 调用方必须已锁定业务订单并确认释放金额只包含尚未成交的保留额；重复调用会因冻结不足失败。
/// 本函数不提交事务，余额、两条流水和订单状态必须由上层一起提交或回滚。
pub(super) async fn apply_spot_wallet_unfreeze(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    if wallet.frozen < *amount {
        return Err(AppError::Validation(format!(
            "insufficient frozen balance for spot cancel: requested {}, frozen {}",
            amount, wallet.frozen
        )));
    }
    let available_after = wallet.available.clone() + amount.clone();
    let frozen_after = wallet.frozen.clone() - amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn insert_spot_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
