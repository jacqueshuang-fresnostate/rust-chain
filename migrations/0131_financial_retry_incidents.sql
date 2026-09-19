CREATE TABLE financial_retry_incidents (
    task_kind VARCHAR(32) NOT NULL,
    item_id BIGINT UNSIGNED NOT NULL,
    owner_admin_id BIGINT UNSIGNED NULL,
    due_at DATETIME(3) NULL,
    version BIGINT UNSIGNED NOT NULL,
    updated_by BIGINT UNSIGNED NOT NULL,
    updated_at DATETIME(3) NOT NULL,
    PRIMARY KEY (task_kind, item_id),
    INDEX idx_financial_retry_incidents_owner_due (owner_admin_id, due_at),
    CONSTRAINT fk_financial_retry_incidents_owner FOREIGN KEY (owner_admin_id) REFERENCES admin_users(id),
    CONSTRAINT fk_financial_retry_incidents_actor FOREIGN KEY (updated_by) REFERENCES admin_users(id),
    CONSTRAINT chk_financial_retry_incidents_kind CHECK (task_kind IN ('earn', 'loan', 'commission', 'seconds')),
    CONSTRAINT chk_financial_retry_incidents_version CHECK (version > 0)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
