-- 一对一在线客服：用户级持久会话、不可变文本消息、双侧已读游标与发送幂等。

CREATE TABLE support_conversations (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '客服会话ID',
    user_id BIGINT UNSIGNED NOT NULL COMMENT '会话所属用户ID',
    assigned_agent_id BIGINT UNSIGNED NULL COMMENT '当前直属接待代理ID，未分配时为空',
    status VARCHAR(16) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'open' COMMENT '会话状态：open或closed',
    user_read_message_id BIGINT UNSIGNED NULL COMMENT '用户端已读到的最大消息ID',
    staff_read_message_id BIGINT UNSIGNED NULL COMMENT '客服端已读到的最大消息ID',
    last_message_id BIGINT UNSIGNED NULL COMMENT '最后一条消息ID',
    last_message_sender_type VARCHAR(16) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL COMMENT '最后消息发送者类型',
    last_message_sender_id BIGINT UNSIGNED NULL COMMENT '最后消息发送者ID',
    last_message_preview VARCHAR(512) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL COMMENT '最后消息文本预览',
    last_message_at DATETIME(6) NULL COMMENT '最后消息提交时间',
    closed_at DATETIME(6) NULL COMMENT '最近一次关闭时间',
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '创建时间',
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
        ON UPDATE CURRENT_TIMESTAMP(6) COMMENT '更新时间',
    UNIQUE KEY uk_support_conversations_user (user_id),
    INDEX idx_support_conversations_agent_queue
        (assigned_agent_id, status, last_message_at, id),
    INDEX idx_support_conversations_admin_queue
        (status, last_message_at, id),
    CONSTRAINT fk_support_conversations_user
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT fk_support_conversations_agent
        FOREIGN KEY (assigned_agent_id) REFERENCES agents(id) ON DELETE SET NULL,
    CONSTRAINT chk_support_conversations_status
        CHECK (status IN ('open', 'closed')),
    CONSTRAINT chk_support_conversations_last_sender
        CHECK (last_message_sender_type IS NULL
            OR last_message_sender_type IN ('user', 'agent', 'admin'))
) ENGINE=InnoDB
  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
  COMMENT='用户唯一的持久在线客服会话';

CREATE TABLE support_messages (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '客服消息ID',
    conversation_id BIGINT UNSIGNED NOT NULL COMMENT '所属客服会话ID',
    sender_type VARCHAR(16) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL COMMENT '发送者类型：user、agent或admin',
    sender_id BIGINT UNSIGNED NOT NULL COMMENT '发送者在对应身份表中的ID',
    client_message_id VARCHAR(64) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL COMMENT '客户端发送幂等标识',
    body TEXT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL COMMENT '不可变的纯文本消息内容',
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '提交时间',
    UNIQUE KEY uk_support_messages_sender_client
        (conversation_id, sender_type, sender_id, client_message_id),
    INDEX idx_support_messages_conversation_page (conversation_id, id),
    INDEX idx_support_messages_conversation_sender_page
        (conversation_id, sender_type, id),
    CONSTRAINT fk_support_messages_conversation
        FOREIGN KEY (conversation_id) REFERENCES support_conversations(id) ON DELETE CASCADE,
    CONSTRAINT chk_support_messages_sender
        CHECK (sender_type IN ('user', 'agent', 'admin')),
    CONSTRAINT chk_support_messages_body
        CHECK (CHAR_LENGTH(body) BETWEEN 1 AND 2000),
    CONSTRAINT chk_support_messages_client_id
        CHECK (CHAR_LENGTH(client_message_id) BETWEEN 8 AND 64)
) ENGINE=InnoDB
  DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
  COMMENT='不可变的在线客服文本消息';
