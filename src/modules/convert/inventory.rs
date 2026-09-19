//! 显式闪兑输出资金配额的事务适配器；不从总账净额或外部持仓推导资金。

use bigdecimal::BigDecimal;
use sqlx::{MySql, Transaction};

use crate::error::{AppError, AppResult};
use crate::modules::wallet::amount_fits_asset_precision;

#[derive(sqlx::FromRow)]
struct Inventory {
    asset_id: u64,
    enabled: bool,
    funded_amount: BigDecimal,
    consumed_amount: BigDecimal,
    revision: u64,
}

/// 已持有交易对锁的调用方使用当前读锁定库存；所有配置与消费都遵循 pair→inventory。
async fn lock_inventory(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<Option<Inventory>> {
    Ok(sqlx::query_as(
        "SELECT asset_id, enabled, funded_amount, consumed_amount, revision FROM convert_inventory_accounts WHERE pair_id = ? FOR UPDATE",
    ).bind(pair_id).fetch_optional(&mut **tx).await?)
}

/// 成交事务持有交易对与资产锁后、写钱包前调用；缺省或停用不建立虚构库存。
/// 启用时按输出币种净到账额消费显式资金并写唯一报价分配，任何后续失败随外层事务回滚。
/// 同报价重放不重复消费；已有分配参数不一致拒绝。无持久化未成交预留，报价本身不保证容量。
pub(crate) async fn consume_output_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    quote_id: &str,
    asset_id: u64,
    amount: &BigDecimal,
) -> AppResult<()> {
    let Some(inventory) = lock_inventory(tx, pair_id).await? else {
        return Ok(());
    };
    let existing: Option<(u64, u64, BigDecimal)> = sqlx::query_as(
        "SELECT pair_id, asset_id, amount FROM convert_inventory_allocations WHERE quote_id = ?",
    )
    .bind(quote_id)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some((old_pair, old_asset, old_amount)) = existing {
        if (old_pair, old_asset, &old_amount) != (pair_id, asset_id, amount) {
            return Err(AppError::Conflict(
                "convert inventory allocation mismatch".into(),
            ));
        }
        return Ok(());
    }
    if !inventory.enabled {
        return Ok(());
    }
    if inventory.asset_id != asset_id {
        return Err(AppError::Validation(
            "convert inventory protection supports only the configured forward output asset; reverse confirmation is disabled".into(),
        ));
    }
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "convert inventory amount must be positive".into(),
        ));
    }
    let next = &inventory.consumed_amount + amount;
    if next > inventory.funded_amount {
        return Err(AppError::Validation(
            "convert output inventory capacity exceeded".into(),
        ));
    }
    sqlx::query("UPDATE convert_inventory_accounts SET consumed_amount = ? WHERE pair_id = ?")
        .bind(&next)
        .bind(pair_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "INSERT INTO convert_inventory_allocations(quote_id,pair_id,asset_id,amount,consumed_before,consumed_after,funded_amount) VALUES(?,?,?,?,?,?,?)",
    ).bind(quote_id).bind(pair_id).bind(asset_id).bind(amount)
        .bind(&inventory.consumed_amount).bind(&next).bind(&inventory.funded_amount)
        .execute(&mut **tx).await?;
    Ok(())
}

/// 库存历史一旦建立就不能换输出币种，即使停用或余额为零也须保留原币种审计。
/// 调用方先持交易对锁，读取当前库存不提交也不修改余额。
pub(crate) async fn ensure_inventory_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    asset_id: u64,
) -> AppResult<()> {
    if let Some(inventory) = lock_inventory(tx, pair_id).await?
        && inventory.asset_id != asset_id
    {
        return Err(AppError::Conflict(
            "cannot change asset with convert inventory history".into(),
        ));
    }
    Ok(())
}

/// 管理员显式配置资金总额；初次 expected_revision=0，后续必须匹配旧版，消费不改变配置版号。
/// 持有交易对锁后调用；资金凭据与原因必填，总额不能低于消耗，不清零历史、不增加用户余额。
/// 配置、独立资金审计和外层交易对审计必须在同一事务提交；停用仅改变开关。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn configure_inventory_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    asset_id: u64,
    admin_id: u64,
    expected_revision: u64,
    enabled: bool,
    funded_amount: &BigDecimal,
    funding_reference: &str,
    reason: &str,
) -> AppResult<()> {
    let funding_reference = funding_reference.trim();
    if funding_reference.is_empty()
        || funding_reference.chars().count() > 255
        || reason.trim().is_empty()
        || reason.chars().count() > 512
    {
        return Err(AppError::Validation(
            "convert inventory funding reference and reason are required".into(),
        ));
    }
    let precision: i32 = sqlx::query_scalar("SELECT precision_scale FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&mut **tx)
        .await?;
    let normalized = funded_amount.normalized();
    let (_, scale) = normalized.as_bigint_and_exponent();
    let max: BigDecimal = "100000000000000000000".parse().expect("decimal constant");
    if funded_amount < &BigDecimal::from(0)
        || funded_amount >= &max
        || scale > 18
        || !amount_fits_asset_precision(funded_amount, precision)
    {
        return Err(AppError::Validation(
            "convert inventory funding amount must be nonnegative and fit asset precision".into(),
        ));
    }
    let before = lock_inventory(tx, pair_id).await?;
    let revision = before.as_ref().map_or(0, |row| row.revision);
    if revision != expected_revision {
        return Err(AppError::Conflict(
            "convert inventory revision is stale".into(),
        ));
    }
    let zero = BigDecimal::from(0);
    let consumed = before.as_ref().map_or(&zero, |row| &row.consumed_amount);
    if before.as_ref().is_some_and(|row| row.asset_id != asset_id) || funded_amount < consumed {
        return Err(AppError::Conflict(
            "convert inventory asset mismatch or funding below consumed amount".into(),
        ));
    }
    let next_revision = revision
        .checked_add(1)
        .ok_or_else(|| AppError::Conflict("inventory revision overflow".into()))?;
    sqlx::query(
        "INSERT INTO convert_inventory_accounts(pair_id,asset_id,enabled,funded_amount,consumed_amount,revision) VALUES(?,?,?,?,0,?) ON DUPLICATE KEY UPDATE enabled=VALUES(enabled),funded_amount=VALUES(funded_amount),revision=VALUES(revision)",
    ).bind(pair_id).bind(asset_id).bind(enabled).bind(funded_amount).bind(next_revision)
        .execute(&mut **tx).await?;
    sqlx::query(
        "INSERT INTO convert_inventory_funding_audits(pair_id,asset_id,admin_id,revision,enabled,funded_before,funded_after,consumed_amount,funding_reference,reason) VALUES(?,?,?,?,?,?,?,?,?,?)",
    ).bind(pair_id).bind(asset_id).bind(admin_id).bind(next_revision).bind(enabled)
        .bind(before.as_ref().map_or(&zero, |row| &row.funded_amount)).bind(funded_amount).bind(consumed)
        .bind(funding_reference).bind(reason.trim()).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_convert_inventory_tests.rs"]
mod tests;
