ALTER TABLE earn_products
    ADD COLUMN principal_capacity DECIMAL(38,18) NULL,
    ADD COLUMN liability_capacity DECIMAL(38,18) NULL,
    ADD CONSTRAINT chk_earn_principal_capacity CHECK (principal_capacity >= 0),
    ADD CONSTRAINT chk_earn_liability_capacity CHECK (liability_capacity >= 0);

CREATE INDEX idx_earn_subscriptions_exposure
    ON earn_subscriptions (product_id, asset_id, status);

CREATE TABLE convert_inventory_accounts (
    pair_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    asset_id BIGINT UNSIGNED NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    funded_amount DECIMAL(38,18) NOT NULL DEFAULT 0,
    consumed_amount DECIMAL(38,18) NOT NULL DEFAULT 0,
    revision BIGINT UNSIGNED NOT NULL DEFAULT 1,
    CONSTRAINT fk_convert_inventory_pair FOREIGN KEY (pair_id) REFERENCES convert_pairs(id),
    CONSTRAINT fk_convert_inventory_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT chk_convert_inventory_balance CHECK (
        funded_amount >= 0 AND consumed_amount >= 0 AND consumed_amount <= funded_amount
    )
) ENGINE=InnoDB;

CREATE TABLE convert_inventory_funding_audits (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    pair_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    admin_id BIGINT UNSIGNED NOT NULL,
    revision BIGINT UNSIGNED NOT NULL,
    enabled BOOLEAN NOT NULL,
    funded_before DECIMAL(38,18) NOT NULL,
    funded_after DECIMAL(38,18) NOT NULL,
    consumed_amount DECIMAL(38,18) NOT NULL,
    funding_reference VARCHAR(255) NOT NULL,
    reason VARCHAR(512) NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_convert_inventory_funding_revision (pair_id, revision),
    CONSTRAINT fk_convert_funding_inventory FOREIGN KEY (pair_id) REFERENCES convert_inventory_accounts(pair_id)
) ENGINE=InnoDB;

CREATE TABLE convert_inventory_allocations (
    quote_id CHAR(36) CHARACTER SET ascii COLLATE ascii_bin NOT NULL PRIMARY KEY,
    pair_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    amount DECIMAL(38,18) NOT NULL,
    consumed_before DECIMAL(38,18) NOT NULL,
    consumed_after DECIMAL(38,18) NOT NULL,
    funded_amount DECIMAL(38,18) NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY idx_convert_inventory_allocations_pair (pair_id),
    CONSTRAINT fk_convert_allocation_inventory FOREIGN KEY (pair_id) REFERENCES convert_inventory_accounts(pair_id),
    CONSTRAINT chk_convert_inventory_allocation CHECK (
        amount > 0 AND consumed_after = consumed_before + amount
        AND consumed_before >= 0 AND consumed_after <= funded_amount
    )
) ENGINE=InnoDB;
