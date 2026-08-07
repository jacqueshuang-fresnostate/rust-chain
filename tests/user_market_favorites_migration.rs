const MIGRATION: &str = include_str!("../migrations/0100_user_market_favorites.sql");

#[test]
fn user_market_favorites_migration_keeps_unique_and_cascade_contracts() {
    assert!(MIGRATION.contains("CREATE TABLE user_market_favorites"));
    assert!(
        MIGRATION
            .contains("UNIQUE KEY uq_user_market_favorites_user_pair (user_id, trading_pair_id)")
    );
    assert!(MIGRATION.contains("FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE"));
    assert!(
        MIGRATION.contains(
            "FOREIGN KEY (trading_pair_id) REFERENCES trading_pairs(id) ON DELETE CASCADE"
        )
    );
    assert!(MIGRATION.contains("created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)"));
}
