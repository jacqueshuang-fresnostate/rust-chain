-- Forward-only carry. Existing billed checkpoints and accrued debt are unchanged.
-- 18 principal decimals * 8 rate decimals require exactly 26 fractional digits.
ALTER TABLE margin_positions
    ADD COLUMN interest_remainder DECIMAL(26,26) NULL;
UPDATE margin_positions SET interest_remainder = 0 WHERE interest_remainder IS NULL;
ALTER TABLE margin_positions
    MODIFY COLUMN interest_remainder DECIMAL(26,26) NOT NULL DEFAULT 0,
    ADD CONSTRAINT chk_margin_interest_remainder
        CHECK (interest_remainder >= 0 AND interest_remainder < 1);
