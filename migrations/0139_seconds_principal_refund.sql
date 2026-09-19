CREATE TABLE seconds_refund_policies (
    product_id BIGINT UNSIGNED PRIMARY KEY,
    version BIGINT UNSIGNED NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    wait_seconds INT UNSIGNED NULL DEFAULT NULL,
    CONSTRAINT fk_seconds_refund_policy_product FOREIGN KEY (product_id) REFERENCES seconds_contract_products(id),
    CONSTRAINT chk_seconds_refund_policy CHECK (
        version > 0 AND ((enabled = FALSE AND wait_seconds IS NULL) OR
                        (enabled = TRUE AND wait_seconds IS NOT NULL)))
);

CREATE TABLE seconds_refund_policy_revisions (
    product_id BIGINT UNSIGNED NOT NULL,
    version BIGINT UNSIGNED NOT NULL,
    enabled BOOLEAN NOT NULL,
    wait_seconds INT UNSIGNED NULL,
    admin_id BIGINT UNSIGNED NOT NULL,
    reason VARCHAR(512) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (product_id, version),
    CONSTRAINT fk_seconds_refund_revision_product FOREIGN KEY (product_id) REFERENCES seconds_contract_products(id),
    CONSTRAINT fk_seconds_refund_revision_admin FOREIGN KEY (admin_id) REFERENCES admin_users(id),
    CONSTRAINT chk_seconds_refund_revision CHECK (
        version > 0 AND CHAR_LENGTH(TRIM(reason)) > 0 AND
        ((enabled = FALSE AND wait_seconds IS NULL) OR
         (enabled = TRUE AND wait_seconds IS NOT NULL)))
);

CREATE TABLE seconds_order_refund_snapshots (
    order_id BIGINT UNSIGNED PRIMARY KEY,
    product_id BIGINT UNSIGNED NOT NULL,
    policy_version BIGINT UNSIGNED NOT NULL,
    wait_seconds INT UNSIGNED NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_seconds_refund_snapshot_order FOREIGN KEY (order_id) REFERENCES seconds_contract_orders(id),
    CONSTRAINT fk_seconds_refund_snapshot_revision FOREIGN KEY (product_id, policy_version)
        REFERENCES seconds_refund_policy_revisions(product_id, version)
);

CREATE TABLE seconds_principal_refunds (
    order_id BIGINT UNSIGNED PRIMARY KEY,
    admin_id BIGINT UNSIGNED NOT NULL,
    idempotency_key VARCHAR(128) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NOT NULL,
    idempotency_key_digest CHAR(64) CHARACTER SET ascii COLLATE ascii_general_ci
        GENERATED ALWAYS AS (SHA2(idempotency_key, 256)) STORED,
    debit_ledger_id BIGINT UNSIGNED NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    amount DECIMAL(38,18) NOT NULL,
    policy_version BIGINT UNSIGNED NOT NULL,
    original_order JSON NOT NULL,
    eligibility_evidence JSON NOT NULL,
    reason VARCHAR(512) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uk_seconds_refund_request (admin_id, idempotency_key_digest),
    UNIQUE KEY uk_seconds_refund_debit (debit_ledger_id),
    CONSTRAINT fk_seconds_refund_order FOREIGN KEY (order_id) REFERENCES seconds_contract_orders(id),
    CONSTRAINT fk_seconds_refund_admin FOREIGN KEY (admin_id) REFERENCES admin_users(id),
    CONSTRAINT fk_seconds_refund_debit FOREIGN KEY (debit_ledger_id) REFERENCES wallet_ledger(id),
    CONSTRAINT fk_seconds_refund_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_seconds_refund_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT chk_seconds_refund_amount CHECK (amount > 0),
    CONSTRAINT chk_seconds_refund_reason CHECK (CHAR_LENGTH(TRIM(reason)) > 0)
);
