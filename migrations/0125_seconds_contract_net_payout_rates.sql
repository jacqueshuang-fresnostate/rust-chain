-- `payout_rate` is a net-profit ratio: 0.4 means 40% profit and a
-- winning 100-unit stake receives 140 units in total. The BTC-USDT and
-- ETH-USDT products returned by the confirmed catalog contained the complete
-- four-cycle schedule below as gross return factors. Select the products by
-- their stable pair symbol *and* that complete schedule, rather than changing
-- every product that happens to use one of these decimal values. A custom
-- product on another pair, or a partial/custom schedule on these pairs, is
-- intentionally left untouched.

DROP TEMPORARY TABLE IF EXISTS _seconds_net_payout_targets;
DROP TEMPORARY TABLE IF EXISTS _seconds_net_payout_rate_map;

CREATE TEMPORARY TABLE _seconds_net_payout_rate_map (
    duration_seconds INT UNSIGNED PRIMARY KEY,
    gross_rate DECIMAL(18,8) NOT NULL,
    net_rate DECIMAL(18,8) NOT NULL
);

INSERT INTO _seconds_net_payout_rate_map (duration_seconds, gross_rate, net_rate) VALUES
    (60, 1.40000000, 0.40000000),
    (120, 1.50000000, 0.50000000),
    (180, 1.60000000, 0.60000000),
    (300, 1.80000000, 0.80000000);

CREATE TEMPORARY TABLE _seconds_net_payout_targets (
    product_id BIGINT UNSIGNED PRIMARY KEY
);

INSERT INTO _seconds_net_payout_targets (product_id)
SELECT products.id
FROM seconds_contract_products AS products
INNER JOIN trading_pairs AS pairs ON pairs.id = products.pair_id
INNER JOIN seconds_contract_product_cycles AS cycles
    ON cycles.product_id = products.id
INNER JOIN _seconds_net_payout_rate_map AS rates
    ON rates.duration_seconds = products.duration_seconds
WHERE pairs.symbol IN ('BTC-USDT', 'ETH-USDT')
  AND products.duration_seconds IN (60, 120, 180, 300)
  AND products.payout_rate = rates.gross_rate
GROUP BY products.id, products.duration_seconds, products.payout_rate
HAVING COUNT(*) = 4
   AND COUNT(DISTINCT cycles.duration_seconds) = 4
   AND SUM(cycles.duration_seconds = 60 AND cycles.payout_rate = 1.40000000) = 1
   AND SUM(cycles.duration_seconds = 120 AND cycles.payout_rate = 1.50000000) = 1
   AND SUM(cycles.duration_seconds = 180 AND cycles.payout_rate = 1.60000000) = 1
   AND SUM(cycles.duration_seconds = 300 AND cycles.payout_rate = 1.80000000) = 1;

-- Map each confirmed gross factor explicitly. The target table is populated
-- once per product, so both product master and cycle rows are updated from the
-- same stable snapshot. No order row is ever selected or rewritten.
UPDATE seconds_contract_product_cycles AS cycles
INNER JOIN _seconds_net_payout_targets AS targets
    ON targets.product_id = cycles.product_id
INNER JOIN _seconds_net_payout_rate_map AS rates
    ON rates.duration_seconds = cycles.duration_seconds
   AND rates.gross_rate = cycles.payout_rate
SET cycles.payout_rate = rates.net_rate;

UPDATE seconds_contract_products AS products
INNER JOIN _seconds_net_payout_targets AS targets
    ON targets.product_id = products.id
INNER JOIN _seconds_net_payout_rate_map AS rates
    ON rates.duration_seconds = products.duration_seconds
   AND rates.gross_rate = products.payout_rate
SET products.payout_rate = rates.net_rate;

DROP TEMPORARY TABLE _seconds_net_payout_targets;
DROP TEMPORARY TABLE _seconds_net_payout_rate_map;

-- Historical seconds_contract_orders rows intentionally remain unchanged:
-- each order's payout_rate is an immutable opening-time settlement snapshot.
