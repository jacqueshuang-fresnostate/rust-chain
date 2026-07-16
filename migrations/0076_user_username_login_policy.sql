ALTER TABLE users
    ADD COLUMN username VARCHAR(64) NULL COMMENT '用户登录用户名，唯一，标准化小写' AFTER id,
    ADD UNIQUE KEY uq_users_username (username);

UPDATE security_policy_configs
SET policy_value = JSON_SET(
    policy_value,
    '$.username_login_enabled',
    COALESCE(JSON_EXTRACT(policy_value, '$.username_login_enabled'), FALSE)
)
WHERE policy_key = 'user_security_policy';
