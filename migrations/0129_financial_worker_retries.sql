CREATE TABLE financial_worker_retries (
    task_kind VARCHAR(32) NOT NULL,
    item_id BIGINT UNSIGNED NOT NULL,
    attempt_count BIGINT UNSIGNED NOT NULL DEFAULT 0,
    last_attempt_at DATETIME(6) NULL,
    next_attempt_at DATETIME(6) NOT NULL,
    outcome VARCHAR(32) NOT NULL DEFAULT 'ready',
    lease_token CHAR(36) NULL,
    PRIMARY KEY (task_kind, item_id),
    INDEX idx_financial_worker_retry_due (task_kind, next_attempt_at, last_attempt_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
  COMMENT='资金后台任务持久化重试调度，不替代业务状态与资金幂等';
