CREATE TABLE user_market_favorites (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    trading_pair_id BIGINT UNSIGNED NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_user_market_favorites_user_pair (user_id, trading_pair_id),
    INDEX idx_user_market_favorites_user_time (user_id, created_at),
    CONSTRAINT fk_user_market_favorites_user
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT fk_user_market_favorites_pair
        FOREIGN KEY (trading_pair_id) REFERENCES trading_pairs(id) ON DELETE CASCADE
);
