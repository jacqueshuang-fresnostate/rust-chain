UPDATE spot_orders orders
INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
SET orders.reserved_asset = CASE
        WHEN orders.side = 'buy' THEN pairs.quote_asset
        ELSE pairs.base_asset
    END
WHERE orders.reserved_asset IS NULL
  AND orders.status IN ('pending', 'open', 'partially_filled')
  AND orders.side IN ('buy', 'sell');
