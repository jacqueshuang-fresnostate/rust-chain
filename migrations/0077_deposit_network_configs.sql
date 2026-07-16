CREATE TABLE deposit_network_configs (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '充值网络配置：记录主键 ID',
    network VARCHAR(32) NOT NULL COMMENT '充值网络配置：网络标识，当前支持 eth/base/tron/btc/solana',
    display_name VARCHAR(64) NOT NULL COMMENT '充值网络配置：前后台显示名称',
    address_group_code VARCHAR(64) NOT NULL COMMENT '充值网络配置：地址集合编号，同一编号共用一类地址',
    address_group_name VARCHAR(128) NULL COMMENT '充值网络配置：地址集合名称，例如 EVM、Bitcoin、Tron',
    asset_symbols_json JSON NULL COMMENT '充值网络配置：该网络支持充值的资产符号列表，空表示不限',
    status VARCHAR(32) NOT NULL DEFAULT 'active' COMMENT '充值网络配置：状态，active 启用，disabled 停用',
    sort_order INT NOT NULL DEFAULT 0 COMMENT '充值网络配置：排序值',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '充值网络配置：创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6) COMMENT '充值网络配置：更新时间',
    UNIQUE KEY uq_deposit_network_configs_network (network),
    INDEX idx_deposit_network_configs_group (address_group_code),
    INDEX idx_deposit_network_configs_status_sort (status, sort_order),
    CONSTRAINT chk_deposit_network_configs_status CHECK (status IN ('active', 'disabled')),
    CONSTRAINT chk_deposit_network_configs_network CHECK (network IN ('eth', 'base', 'tron', 'btc', 'solana'))
) COMMENT='充值网络配置';

INSERT INTO deposit_network_configs
    (network, display_name, address_group_code, address_group_name, asset_symbols_json, status, sort_order)
VALUES
    ('eth', 'Ethereum', 'A', 'EVM', JSON_ARRAY('ETH', 'USDT', 'USDC'), 'active', 10),
    ('base', 'Base', 'A', 'EVM', JSON_ARRAY('ETH', 'USDT', 'USDC'), 'active', 20),
    ('btc', 'Bitcoin', 'B', 'Bitcoin', JSON_ARRAY('BTC', 'USDT'), 'active', 30),
    ('tron', 'Tron', 'C', 'Tron', JSON_ARRAY('TRX', 'USDT', 'USDC'), 'active', 40),
    ('solana', 'Solana', 'D', 'Solana', JSON_ARRAY('SOL', 'USDT', 'USDC'), 'active', 50);

ALTER TABLE deposit_address_pool
    ADD COLUMN address_group_code VARCHAR(64) NULL COMMENT '充值地址池：地址集合编号，同一编号可被多个充值网络共用' AFTER network;

UPDATE deposit_address_pool
SET address_group_code = CASE network
    WHEN 'eth' THEN 'A'
    WHEN 'base' THEN 'A'
    WHEN 'btc' THEN 'B'
    WHEN 'tron' THEN 'C'
    WHEN 'solana' THEN 'D'
    ELSE network
END
WHERE address_group_code IS NULL;

ALTER TABLE deposit_address_pool
    MODIFY COLUMN address_group_code VARCHAR(64) NOT NULL COMMENT '充值地址池：地址集合编号，同一编号可被多个充值网络共用';

ALTER TABLE deposit_address_pool
    ADD INDEX idx_deposit_address_pool_assignment_group (assigned_user_id, address_group_code, assigned_asset_symbol),
    ADD INDEX idx_deposit_address_pool_status_group (status, address_group_code, asset_symbol);
