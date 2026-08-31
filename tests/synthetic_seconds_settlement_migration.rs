const MIGRATION: &str = include_str!("../migrations/0119_synthetic_seconds_settlement_safety.sql");

#[test]
fn synthetic_seconds_settlement_migration_is_additive_and_auditable() {
    let uppercase = MIGRATION.to_ascii_uppercase();
    for destructive in ["DROP TABLE", "DROP COLUMN", "TRUNCATE TABLE", "DELETE FROM"] {
        assert!(
            !uppercase.contains(destructive),
            "0119 must remain additive and immutable: {destructive}"
        );
    }
    for required in [
        "strategy_id BIGINT UNSIGNED NULL",
        "strategy_version INT NULL",
        "uq_market_price_ticks_strategy_event",
        "(strategy_id, observed_at)",
        "chk_market_price_ticks_strategy_identity_pair",
        "chk_market_price_ticks_strategy_identity_values",
        "settlement_failure_code VARCHAR(64) NULL",
        "settlement_failed_at TIMESTAMP(6) NULL",
        "settlement_window_start TIMESTAMP(6) NULL",
        "settlement_window_end TIMESTAMP(6) NULL",
        "chk_seconds_contract_orders_manual_review_evidence",
        "CREATE TABLE seconds_contract_settlement_exceptions",
        "UNIQUE KEY uq_seconds_contract_settlement_exceptions_order (order_id)",
        "CHECK (window_end > window_start)",
    ] {
        assert!(
            MIGRATION.contains(required),
            "0119 is missing settlement safety contract: {required}"
        );
    }
}
