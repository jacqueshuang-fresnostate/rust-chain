ALTER TABLE agents
    ADD COLUMN parent_agent_id BIGINT UNSIGNED NULL AFTER user_id,
    ADD COLUMN root_agent_id BIGINT UNSIGNED NULL AFTER parent_agent_id,
    ADD COLUMN path VARCHAR(2048) NULL AFTER level;

-- 历史代理没有父子关系，统一作为独立的一级总代理保留原有团队归属。
UPDATE agents
SET parent_agent_id = NULL,
    root_agent_id = id,
    level = 1,
    path = CONCAT('/agent:', id);

ALTER TABLE agents
    MODIFY COLUMN path VARCHAR(2048) NOT NULL,
    ADD INDEX idx_agents_parent_status (parent_agent_id, status),
    ADD INDEX idx_agents_root_level (root_agent_id, level),
    ADD INDEX idx_agents_path (path(255)),
    ADD CONSTRAINT fk_agents_parent
        FOREIGN KEY (parent_agent_id) REFERENCES agents(id),
    ADD CONSTRAINT fk_agents_root
        FOREIGN KEY (root_agent_id) REFERENCES agents(id) ON DELETE SET NULL,
    ADD CONSTRAINT chk_agents_level
        CHECK (level BETWEEN 1 AND 3);
