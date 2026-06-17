# Polymarket Model Research

## Sources

* <https://docs.polymarket.com/>
* <https://docs.polymarket.com/llms.txt>
* <https://docs.polymarket.com/concepts/markets-events>
* <https://docs.polymarket.com/concepts/prices-orderbook>
* <https://docs.polymarket.com/concepts/positions-tokens>
* <https://docs.polymarket.com/concepts/order-lifecycle>
* <https://docs.polymarket.com/concepts/resolution>
* <https://docs.polymarket.com/market-data/fetching-markets>
* <https://docs.polymarket.com/api-reference/market-data/get-order-book>
* <https://docs.polymarket.com/api-reference/trade/post-a-new-order>
* <https://docs.polymarket.com/api-reference/trade/get-user-orders>

## Key Polymarket Concepts

* Events group one or more binary Yes/No markets.
* A market is the tradable unit and has outcome tokens, usually Yes and No.
* Prices range from 0.00 to 1.00 and represent implied probability.
* Trading happens through a CLOB: bids, asks, spread, tick size, order book snapshots, and last trade price.
* Polymarket says all orders are technically limit orders; market orders are just limit orders priced to execute immediately.
* Order time-in-force includes GTC, GTD, FOK, and FAK.
* Orders can rest, partially fill, be cancelled, expire, match, or settle.
* Positions are balances of outcome tokens. Winning outcome tokens redeem for 1.00 collateral; losing tokens redeem for 0.
* Polymarket collateral is pUSD; this project must replace that with platform virtual crypto assets.

## API Shapes To Mirror

* Public event/market discovery:
  * list events
  * fetch event by slug
  * list markets
  * fetch market by slug/token
* Market data:
  * order book by token/outcome
  * midpoint price
  * market price by side
  * last trade price
  * price history
* Trading:
  * post order
  * cancel order(s)
  * get user orders
  * get trades
* Realtime:
  * market channel for order book/price/trade updates
  * user channel for private order/trade updates

## Repo Mapping

* Use existing `wallet_accounts` and `wallet_ledger` patterns for virtual asset accounting.
* Reuse order-number display convention for user-facing IDs.
* Admin resource pages already support resource configs and side sheets.
* PC already has market/trade-style pages and private websocket refresh patterns.
* Existing seconds contract module is simpler fixed-odds betting; prediction markets should not reuse fixed payout logic directly.
* Existing spot module has order book / freeze / cancel / settlement patterns closest to a CLOB.

## Feasible Approaches

### Approach A: Local Virtual Prediction Market CLOB (Recommended)

Build a local prediction market exchange that mirrors Polymarket semantics but uses platform wallet assets. Public data can be seeded manually or synced from Polymarket. Orders match locally, positions settle locally, and admin controls allowed stake assets.

Pros:
* Fits the user requirement that betting currency is virtual crypto.
* Avoids Polymarket wallet signing, API credentials, geoblock, pUSD, bridge, on-chain settlement, and custody complexity.
* Can reuse existing wallet ledger and internal private websocket patterns.
* Gives admin control over markets, allowed currencies, settlement, and risk.

Cons:
* Liquidity is local unless the platform seeds markets or later integrates external liquidity.
* Requires implementing enough CLOB behavior locally.

### Approach B: Read Polymarket Markets, Local Bets Only

Sync Polymarket events/markets/orderbook as readonly reference data, but user bets are local platform bets with virtual assets and simplified settlement.

Pros:
* Fastest MVP with familiar Polymarket market discovery UX.
* Avoids real Polymarket trading.

Cons:
* User prices may not execute against real Polymarket liquidity.
* If market data changes or resolves externally, local settlement policy needs clear rules.

### Approach C: Proxy Real Polymarket CLOB Orders

The platform stores user virtual asset accounting but posts orders to Polymarket CLOB using platform credentials or user wallets.

Pros:
* Access to real Polymarket liquidity and order books.

Cons:
* High risk and much larger scope: wallet signing, L1/L2 auth, pUSD, on-chain settlement, geographic restrictions, compliance, balance reconciliation, and failure recovery.
* Hard to reconcile virtual asset balances with real Polymarket order settlement unless the platform acts as principal/custodian.

## Recommended MVP

Start with Approach B, per the final product decision:

* Admin configures Polymarket tags/categories to sync.
* Backend imports active Polymarket events/markets as readonly source data.
* Admin configures allowed stake assets, per-asset payout caps, default fees, invalid-market refund policy, and settlement mode.
* PC lists synced markets and shows Yes/No probability prices.
* User requests a backend quote, then places a local prediction bet with an allowed virtual asset.
* Backend freezes the stake, charges the configured fee separately, and records a local position/share order.
* No early sell/exit is supported in MVP.
* External resolution is synced from Polymarket; local wallet settlement either waits for admin confirmation or runs automatically based on configuration.
* Winning positions redeem probability shares into the same stake asset subject to configured payout caps; losing positions settle to zero.

Out of MVP:

* Real Polymarket order posting.
* Bridge/deposit/withdraw/pUSD.
* Negative risk, rewards, maker rebates, and market-maker programs.
* Admin-created local prediction markets.
* Local CLOB matching and secondary-market exits.
