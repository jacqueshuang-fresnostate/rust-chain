UPDATE margin_positions
SET borrowed_amount = GREATEST(notional_amount - margin_amount, 0),
    interest_accrued_at = COALESCE(interest_accrued_at, opened_at)
WHERE status = 'opened'
  AND borrowed_amount = 0;
