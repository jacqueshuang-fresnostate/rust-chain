CREATE INDEX idx_seconds_return_user_status_settled
    ON seconds_contract_orders (user_id, status, settled_at);

CREATE INDEX idx_prediction_return_user_status_settled
    ON prediction_orders (user_id, status, settled_at);

CREATE INDEX idx_margin_return_user_status_closed
    ON margin_positions (user_id, status, closed_at);

CREATE INDEX idx_earn_return_user_status_redeemed
    ON earn_subscriptions (user_id, status, redeemed_at);
