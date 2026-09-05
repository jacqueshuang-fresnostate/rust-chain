-- listed_at remains the compatible configured/planned time; never infer an event from it.
ALTER TABLE new_coin_projects
    ADD COLUMN actual_listed_at TIMESTAMP(6) NULL COMMENT '实际上市事件时间，由后台命令记录' AFTER listed_at;

-- Recover only recorded listing/create events. Unknown legacy event times stay NULL.
UPDATE new_coin_projects projects
JOIN (
    SELECT project_id, MIN(created_at) AS actual_at
    FROM new_coin_lifecycle_events
    WHERE (event_type = 'new_coin_project.lifecycle.update'
           AND JSON_UNQUOTE(JSON_EXTRACT(payload_json, '$.before.lifecycle_status')) = 'distribution'
           AND JSON_UNQUOTE(JSON_EXTRACT(payload_json, '$.after.lifecycle_status')) = 'listed')
       OR (event_type = 'new_coin_project.create'
           AND JSON_UNQUOTE(JSON_EXTRACT(payload_json, '$.lifecycle_status')) = 'listed')
    GROUP BY project_id
) events ON events.project_id = projects.id
SET projects.actual_listed_at = events.actual_at
WHERE projects.lifecycle_status = 'listed';

-- Only NEW immediate-on-listing allocations opt in. Historical maturity is untouched.
ALTER TABLE asset_lock_positions
    ADD COLUMN listing_project_id BIGINT UNSIGNED NULL COMMENT '新锁仓等待实际上市的项目，NULL 沿用时间快照' AFTER unlock_at,
    ADD INDEX idx_lock_listing_project (listing_project_id),
    ADD CONSTRAINT fk_lock_listing_project FOREIGN KEY (listing_project_id) REFERENCES new_coin_projects(id);
