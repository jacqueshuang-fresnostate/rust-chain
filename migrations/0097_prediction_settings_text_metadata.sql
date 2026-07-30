ALTER TABLE prediction_settings
    MODIFY COLUMN default_settlement_mode VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'manual_confirm',
    MODIFY COLUMN default_invalid_refund_policy VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'refund_stake_and_fee',
    MODIFY COLUMN last_sync_status VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL,
    MODIFY COLUMN last_sync_error VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL;
