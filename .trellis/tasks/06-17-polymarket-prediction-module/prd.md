# brainstorm: polymarket-style prediction module

## Goal

Build a Polymarket-style prediction/guessing module for this exchange platform. Users should be able to browse prediction events/markets, place orders or bets with platform virtual crypto assets instead of Polymarket pUSD, and view positions/orders. Admins should be able to configure which virtual assets can be used for betting.

## What I Already Know

* User wants a dedicated guessing/prediction module.
* UX and API semantics should reference Polymarket.
* Official documentation source: <https://docs.polymarket.com>.
* The betting currency must be platform virtual assets, not Polymarket pUSD.
* Admin must configure which virtual assets are allowed for betting.
* This project already has wallet accounts, wallet ledger, admin resources, PC pages, private websocket, order-number display conventions, and multiple product/order modules.

## Assumptions (Temporary)

* MVP should not custody real Polymarket funds or place real on-chain Polymarket orders unless explicitly required.
* MVP should model prediction markets locally, while optionally syncing public Polymarket market data.
* Platform virtual asset settlement should use existing wallet freeze/settle ledger patterns.
* Admin-created/configured markets can coexist with Polymarket-synced markets in a future phase, but are not in MVP.

## Open Questions

* None. Awaiting final implementation confirmation.

## Requirements (Evolving)

* Admin can configure allowed stake assets for prediction betting using a global allowlist with optional per-market overrides.
* PC has a prediction/guessing module inspired by Polymarket.
* Backend exposes public market/event data and authenticated order/position APIs.
* Order and wallet accounting must avoid internal database IDs in user-facing displays.
* MVP does not support selling/exiting a prediction position before resolution; users hold Yes/No bets until settlement.
* Admin configures Polymarket tags/categories to sync, and the platform syncs active markets under those configured categories.
* Winning prediction bets use probability-share settlement with payout caps: accepted shares are calculated from stake amount and accepted probability price; winning shares redeem at 1 unit of the same stake asset per share, but payout is limited by configured risk caps.
* Losing prediction bets settle to 0.
* Payout caps are configured as global per-asset defaults with optional per-market overrides.
* The order-acceptance risk check must reject orders whose theoretical winning payout exceeds the effective cap for the stake asset and market.
* Local bet pricing uses backend-issued quotes. The PC requests a quote for a market, outcome, stake asset, and stake amount; the backend returns a `quote_id`, accepted probability price, calculated shares, theoretical payout, cap check result, and short expiry time.
* Placing a prediction bet must submit a valid `quote_id`; the backend must not trust a probability price submitted directly by the frontend.
* Quotes are bound to the requesting user and order parameters, expire quickly, and are single-use after successful order creation.
* Prediction betting charges a platform fee based on stake amount percentage.
* Fee configuration uses a global default fee rate with optional per-market overrides.
* Fees are charged at successful order creation and recorded separately from stake freeze and settlement payout in wallet ledger/history.
* The effective allowed stake assets for a market use the market override when present; otherwise they fall back to the global allowed stake asset list.
* If a market is canceled, invalid, or cannot resolve to YES/NO, the backend applies a configurable default invalid-market refund policy.
* Admin can dynamically switch the default invalid-market refund policy without code changes. Supported policies are refund stake plus fee, refund stake only, and manual selection during invalid settlement.
* Invalid-market settlement records the refund policy that was actually used, so later configuration changes do not rewrite historical settlement meaning.
* Polymarket market data sync supports admin-configured intervals and manual "sync now" triggering.
* Admin can enable/disable the prediction market sync job, configure the interval, view last sync status, and inspect sync errors.
* MVP only supports Polymarket-synced prediction markets; admin-created local prediction markets are out of scope for the first version.
* Prediction market rows should preserve a `source`/external identifier model so future local/admin-created markets can be added without rewriting the core order and settlement model.
* Market resolution supports switching between two backend modes:
  * Sync external Polymarket result, then require admin confirmation before local wallet settlement.
  * Sync external Polymarket result and automatically settle local wallets.
* The safer default settlement mode is external result plus admin confirmation.
* Settlement mode configuration uses a global default with optional per-market overrides.

## Acceptance Criteria (Evolving)

* [ ] Admin can enable/disable allowed betting assets.
* [ ] Admin can override allowed stake assets for an individual prediction market.
* [ ] User can list prediction events/markets.
* [ ] Only markets from admin-configured Polymarket tags/categories are shown.
* [ ] MVP does not expose admin-created local prediction market creation.
* [ ] Imported market records preserve source/external identifiers for future extensibility.
* [ ] User can place a prediction order/bet with an allowed virtual asset.
* [ ] Wallet balances are frozen/settled with ledger records.
* [ ] User can view orders and positions.
* [ ] User cannot sell/exit prediction positions before market settlement in MVP.
* [ ] Winning settlement follows probability-share redemption and enforces configured payout caps.
* [ ] Bets that would exceed configured payout caps are rejected with a clear validation error before freezing funds.
* [ ] Admin can configure default payout caps per allowed stake asset.
* [ ] Admin can override payout caps for an individual prediction market.
* [ ] User order creation requires a backend-issued quote id instead of a frontend-submitted price.
* [ ] Expired, reused, mismatched, or cap-exceeding quotes are rejected before wallet funds are frozen.
* [ ] Admin can configure a global default prediction betting fee rate and override it for an individual prediction market.
* [ ] Successful order creation charges the configured fee based on stake amount and records the fee separately from stake and settlement ledger entries.
* [ ] Admin can configure and dynamically switch the default invalid-market refund policy.
* [ ] Canceled, invalid, or unresolvable markets apply the configured refund policy idempotently and record stake and fee refund ledger entries separately.
* [ ] Invalid-market settlement records the policy used for audit and user-facing order history.
* [ ] Admin can configure Polymarket sync interval, enable/disable syncing, and manually trigger sync now.
* [ ] The system records last sync status, last successful sync time, imported/updated counts, and error messages for admin troubleshooting.
* [ ] Admin can switch settlement behavior between "external result + manual confirmation" and "external result + automatic settlement".
* [ ] Admin can configure a global default settlement mode and override it for an individual prediction market.
* [ ] The system records external resolution data separately from local settlement status for auditability.
* [ ] Automatic settlement and manual confirmation settlement both use the same idempotent wallet settlement path.
* [ ] PC and backend mappings are covered by tests.

## Definition of Done

* Tests added/updated for backend contracts and PC adapters.
* Type-check/lint/test commands pass for touched layers.
* Docs/spec/progress updated.
* Migration is additive and old applied migrations are not modified.

## Out of Scope (Temporary)

* Real-money compliance, KYC gating, or geographic restrictions beyond existing platform policies.
* Real Polymarket wallet signing / on-chain settlement until the integration direction is confirmed.
* Secondary market selling / early exit before resolution.
* Admin-created local prediction markets in MVP; the first version only supports Polymarket-synced markets.
* Full market-maker tooling, negative-risk markets, bridge/deposit flows, and rewards in MVP.

## Research References

* Polymarket documentation index: <https://docs.polymarket.com/llms.txt>
* [`research/polymarket-model.md`](research/polymarket-model.md) — maps Polymarket concepts/API shapes to this project's virtual-asset wallet model.

## Research Notes

### What Similar Tools Do

* Polymarket structures predictions as events containing one or more binary Yes/No markets.
* Prices are 0.00 to 1.00 probability prices.
* Trading happens through a CLOB with bids, asks, spreads, tick sizes, market/limit behavior, and user order/trade history.
* Positions are outcome-token balances, and resolved winning outcomes redeem for 1.00 collateral per token.
* Polymarket's collateral is pUSD; this platform must replace that settlement asset with configured virtual crypto assets.

### Constraints From This Repo

* Wallet accounting must use existing `wallet_accounts` / `wallet_ledger` style freeze, settle, and reference metadata.
* Admin pages are mostly resource-driven and can expose prediction markets/assets via resource configs.
* PC already has exchange-like pages, user order pages, i18n, backend adapters, and private websocket refresh patterns.
* Existing seconds-contract logic is fixed-payout betting; Polymarket-style markets are order-book/position based and should not be reduced to fixed odds.

### Feasible Approaches

**Approach A: Local Virtual Prediction Market CLOB (Recommended)**

* How it works: mirror Polymarket market/order concepts locally, use platform virtual assets for collateral, match orders locally, and settle locally after admin resolution.
* Pros: matches the virtual-currency requirement, keeps admin control, avoids real Polymarket custody/auth/geoblock complexity, reuses current wallet ledger.
* Cons: liquidity is local unless the platform imports/seeds liquidity later.

**Approach B: Read Polymarket Markets, Local Bets Only**

* How it works: sync/display Polymarket markets and prices, but user betting is a simplified local contract against platform liquidity.
* Pros: fastest UI/data MVP.
* Cons: weaker Polymarket parity; local settlement policy can diverge from external orderbook behavior.

**Approach C: Proxy Real Polymarket CLOB Orders**

* How it works: users place orders here, backend posts real CLOB orders to Polymarket.
* Pros: real Polymarket liquidity.
* Cons: requires wallet signing, L1/L2 auth, pUSD, bridge/on-chain settlement, geoblock/compliance, reconciliation, and custody design.

## Decision (ADR-lite)

**Context**: The user wants a Polymarket-style prediction module, but bets should use platform virtual crypto assets configured by admin rather than Polymarket pUSD.

**Decision**: Use Approach B — sync/reference Polymarket market content and prices, but execute bets locally with platform virtual assets.

**Consequences**:

* The PC experience can look and feel close to Polymarket, with real-world Polymarket events, market titles, outcome labels, and probability-style prices.
* User funds, orders, and settlement remain inside this platform's wallet/ledger system.
* We avoid real Polymarket CLOB authentication, wallet signing, pUSD, bridge, on-chain settlement, geoblock, and reconciliation complexity in MVP.
* Local betting rules must be explicit because execution is not actually entering Polymarket's order book.

## Settlement Decision

**Context**: The platform will not route MVP bets into Polymarket's real CLOB. Users still expect Polymarket-style probability pricing, but the platform must control virtual-asset payout risk.

**Decision**: Use probability-share settlement with payout caps. For an accepted order, `shares = stake_amount / accepted_probability_price`. When the market resolves, winning shares redeem at `1` unit of the same stake asset per share, capped by configured platform risk limits; losing shares settle to `0`.

**Consequences**:

* The user-facing model remains close to Polymarket's "price as probability, winning token redeems to 1" mental model.
* The platform can reject orders whose theoretical payout exceeds the configured cap before funds are frozen.
* Admin payout-cap configuration is now required before implementation, because the cap dimension affects schema, API validation, and admin UI.

## Risk Cap Decision

**Context**: Local bets are settled by this platform rather than by Polymarket, so every accepted bet creates platform payout exposure in the selected virtual asset.

**Decision**: Configure payout caps as global per-asset defaults with optional per-market overrides. The effective cap for an order is the market override when present; otherwise it falls back to the selected stake asset's global cap.

**Consequences**:

* Admin can set a simple default risk profile for each allowed betting asset.
* High-risk or high-volume markets can use stricter market-specific limits without duplicating every asset setting globally.
* Backend validation must calculate theoretical payout at order time and reject orders that exceed the effective cap before wallet funds are frozen.

## Resolution Decision

**Context**: Market outcomes will be referenced from Polymarket, but local settlement changes this platform's wallet balances. The platform needs both an operationally safe mode and a low-touch automated mode.

**Decision**: Support switching between external-result manual confirmation and external-result automatic settlement. The default mode is external-result manual confirmation. The switch is configured as a global default with optional per-market overrides. In manual mode, synced Polymarket resolution moves the market to a pending-confirmation state; an admin action performs local settlement. In automatic mode, a trusted synced resolution triggers local settlement directly.

**Consequences**:

* Operators can use manual confirmation for high-risk launches, then switch to automatic settlement after confidence improves.
* Most markets can follow the global default while high-risk markets opt into stricter manual confirmation.
* The backend must store external resolution state separately from local settlement state so repeated syncs do not double-settle orders.
* Wallet settlement must be idempotent and auditable in both modes.

## Quote Decision

**Context**: Local prediction bets use synced Polymarket probability data, but order creation freezes and later settles this platform's virtual assets. The backend must prevent stale, tampered, or replayed prices from creating wallet exposure.

**Decision**: Use backend-issued quotes. The PC requests a quote for the selected market, YES/NO outcome, stake asset, and stake amount. The backend calculates the accepted probability price, shares, theoretical payout, and effective cap, then returns a short-lived `quote_id`. Order creation requires the valid `quote_id`; quotes are bound to the user and exact order parameters, expire quickly, and are consumed after successful order creation.

**Consequences**:

* The frontend never controls the final accepted probability price.
* Risk-cap validation happens before wallet funds are frozen.
* The backend needs quote persistence or a signed quote mechanism with replay protection.

## Fee Decision

**Context**: Prediction bets are local platform products using virtual assets, so the platform needs a configurable revenue model and clear wallet accounting.

**Decision**: Charge a platform fee at successful order creation based on a percentage of the stake amount. The fee configuration uses a global default rate with optional per-market overrides. The fee is recorded separately from the stake freeze and later settlement payout.

**Consequences**:

* Fee behavior is predictable for users because it is known before order submission.
* Admin can run a default platform-wide fee while adjusting individual markets when needed.
* Wallet history needs a distinct prediction fee ledger/source type so PC and admin transaction records do not mix fees with stake or payout movements.

## Stake Asset Decision

**Context**: The platform supports multiple virtual assets, but not every prediction market should necessarily accept every enabled asset.

**Decision**: Configure allowed stake assets with a global allowlist and optional per-market overrides. If a market has its own allowlist, quotes and orders can only use that market list; otherwise they fall back to the global allowed assets.

**Consequences**:

* Admin can quickly enable common assets across all prediction markets.
* Individual high-risk or campaign-specific markets can restrict staking to selected assets.
* Quote creation and order creation must both validate the effective allowed asset list so unsupported assets cannot be used.

## Invalid Refund Decision

**Context**: Some synced prediction markets may be canceled, marked invalid, or become impossible to resolve to YES/NO. Those outcomes should not be hardcoded because platform operations may need different refund behavior over time.

**Decision**: Configure a global default invalid-market refund policy in admin settings and allow it to be switched dynamically. Supported policies are refund stake plus fee, refund stake only, and manual selection during invalid settlement. The policy used for each invalid settlement is persisted with the settlement record.

**Consequences**:

* Operators can change the default abnormal-settlement behavior without code changes.
* Wallet ledger entries must distinguish stake refunds from fee refunds.
* Settlement must store the effective policy used so historical orders remain auditable even after the default configuration changes.

## Sync Decision

**Context**: The platform should show active markets from admin-configured Polymarket tags/categories, but operators need control over freshness, failures, and manual troubleshooting.

**Decision**: Support admin-configured sync intervals plus a manual "sync now" action. Admin can enable or disable the sync job, choose the sync interval, and view sync status including last run time, last successful run time, imported/updated counts, and errors.

**Consequences**:

* Operators can tune freshness without deployments.
* Manual sync gives a simple recovery path after changing categories/tags or investigating stale markets.
* The backend needs sync job state and sync log/audit records, not just imported market rows.

## Market Source Decision

**Context**: Supporting both Polymarket-synced and fully admin-created local markets in the first version would add extra admin creation forms, validation, image/content management, and local-only resolution paths.

**Decision**: MVP only supports Polymarket-synced markets from admin-configured categories/tags. Admin-created local markets are out of scope for the first version, but the data model should preserve source/external identifiers so local sources can be added later.

**Consequences**:

* The first version can focus on syncing, local betting, wallet accounting, and settlement reliability.
* Admin UX is smaller and less error-prone for MVP.
* Future local market creation can reuse the order, quote, risk, fee, and settlement model by adding a new market source.

## Proposed MVP (Pending Confirmation)

* Use Approach B.
* Only Polymarket-synced markets are included in MVP.
* Backend module: `prediction`.
* Public/user APIs:
  * sync/list Polymarket-sourced events and markets
  * get market detail, displayed prices, and recent price/orderbook snapshot when available
  * request a short-lived local quote for YES/NO betting
  * place local virtual-asset prediction bets
  * list user bets/positions/history
* Admin APIs:
  * configure global allowed stake assets
  * configure default payout caps per allowed stake asset
  * configure global default fee rate for prediction betting
  * configure global default invalid-market refund policy
  * configure global default settlement mode for Polymarket-synced resolution
  * configure Polymarket sync categories/tags, sync interval, sync enablement, and enabled markets
  * trigger manual Polymarket sync and view sync status/logs
  * optionally override allowed stake assets, payout caps, fee rate, settlement mode, and hide imported events or markets
  * resolve or reconcile local market outcome
* PC:
  * add a prediction/竞猜 entry in header
  * market list/detail page similar to Polymarket
  * order panel with Yes/No outcome, current probability price, amount, stake asset
  * user center prediction bets/positions
* Backend settlement:
  * local bet freezes allowed stake asset amount and charges the configured fee separately
  * local position/share record is created at accepted price
  * positions cannot be sold or exited early in MVP
  * resolution credits winning positions according to probability-share redemption, limited by configured payout caps
  * losing positions settle to 0
  * invalid or canceled markets refund according to the configured invalid-market refund policy

## Technical Notes

* Polymarket docs organize core concepts around Markets & Events, Prices & Orderbook, Positions & Tokens, pUSD, Order Lifecycle, and Resolution.
* Polymarket API references include public market/event data, CLOB market data, order book endpoints, trade/order endpoints, and WebSocket market/user channels.
* Existing project specs read: backend index, cross-layer thinking guide, code-reuse thinking guide.
* Existing project modules inspected: wallet, spot, seconds contract, convert, router registration, admin resource configs, PC header/router patterns.
