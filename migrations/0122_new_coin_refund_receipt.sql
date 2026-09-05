-- 零派发只允许作为有关联申购单的全额退款收据；普通派发继续要求正数。
ALTER TABLE new_coin_distributions
    DROP CHECK chk_new_coin_distributions_quantity,
    ADD CONSTRAINT chk_new_coin_distributions_quantity CHECK (
        quantity > 0 OR (quantity = 0 AND status = 'refunded' AND subscription_id IS NOT NULL AND lock_position_id IS NULL)
    );
