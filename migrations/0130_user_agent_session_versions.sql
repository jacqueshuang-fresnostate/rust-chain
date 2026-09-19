ALTER TABLE users
    ADD COLUMN auth_session_version BIGINT UNSIGNED NOT NULL DEFAULT 0
        COMMENT '账号安全变更的会话代际';
ALTER TABLE agent_admin_users
    ADD COLUMN auth_session_version BIGINT UNSIGNED NOT NULL DEFAULT 0
        COMMENT '代理账号安全变更及上级停用的会话代际';
ALTER TABLE login_two_factor_challenges
    ADD COLUMN auth_session_version BIGINT UNSIGNED NOT NULL DEFAULT 0
        COMMENT '密码验证时的用户会话代际，禁止旧挑战升级凭证';
