//! 手动释放与自动扫描共用的资格谓词；表别名固定为 unlocks/positions，时间必须绑定。
pub(super) const UNLOCK_IDENTITY_SQL: &str = r#"unlocks.status = 'pending'
    AND unlocks.user_id = positions.user_id AND unlocks.asset_id = positions.asset_id
    AND unlocks.unlock_quantity > 0 AND positions.status = 'active'
    AND positions.remaining_amount >= unlocks.unlock_quantity"#;
pub(super) const UNLOCK_MATURITY_SQL: &str = r#"(
    (positions.listing_project_id IS NULL AND positions.unlock_at <= ?)
    OR (positions.listing_project_id IS NOT NULL AND EXISTS (
        SELECT 1 FROM new_coin_projects listing_project
        WHERE listing_project.id = positions.listing_project_id
          AND listing_project.asset_id = positions.asset_id
          AND listing_project.lifecycle_status = 'listed'
          AND listing_project.actual_listed_at IS NOT NULL
          AND listing_project.actual_listed_at <= ?
    )))"#;
pub(super) const UNLOCK_FEE_EVIDENCE_SQL: &str = r#"(
                    unlocks.unlock_fee_enabled = false
                    OR (
                        unlocks.fee_paid_status = 'not_required'
                        AND unlocks.unlock_fee_asset IS NOT NULL
                        AND unlocks.unlock_fee_amount = 0
                    )
                    OR (
                        unlocks.fee_paid_status = 'paid'
                        AND unlocks.unlock_fee_amount > 0
                        AND unlocks.fee_paid_at IS NOT NULL
                        AND unlocks.unlock_fee_payment_ledger_id IS NOT NULL
                        AND EXISTS (
                            SELECT 1 FROM wallet_ledger ledger
                            WHERE ledger.id = unlocks.unlock_fee_payment_ledger_id
                              AND ledger.user_id = unlocks.user_id
                              AND ledger.asset_id = unlocks.unlock_fee_asset
                              AND ledger.change_type = 'new_coin_unlock_fee_payment'
                              AND ledger.amount = -unlocks.unlock_fee_amount
                              AND ledger.balance_type = 'available'
                              AND ledger.ref_type = 'new_coin_unlock'
                              AND ledger.ref_id = unlocks.idempotency_key
                        )
                        AND EXISTS (
                            SELECT 1 FROM platform_financial_journal journal
                            WHERE journal.transaction_key = CONCAT('new_coin_unlock_fee:', unlocks.id)
                              AND journal.context = 'new_coin_unlock_fee'
                              AND journal.account_code = 'user_unlock_fee_expense'
                              AND journal.asset_id = unlocks.unlock_fee_asset
                              AND journal.amount = -unlocks.unlock_fee_amount
                              AND journal.ref_type = 'new_coin_unlock'
                              AND journal.ref_id = CAST(unlocks.id AS CHAR)
                        )
                        AND EXISTS (
                            SELECT 1 FROM platform_financial_journal journal
                            WHERE journal.transaction_key = CONCAT('new_coin_unlock_fee:', unlocks.id)
                              AND journal.context = 'new_coin_unlock_fee'
                              AND journal.account_code = 'platform_unlock_fee_revenue'
                              AND journal.asset_id = unlocks.unlock_fee_asset
                              AND journal.amount = unlocks.unlock_fee_amount
                              AND journal.ref_type = 'new_coin_unlock'
                              AND journal.ref_id = CAST(unlocks.id AS CHAR)
                        )
                    )
                 )"#;
pub(crate) const UNLOCK_NOT_READY: &str =
    "unlock is not releasable until unlock time is reached and required fee is paid";
