use super::*;
use sqlx::mysql::MySqlPoolOptions;
use uuid::Uuid;

#[test]
fn reconciliation_only_links_distributions_to_matching_business_identity() {
    let source = include_str!("../../src/modules/admin/infrastructure/new_coin_reconciliation.rs");

    for invariant in [
        "subscriptions.project_id = distributions.project_id",
        "subscriptions.user_id = distributions.user_id",
        "distributions.asset_id = projects.asset_id",
        "subscriptions.quote_asset = projects.quote_asset_id",
    ] {
        assert!(
            source.contains(invariant),
            "missing distribution link invariant: {invariant}"
        );
    }
    assert!(source.contains("invalid_subscription_link_count"));
}

/// 在 CI 的最新迁移 schema 中创建最小空项目，确保全部聚合和 Decimal 解码真实执行。
/// 本地未配置 DATABASE_URL 时跳过，避免普通纯单元测试强制依赖守护进程。
#[tokio::test]
async fn new_coin_reconciliation_query_runs_against_migrated_mysql() {
    if std::env::var("RUN_REAL_DEPENDENCY_SMOKE").as_deref() != Ok("1") {
        eprintln!(
            "skipping new coin reconciliation query test because RUN_REAL_DEPENDENCY_SMOKE is not 1"
        );
        return;
    }
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL is required when real dependency smoke is enabled");
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("connect migrated MySQL for new coin reconciliation");
    let suffix = Uuid::now_v7().simple().to_string();
    let asset_symbol = format!("RC{}", &suffix[..12]).to_ascii_uppercase();
    let project_symbol = format!("{}-USDT", asset_symbol);

    let asset_id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 8, 'coin', 'active')",
    )
    .bind(&asset_symbol)
    .bind(format!("Reconciliation {suffix}"))
    .execute(&pool)
    .await
    .expect("insert reconciliation fixture asset")
    .last_insert_id();
    let project_id = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, unlock_type,
            status, reserved_supply, allocated_supply, remaining_supply)
           VALUES (?, ?, 'preheat', 100, 1, 'fixed_time', 'active', 0, 0, 100)"#,
    )
    .bind(asset_id)
    .bind(&project_symbol)
    .execute(&pool)
    .await
    .expect("insert reconciliation fixture project")
    .last_insert_id();

    let record = load_admin_new_coin_reconciliation(&pool, project_id)
        .await
        .expect("execute new coin reconciliation query");
    assert_eq!(record.project_id, project_id);
    assert_eq!(record.symbol, project_symbol);
    assert_eq!(record.subscription_count, 0);
    assert_eq!(record.distribution_quantity, BigDecimal::from(0));

    sqlx::query("DELETE FROM new_coin_projects WHERE id = ?")
        .bind(project_id)
        .execute(&pool)
        .await
        .expect("delete reconciliation fixture project");
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await
        .expect("delete reconciliation fixture asset");
    pool.close().await;
}
