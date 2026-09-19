ALTER TABLE loan_products
    ADD COLUMN user_principal_limit DECIMAL(38,18) NULL,
    ADD COLUMN product_principal_capacity DECIMAL(38,18) NULL,
    ADD COLUMN deny_borrowing_while_overdue BOOLEAN NOT NULL DEFAULT FALSE,
    ADD CONSTRAINT chk_loan_user_principal_limit CHECK (user_principal_limit >= 0),
    ADD CONSTRAINT chk_loan_product_principal_capacity CHECK (product_principal_capacity >= 0);

-- Loan-local serialization avoids reversing existing order/wallet/user FK locks.
CREATE TABLE loan_user_exposure_locks (
    user_id BIGINT UNSIGNED NOT NULL PRIMARY KEY
) ENGINE=InnoDB;

CREATE INDEX idx_loan_orders_user_asset_exposure ON loan_orders (user_id, asset_id, status);
CREATE INDEX idx_loan_orders_product_asset_exposure ON loan_orders (product_id, asset_id, status);
