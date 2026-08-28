CREATE TABLE margin_position_close_executions (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    position_id BIGINT UNSIGNED NOT NULL,
    idempotency_key VARCHAR(128) NOT NULL,
    close_percentage SMALLINT UNSIGNED NOT NULL,
    close_margin_amount DECIMAL(38,18) NOT NULL,
    close_notional_amount DECIMAL(38,18) NOT NULL,
    close_borrowed_amount DECIMAL(38,18) NOT NULL,
    close_interest_amount DECIMAL(38,18) NOT NULL,
    exit_price DECIMAL(38,18) NOT NULL,
    realized_pnl DECIMAL(38,18) NOT NULL,
    settlement_amount DECIMAL(38,18) NOT NULL,
    fully_closed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_margin_close_executions_user_key (user_id, idempotency_key),
    INDEX idx_margin_close_executions_position_time (position_id, created_at, id),
    CONSTRAINT fk_margin_close_executions_user
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE RESTRICT,
    CONSTRAINT fk_margin_close_executions_position
        FOREIGN KEY (position_id) REFERENCES margin_positions(id) ON DELETE RESTRICT,
    CONSTRAINT chk_margin_close_executions_percentage
        CHECK (close_percentage BETWEEN 1 AND 100),
    CONSTRAINT chk_margin_close_executions_amounts
        CHECK (close_margin_amount > 0
           AND close_notional_amount > 0
           AND close_borrowed_amount >= 0
           AND close_interest_amount >= 0),
    CONSTRAINT chk_margin_close_executions_exit_price CHECK (exit_price > 0)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
