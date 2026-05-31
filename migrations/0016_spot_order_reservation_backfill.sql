UPDATE spot_orders orders
INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
SET orders.reserved_asset = CASE
        WHEN orders.side = 'buy' THEN pairs.quote_asset
        ELSE pairs.base_asset
    END,
    orders.reserved_amount = CASE
        WHEN orders.side = 'buy' AND orders.price IS NOT NULL
            THEN (orders.quantity - orders.filled_quantity) * orders.price
        WHEN orders.side = 'sell'
            THEN orders.quantity - orders.filled_quantity
        ELSE NULL
    END
WHERE orders.reserved_asset IS NULL
  AND orders.reserved_amount IS NULL
  AND orders.status IN ('pending', 'open', 'partially_filled')
  AND (orders.side = 'sell' OR orders.price IS NOT NULL);
