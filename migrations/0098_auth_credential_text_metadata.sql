ALTER TABLE users
    MODIFY COLUMN password_hash VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL,
    MODIFY COLUMN status VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active';

ALTER TABLE admin_users
    MODIFY COLUMN password_hash VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL,
    MODIFY COLUMN status VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active';

ALTER TABLE agent_admin_users
    MODIFY COLUMN password_hash VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL,
    MODIFY COLUMN status VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active';
