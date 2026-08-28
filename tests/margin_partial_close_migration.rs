const MIGRATION: &str = include_str!("../migrations/0117_margin_partial_close.sql");

#[test]
fn partial_close_migration_owns_an_immutable_idempotent_execution_record() {
    for fragment in [
        "CREATE TABLE margin_position_close_executions",
        "UNIQUE KEY uq_margin_close_executions_user_key (user_id, idempotency_key)",
        "close_percentage SMALLINT UNSIGNED NOT NULL",
        "close_margin_amount DECIMAL(38,18) NOT NULL",
        "close_notional_amount DECIMAL(38,18) NOT NULL",
        "close_borrowed_amount DECIMAL(38,18) NOT NULL",
        "close_interest_amount DECIMAL(38,18) NOT NULL",
        "settlement_amount DECIMAL(38,18) NOT NULL",
        "CHECK (close_percentage BETWEEN 1 AND 100)",
        "DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
    ] {
        assert!(
            MIGRATION.contains(fragment),
            "missing migration contract: {fragment}"
        );
    }
}
