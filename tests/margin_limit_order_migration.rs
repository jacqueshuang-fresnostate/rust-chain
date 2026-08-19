const MIGRATION: &str = include_str!("../migrations/0106_margin_limit_orders.sql");
const DASHBOARD_QUERY: &str =
    include_str!("../src/modules/admin/infrastructure/dashboard_audit.rs");
const POSITION_QUERIES: &str =
    include_str!("../src/modules/margin/infrastructure/position_queries.rs");
const FILL_QUERY: &str = include_str!("../src/modules/margin/infrastructure/positions.rs");

#[test]
fn margin_limit_order_migration_backfills_and_constrains_order_intent() {
    for required in [
        "ADD COLUMN order_type VARCHAR(16) NULL",
        "ADD COLUMN limit_price DECIMAL(38,18) NULL",
        "SET order_type = 'market'",
        "NOT NULL DEFAULT 'market'",
        "CHECK (order_type IN ('market', 'limit'))",
        "order_type = 'market' AND limit_price IS NULL",
        "order_type = 'limit' AND limit_price > 0",
        "idx_margin_positions_limit_trigger",
        "(pair_id, status, order_type, entry_price, direction, limit_price, id)",
    ] {
        assert!(
            MIGRATION.contains(required),
            "missing margin limit migration contract: {required}"
        );
    }
}

#[test]
fn pending_margin_limits_are_excluded_from_position_and_interest_aggregates() {
    assert!(
        DASHBOARD_QUERY.contains("WHERE status = 'opened' AND entry_price IS NOT NULL"),
        "dashboard holdings must exclude pending margin limits"
    );
    assert!(
        POSITION_QUERIES.contains("push_admin_margin_position_filters(builder, user_id, email.clone(), pair_id, status, true)"),
        "interest rows and group counts must share the filled-only predicate"
    );
    assert!(
        POSITION_QUERIES.contains("builder.push(\" AND entry_price IS NOT NULL\")"),
        "the filled-only predicate must be expressed in SQL"
    );
}

#[test]
fn margin_limit_fill_resets_real_open_and_interest_times_together() {
    assert!(FILL_QUERY.contains(
        "SET entry_price = ?, opened_at = CURRENT_TIMESTAMP(6),\n               interest_accrued_at = CURRENT_TIMESTAMP(6)"
    ));
}
