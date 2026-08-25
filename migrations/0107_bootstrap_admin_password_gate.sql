-- 引导管理员必须在首次登录后轮换一次性口令；存量管理员保持原行为。
ALTER TABLE admin_users
    ADD COLUMN must_change_password BOOLEAN NOT NULL DEFAULT FALSE AFTER password_hash,
    ADD COLUMN password_changed_at TIMESTAMP(6) NULL AFTER must_change_password,
    ADD COLUMN auth_session_version BIGINT UNSIGNED NOT NULL DEFAULT 0 AFTER password_changed_at;

-- 会话代际同时固化到两类刷新令牌和管理员 2FA 挑战，避免改密并发窗口把旧凭据升级成新会话。
ALTER TABLE refresh_tokens
    ADD COLUMN auth_session_version BIGINT UNSIGNED NOT NULL DEFAULT 0 AFTER actor_id;

ALTER TABLE admin_login_two_factor_challenges
    ADD COLUMN auth_session_version BIGINT UNSIGNED NOT NULL DEFAULT 0 AFTER admin_id;
