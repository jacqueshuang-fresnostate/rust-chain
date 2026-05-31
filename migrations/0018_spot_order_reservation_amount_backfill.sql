UPDATE spot_orders orders
INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
LEFT JOIN (
    SELECT buy_order_id AS order_id, SUM(price * quantity) AS spent_amount
    FROM spot_trades
    GROUP BY buy_order_id
) buy_fills ON buy_fills.order_id = orders.id
SET orders.reserved_asset = CASE
        WHEN orders.side = 'buy' THEN pairs.quote_asset
        ELSE pairs.base_asset
    END,
    orders.reserved_amount = CASE
        WHEN orders.side = 'buy' AND orders.price IS NOT NULL
            THEN orders.quantity * orders.price
        WHEN orders.side = 'buy' AND orders.price IS NULL
            THEN COALESCE(buy_fills.spent_amount, 0)
        WHEN orders.side = 'sell'
            THEN orders.quantity
        ELSE orders.reserved_amount
    END
WHERE orders.status IN ('pending', 'open', 'partially_filled')
  AND orders.side IN ('buy', 'sell')
  AND (orders.reserved_asset IS NULL OR orders.reserved_amount IS NULL);
