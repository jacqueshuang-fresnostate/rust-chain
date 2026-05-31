CREATE TABLE spot_orders (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    pair_id BIGINT UNSIGNED NOT NULL,
    side VARCHAR(16) NOT NULL,
    order_type VARCHAR(16) NOT NULL,
    price DECIMAL(38,18) NULL,
    quantity DECIMAL(38,18) NOT NULL,
    filled_quantity DECIMAL(38,18) NOT NULL DEFAULT 0,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    idempotency_key VARCHAR(255) NULL UNIQUE,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_spot_orders_user_time (user_id, created_at),
    INDEX idx_spot_orders_pair_status (pair_id, status),
    CONSTRAINT fk_spot_orders_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_spot_orders_pair FOREIGN KEY (pair_id) REFERENCES trading_pairs(id)
);

CREATE TABLE spot_trades (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    pair_id BIGINT UNSIGNED NOT NULL,
    buy_order_id BIGINT UNSIGNED NOT NULL,
    sell_order_id BIGINT UNSIGNED NOT NULL,
    price DECIMAL(38,18) NOT NULL,
    quantity DECIMAL(38,18) NOT NULL,
    fee DECIMAL(38,18) NOT NULL DEFAULT 0,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_spot_trades_pair_time (pair_id, created_at),
    CONSTRAINT fk_spot_trades_pair FOREIGN KEY (pair_id) REFERENCES trading_pairs(id),
    CONSTRAINT fk_spot_trades_buy_order FOREIGN KEY (buy_order_id) REFERENCES spot_orders(id),
    CONSTRAINT fk_spot_trades_sell_order FOREIGN KEY (sell_order_id) REFERENCES spot_orders(id)
);

CREATE TABLE order_events (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    order_id BIGINT UNSIGNED NOT NULL,
    event_type VARCHAR(64) NOT NULL,
    payload_json JSON NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_order_events_order_time (order_id, created_at),
    CONSTRAINT fk_order_events_order FOREIGN KEY (order_id) REFERENCES spot_orders(id)
);
