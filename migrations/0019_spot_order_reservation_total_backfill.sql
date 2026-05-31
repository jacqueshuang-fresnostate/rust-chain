UPDATE spot_orders orders
INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
SET orders.reserved_asset = CASE
        WHEN orders.side = 'buy' THEN pairs.quote_asset
        ELSE pairs.base_asset
    END,
    orders.reserved_amount = CASE
        WHEN orders.side = 'buy' AND orders.price IS NOT NULL
            THEN orders.quantity * orders.price
        WHEN orders.side = 'sell'
            THEN orders.quantity
        ELSE orders.reserved_amount
    END
WHERE orders.status IN ('pending', 'open', 'partially_filled')
  AND orders.side IN ('buy', 'sell')
  AND (
      orders.reserved_asset IS NULL
      OR orders.reserved_amount IS NULL
      OR (orders.side = 'buy' AND orders.price IS NOT NULL AND orders.reserved_amount <> orders.quantity * orders.price)
      OR (orders.side = 'sell' AND orders.reserved_amount <> orders.quantity)
  );
