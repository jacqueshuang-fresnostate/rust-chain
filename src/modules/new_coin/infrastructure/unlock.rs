//! 新币解禁费结算与到期释放的 MySQL 事务适配。
//!
//! 本子模块只负责两条紧密关联的资金链：真实扣除解禁费并固化结算证据，
//! 以及在到期后复核证据、把 locked 原子迁移到 available。公共适配器类型仍由父 façade 导出。

use super::{MySqlNewCoinReadRepository, debit_wallet_available, lock_wallet_row};
use crate::{
    error::{AppError, AppResult},
    modules::new_coin::{
        repository::{
            NewCoinLedgerMetadata, NewCoinUnlockFeeRepository, NewCoinUnlockReleaseRepository,
            ReleaseUnlockOutcome, UnlockFeeExpectation, UnlockFeePaymentWrite,
        },
        service::{
            ensure_new_coin_amount_precision, ensure_positive_amount,
            ensure_unlock_fee_payment_matches,
        },
    },
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

#[derive(Debug, sqlx::FromRow)]
struct UnlockFeeExpectationRow {
    unlock_fee_enabled: bool,
    unlock_fee_asset: Option<u64>,
    unlock_fee_amount: Option<BigDecimal>,
}

#[derive(Debug, sqlx::FromRow)]
struct UnlockFeePaymentRow {
    id: u64,
    unlock_fee_enabled: bool,
    unlock_fee_asset: Option<u64>,
    unlock_fee_amount: Option<BigDecimal>,
    fee_paid_status: String,
    fee_paid_at: Option<chrono::DateTime<chrono::Utc>>,
    unlock_fee_payment_ledger_id: Option<u64>,
}

#[derive(Debug, sqlx::FromRow)]
struct ReleasableUnlockRow {
    unlock_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    unlock_quantity: BigDecimal,
    remaining_amount: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct UnlockReleaseLocatorRow {
    asset_id: u64,
    unlock_quantity: BigDecimal,
    status: String,
}

impl From<UnlockFeeExpectationRow> for UnlockFeeExpectation {
    /// 平移解禁应收费用的三列查询结果，只回答「是否收费、收什么资产、收多少」。
    /// 刻意不携带缴费状态，使调用方无法把「应收」误当成「已收」，是否已缴需要另行查询确认。
    fn from(row: UnlockFeeExpectationRow) -> Self {
        Self {
            unlock_fee_enabled: row.unlock_fee_enabled,
            unlock_fee_asset: row.unlock_fee_asset,
            unlock_fee_amount: row.unlock_fee_amount,
        }
    }
}

#[async_trait]
impl NewCoinUnlockFeeRepository for MySqlNewCoinReadRepository {
    /// 按解禁幂等键与 `user_id` 回读该条记录应收的手续费口径，即是否启用收费、支付资产和应付金额。
    /// 返回值刻意不含 `fee_paid_status`，只回答「应该收多少」，是否已收需另行查询，
    /// 两者分离可避免调用方把「应收」直接当成「已收」而错误放行。
    /// 记录不存在返回 `None`；查询不加行锁，结果返回后仍可能被并发缴费改写，
    /// 因此只适合做缴费前的金额比对，不能替代事务内的重复收费守卫。
    async fn find_unlock_fee_expectation(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<Option<UnlockFeeExpectation>> {
        let row = sqlx::query_as::<_, UnlockFeeExpectationRow>(
            r#"SELECT unlock_fee_enabled, unlock_fee_asset, unlock_fee_amount
               FROM asset_unlock_records
               WHERE idempotency_key = ? AND user_id = ?
               LIMIT 1"#,
        )
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// 锁定应收和钱包后校验不可变资产/金额，扣 available、写用户流水和平台双腿后才置 paid。
    /// 同参并发重放由解禁记录行锁串行化，首次返回 `true`，完整证据已存在时返回 `false`。
    async fn mark_unlock_fee_paid(&self, payment: UnlockFeePaymentWrite) -> AppResult<bool> {
        pay_unlock_fee_in_tx(&self.pool, payment).await
    }
}

/// 解禁费缴纳的唯一资金入口：锁应收快照后扣用户钱包，写用户流水和平台双腿分录，最后置 paid。
pub(super) async fn pay_unlock_fee_in_tx(
    pool: &Pool<MySql>,
    payment: UnlockFeePaymentWrite,
) -> AppResult<bool> {
    let preflight = sqlx::query_as::<_, UnlockFeePaymentRow>(
        r#"SELECT id, unlock_fee_enabled, unlock_fee_asset, unlock_fee_amount,
                  fee_paid_status, fee_paid_at, unlock_fee_payment_ledger_id
           FROM asset_unlock_records
           WHERE idempotency_key = ? AND user_id = ?
           LIMIT 1"#,
    )
    .bind(&payment.unlock_idempotency_key)
    .bind(payment.user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    ensure_unlock_fee_payment_matches(
        &UnlockFeeExpectation {
            unlock_fee_enabled: preflight.unlock_fee_enabled,
            unlock_fee_asset: preflight.unlock_fee_asset,
            unlock_fee_amount: preflight.unlock_fee_amount.clone(),
        },
        payment.payment_asset_id,
        &payment.amount,
    )?;
    if preflight.fee_paid_status == "paid" {
        // 已结算重放不再依赖资产当前是否 active；锁定应收后只核验原始三腿证据并原样返回。
        let mut tx = pool.begin().await?;
        let row = lock_unlock_fee_payment_row_in_tx(
            &mut tx,
            &payment.unlock_idempotency_key,
            payment.user_id,
        )
        .await?;
        ensure_unlock_fee_payment_matches(
            &UnlockFeeExpectation {
                unlock_fee_enabled: row.unlock_fee_enabled,
                unlock_fee_asset: row.unlock_fee_asset,
                unlock_fee_amount: row.unlock_fee_amount.clone(),
            },
            payment.payment_asset_id,
            &payment.amount,
        )?;
        if row.fee_paid_status != "paid"
            || row.fee_paid_at.is_none()
            || row.unlock_fee_payment_ledger_id.is_none()
        {
            return Err(AppError::Internal(
                "paid unlock fee is missing its wallet settlement evidence".to_owned(),
            ));
        }
        ensure_paid_unlock_fee_evidence_in_tx(
            &mut tx,
            &row,
            payment.user_id,
            payment.payment_asset_id,
            &payment.amount,
            &payment.unlock_idempotency_key,
        )
        .await?;
        tx.commit().await?;
        return Ok(false);
    }

    let mut tx = pool.begin().await?;
    // 资金路径统一按资产、钱包、业务记录取锁，避免与下单/释放形成钱包和解禁记录的反向等待。
    let precision = lock_asset_precision_in_tx(&mut tx, payment.payment_asset_id).await?;
    ensure_new_coin_amount_precision(&payment.amount, precision, "unlock fee amount")?;
    lock_wallet_row(&mut tx, payment.user_id, payment.payment_asset_id).await?;
    let row = lock_unlock_fee_payment_row_in_tx(
        &mut tx,
        &payment.unlock_idempotency_key,
        payment.user_id,
    )
    .await?;

    ensure_unlock_fee_payment_matches(
        &UnlockFeeExpectation {
            unlock_fee_enabled: row.unlock_fee_enabled,
            unlock_fee_asset: row.unlock_fee_asset,
            unlock_fee_amount: row.unlock_fee_amount.clone(),
        },
        payment.payment_asset_id,
        &payment.amount,
    )?;
    if row.fee_paid_status == "paid" {
        if row.fee_paid_at.is_none() || row.unlock_fee_payment_ledger_id.is_none() {
            return Err(AppError::Internal(
                "paid unlock fee is missing its wallet settlement evidence".to_owned(),
            ));
        }
        ensure_paid_unlock_fee_evidence_in_tx(
            &mut tx,
            &row,
            payment.user_id,
            payment.payment_asset_id,
            &payment.amount,
            &payment.unlock_idempotency_key,
        )
        .await?;
        tx.commit().await?;
        return Ok(false);
    }
    if row.fee_paid_status != "pending" {
        return Err(AppError::Internal(format!(
            "fee-bearing unlock has invalid unpaid status: {}",
            row.fee_paid_status
        )));
    }

    let wallet_ledger_id = debit_wallet_available(
        &mut tx,
        payment.user_id,
        payment.payment_asset_id,
        &payment.amount,
        NewCoinLedgerMetadata {
            change_type: "new_coin_unlock_fee_payment",
            ref_type: "new_coin_unlock",
            ref_id: &payment.unlock_idempotency_key,
        },
    )
    .await?;
    let transaction_key = format!("new_coin_unlock_fee:{}", row.id);
    let ref_id = row.id.to_string();
    sqlx::query(
        r#"INSERT INTO platform_financial_journal
           (transaction_key, context, account_code, asset_id, amount, ref_type, ref_id, metadata_json)
           VALUES (?, 'new_coin_unlock_fee', 'user_unlock_fee_expense', ?, ?, 'new_coin_unlock', ?, JSON_OBJECT('user_id', ?)),
                  (?, 'new_coin_unlock_fee', 'platform_unlock_fee_revenue', ?, ?, 'new_coin_unlock', ?, JSON_OBJECT('user_id', ?))"#,
    )
    .bind(&transaction_key)
    .bind(payment.payment_asset_id)
    .bind(-payment.amount.clone())
    .bind(&ref_id)
    .bind(payment.user_id)
    .bind(&transaction_key)
    .bind(payment.payment_asset_id)
    .bind(&payment.amount)
    .bind(&ref_id)
    .bind(payment.user_id)
    .execute(&mut *tx)
    .await?;

    let updated = sqlx::query(
        r#"UPDATE asset_unlock_records
           SET fee_paid_status = 'paid', fee_paid_at = CURRENT_TIMESTAMP(6),
               unlock_fee_payment_ledger_id = ?
           WHERE id = ? AND fee_paid_status <> 'paid'"#,
    )
    .bind(wallet_ledger_id)
    .bind(row.id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "unlock fee was paid concurrently".to_owned(),
        ));
    }
    tx.commit().await?;
    Ok(true)
}

async fn lock_unlock_fee_payment_row_in_tx(
    tx: &mut Transaction<'_, MySql>,
    unlock_idempotency_key: &str,
    user_id: u64,
) -> AppResult<UnlockFeePaymentRow> {
    sqlx::query_as::<_, UnlockFeePaymentRow>(
        r#"SELECT id, unlock_fee_enabled, unlock_fee_asset, unlock_fee_amount,
                  fee_paid_status, fee_paid_at, unlock_fee_payment_ledger_id
           FROM asset_unlock_records
           WHERE idempotency_key = ? AND user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(unlock_idempotency_key)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 已缴费重放必须同时找到原始钱包扣款和平台收支双腿，不能只信任 paid 状态位。
async fn ensure_paid_unlock_fee_evidence_in_tx(
    tx: &mut Transaction<'_, MySql>,
    row: &UnlockFeePaymentRow,
    user_id: u64,
    payment_asset_id: u64,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<()> {
    let transaction_key = format!("new_coin_unlock_fee:{}", row.id);
    let evidence: i64 = sqlx::query_scalar(
        r#"SELECT (
               EXISTS (
                   SELECT 1 FROM wallet_ledger ledger
                   WHERE ledger.id = ? AND ledger.user_id = ? AND ledger.asset_id = ?
                     AND ledger.change_type = 'new_coin_unlock_fee_payment'
                     AND ledger.amount = -? AND ledger.balance_type = 'available'
                     AND ledger.ref_type = 'new_coin_unlock' AND ledger.ref_id = ?
               )
               AND EXISTS (
                   SELECT 1 FROM platform_financial_journal journal
                   WHERE journal.transaction_key = ?
                     AND journal.context = 'new_coin_unlock_fee'
                     AND journal.account_code = 'user_unlock_fee_expense'
                     AND journal.asset_id = ? AND journal.amount = -?
                     AND journal.ref_type = 'new_coin_unlock' AND journal.ref_id = ?
               )
               AND EXISTS (
                   SELECT 1 FROM platform_financial_journal journal
                   WHERE journal.transaction_key = ?
                     AND journal.context = 'new_coin_unlock_fee'
                     AND journal.account_code = 'platform_unlock_fee_revenue'
                     AND journal.asset_id = ? AND journal.amount = ?
                     AND journal.ref_type = 'new_coin_unlock' AND journal.ref_id = ?
               )
           )"#,
    )
    .bind(row.unlock_fee_payment_ledger_id)
    .bind(user_id)
    .bind(payment_asset_id)
    .bind(amount)
    .bind(idempotency_key)
    .bind(&transaction_key)
    .bind(payment_asset_id)
    .bind(amount)
    .bind(row.id.to_string())
    .bind(&transaction_key)
    .bind(payment_asset_id)
    .bind(amount)
    .bind(row.id.to_string())
    .fetch_one(&mut **tx)
    .await?;
    if evidence != 1 {
        return Err(AppError::Internal(
            "paid unlock fee settlement evidence is incomplete".to_owned(),
        ));
    }
    Ok(())
}

#[async_trait]
impl NewCoinUnlockReleaseRepository for MySqlNewCoinReadRepository {
    /// 在单个事务内完成一笔到期解禁的资金释放，把锁仓额度转成可用余额并留下完整审计。
    /// 进入事务前先无锁确认该幂等键与用户存在对应记录，缺失直接返回 `NotFound`，不为非法键开事务。
    /// 事务内按固定顺序取锁：资产、钱包，再用联表 `FOR UPDATE` 锁解禁记录与锁仓位置，
    /// 与下单及缴费路径保持同向，避免释放与新分配形成反向等待。
    /// 放行条件必须同时成立：记录未释放、锁仓仍为 active、解禁时点已到、剩余量足够本次数量，
    /// 且项目未开启解禁收费或该记录已缴费。
    /// 条件不成立时若记录已是 released，判定为重放，提交空事务并以 `released = false`
    /// 回吐既有资产与数量；否则返回 `Validation` 表示未到期或未缴费，事务回滚不留痕迹。
    /// 资金只有一个流向：从 `wallet_accounts.locked` 扣减并等额加到 `available`，
    /// 全程不经过 `frozen` 中转；锁仓行同步累加 `released_amount`、扣减 `remaining_amount`，
    /// 减到零才把位置状态由 active 改为 released。
    /// 每次真实释放固定写两条 change_type 为 `new_coin_unlock_release` 的账本，
    /// 分别记录 locked 腿的负变动与 available 腿的正变动，ref_id 取解禁幂等键便于反查。
    /// 钱包账户缺失、locked 余额不足或锁仓剩余量被并发占用时整体回滚，
    /// 绝不出现只改了余额却没有账本、或只释放锁仓却没入账的中间态。
    async fn release_due_paid_unlock(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<ReleaseUnlockOutcome> {
        let locator = sqlx::query_as::<_, UnlockReleaseLocatorRow>(
            r#"SELECT asset_id, unlock_quantity, status
               FROM asset_unlock_records
               WHERE idempotency_key = ? AND user_id = ?
               LIMIT 1"#,
        )
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AppError::NotFound)?;
        ensure_positive_amount(&locator.unlock_quantity, "unlock_quantity")?;
        if locator.status == "released" {
            return Ok(ReleaseUnlockOutcome {
                asset_id: locator.asset_id,
                unlock_quantity: locator.unlock_quantity,
                released: false,
            });
        }

        let mut tx = self.pool.begin().await?;
        let precision = lock_asset_precision_in_tx(&mut tx, locator.asset_id).await?;
        ensure_new_coin_amount_precision(&locator.unlock_quantity, precision, "unlock_quantity")?;
        let wallet = lock_wallet_row(&mut tx, user_id, locator.asset_id).await?;
        let Some(row) = sqlx::query_as::<_, ReleasableUnlockRow>(
            r#"SELECT unlocks.id AS unlock_id, unlocks.asset_id, unlocks.lock_position_id,
                      unlocks.unlock_quantity, positions.remaining_amount
               FROM asset_unlock_records unlocks
               INNER JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
               WHERE unlocks.idempotency_key = ? AND unlocks.user_id = ?
                 AND unlocks.status <> 'released'
                 AND positions.status = 'active'
                 AND positions.unlock_at <= CURRENT_TIMESTAMP(6)
                 AND positions.remaining_amount >= unlocks.unlock_quantity
                 AND (
                    unlocks.unlock_fee_enabled = false
                    OR (
                        unlocks.fee_paid_status = 'not_required'
                        AND unlocks.unlock_fee_asset IS NOT NULL
                        AND unlocks.unlock_fee_amount = 0
                    )
                    OR (
                        unlocks.fee_paid_status = 'paid'
                        AND unlocks.fee_paid_at IS NOT NULL
                        AND unlocks.unlock_fee_payment_ledger_id IS NOT NULL
                        AND EXISTS (
                            SELECT 1 FROM wallet_ledger ledger
                            WHERE ledger.id = unlocks.unlock_fee_payment_ledger_id
                              AND ledger.user_id = unlocks.user_id
                              AND ledger.asset_id = unlocks.unlock_fee_asset
                              AND ledger.change_type = 'new_coin_unlock_fee_payment'
                              AND ledger.amount = -unlocks.unlock_fee_amount
                              AND ledger.balance_type = 'available'
                              AND ledger.ref_type = 'new_coin_unlock'
                              AND ledger.ref_id = unlocks.idempotency_key
                        )
                        AND EXISTS (
                            SELECT 1 FROM platform_financial_journal journal
                            WHERE journal.transaction_key = CONCAT('new_coin_unlock_fee:', unlocks.id)
                              AND journal.context = 'new_coin_unlock_fee'
                              AND journal.account_code = 'user_unlock_fee_expense'
                              AND journal.asset_id = unlocks.unlock_fee_asset
                              AND journal.amount = -unlocks.unlock_fee_amount
                              AND journal.ref_type = 'new_coin_unlock'
                              AND journal.ref_id = CAST(unlocks.id AS CHAR)
                        )
                        AND EXISTS (
                            SELECT 1 FROM platform_financial_journal journal
                            WHERE journal.transaction_key = CONCAT('new_coin_unlock_fee:', unlocks.id)
                              AND journal.context = 'new_coin_unlock_fee'
                              AND journal.account_code = 'platform_unlock_fee_revenue'
                              AND journal.asset_id = unlocks.unlock_fee_asset
                              AND journal.amount = unlocks.unlock_fee_amount
                              AND journal.ref_type = 'new_coin_unlock'
                              AND journal.ref_id = CAST(unlocks.id AS CHAR)
                        )
                    )
                 )
               LIMIT 1
               FOR UPDATE"#,
        )
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        else {
            if let Some((asset_id, unlock_quantity)) = sqlx::query_as::<_, (u64, BigDecimal)>(
                r#"SELECT asset_id, unlock_quantity
                   FROM asset_unlock_records
                   WHERE idempotency_key = ? AND user_id = ? AND status = 'released'
                   LIMIT 1
                   FOR UPDATE"#,
            )
            .bind(unlock_idempotency_key)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
            {
                tx.commit().await?;
                return Ok(ReleaseUnlockOutcome {
                    asset_id,
                    unlock_quantity,
                    released: false,
                });
            }
            return Err(AppError::Validation(
                "unlock is not releasable until unlock time is reached and required fee is paid"
                    .to_owned(),
            ));
        };

        if row.asset_id != locator.asset_id
            || row.unlock_quantity.normalized() != locator.unlock_quantity.normalized()
        {
            return Err(AppError::Internal(
                "unlock identity changed while release was being locked".to_owned(),
            ));
        }
        if wallet.locked < row.unlock_quantity {
            return Err(AppError::Validation(
                "wallet locked balance is insufficient for unlock release".to_owned(),
            ));
        }

        let available_after = wallet.available.clone() + row.unlock_quantity.clone();
        let locked_after = wallet.locked.clone() - row.unlock_quantity.clone();
        let remaining_after = row.remaining_amount - row.unlock_quantity.clone();
        let lock_status = if remaining_after == 0 {
            "released"
        } else {
            "active"
        };

        // 锁仓释放、解锁记录状态、钱包余额和双向流水必须在一个事务中完成，避免余额变化缺少审计记录。
        let position_updated = sqlx::query(
            r#"UPDATE asset_lock_positions
               SET released_amount = released_amount + ?,
                   remaining_amount = ?,
                   status = ?
               WHERE id = ? AND remaining_amount >= ?"#,
        )
        .bind(&row.unlock_quantity)
        .bind(&remaining_after)
        .bind(lock_status)
        .bind(row.lock_position_id)
        .bind(&row.unlock_quantity)
        .execute(&mut *tx)
        .await?;
        if position_updated.rows_affected() != 1 {
            return Err(AppError::Conflict(
                "unlock lock position changed concurrently".to_owned(),
            ));
        }

        let unlock_updated = sqlx::query(
            "UPDATE asset_unlock_records SET status = 'released' WHERE id = ? AND status <> 'released'",
        )
            .bind(row.unlock_id)
            .execute(&mut *tx)
            .await?;
        if unlock_updated.rows_affected() != 1 {
            return Err(AppError::Conflict(
                "unlock was released concurrently".to_owned(),
            ));
        }

        let wallet_updated = sqlx::query(
            "UPDATE wallet_accounts SET available = ?, locked = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(&locked_after)
        .bind(user_id)
        .bind(row.asset_id)
        .execute(&mut *tx)
        .await?;
        if wallet_updated.rows_affected() != 1 {
            return Err(AppError::Internal(
                "unlock wallet balance could not be updated".to_owned(),
            ));
        }

        sqlx::query(
            r#"INSERT INTO wallet_ledger
               (user_id, asset_id, change_type, amount, balance_type, balance_after,
                available_after, frozen_after, locked_after, ref_type, ref_id)
               VALUES (?, ?, 'new_coin_unlock_release', ?, 'locked', ?, ?, ?, ?, 'new_coin_unlock', ?),
                      (?, ?, 'new_coin_unlock_release', ?, 'available', ?, ?, ?, ?, 'new_coin_unlock', ?)"#,
        )
        .bind(user_id)
        .bind(row.asset_id)
        .bind(-row.unlock_quantity.clone())
        .bind(&locked_after)
        .bind(&available_after)
        .bind(&wallet.frozen)
        .bind(&locked_after)
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .bind(row.asset_id)
        .bind(&row.unlock_quantity)
        .bind(&available_after)
        .bind(&available_after)
        .bind(&wallet.frozen)
        .bind(&locked_after)
        .bind(unlock_idempotency_key)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(ReleaseUnlockOutcome {
            asset_id: row.asset_id,
            unlock_quantity: row.unlock_quantity,
            released: true,
        })
    }
}

/// 解禁必须允许已停用资产归还给用户，但精度元数据仍须在钱包锁之前稳定并接受 0..=18 校验。
async fn lock_asset_precision_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<i32> {
    sqlx::query_scalar::<_, i32>(
        "SELECT precision_scale FROM assets WHERE id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}
