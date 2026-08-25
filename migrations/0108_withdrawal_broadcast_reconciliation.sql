-- 广播歧义状态必须保留冻结，并以稳定 request id 查询后再决定确认或释放。
ALTER TABLE wallet_chain_gateways
    ADD COLUMN withdrawal_status_url VARCHAR(1000) NULL AFTER broadcast_url;

ALTER TABLE wallet_withdrawal_requests
    ADD COLUMN broadcast_error_class VARCHAR(40) NULL AFTER failure_reason,
    ADD COLUMN broadcast_last_error VARCHAR(500) NULL AFTER broadcast_error_class,
    ADD COLUMN broadcast_resolution VARCHAR(40) NULL AFTER broadcast_last_error,
    ADD COLUMN acceptance_evidence_at TIMESTAMP(6) NULL AFTER broadcast_resolution,
    ADD COLUMN gateway_query_count INT UNSIGNED NOT NULL DEFAULT 0 AFTER retry_count,
    ADD COLUMN last_gateway_query_at TIMESTAMP(6) NULL AFTER gateway_query_count,
    ADD COLUMN manual_review_at TIMESTAMP(6) NULL AFTER last_gateway_query_at;

CREATE TABLE wallet_withdrawal_broadcast_audits (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    withdrawal_id BIGINT UNSIGNED NOT NULL,
    gateway_request_id CHAR(36) NOT NULL,
    event_key VARCHAR(160) NOT NULL,
    event_type VARCHAR(40) NOT NULL,
    outcome_class VARCHAR(40) NOT NULL,
    tx_hash VARCHAR(255) NULL,
    detail VARCHAR(500) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_wallet_withdrawal_broadcast_audit (withdrawal_id, event_key),
    INDEX idx_wallet_withdrawal_broadcast_audit_request (gateway_request_id, created_at),
    CONSTRAINT fk_wallet_withdrawal_broadcast_audit_request
        FOREIGN KEY (withdrawal_id) REFERENCES wallet_withdrawal_requests(id)
        ON DELETE CASCADE
);

-- 存量库可能含旧版未被当前状态机识别的文本，或“已广播但无哈希”之类矛盾快照。
-- 迁移不猜测远端是否受理，一律保留冻结并收敛到人工审核，再加约束避免发布被历史数据卡住。
UPDATE wallet_withdrawal_requests
SET status = 'manual_review',
    manual_review_at = COALESCE(manual_review_at, CURRENT_TIMESTAMP(6)),
    failure_reason = COALESCE(failure_reason, 'legacy withdrawal state requires manual review'),
    next_attempt_at = NULL
WHERE status NOT IN (
        'pending_review', 'approved', 'broadcasting', 'unknown_broadcast',
        'broadcasted', 'confirmed', 'manual_review', 'rejected', 'failed'
    )
   OR (status IN ('broadcasted', 'confirmed') AND tx_hash IS NULL)
   OR (status IN (
           'pending_review', 'approved', 'broadcasting', 'unknown_broadcast',
           'rejected', 'failed'
       ) AND tx_hash IS NOT NULL);

-- 任何已持久化交易哈希都是不可逆的远端受理证据；后续相反回执只能转人工，不能自动退冻。
UPDATE wallet_withdrawal_requests
SET acceptance_evidence_at = COALESCE(
        acceptance_evidence_at, broadcast_at, confirmed_at, created_at
    )
WHERE tx_hash IS NOT NULL
  AND acceptance_evidence_at IS NULL;

-- 数据库拒绝应用状态机之外的值，并把“已受理/权威未受理”与交易哈希证据保持一致。
ALTER TABLE wallet_withdrawal_requests
    ADD CONSTRAINT chk_wallet_withdrawal_status_v2 CHECK (
        status IN (
            'pending_review', 'approved', 'broadcasting', 'unknown_broadcast',
            'broadcasted', 'confirmed', 'manual_review', 'rejected', 'failed'
        )
    ),
    ADD CONSTRAINT chk_wallet_withdrawal_error_class CHECK (
        broadcast_error_class IS NULL OR broadcast_error_class IN (
            'deterministic_rejected', 'unknown', 'retryable_before_acceptance'
        )
    ),
    ADD CONSTRAINT chk_wallet_withdrawal_resolution CHECK (
        broadcast_resolution IS NULL OR broadcast_resolution IN (
            'authoritative_not_accepted', 'accepted'
        )
    ),
    ADD CONSTRAINT chk_wallet_withdrawal_chain_evidence CHECK (
        (status NOT IN ('broadcasted', 'confirmed') OR tx_hash IS NOT NULL)
        AND (status IN ('broadcasted', 'confirmed', 'manual_review') OR tx_hash IS NULL)
        AND (tx_hash IS NULL OR acceptance_evidence_at IS NOT NULL)
        AND (acceptance_evidence_at IS NULL OR status IN ('broadcasted', 'confirmed', 'manual_review'))
        AND (broadcast_resolution <> 'accepted' OR tx_hash IS NOT NULL)
        AND (broadcast_resolution <> 'authoritative_not_accepted'
             OR (tx_hash IS NULL AND acceptance_evidence_at IS NULL))
    );
