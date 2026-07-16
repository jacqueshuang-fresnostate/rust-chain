//! new_coin bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::new_coin::{
        LifecycleStatus,
        repository::{
            NewCoinDistributionRead, NewCoinLedgerMetadata, NewCoinLockPositionWrite,
            NewCoinOrderRepository, NewCoinPairRead, NewCoinProjectRead, NewCoinProjectRuleRead,
            NewCoinPurchaseOrderWrite, NewCoinPurchaseRead, NewCoinReadRepository,
            NewCoinSubscriptionOrderWrite, NewCoinSubscriptionRead, NewCoinUnlockFeeRepository,
            NewCoinUnlockRead, NewCoinUnlockReleaseRepository, NewCoinWalletRead,
            ReleaseUnlockOutcome, UnlockFeeExpectation, UnlockFeePaymentWrite,
        },
        service::{
            ensure_post_listing_purchase_enabled, lifecycle_status, lock_positions_for_project,
            unlock_fee_fields,
        },
    },
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use chrono::Utc;
use sqlx::{MySql, Pool, QueryBuilder, Transaction};

#[derive(Debug)]
pub struct InfrastructureLayerMarker;

impl InfrastructureLayer for InfrastructureLayerMarker {}

#[derive(Debug, Clone)]
pub(crate) struct MySqlNewCoinReadRepository {
    pool: Pool<MySql>,
}

impl MySqlNewCoinReadRepository {
    pub(crate) fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NewCoinReadRepository for MySqlNewCoinReadRepository {
    async fn list_active_projects(&self, limit: u32) -> AppResult<Vec<NewCoinProjectRead>> {
        let rows = sqlx::query_as::<_, NewCoinProjectReadRow>(
            r#"SELECT id, asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
                      unlock_type, fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
                      unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                      post_listing_purchase_enabled, post_listing_pair_id, status
               FROM new_coin_projects
               WHERE status = 'active'
               ORDER BY id DESC
               LIMIT ?"#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_active_project_by_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<NewCoinProjectRead>> {
        let row = sqlx::query_as::<_, NewCoinProjectReadRow>(
            r#"SELECT id, asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
                      unlock_type, fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
                      unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                      post_listing_purchase_enabled, post_listing_pair_id, status
               FROM new_coin_projects
               WHERE symbol = ? AND status = 'active'
               LIMIT 1"#,
        )
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn list_user_subscriptions(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinSubscriptionRead>> {
        let rows = sqlx::query_as::<_, NewCoinSubscriptionReadRow>(
            r#"SELECT id, project_id, user_id, quote_asset, quote_amount, requested_quantity,
                      allocated_quantity, status, idempotency_key, created_at
               FROM new_coin_subscriptions
               WHERE user_id = ?
               ORDER BY id DESC
               LIMIT ?"#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_user_distributions(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinDistributionRead>> {
        let rows = sqlx::query_as::<_, NewCoinDistributionReadRow>(
            r#"SELECT id, project_id, user_id, subscription_id, asset_id, quantity,
                      lock_position_id, status, idempotency_key, created_at
               FROM new_coin_distributions
               WHERE user_id = ?
               ORDER BY id DESC
               LIMIT ?"#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_user_purchases(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinPurchaseRead>> {
        let rows = sqlx::query_as::<_, NewCoinPurchaseReadRow>(
            r#"SELECT id, project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                      quote_amount, lock_position_id, status, idempotency_key, created_at
               FROM new_coin_purchase_orders
               WHERE user_id = ?
               ORDER BY id DESC
               LIMIT ?"#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_user_unlocks(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinUnlockRead>> {
        let rows = sqlx::query_as::<_, NewCoinUnlockReadRow>(
            r#"SELECT id, user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
                      unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                      unlock_fee_amount, fee_paid_status, status, idempotency_key, created_at
               FROM asset_unlock_records
               WHERE user_id = ?
               ORDER BY id DESC
               LIMIT ?"#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[async_trait]
impl NewCoinUnlockFeeRepository for MySqlNewCoinReadRepository {
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

    async fn mark_unlock_fee_paid(&self, payment: UnlockFeePaymentWrite) -> AppResult<bool> {
        // 手续费支付状态使用幂等更新，重复支付同一解锁记录时不能重复改变业务状态。
        let result = sqlx::query(
            r#"UPDATE asset_unlock_records
               SET fee_paid_status = 'paid',
                   unlock_fee_asset = ?,
                   unlock_fee_amount = ?
               WHERE idempotency_key = ?
                 AND user_id = ?
                 AND fee_paid_status <> 'paid'"#,
        )
        .bind(payment.payment_asset_id)
        .bind(payment.amount)
        .bind(payment.unlock_idempotency_key)
        .bind(payment.user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }
}

#[async_trait]
impl NewCoinUnlockReleaseRepository for MySqlNewCoinReadRepository {
    async fn release_due_paid_unlock(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<ReleaseUnlockOutcome> {
        let exists = sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM asset_unlock_records WHERE idempotency_key = ? AND user_id = ? LIMIT 1",
        )
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        if exists.is_none() {
            return Err(AppError::NotFound);
        }

        let mut tx = self.pool.begin().await?;
        let Some(row) = sqlx::query_as::<_, ReleasableUnlockRow>(
            r#"SELECT unlocks.id AS unlock_id, unlocks.asset_id, unlocks.lock_position_id,
                      unlocks.unlock_quantity
               FROM asset_unlock_records unlocks
               INNER JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
               WHERE unlocks.idempotency_key = ? AND unlocks.user_id = ?
                 AND unlocks.status <> 'released'
                 AND positions.status = 'active'
                 AND positions.unlock_at <= CURRENT_TIMESTAMP(6)
                 AND positions.remaining_amount >= unlocks.unlock_quantity
                 AND (unlocks.unlock_fee_enabled = false OR unlocks.fee_paid_status = 'paid')
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
                   LIMIT 1"#,
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

        let Some((available, frozen, locked)) =
            sqlx::query_as::<_, (BigDecimal, BigDecimal, BigDecimal)>(
                "SELECT available, frozen, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ? FOR UPDATE",
            )
            .bind(user_id)
            .bind(row.asset_id)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Err(AppError::Validation(
                "wallet account is required before unlock release".to_owned(),
            ));
        };

        if locked < row.unlock_quantity {
            return Err(AppError::Validation(
                "wallet locked balance is insufficient for unlock release".to_owned(),
            ));
        }

        let available_after = available + row.unlock_quantity.clone();
        let locked_after = locked - row.unlock_quantity.clone();

        let (remaining_before,) = sqlx::query_as::<_, (BigDecimal,)>(
            "SELECT remaining_amount FROM asset_lock_positions WHERE id = ? FOR UPDATE",
        )
        .bind(row.lock_position_id)
        .fetch_one(&mut *tx)
        .await?;
        let remaining_after = remaining_before - row.unlock_quantity.clone();
        let lock_status = if remaining_after == 0 {
            "released"
        } else {
            "active"
        };

        // 锁仓释放、解锁记录状态、钱包余额和双向流水必须在一个事务中完成，避免余额变化缺少审计记录。
        sqlx::query(
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

        sqlx::query("UPDATE asset_unlock_records SET status = 'released' WHERE id = ?")
            .bind(row.unlock_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "UPDATE wallet_accounts SET available = ?, locked = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(&locked_after)
        .bind(user_id)
        .bind(row.asset_id)
        .execute(&mut *tx)
        .await?;

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
        .bind(&frozen)
        .bind(&locked_after)
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .bind(row.asset_id)
        .bind(&row.unlock_quantity)
        .bind(&available_after)
        .bind(&available_after)
        .bind(&frozen)
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

#[async_trait]
impl NewCoinOrderRepository for MySqlNewCoinReadRepository {
    async fn find_project_rule_by_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<NewCoinProjectRuleRead>> {
        let sql = new_coin_project_rule_select_sql("symbol = ?", "LIMIT 1");
        let row = sqlx::query_as::<_, NewCoinProjectRuleReadRow>(&sql)
            .bind(symbol)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(Into::into))
    }

    async fn find_pair_for_purchase(
        &self,
        pair_id: u64,
        project_asset_id: u64,
    ) -> AppResult<Option<NewCoinPairRead>> {
        let row = sqlx::query_as::<_, NewCoinPairReadRow>(new_coin_pair_select_sql(false))
            .bind(pair_id)
            .bind(project_asset_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(Into::into))
    }

    async fn create_subscription_order(
        &self,
        order: NewCoinSubscriptionOrderWrite,
    ) -> AppResult<Option<u64>> {
        let mut tx = self.pool.begin().await?;
        if idempotency_key_exists(&mut tx, "new_coin_subscriptions", &order.idempotency_key).await?
        {
            return Err(AppError::Conflict(
                "new coin subscription has already been created".to_owned(),
            ));
        }
        sqlx::query(
            r#"INSERT INTO new_coin_subscriptions
               (project_id, user_id, quote_asset, quote_amount, requested_quantity,
                allocated_quantity, status, idempotency_key)
               VALUES (?, ?, ?, ?, ?, 0, 'pending', ?)"#,
        )
        .bind(order.project.id)
        .bind(order.user_id)
        .bind(order.quote_asset_id)
        .bind(&order.quote_amount)
        .bind(&order.quantity)
        .bind(&order.idempotency_key)
        .execute(&mut *tx)
        .await?;

        debit_wallet_available(
            &mut tx,
            order.user_id,
            order.quote_asset_id,
            &order.quote_amount,
            NewCoinLedgerMetadata {
                change_type: "new_coin_subscription_payment",
                ref_type: "new_coin_subscription",
                ref_id: &order.idempotency_key,
            },
        )
        .await?;
        let lock_position_id = apply_new_coin_allocation(
            &mut tx,
            order.user_id,
            order.project.asset_id,
            &order.quantity,
            &order.lock_positions,
            &order.project.issue_price,
            &order.quote_amount,
            &order.project,
            NewCoinLedgerMetadata {
                change_type: "new_coin_subscription_lock",
                ref_type: "new_coin_subscription",
                ref_id: &order.idempotency_key,
            },
        )
        .await?;
        sqlx::query(
            "UPDATE new_coin_subscriptions SET allocated_quantity = ?, status = 'allocated' WHERE idempotency_key = ?",
        )
        .bind(&order.quantity)
        .bind(&order.idempotency_key)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(lock_position_id)
    }

    async fn create_purchase_order(
        &self,
        order: NewCoinPurchaseOrderWrite,
    ) -> AppResult<Option<u64>> {
        let mut tx = self.pool.begin().await?;
        // 下单事务内重新锁定项目和交易对，避免后台刚关闭认购或调整规则后用户仍按旧快照成交。
        let locked_project =
            lock_purchase_project_in_tx(&mut tx, order.project.id, order.pair_id).await?;
        let locked_pair =
            lock_pair_for_purchase_in_tx(&mut tx, order.pair_id, locked_project.asset_id).await?;
        let lock_positions = lock_positions_for_project(
            &locked_project,
            order.user_id,
            locked_project.asset_id,
            &order.idempotency_key,
            order.quantity.clone(),
            Utc::now(),
            "new_coin_purchase",
        )?;
        if idempotency_key_exists(&mut tx, "new_coin_purchase_orders", &order.idempotency_key)
            .await?
        {
            return Err(AppError::Conflict(
                "new coin purchase has already been created".to_owned(),
            ));
        }
        sqlx::query(
            r#"INSERT INTO new_coin_purchase_orders
               (project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                quote_amount, lock_position_id, status, idempotency_key)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, 'pending', ?)"#,
        )
        .bind(locked_project.id)
        .bind(order.user_id)
        .bind(order.pair_id)
        .bind(locked_pair.base_asset_id)
        .bind(locked_pair.quote_asset_id)
        .bind(&order.price)
        .bind(&order.quantity)
        .bind(&order.quote_amount)
        .bind(&order.idempotency_key)
        .execute(&mut *tx)
        .await?;

        debit_wallet_available(
            &mut tx,
            order.user_id,
            locked_pair.quote_asset_id,
            &order.quote_amount,
            NewCoinLedgerMetadata {
                change_type: "new_coin_purchase_payment",
                ref_type: "new_coin_purchase",
                ref_id: &order.idempotency_key,
            },
        )
        .await?;
        let lock_position_id = apply_new_coin_allocation(
            &mut tx,
            order.user_id,
            locked_project.asset_id,
            &order.quantity,
            &lock_positions,
            &order.price,
            &order.quote_amount,
            &locked_project,
            NewCoinLedgerMetadata {
                change_type: "new_coin_purchase_lock",
                ref_type: "new_coin_purchase",
                ref_id: &order.idempotency_key,
            },
        )
        .await?;
        sqlx::query(
            "UPDATE new_coin_purchase_orders SET lock_position_id = ?, status = 'locked' WHERE idempotency_key = ?",
        )
        .bind(lock_position_id)
        .bind(&order.idempotency_key)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(lock_position_id)
    }
}

async fn lock_purchase_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    requested_pair_id: u64,
) -> AppResult<NewCoinProjectRuleRead> {
    let sql = new_coin_project_rule_select_sql("id = ?", "LIMIT 1 FOR UPDATE");
    let project = sqlx::query_as::<_, NewCoinProjectRuleReadRow>(&sql)
        .bind(project_id)
        .fetch_optional(&mut **tx)
        .await?
        .map(NewCoinProjectRuleRead::from)
        .ok_or(AppError::NotFound)?;
    if lifecycle_status(&project.lifecycle_status)? != LifecycleStatus::Listed {
        return Err(AppError::Validation(
            "post-listing new coin purchase is not open for this project".to_owned(),
        ));
    }
    ensure_post_listing_purchase_enabled(&project, requested_pair_id)?;
    Ok(project)
}

async fn lock_pair_for_purchase_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    project_asset_id: u64,
) -> AppResult<NewCoinPairRead> {
    sqlx::query_as::<_, NewCoinPairReadRow>(new_coin_pair_select_sql(true))
        .bind(pair_id)
        .bind(project_asset_id)
        .fetch_optional(&mut **tx)
        .await?
        .map(NewCoinPairRead::from)
        .ok_or(AppError::NotFound)
}

fn new_coin_project_rule_select_sql(predicate: &str, suffix: &str) -> String {
    format!(
        r#"SELECT id, asset_id, lifecycle_status, issue_price, listed_at, unlock_type,
                  fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
                  unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                  post_listing_purchase_enabled, post_listing_pair_id
           FROM new_coin_projects
           WHERE {predicate} AND status = 'active'
           {suffix}"#,
    )
}

fn new_coin_pair_select_sql(for_update: bool) -> &'static str {
    if for_update {
        r#"SELECT base_asset AS base_asset_id, quote_asset AS quote_asset_id
           FROM trading_pairs
           WHERE id = ? AND base_asset = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#
    } else {
        r#"SELECT base_asset AS base_asset_id, quote_asset AS quote_asset_id
           FROM trading_pairs
           WHERE id = ? AND base_asset = ? AND status = 'active'
           LIMIT 1"#
    }
}

async fn idempotency_key_exists(
    tx: &mut Transaction<'_, MySql>,
    table_name: &str,
    idempotency_key: &str,
) -> AppResult<bool> {
    let mut query = QueryBuilder::<MySql>::new("SELECT id FROM ");
    query
        .push(table_name)
        .push(" WHERE idempotency_key = ")
        .push_bind(idempotency_key)
        .push(" LIMIT 1 FOR UPDATE");
    let exists: Option<(u64,)> = query.build_query_as().fetch_optional(&mut **tx).await?;
    Ok(exists.is_some())
}

#[allow(clippy::too_many_arguments)]
async fn apply_new_coin_allocation(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    quantity: &BigDecimal,
    lock_positions: &[NewCoinLockPositionWrite],
    unlock_price: &BigDecimal,
    purchase_cost: &BigDecimal,
    project: &NewCoinProjectRuleRead,
    ledger: NewCoinLedgerMetadata<'_>,
) -> AppResult<Option<u64>> {
    if lock_positions.is_empty() {
        credit_wallet_available(
            tx,
            user_id,
            asset_id,
            quantity,
            ledger.change_type,
            ledger.ref_type,
            ledger.ref_id,
        )
        .await?;
        return Ok(None);
    }

    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let locked_after = wallet.locked.clone() + quantity.clone();
    sqlx::query("UPDATE wallet_accounts SET locked = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&locked_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_new_coin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        quantity.clone(),
        "locked",
        &locked_after,
        &wallet.available,
        &wallet.frozen,
        &locked_after,
        ledger.change_type,
        ledger.ref_type,
        ledger.ref_id,
    )
    .await?;

    let mut first_lock_position_id = None;
    for position in lock_positions {
        let position_id = upsert_lock_position(tx, position).await?;
        ensure_unlock_record(
            tx,
            user_id,
            asset_id,
            position_id,
            &position.amount,
            unlock_price,
            purchase_cost,
            project,
            &position.source_id,
        )
        .await?;
        if first_lock_position_id.is_none() {
            first_lock_position_id = Some(position_id);
        }
    }
    Ok(first_lock_position_id)
}

#[allow(clippy::too_many_arguments)]
async fn ensure_unlock_record(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    quantity: &BigDecimal,
    unlock_price: &BigDecimal,
    purchase_cost: &BigDecimal,
    project: &NewCoinProjectRuleRead,
    source_id: &str,
) -> AppResult<()> {
    let (fee_paid_status, unlock_fee_amount) =
        unlock_fee_fields(project, quantity, unlock_price, purchase_cost)?;
    sqlx::query(
        r#"INSERT INTO asset_unlock_records
           (user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
            unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
            unlock_fee_amount, fee_paid_status, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(lock_position_id)
    .bind(quantity)
    .bind(unlock_price)
    .bind(project.unlock_fee_enabled)
    .bind(&project.unlock_fee_rate)
    .bind(&project.unlock_fee_basis)
    .bind(project.unlock_fee_asset)
    .bind(&unlock_fee_amount)
    .bind(fee_paid_status)
    .bind(source_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn debit_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    ledger: NewCoinLedgerMetadata<'_>,
) -> AppResult<()> {
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for new coin order: requested {}, available {}, locked {}",
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
    insert_new_coin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        ledger.change_type,
        ledger.ref_type,
        ledger.ref_id,
    )
    .await
}

async fn credit_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let available_after = wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_new_coin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await
}

async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<NewCoinWalletRead> {
    sqlx::query_as::<_, NewCoinWalletReadRow>(
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
    .map(NewCoinWalletRead::from)
    .ok_or_else(|| AppError::Validation("wallet account is required for new coin order".to_owned()))
}

async fn lock_or_create_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<NewCoinWalletRead> {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    lock_wallet_row(tx, user_id, asset_id).await
}

async fn upsert_lock_position(
    tx: &mut Transaction<'_, MySql>,
    position: &NewCoinLockPositionWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount,
            released_amount, remaining_amount, merge_key, status)
           VALUES (?, ?, ?, ?, 0, 0, 0, ?, 'active')
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(position.user_id)
    .bind(position.asset_id)
    .bind(&position.unlock_type)
    .bind(position.unlock_at.naive_utc())
    .bind(&position.merge_key)
    .execute(&mut **tx)
    .await?;

    let position_id = if result.last_insert_id() == 0 {
        sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM asset_lock_positions WHERE merge_key = ? LIMIT 1 FOR UPDATE",
        )
        .bind(&position.merge_key)
        .fetch_one(&mut **tx)
        .await?
        .0
    } else {
        result.last_insert_id()
    };

    let inserted = sqlx::query(
        r#"INSERT IGNORE INTO asset_lock_position_sources
           (lock_position_id, source_type, source_id, source_amount, source_time)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(position_id)
    .bind(&position.source_type)
    .bind(&position.source_id)
    .bind(&position.amount)
    .bind(position.unlock_at.naive_utc())
    .execute(&mut **tx)
    .await?;

    if inserted.rows_affected() > 0 {
        sqlx::query(
            r#"UPDATE asset_lock_positions
               SET locked_amount = locked_amount + ?,
                   remaining_amount = remaining_amount + ?,
                   status = 'active'
               WHERE id = ?"#,
        )
        .bind(&position.amount)
        .bind(&position.amount)
        .bind(position_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(position_id)
}

#[allow(clippy::too_many_arguments)]
async fn insert_new_coin_wallet_ledger(
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

#[derive(Debug, sqlx::FromRow)]
struct NewCoinProjectReadRow {
    id: u64,
    asset_id: u64,
    symbol: String,
    lifecycle_status: String,
    total_supply: BigDecimal,
    issue_price: BigDecimal,
    listed_at: Option<chrono::DateTime<chrono::Utc>>,
    unlock_type: String,
    fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    relative_unlock_seconds: Option<u64>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    post_listing_purchase_enabled: bool,
    post_listing_pair_id: Option<u64>,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinSubscriptionReadRow {
    id: u64,
    project_id: u64,
    user_id: u64,
    quote_asset: u64,
    quote_amount: BigDecimal,
    requested_quantity: BigDecimal,
    allocated_quantity: BigDecimal,
    status: String,
    idempotency_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinDistributionReadRow {
    id: u64,
    project_id: u64,
    user_id: u64,
    subscription_id: Option<u64>,
    asset_id: u64,
    quantity: BigDecimal,
    lock_position_id: Option<u64>,
    status: String,
    idempotency_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinPurchaseReadRow {
    id: u64,
    project_id: u64,
    user_id: u64,
    pair_id: u64,
    base_asset: u64,
    quote_asset: u64,
    price: BigDecimal,
    quantity: BigDecimal,
    quote_amount: BigDecimal,
    lock_position_id: Option<u64>,
    status: String,
    idempotency_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinUnlockReadRow {
    id: u64,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    unlock_quantity: BigDecimal,
    unlock_price: Option<BigDecimal>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    unlock_fee_amount: Option<BigDecimal>,
    fee_paid_status: String,
    status: String,
    idempotency_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct UnlockFeeExpectationRow {
    unlock_fee_enabled: bool,
    unlock_fee_asset: Option<u64>,
    unlock_fee_amount: Option<BigDecimal>,
}

#[derive(Debug, sqlx::FromRow)]
struct ReleasableUnlockRow {
    unlock_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    unlock_quantity: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinProjectRuleReadRow {
    id: u64,
    asset_id: u64,
    lifecycle_status: String,
    issue_price: BigDecimal,
    listed_at: Option<chrono::DateTime<chrono::Utc>>,
    unlock_type: String,
    fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    relative_unlock_seconds: Option<u64>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    post_listing_purchase_enabled: bool,
    post_listing_pair_id: Option<u64>,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinPairReadRow {
    base_asset_id: u64,
    quote_asset_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinWalletReadRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

impl From<NewCoinProjectReadRow> for NewCoinProjectRead {
    fn from(row: NewCoinProjectReadRow) -> Self {
        Self {
            id: row.id,
            asset_id: row.asset_id,
            symbol: row.symbol,
            lifecycle_status: row.lifecycle_status,
            total_supply: row.total_supply,
            issue_price: row.issue_price,
            listed_at: row.listed_at,
            unlock_type: row.unlock_type,
            fixed_unlock_at: row.fixed_unlock_at,
            relative_unlock_seconds: row.relative_unlock_seconds,
            unlock_fee_enabled: row.unlock_fee_enabled,
            unlock_fee_rate: row.unlock_fee_rate,
            unlock_fee_basis: row.unlock_fee_basis,
            unlock_fee_asset: row.unlock_fee_asset,
            post_listing_purchase_enabled: row.post_listing_purchase_enabled,
            post_listing_pair_id: row.post_listing_pair_id,
            status: row.status,
        }
    }
}

impl From<NewCoinSubscriptionReadRow> for NewCoinSubscriptionRead {
    fn from(row: NewCoinSubscriptionReadRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            user_id: row.user_id,
            quote_asset: row.quote_asset,
            quote_amount: row.quote_amount,
            requested_quantity: row.requested_quantity,
            allocated_quantity: row.allocated_quantity,
            status: row.status,
            idempotency_key: row.idempotency_key,
            created_at: row.created_at,
        }
    }
}

impl From<NewCoinDistributionReadRow> for NewCoinDistributionRead {
    fn from(row: NewCoinDistributionReadRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            user_id: row.user_id,
            subscription_id: row.subscription_id,
            asset_id: row.asset_id,
            quantity: row.quantity,
            lock_position_id: row.lock_position_id,
            status: row.status,
            idempotency_key: row.idempotency_key,
            created_at: row.created_at,
        }
    }
}

impl From<NewCoinPurchaseReadRow> for NewCoinPurchaseRead {
    fn from(row: NewCoinPurchaseReadRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            user_id: row.user_id,
            pair_id: row.pair_id,
            base_asset: row.base_asset,
            quote_asset: row.quote_asset,
            price: row.price,
            quantity: row.quantity,
            quote_amount: row.quote_amount,
            lock_position_id: row.lock_position_id,
            status: row.status,
            idempotency_key: row.idempotency_key,
            created_at: row.created_at,
        }
    }
}

impl From<NewCoinUnlockReadRow> for NewCoinUnlockRead {
    fn from(row: NewCoinUnlockReadRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            asset_id: row.asset_id,
            lock_position_id: row.lock_position_id,
            unlock_quantity: row.unlock_quantity,
            unlock_price: row.unlock_price,
            unlock_fee_enabled: row.unlock_fee_enabled,
            unlock_fee_rate: row.unlock_fee_rate,
            unlock_fee_basis: row.unlock_fee_basis,
            unlock_fee_asset: row.unlock_fee_asset,
            unlock_fee_amount: row.unlock_fee_amount,
            fee_paid_status: row.fee_paid_status,
            status: row.status,
            idempotency_key: row.idempotency_key,
            created_at: row.created_at,
        }
    }
}

impl From<UnlockFeeExpectationRow> for UnlockFeeExpectation {
    fn from(row: UnlockFeeExpectationRow) -> Self {
        Self {
            unlock_fee_enabled: row.unlock_fee_enabled,
            unlock_fee_asset: row.unlock_fee_asset,
            unlock_fee_amount: row.unlock_fee_amount,
        }
    }
}

impl From<NewCoinProjectRuleReadRow> for NewCoinProjectRuleRead {
    fn from(row: NewCoinProjectRuleReadRow) -> Self {
        Self {
            id: row.id,
            asset_id: row.asset_id,
            lifecycle_status: row.lifecycle_status,
            issue_price: row.issue_price,
            listed_at: row.listed_at,
            unlock_type: row.unlock_type,
            fixed_unlock_at: row.fixed_unlock_at,
            relative_unlock_seconds: row.relative_unlock_seconds,
            unlock_fee_enabled: row.unlock_fee_enabled,
            unlock_fee_rate: row.unlock_fee_rate,
            unlock_fee_basis: row.unlock_fee_basis,
            unlock_fee_asset: row.unlock_fee_asset,
            post_listing_purchase_enabled: row.post_listing_purchase_enabled,
            post_listing_pair_id: row.post_listing_pair_id,
        }
    }
}

impl From<NewCoinPairReadRow> for NewCoinPairRead {
    fn from(row: NewCoinPairReadRow) -> Self {
        Self {
            base_asset_id: row.base_asset_id,
            quote_asset_id: row.quote_asset_id,
        }
    }
}

impl From<NewCoinWalletReadRow> for NewCoinWalletRead {
    fn from(row: NewCoinWalletReadRow) -> Self {
        Self {
            available: row.available,
            frozen: row.frozen,
            locked: row.locked,
        }
    }
}
