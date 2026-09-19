-- Keep request text decodable by SQLx while retaining exact-byte uniqueness.
ALTER TABLE agent_commission_reversals
    DROP INDEX uk_commission_reversal_request,
    MODIFY COLUMN idempotency_key VARCHAR(128) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NOT NULL,
    ADD COLUMN idempotency_key_digest CHAR(64) CHARACTER SET ascii COLLATE ascii_general_ci
        GENERATED ALWAYS AS (SHA2(idempotency_key, 256)) STORED,
    ADD UNIQUE KEY uk_commission_reversal_request (admin_id, idempotency_key_digest);
