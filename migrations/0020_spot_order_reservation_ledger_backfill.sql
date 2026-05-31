UPDATE spot_orders orders
LEFT JOIN (
    SELECT
        CAST(ref_id AS UNSIGNED) AS order_id,
        asset_id,
        SUM(CASE WHEN balance_type = 'frozen' THEN amount ELSE 0 END) AS frozen_amount
    FROM wallet_ledger
    WHERE ref_type = 'spot_order'
      AND change_type = 'spot_freeze'
      AND ref_id REGEXP '^[0-9]+$'
    GROUP BY CAST(ref_id AS UNSIGNED), asset_id
) freezes ON freezes.order_id = orders.id
SET orders.reserved_asset = COALESCE(orders.reserved_asset, freezes.asset_id),
    orders.reserved_amount = freezes.frozen_amount
WHERE orders.status IN ('pending', 'open', 'partially_filled')
  AND freezes.frozen_amount IS NOT NULL
  AND (
      orders.reserved_asset IS NULL
      OR orders.reserved_amount IS NULL
      OR orders.reserved_amount <> freezes.frozen_amount
  );
