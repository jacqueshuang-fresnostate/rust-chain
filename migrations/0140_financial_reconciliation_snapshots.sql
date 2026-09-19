-- Manual observations only: no daily close, historical backfill or balance writes.
CREATE TABLE financial_reconciliation_snapshots (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    asset_id BIGINT UNSIGNED NOT NULL,
    asset_symbol VARCHAR(64) NOT NULL,
    precision_scale INT NOT NULL,
    schema_version INT UNSIGNED NOT NULL,
    captured_at DATETIME(3) NOT NULL,
    admin_id BIGINT UNSIGNED NOT NULL,
    reason VARCHAR(500) NOT NULL,
    idempotency_key VARBINARY(128) NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    report_hash BINARY(32) NOT NULL,
    report_json JSON NOT NULL,
    UNIQUE KEY uk_reconciliation_capture_key (admin_id, key_hash),
    KEY idx_reconciliation_capture_asset (asset_id, id),
    CONSTRAINT chk_reconciliation_capture_precision CHECK (precision_scale BETWEEN 0 AND 18),
    CONSTRAINT chk_reconciliation_capture_schema CHECK (schema_version = 1),
    CONSTRAINT chk_reconciliation_capture_partial CHECK (
        JSON_UNQUOTE(JSON_EXTRACT(report_json, '$.coverage')) = 'partial'
        AND JSON_EXTRACT(report_json, '$.coverage') IS NOT NULL
    )
);

CREATE TABLE financial_reconciliation_followups (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    snapshot_id BIGINT UNSIGNED NOT NULL,
    version BIGINT UNSIGNED NOT NULL,
    owner_admin_id BIGINT UNSIGNED NULL,
    due_at DATETIME(3) NULL,
    notes VARCHAR(2000) NOT NULL,
    admin_id BIGINT UNSIGNED NOT NULL,
    reason VARCHAR(500) NOT NULL,
    recorded_at DATETIME(3) NOT NULL,
    idempotency_key VARBINARY(128) NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    UNIQUE KEY uk_reconciliation_followup_version (snapshot_id, version),
    UNIQUE KEY uk_reconciliation_followup_key (admin_id, key_hash),
    CONSTRAINT fk_reconciliation_followup_snapshot FOREIGN KEY (snapshot_id)
        REFERENCES financial_reconciliation_snapshots(id),
    CONSTRAINT chk_reconciliation_followup_version CHECK (version > 0)
);

-- No FK to live asset/admin rows: later edits/removals cannot rewrite evidence.
CREATE TRIGGER reconciliation_snapshot_no_update BEFORE UPDATE ON financial_reconciliation_snapshots
FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'Reconciliation evidence is immutable';
CREATE TRIGGER reconciliation_snapshot_no_delete BEFORE DELETE ON financial_reconciliation_snapshots
FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'Reconciliation evidence is immutable';
CREATE TRIGGER reconciliation_followup_no_update BEFORE UPDATE ON financial_reconciliation_followups
FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'Reconciliation follow-up is append-only';
CREATE TRIGGER reconciliation_followup_no_delete BEFORE DELETE ON financial_reconciliation_followups
FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'Reconciliation follow-up is append-only';
