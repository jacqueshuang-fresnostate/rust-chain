CREATE TABLE quick_recharge_configs (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '快速充值配置：记录主键 ID',
    name VARCHAR(64) NOT NULL UNIQUE DEFAULT 'default' COMMENT '快速充值配置：配置名称，默认配置为 default',
    provider VARCHAR(32) NOT NULL DEFAULT 'gmpay' COMMENT '快速充值配置：支付服务商，目前支持 GMPay/Epusdt',
    enabled BOOLEAN NOT NULL DEFAULT FALSE COMMENT '快速充值配置：是否启用 PC 端快速充值入口',
    api_base_url VARCHAR(512) NULL COMMENT '快速充值配置：Epusdt/GMPay API 基础地址',
    merchant_pid VARCHAR(128) NULL COMMENT '快速充值配置：GMPay 商户 PID',
    merchant_secret_ciphertext TEXT NULL COMMENT '快速充值配置：GMPay 商户 Secret Key 加密密文',
    merchant_secret_mask VARCHAR(64) NULL COMMENT '快速充值配置：GMPay 商户 Secret Key 脱敏显示值',
    currency VARCHAR(16) NOT NULL DEFAULT 'cny' COMMENT '快速充值配置：用户输入充值金额的法币币种，如 cny/usd',
    token VARCHAR(32) NOT NULL DEFAULT 'usdt' COMMENT '快速充值配置：到账资产符号，如 USDT',
    network VARCHAR(32) NOT NULL DEFAULT 'tron' COMMENT '快速充值配置：GMPay 收款网络，如 tron/ethereum/solana',
    notify_url VARCHAR(512) NULL COMMENT '快速充值配置：GMPay 支付成功异步回调地址',
    redirect_url VARCHAR(512) NULL COMMENT '快速充值配置：支付完成后的同步跳转地址',
    min_amount DECIMAL(36, 18) NOT NULL DEFAULT 0.010000000000000000 COMMENT '快速充值配置：单笔最小充值金额',
    max_amount DECIMAL(36, 18) NULL COMMENT '快速充值配置：单笔最大充值金额；为空表示不限制',
    updated_by BIGINT UNSIGNED NULL COMMENT '快速充值配置：最后更新该配置的管理员 ID',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '快速充值配置：创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6) COMMENT '快速充值配置：更新时间',
    INDEX idx_quick_recharge_configs_updated_by (updated_by),
    CONSTRAINT fk_quick_recharge_configs_updated_by FOREIGN KEY (updated_by) REFERENCES admin_users(id),
    CONSTRAINT chk_quick_recharge_configs_provider CHECK (provider IN ('gmpay')),
    CONSTRAINT chk_quick_recharge_configs_amount CHECK (min_amount > 0 AND (max_amount IS NULL OR max_amount >= min_amount))
) COMMENT='快速充值配置';

CREATE TABLE quick_recharge_orders (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '快速充值订单：记录主键 ID',
    order_id VARCHAR(32) NOT NULL UNIQUE COMMENT '快速充值订单：平台侧商户订单号，传给 GMPay',
    user_id BIGINT UNSIGNED NOT NULL COMMENT '快速充值订单：发起充值的用户 ID',
    user_email VARCHAR(255) NULL COMMENT '快速充值订单：用户邮箱快照，便于后台查账',
    asset_id BIGINT UNSIGNED NOT NULL COMMENT '快速充值订单：到账资产 ID',
    asset_symbol VARCHAR(32) NOT NULL COMMENT '快速充值订单：到账资产符号',
    currency VARCHAR(16) NOT NULL COMMENT '快速充值订单：用户提交的法币币种',
    token VARCHAR(32) NOT NULL COMMENT '快速充值订单：GMPay 实际收款币种',
    network VARCHAR(32) NOT NULL COMMENT '快速充值订单：GMPay 实际收款网络',
    fiat_amount DECIMAL(36, 18) NOT NULL COMMENT '快速充值订单：用户提交的法币充值金额',
    actual_amount DECIMAL(36, 18) NULL COMMENT '快速充值订单：GMPay 返回的实际需支付加密货币数量，也是入账数量',
    provider_trade_id VARCHAR(128) NULL UNIQUE COMMENT '快速充值订单：GMPay 交易号',
    receive_address VARCHAR(255) NULL COMMENT '快速充值订单：GMPay 分配的链上收款地址',
    payment_url VARCHAR(1024) NULL COMMENT '快速充值订单：GMPay 收银台支付链接',
    expiration_time BIGINT NULL COMMENT '快速充值订单：GMPay 订单过期秒级时间戳',
    status VARCHAR(32) NOT NULL DEFAULT 'created' COMMENT '快速充值订单：状态，created 已创建，pending 待支付，paid 已支付，failed 失败，expired 已过期',
    block_transaction_id VARCHAR(255) NULL COMMENT '快速充值订单：GMPay 回调的链上交易哈希',
    callback_payload_json JSON NULL COMMENT '快速充值订单：最近一次 GMPay 回调原始数据',
    paid_at TIMESTAMP(6) NULL COMMENT '快速充值订单：确认支付并入账时间',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '快速充值订单：创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6) COMMENT '快速充值订单：更新时间',
    INDEX idx_quick_recharge_orders_user_time (user_id, created_at),
    INDEX idx_quick_recharge_orders_status_time (status, created_at),
    INDEX idx_quick_recharge_orders_asset_time (asset_id, created_at),
    CONSTRAINT fk_quick_recharge_orders_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_quick_recharge_orders_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT chk_quick_recharge_orders_status CHECK (status IN ('created', 'pending', 'paid', 'failed', 'expired')),
    CONSTRAINT chk_quick_recharge_orders_amount CHECK (fiat_amount > 0 AND (actual_amount IS NULL OR actual_amount > 0))
) COMMENT='快速充值订单';

INSERT INTO quick_recharge_configs (
    name,
    provider,
    enabled,
    currency,
    token,
    network,
    min_amount
)
VALUES (
    'default',
    'gmpay',
    FALSE,
    'cny',
    'usdt',
    'tron',
    0.010000000000000000
)
ON DUPLICATE KEY UPDATE name = name;
