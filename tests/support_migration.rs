const MIGRATION: &str = include_str!("../migrations/0105_agent_routed_online_support.sql");

#[test]
fn support_migration_is_additive_and_encodes_the_durable_contract() {
    assert_eq!(MIGRATION.matches("CREATE TABLE support_").count(), 2);
    for destructive_statement in ["ALTER TABLE", "DROP TABLE", "TRUNCATE", "DELETE FROM"] {
        assert!(
            !MIGRATION.contains(destructive_statement),
            "0105 must remain an additive immutable migration: {destructive_statement}"
        );
    }

    assert!(MIGRATION.contains("CREATE TABLE support_conversations"));
    assert!(MIGRATION.contains("UNIQUE KEY uk_support_conversations_user (user_id)"));
    assert!(MIGRATION.contains("assigned_agent_id BIGINT UNSIGNED NULL"));
    assert!(MIGRATION.contains("CHECK (status IN ('open', 'closed'))"));
    for field in [
        "user_read_message_id",
        "staff_read_message_id",
        "last_message_id",
        "last_message_sender_type",
        "last_message_sender_id",
        "last_message_preview",
        "last_message_at",
    ] {
        assert!(
            MIGRATION.contains(field),
            "missing conversation field {field}"
        );
    }
    assert!(MIGRATION.contains(
        "INDEX idx_support_conversations_agent_queue\n        (assigned_agent_id, status, last_message_at, id)"
    ));
    assert!(MIGRATION.contains(
        "INDEX idx_support_conversations_admin_queue\n        (status, last_message_at, id)"
    ));
    assert!(MIGRATION.contains("FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE"));
    assert!(
        MIGRATION
            .contains("FOREIGN KEY (assigned_agent_id) REFERENCES agents(id) ON DELETE SET NULL")
    );

    assert!(MIGRATION.contains("CREATE TABLE support_messages"));
    assert!(MIGRATION.contains("client_message_id VARCHAR(64)"));
    assert!(MIGRATION.contains("body TEXT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"));
    assert!(MIGRATION.contains(
        "UNIQUE KEY uk_support_messages_sender_client\n        (conversation_id, sender_type, sender_id, client_message_id)"
    ));
    assert!(
        MIGRATION.contains("INDEX idx_support_messages_conversation_page (conversation_id, id)")
    );
    assert!(MIGRATION.contains(
        "FOREIGN KEY (conversation_id) REFERENCES support_conversations(id) ON DELETE CASCADE"
    ));
    assert!(MIGRATION.contains("CHECK (CHAR_LENGTH(body) BETWEEN 1 AND 2000)"));

    assert_eq!(
        MIGRATION
            .matches("DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci")
            .count(),
        2
    );
    assert_eq!(MIGRATION.matches("ENGINE=InnoDB").count(), 2);
    assert!(!MIGRATION.contains("utf8mb3"));
    assert!(!MIGRATION.contains("latin1"));
}
