ALTER TABLE loan_orders
    ADD COLUMN overdue_at TIMESTAMP(6) NULL COMMENT '逾期标记时间' AFTER due_at,
    ADD INDEX idx_loan_orders_status_due (status, due_at);

ALTER TABLE loan_orders
    DROP CHECK chk_loan_orders_status,
    MODIFY COLUMN status VARCHAR(32) NOT NULL DEFAULT 'pending' COMMENT '订单状态：pending待审核，disbursed已放款，rejected已拒绝，cancelled已取消，repaid已还款，overdue已逾期';

ALTER TABLE loan_orders
    ADD CONSTRAINT chk_loan_orders_status CHECK (status IN ('pending', 'disbursed', 'rejected', 'cancelled', 'repaid', 'overdue'));
