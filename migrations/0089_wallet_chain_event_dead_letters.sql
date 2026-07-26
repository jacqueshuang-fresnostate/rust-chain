-- 链上事件确定性失败进入死信表并推进游标，避免单条坏事件永久阻塞整个网络的充提。
CREATE TABLE wallet_chain_event_dead_letters (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    gateway_id BIGINT UNSIGNED NOT NULL,
    network VARCHAR(64) NOT NULL,
    event_kind VARCHAR(16) NOT NULL,
    dedup_key VARCHAR(512) NOT NULL,
    request_id VARCHAR(128) NULL,
    tx_hash VARCHAR(255) NULL,
    event_index INT UNSIGNED NULL,
    payload_json JSON NOT NULL,
    failure_reason VARCHAR(500) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_wallet_chain_dead_letters_dedup (dedup_key),
    INDEX idx_wallet_chain_dead_letters_network_time (network, created_at),
    CONSTRAINT chk_wallet_chain_dead_letters_kind CHECK (event_kind IN ('deposit', 'withdrawal'))
);
