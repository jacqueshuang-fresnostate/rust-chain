CREATE TABLE deposit_address_pool (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '充值地址池：记录主键 ID',
    network VARCHAR(32) NOT NULL COMMENT '充值地址池：链网络，支持 eth/base/tron/btc/solana',
    address VARCHAR(255) NOT NULL COMMENT '充值地址池：链上充值地址',
    asset_symbol VARCHAR(32) NULL COMMENT '充值地址池：限定可使用该地址的资产符号，空表示该网络任意资产可用',
    status VARCHAR(32) NOT NULL DEFAULT 'available' COMMENT '充值地址池：地址状态，available 可分配，assigned 已分配，disabled 禁用',
    assigned_user_id BIGINT UNSIGNED NULL COMMENT '充值地址池：当前分配给的用户 ID',
    assigned_user_email VARCHAR(255) NULL COMMENT '充值地址池：当前分配用户邮箱快照',
    assigned_asset_symbol VARCHAR(32) NULL COMMENT '充值地址池：当前用户申请充值的资产符号',
    assigned_at TIMESTAMP(6) NULL COMMENT '充值地址池：分配给用户的时间',
    memo VARCHAR(255) NULL COMMENT '充值地址池：地址备注或 Memo 标签',
    remark VARCHAR(512) NULL COMMENT '充值地址池：后台备注',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '充值地址池：创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6) COMMENT '充值地址池：更新时间',
    UNIQUE KEY uq_deposit_address_pool_network_address (network, address),
    INDEX idx_deposit_address_pool_assignment (assigned_user_id, network, assigned_asset_symbol),
    INDEX idx_deposit_address_pool_status_network (status, network, asset_symbol),
    CONSTRAINT fk_deposit_address_pool_user FOREIGN KEY (assigned_user_id) REFERENCES users(id),
    CONSTRAINT chk_deposit_address_pool_status CHECK (status IN ('available', 'assigned', 'disabled')),
    CONSTRAINT chk_deposit_address_pool_network CHECK (network IN ('eth', 'base', 'tron', 'btc', 'solana')),
    CONSTRAINT chk_deposit_address_pool_assignment CHECK (
        (status = 'assigned' AND assigned_user_id IS NOT NULL AND assigned_asset_symbol IS NOT NULL AND assigned_at IS NOT NULL)
        OR (status <> 'assigned' AND assigned_user_id IS NULL AND assigned_asset_symbol IS NULL AND assigned_at IS NULL)
    )
) COMMENT='充值地址池';
