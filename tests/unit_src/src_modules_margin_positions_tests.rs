/// 借款利率必须在借款开始时固化到持仓上，否则管理员改配产品利率会追溯整个未计费窗口。
#[test]
fn opening_a_position_snapshots_the_hourly_interest_rate() {
    let source = include_str!("../../src/modules/margin/infrastructure/positions.rs");

    assert!(source.contains("borrowed_amount, hourly_interest_rate, interest_amount"));
    assert!(source.contains(".bind(&product.hourly_interest_rate)"));
}

/// 限价单在成交才真正开始借款，因此快照要取成交时刻的产品利率，而不是下单时刻的。
#[test]
fn filling_a_limit_order_resnapshots_the_rate_from_the_product() {
    let source = include_str!("../../src/modules/margin/infrastructure/positions.rs");

    assert!(source.contains("positions.hourly_interest_rate = products.hourly_interest_rate"));
}

#[test]
fn margin_positions_store_a_backfilled_rate_snapshot() {
    let migration =
        include_str!("../../migrations/0128_margin_position_interest_rate_snapshot.sql");

    assert!(migration.contains("ADD COLUMN hourly_interest_rate"));
    assert!(
        migration.contains("margin_products"),
        "existing positions must be backfilled from the product they were opened on"
    );
}
