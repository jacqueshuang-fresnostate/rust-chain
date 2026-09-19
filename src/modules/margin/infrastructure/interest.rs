//! 前瞻性利息检查点与资产精度读取；worker 和主动平仓共享同一事务适配器。

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Transaction};

use crate::{
    error::{AppError, AppResult},
    modules::margin::amounts::{generated_amount, validate_precision},
    numeric::{ensure_amount_storage, ensure_decimal_storage},
};

/// 在已有业务事务内读取真实保证金资产的精度，不依赖交易对精度或客户端元数据。
pub(crate) async fn load_margin_asset_precision(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<i32> {
    let precision = sqlx::query_scalar::<_, i32>("SELECT precision_scale FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    validate_precision(precision)?;
    Ok(precision)
}

/// 计提只使用仓位固化利率和已提交检查点；已计提金额与新余数独立保存，禁止从开仓时间重算历史。
#[derive(sqlx::FromRow)]
struct InterestCheckpoint {
    margin_asset: u64,
    borrowed_amount: BigDecimal,
    interest_amount: BigDecimal,
    interest_remainder: BigDecimal,
    hourly_interest_rate: BigDecimal,
    accrued_from: DateTime<Utc>,
}

/// 为已锁定仓位结清完整小时并原子保存新余数；返回更新后债务，未到窗口则返回空。
/// 全仓调用方必须先持有账户锁并负责刷新账户版本。部分平仓之前调用，余数不随本金缩减而丢失。
/// 零资产单位的增量也推进检查点并存余数，避免批次长度改变总债务；终态不再收费。
pub(crate) async fn accrue_locked_position_interest(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
    now: DateTime<Utc>,
) -> AppResult<Option<BigDecimal>> {
    let Some(checkpoint) = sqlx::query_as::<_, InterestCheckpoint>(
        r#"SELECT margin_asset, borrowed_amount, interest_amount, interest_remainder,
                  hourly_interest_rate, COALESCE(interest_accrued_at, opened_at) AS accrued_from
           FROM margin_positions
           WHERE id = ? AND status = 'opened' AND entry_price IS NOT NULL
           FOR UPDATE"#,
    )
    .bind(position_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };
    let hours = (now - checkpoint.accrued_from).num_hours();
    if hours <= 0 || checkpoint.borrowed_amount <= 0 || checkpoint.hourly_interest_rate <= 0 {
        return Ok(None);
    }
    let hours = u64::try_from(hours)
        .map_err(|_| AppError::Validation("invalid margin interest duration".to_owned()))?;
    let precision = load_margin_asset_precision(tx, checkpoint.margin_asset).await?;
    let (delta, remainder) = interest_increment(
        &checkpoint.borrowed_amount,
        &checkpoint.hourly_interest_rate,
        &checkpoint.interest_remainder,
        hours,
        precision,
    )?;
    let interest_after = checkpoint.interest_amount + delta;
    ensure_amount_storage(&interest_after, "margin accrued interest")?;
    let end = billed_window_end(checkpoint.accrued_from, hours)?;
    let result = sqlx::query(
        r#"UPDATE margin_positions
           SET interest_amount = ?, interest_remainder = ?, interest_accrued_at = ?
           WHERE id = ? AND status = 'opened' AND entry_price IS NOT NULL"#,
    )
    .bind(&interest_after)
    .bind(&remainder)
    .bind(end.naive_utc())
    .bind(position_id)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "margin interest checkpoint changed".to_owned(),
        ));
    }
    Ok(Some(interest_after))
}

/// 新利息加前向余数后仅量化增量一次；26 位余数精确保留本金十八位乘利率八位的乘积。
/// 既有累计债务不参与量化，部分平仓扣过的债务不会在下一轮重新出现。
pub(crate) fn interest_increment(
    borrowed: &BigDecimal,
    rate: &BigDecimal,
    remainder: &BigDecimal,
    hours: u64,
    precision: i32,
) -> AppResult<(BigDecimal, BigDecimal)> {
    ensure_amount_storage(borrowed, "margin borrowed amount")?;
    ensure_decimal_storage(rate, 18, 8, "margin interest rate")?;
    ensure_decimal_storage(remainder, 26, 26, "margin interest remainder")?;
    if borrowed < &BigDecimal::from(0)
        || rate < &BigDecimal::from(0)
        || remainder < &BigDecimal::from(0)
        || remainder >= &BigDecimal::from(1)
    {
        return Err(AppError::Validation(
            "invalid margin interest inputs".to_owned(),
        ));
    }
    let raw = borrowed * rate * BigDecimal::from(hours) + remainder;
    let delta = generated_amount(&raw, precision, "margin interest increment")?;
    let remainder = raw - &delta;
    ensure_decimal_storage(&remainder, 26, 26, "margin interest remainder")?;
    Ok((delta, remainder))
}

/// 时间推进与计费使用同一个小时数，极端时间溢出必须拒绝，不能多收费而只推进较短窗口。
pub(crate) fn billed_window_end(from: DateTime<Utc>, hours: u64) -> AppResult<DateTime<Utc>> {
    let hours = i64::try_from(hours)
        .map_err(|_| AppError::Validation("margin interest duration overflow".to_owned()))?;
    let duration = chrono::TimeDelta::try_hours(hours)
        .ok_or_else(|| AppError::Validation("margin interest duration overflow".to_owned()))?;
    let end = from
        .checked_add_signed(duration)
        .ok_or_else(|| AppError::Validation("margin interest checkpoint overflow".to_owned()))?;
    crate::time::ensure_timestamp_storage(&end, "margin interest checkpoint")?;
    Ok(end)
}

#[cfg(test)]
#[path = "../../../../tests/unit_src/src_modules_margin_interest_tests.rs"]
mod tests;
