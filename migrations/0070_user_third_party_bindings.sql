CREATE TABLE IF NOT EXISTS user_third_party_bindings (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY COMMENT '用户第三方绑定ID',
    user_id BIGINT UNSIGNED NOT NULL COMMENT '用户ID',
    provider VARCHAR(64) NOT NULL COMMENT '绑定提供方：coinbase_wallet Coinbase钱包，telegram_account TG账号',
    account_identifier VARCHAR(255) NOT NULL COMMENT '第三方账号标识，比如钱包地址或TG用户名',
    display_name VARCHAR(255) NULL COMMENT '第三方账号显示名称',
    status VARCHAR(32) NOT NULL DEFAULT 'bound' COMMENT '绑定状态：bound已绑定，disabled已停用',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6) COMMENT '更新时间',
    UNIQUE KEY uq_user_third_party_bindings_user_provider (user_id, provider),
    KEY idx_user_third_party_bindings_provider_identifier (provider, account_identifier),
    CONSTRAINT fk_user_third_party_bindings_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT chk_user_third_party_bindings_provider CHECK (provider IN ('coinbase_wallet', 'telegram_account')),
    CONSTRAINT chk_user_third_party_bindings_status CHECK (status IN ('bound', 'disabled'))
) COMMENT='用户第三方账号绑定表';

UPDATE security_policy_configs
SET policy_value = JSON_SET(
    policy_value,
    '$.third_party_bindings',
    COALESCE(
        JSON_EXTRACT(policy_value, '$.third_party_bindings'),
        JSON_OBJECT(
            'coinbase_wallet_enabled', FALSE,
            'telegram_account_enabled', FALSE
        )
    )
)
WHERE policy_key = 'user_security_policy';
