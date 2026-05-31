ALTER TABLE agent_commission_records
    ADD COLUMN source_id VARCHAR(64) NULL AFTER source_type;

UPDATE agent_commission_records
SET source_id = CONCAT('legacy:', id)
WHERE source_id IS NULL;

ALTER TABLE agent_commission_records
    MODIFY source_id VARCHAR(64) NOT NULL,
    ADD UNIQUE KEY uk_agent_commission_source (agent_id, source_type, source_id);
