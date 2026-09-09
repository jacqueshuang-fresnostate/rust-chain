use super::*;
use sqlx::mysql::MySqlPoolOptions;

#[test]
fn financial_audit_understands_real_ledger_reference_formats() {
    let source = include_str!("../../src/modules/admin/infrastructure/financial_idempotency.rs");

    assert!(source.contains("ledger.ref_id NOT REGEXP '^[0-9]+:[0-9]+$'"));
    assert!(source.contains(
        "trades.buy_order_id = CAST(SUBSTRING_INDEX(ledger.ref_id, ':', 1) AS UNSIGNED)"
    ));
    assert!(source.contains(
        "trades.sell_order_id = CAST(SUBSTRING_INDEX(ledger.ref_id, ':', -1) AS UNSIGNED)"
    ));
    assert!(
        !source.contains("spot_trades trades WHERE trades.id = CAST(ledger.ref_id AS UNSIGNED)")
    );
    for ref_type in [
        "loan_liquidation",
        "wallet_deposit_event",
        "quick_recharge",
        "deposit_record",
        "withdraw_record",
    ] {
        assert!(
            source.contains(ref_type),
            "missing ledger reference audit for {ref_type}"
        );
    }
}

/// CI 注入已完成迁移的 DATABASE_URL；本地没有真实 MySQL 时保持普通单元测试可运行。
/// 一旦提供连接串，整条跨表审计 SQL 必须能被当前 schema 执行和解码。
#[tokio::test]
async fn financial_idempotency_query_runs_against_migrated_mysql() {
    if std::env::var("RUN_REAL_DEPENDENCY_SMOKE").as_deref() != Ok("1") {
        eprintln!("skipping financial audit query test because RUN_REAL_DEPENDENCY_SMOKE is not 1");
        return;
    }
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL is required when real dependency smoke is enabled");
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("connect migrated MySQL for financial audit");

    let record = load_admin_financial_idempotency_audit(&pool)
        .await
        .expect("execute financial idempotency audit query");
    for value in [
        record.duplicate_idempotency_groups,
        record.missing_idempotency_keys,
        record.orphan_ledger_entries,
        record.expired_seconds_orders,
        record.expired_prediction_orders,
        record.pending_loan_orders,
        record.pending_new_coin_subscriptions,
    ] {
        assert!(value >= 0, "audit counts must be non-negative");
    }

    pool.close().await;
}
