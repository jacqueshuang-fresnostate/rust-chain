use super::*;

#[tokio::test]
async fn margin_numeric_interest_carry_batches_zero_delta_replay_and_overflow()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    close_previous_margin_worker_positions(&pool).await?;
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let first = seed_margin_position(&pool, "long", Some(&decimal("100"))).await?;
    let whole = seed_margin_position(&pool, "long", Some(&decimal("100"))).await?;
    let tiny = seed_margin_position(&pool, "long", Some(&decimal("100"))).await?;
    let overflow = seed_margin_position(&pool, "long", Some(&decimal("100"))).await?;
    for fixture in [&first, &whole, &tiny, &overflow] {
        sqlx::query("UPDATE margin_positions SET borrowed_amount=0.00000000015, hourly_interest_rate=0.00000001, interest_amount=4.000000000000000001, interest_accrued_at=?, opened_at=? WHERE id=?")
            .bind(start.naive_utc()).bind(start.naive_utc()).bind(fixture.position_id).execute(&pool).await?;
    }
    sqlx::query("UPDATE margin_positions SET status='closed' WHERE id=?")
        .bind(whole.position_id)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE assets SET precision_scale=8 WHERE id=?")
        .bind(tiny.margin_asset)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE margin_positions SET borrowed_amount=0.00000015,hourly_interest_rate=0.01 WHERE id=?")
        .bind(tiny.position_id).execute(&pool).await?;
    sqlx::query("UPDATE margin_positions SET borrowed_amount=99999999999999999999,hourly_interest_rate=1 WHERE id=?")
        .bind(overflow.position_id).execute(&pool).await?;
    let one = run_margin_interest_once(&pool, start + chrono::TimeDelta::hours(1), 100).await?;
    assert_eq!(one.accrued, 2);
    assert_eq!(one.failed, 1);
    let first_after: (BigDecimal, BigDecimal, chrono::DateTime<Utc>) = sqlx::query_as(
        "SELECT interest_amount,interest_remainder,interest_accrued_at FROM margin_positions WHERE id=?")
        .bind(first.position_id).fetch_one(&pool).await?;
    assert_eq!(first_after.0, decimal("4.000000000000000002"));
    assert_eq!(first_after.1, decimal("0.0000000000000000005"));
    assert_eq!(first_after.2, start + chrono::TimeDelta::hours(1));
    let tiny_after: (BigDecimal, BigDecimal, chrono::DateTime<Utc>) = sqlx::query_as(
        "SELECT interest_amount,interest_remainder,interest_accrued_at FROM margin_positions WHERE id=?")
        .bind(tiny.position_id).fetch_one(&pool).await?;
    assert_eq!(tiny_after.0, decimal("4.000000000000000001"));
    assert_eq!(tiny_after.1, decimal("0.0000000015"));
    assert_eq!(tiny_after.2, start + chrono::TimeDelta::hours(1));
    let overflow_after: (BigDecimal, BigDecimal, chrono::DateTime<Utc>) = sqlx::query_as(
        "SELECT interest_amount,interest_remainder,interest_accrued_at FROM margin_positions WHERE id=?")
        .bind(overflow.position_id).fetch_one(&pool).await?;
    assert_eq!(
        overflow_after,
        (decimal("4.000000000000000001"), decimal("0"), start)
    );

    sqlx::query("UPDATE margin_positions SET status='opened' WHERE id=?")
        .bind(whole.position_id)
        .execute(&pool)
        .await?;
    let two = run_margin_interest_once(&pool, start + chrono::TimeDelta::hours(2), 100).await?;
    assert_eq!(two.accrued, 3);
    assert_eq!(two.failed, 1);
    let batches: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT interest_amount,interest_remainder FROM margin_positions WHERE id=?",
    )
    .bind(first.position_id)
    .fetch_one(&pool)
    .await?;
    let whole_result: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT interest_amount,interest_remainder FROM margin_positions WHERE id=?",
    )
    .bind(whole.position_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(batches, whole_result);
    assert_eq!(batches, (decimal("4.000000000000000004"), decimal("0")));
    let replay = run_margin_interest_once(&pool, start + chrono::TimeDelta::hours(2), 100).await?;
    assert_eq!(replay.accrued, 0);
    let debt: BigDecimal =
        sqlx::query_scalar("SELECT interest_amount FROM margin_positions WHERE id=?")
            .bind(first.position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(debt, batches.0);
    Ok(())
}

#[tokio::test]
async fn margin_numeric_liquidation_uses_one_quantized_pnl_and_keeps_wallet_dust()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let redis = redis::Client::open(redis_url)?
        .get_connection_manager()
        .await?;
    close_previous_margin_worker_positions(&pool).await?;
    let fixture = seed_margin_position(&pool, "long", Some(&decimal("3"))).await?;
    sqlx::query("UPDATE assets SET precision_scale=8 WHERE id=?")
        .bind(fixture.margin_asset)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE margin_positions SET margin_amount=20.000000000000000001, notional_amount=100.00000001, interest_amount=0.010000000000000001 WHERE id=?")
        .bind(fixture.position_id).execute(&pool).await?;
    sqlx::query("UPDATE wallet_accounts SET available=80.000000000000000007,frozen=0.000000000000000002,locked=0.000000000000000003 WHERE user_id=? AND asset_id=?")
        .bind(fixture.user_id).bind(fixture.margin_asset).execute(&pool).await?;
    let now = Utc::now();
    cache_ticker(&redis, &fixture.pair_symbol, "2.5", now).await?;
    let result = run_once_with_dependencies(&pool, &redis, now, 100).await?;
    assert_eq!(result.liquidated, 1);
    assert_eq!(result.failed, 0);
    let record: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT realized_pnl,equity,payout_amount FROM margin_liquidation_records WHERE position_id=?")
        .bind(fixture.position_id).fetch_one(&pool).await?;
    assert_eq!(record.0, decimal("-16.66666666"));
    assert_eq!(record.1, decimal("3.32333334"));
    assert_eq!(record.2, record.1);
    let wallet: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available,frozen,locked FROM wallet_accounts WHERE user_id=? AND asset_id=?",
    )
    .bind(fixture.user_id)
    .bind(fixture.margin_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(wallet.0, decimal("83.323333340000000007"));
    assert_eq!(wallet.1, decimal("0.000000000000000002"));
    assert_eq!(wallet.2, decimal("0.000000000000000003"));
    let legs: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT COALESCE(SUM(amount),0),COALESCE(SUM(CASE WHEN account_code='platform_margin_trading_income' THEN amount ELSE 0 END),0) FROM platform_financial_journal WHERE transaction_key=?")
        .bind(format!("margin:{}:liquidate", fixture.position_id)).fetch_one(&pool).await?;
    assert_eq!(legs.0, decimal("0"));
    assert_eq!(legs.1, record.0);
    let replay = run_once_with_dependencies(&pool, &redis, now, 100).await?;
    assert_eq!(replay.liquidated, 0);
    Ok(())
}

#[tokio::test]
async fn margin_numeric_cross_liquidation_reuses_position_pnl_in_account_journal()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let redis = redis::Client::open(redis_url)?
        .get_connection_manager()
        .await?;
    close_previous_margin_worker_positions(&pool).await?;
    let fixture = seed_margin_position(&pool, "long", Some(&decimal("3"))).await?;
    sqlx::query("UPDATE assets SET precision_scale=8 WHERE id=?")
        .bind(fixture.margin_asset)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE margin_positions SET margin_mode='cross',wallet_scope='margin',margin_amount=20.000000000000000001,notional_amount=100.00000001,interest_amount=0.010000000000000001 WHERE id=?")
        .bind(fixture.position_id).execute(&pool).await?;
    sqlx::query("INSERT INTO margin_wallet_accounts(user_id,asset_id,available,frozen,locked) VALUES (?,?,0.000000000000000007,0.000000000000000002,0.000000000000000003)")
        .bind(fixture.user_id).bind(fixture.margin_asset).execute(&pool).await?;
    sqlx::query("INSERT INTO margin_cross_accounts(user_id,margin_asset) VALUES (?,?)")
        .bind(fixture.user_id)
        .bind(fixture.margin_asset)
        .execute(&pool)
        .await?;
    let now = Utc::now();
    cache_ticker(&redis, &fixture.pair_symbol, "2.5", now).await?;
    let result = run_once_with_dependencies(&pool, &redis, now, 100).await?;
    assert_eq!(result.liquidated, 1);
    assert_eq!(result.failed, 0);
    let record: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT realized_pnl,equity,payout_amount FROM margin_liquidation_records WHERE position_id=?")
        .bind(fixture.position_id).fetch_one(&pool).await?;
    assert_eq!(
        record,
        (decimal("-16.66666666"), decimal("3.32333334"), decimal("0"))
    );
    let account: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT last_equity,last_bad_debt FROM margin_cross_accounts WHERE user_id=? AND margin_asset=?")
        .bind(fixture.user_id).bind(fixture.margin_asset).fetch_one(&pool).await?;
    assert_eq!(account, (decimal("3.323333340000000007"), decimal("0")));
    let wallet: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available,frozen,locked FROM margin_wallet_accounts WHERE user_id=? AND asset_id=?",
    )
    .bind(fixture.user_id)
    .bind(fixture.margin_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        wallet,
        (
            decimal("0"),
            decimal("0.000000000000000002"),
            decimal("0.000000000000000003")
        )
    );
    let legs: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT COALESCE(SUM(amount),0),COALESCE(SUM(CASE WHEN account_code='platform_margin_trading_income' THEN amount ELSE 0 END),0),COALESCE(SUM(CASE WHEN account_code='platform_margin_liquidation_income' THEN amount ELSE 0 END),0) FROM platform_financial_journal WHERE asset_id=?")
        .bind(fixture.margin_asset).fetch_one(&pool).await?;
    assert_eq!(legs, (decimal("0"), record.0, -account.0));
    let untouched_spot: BigDecimal =
        sqlx::query_scalar("SELECT available FROM wallet_accounts WHERE user_id=? AND asset_id=?")
            .bind(fixture.user_id)
            .bind(fixture.margin_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(untouched_spot, decimal("80"));
    Ok(())
}
