# Margin Trading Action Contracts

## Scenario: Safe Margin Actions And Wallet Settlement

### 1. Scope / Trigger

- Trigger: opening, ticker-filling, closing, canceling, bulk-operating, transferring funds, liquidating a position, or reading/updating user margin settings.
- Applies to user margin routes, authoritative market ingestion, margin wallet/ledger persistence, and liquidation/interest workers.

### 2. Signatures

- Open/position routes: `/api/v1/margin/positions`, `/close`, `/close-all`, `/cancel`, `/cancel-all`.
- Single close request: optional integer `percentage` in `1..=100` plus
  `idempotency_key`; a body-less or empty legacy request remains a 100% close.
- Transfer: `POST /api/v1/margin/transfers` with asset, `from`, `to`, `amount`, and optional `idempotency_key`.
- Transfer eligibility: `assets.margin_transfer_enabled`; new assets default to `FALSE` and the admin asset API owns this flag.
- Margin wallet catalog: `GET /api/v1/margin/wallets` returns `asset_id`, `asset_symbol`, optional `logo_url`, `margin_transfer_enabled`, and the three balance buckets.
- Settings: `GET /api/v1/margin/settings/{product_id}` plus leverage/mode PATCH routes.
- Product catalog: anonymous `GET /api/v1/margin/products`; it contains no user funds or settings. Wallets, settings, risk, and every mutation remain user-authenticated.
- Position risk: authenticated `GET /api/v1/margin/positions/{id}/risk`, scoped by both position id and JWT user id.
- Position close history: authenticated `GET /api/v1/margin/positions/{id}/executions`
  returns `{ executions: [...] }`, scoped by both position id and JWT user id.
- Persistence: `margin_positions.wallet_scope`,
  `margin_position_close_executions(user_id, idempotency_key, position_id,
  close_percentage, allocated amounts, mark, PnL, settlement, terminal flag)`,
  and `margin_transfers(user_id, idempotency_key, transfer_id, request fields)`.
- Cross account persistence: `margin_cross_accounts(user_id, margin_asset)` stores the latest account-level equity, PnL, interest, maintenance margin, ratio, status, and version.
- Market cache: `market:ticker:{SANITIZED_SYMBOL}`, positive price observed within 60 seconds.
- Margin order intent: `margin_positions.order_type = market|limit`, nullable `limit_price`, and nullable `entry_price`; `entry_price IS NULL` is an unfilled order, not a risk-bearing position.
- Open request: missing `order_type` remains legacy-compatible `market`; `limit` requires `price`, while `trigger_price` is unsupported for both types.

### 3. Contracts

- Opening and closing require a fresh positive server ticker. A missing opening ticker must fail before position insertion or collateral debit.
- Market opens forbid `price` and fill immediately at that server ticker. Limit opens require a positive price within the pair `price_precision`; the client price is only a trigger boundary and is never a fill price.
- A long limit fills when server market price is less than or equal to its limit; a short limit fills when server market price is greater than or equal to its limit. An immediately marketable limit stores the current server ticker as `entry_price`.
- A non-marketable limit commits as `status = opened`, `entry_price = NULL`, persists its normalized `order_type`/`limit_price`, and debits/reserves collateral through the same wallet transaction as a market order.
- Only a Redis-CAS-accepted ticker may scan pending margin limits. Each candidate gets its own transaction, locks the position first, rechecks `opened + limit + entry_price IS NULL`, and stores that accepted ticker as the fill price. Stale/rejected ticker frames and depth snapshots never trigger margin fills.
- Cancel and ticker-fill use the same position-first lock order. Exactly one may transition a pending order; repeated tickers, retries, and competing service instances produce no duplicate mutation.
- Pending limits have `interest_accrued_at = NULL`, create no cross account, commission, or filled event, and are excluded by every interest, position/risk/return aggregate, and isolated/cross liquidation query. User/admin order-history lists intentionally retain them so they remain cancelable and auditable. The fill transaction sets both `opened_at` and the interest start to the real database fill time, while `created_at` remains the original placement time; it then creates the cross account when needed, inserts one commission, and publishes one post-commit filled event.
- `MarginPositionResponse`, user/admin reads, and private filled events preserve real `order_type`, optional `limit_price`, and optional `entry_price`. Clients classify filled holdings by positive non-null `entry_price` and pending orders by opened/null entry price.
- `MarginPositionResponse` additively exposes the row's real `opened_at` and
  immutable `created_at` as Unix-millisecond JSON integers. Existing fields and
  names remain unchanged; every SQLx projection into that response selects both
  timestamps. A pending limit retains its placement-time `created_at`, while
  its `opened_at` moves to the authoritative database fill time on first fill.
- Position close history first proves ownership with `(JWT user_id,
  position_id)`, then reads `margin_position_close_executions` with both keys.
  A missing or foreign position is the same NotFound response; an owned
  position without executions returns an empty list. Rows are ordered by
  `created_at ASC, id ASC`, all DECIMAL amounts remain strings, and the read
  never accrues interest, reads a ticker, revalues risk, opens a transaction,
  writes a wallet/ledger, or changes position state.
- `wallet_scope` snapshots whether collateral came from spot or margin. Active close, cancel, and isolated liquidation return funds to that same scope. Account-level cross liquidation is the explicit exception: it only consumes the shared margin wallet's `available` bucket and never credits a position payout.
- Position state, wallet balance, and ledger entry commit in one transaction.
- An explicit single-close percentage allocates that share of the currently
  locked remaining margin, notional, borrowed principal, and accrued interest.
  Percentages below 100 keep the row `opened` with exact remainders; 100 closes
  every remaining amount. Allocation rounds the closed slice down to the
  database's 18-decimal scale and derives the remainder by subtraction, so no
  amount disappears. A nonzero partial request that would create a zero closed
  or remaining margin/notional is rejected before wallet mutation.
- Realized PnL uses only the allocated notional and the fresh server mark.
  Isolated settlement credits the nonnegative allocated equity back to the
  recorded wallet scope; cross settlement applies the allocated signed equity
  to the shared margin wallet. The execution row, wallet delta/ledger, position
  remainder or terminal transition, and cross-account version bump commit in
  one transaction.
- Explicit close requests require a non-empty idempotency key no longer than
  128 bytes. Same user/key/position/percentage replay returns the original
  execution without another wallet or position mutation. Reusing a key for a
  different position or percentage is a conflict. Legacy body-less full close
  retains terminal-state replay compatibility.
- `realized_pnl` on an opened partially closed row is cumulative; a later final
  close or liquidation adds its remaining-slice PnL before storing the terminal
  result. Partial execution price and allocated amounts remain immutable in the
  execution table. Liquidation audit rows keep the liquidation slice PnL, while
  the terminal position and private liquidation event expose cumulative PnL.
- Margin open idempotency compares product, direction, explicit mode, margin, leverage, normalized order type, and optional limit price. Same-key same-request replay returns the original pending or filled row without another debit; any order-type or limit-price change is a conflict.
- Transfers lock spot then margin wallet in both directions, update both balances and ledgers atomically, and validate asset precision.
- A new `spot -> margin` transfer requires an active asset with `margin_transfer_enabled = TRUE`. Disabling the flag blocks only new inbound transfers; an existing margin balance remains visible and may still move `margin -> spot`.
- Margin-wallet reads include every active asset whose transfer flag is enabled, using zero balance buckets when the user has no lazy-created wallet row. They also retain every existing user margin-wallet row after the flag is disabled so configuration changes never hide stored funds.
- Same user/key/request replay returns the original `transfer_id` and original post-transfer ledger snapshots without moving funds again, even if the asset later becomes inactive.
- Same key with different asset, direction, or amount returns conflict.
- User leverage must be a configured product level. Persisted settings are readable through the GET route.
- Product listing returns a capability envelope. Implemented values are `order_types=["market", "limit"]`, `margin_modes=["isolated", "cross"]`, `bulk_close=true`, and `position_risk=true`; unimplemented `take_profit_stop_loss` and `strategy_orders` remain false. Clients render only advertised behavior, while missing-order-type PC requests remain market-compatible.
- The position risk response preserves the legacy `realized_pnl` alias and also returns the semantically correct `unrealized_pnl`, base quantity, return rate, margin ratio, isolated estimated liquidation price, and liquidation-distance rate. All decimals stay strings at the HTTP boundary.
- Risk display derivatives use the same ticker and worker risk state as liquidation. Quantity is notional divided by entry price, return rate is unrealized PnL divided by margin, and margin ratio is equity divided by maintenance margin. Invalid denominators produce null rather than fabricated zero ratios.
- An isolated liquidation estimate solves the mark price where equity equals maintenance margin. Isolated responses keep the existing top-level shape and omit `cross_account_risk`.
- A cross response retains the legacy top-level per-position fields, where the two isolated liquidation estimate fields remain null, and adds authoritative `cross_account_risk`. The object contains `margin_asset`, `reference_pair_id`, `price_assumption`, equity, maintenance margin, liquidation buffer, margin ratio, unrealized PnL, interest, liquidation decision, net/gross reference-pair quantity, estimate status, optional conditional price/distance, and min/max mark observation times.
- Cross risk rows are selected strictly by the JWT user, the reference position's `margin_asset`, `margin_mode='cross'`, `status='opened'`, and non-null `entry_price`. Isolated, pending, terminal, other-user, and other-asset rows never enter the account aggregate.
- Cross risk fetches each unique pair ticker once and reuses it for every leg on that pair. A missing, stale, or invalid ticker for any pair rejects the whole account snapshot; partial account estimates are forbidden.
- For reference mark `P0`, liquidation buffer `Buffer = equity - maintenance_margin`, and signed reference-pair net quantity `D` (long positive, short negative), the conditional account boundary is `P* = P0 - Buffer / D`; every other pair mark remains fixed. The implementation uses `BigDecimal` and conservatively rounds to pair tick precision (net long upward, net short downward).
- Conditional estimates expose stable states. `estimated` is the only state with a price/distance; `already_liquidatable`, `net_delta_zero`, `net_delta_near_zero`, `invalid_exposure`, `no_positive_boundary`, and `wrong_adverse_direction` return null price/distance. Near-zero means `abs(D) / gross_quantity <= 0.000001`, defined by the named backend constant `CROSS_NET_DELTA_EPSILON`.
- `cross` accounts are scoped by `(user_id, margin_asset)`. All open cross positions using that asset share wallet equity, initial margin, unrealized PnL, and accrued interest.
- Cross equity is `wallet_equity + sum(filled_open_position.margin_amount) + sum(unrealized_pnl) - sum(interest_amount)`; maintenance margin is the sum of each filled position's notional times its configured maintenance rate. Pending limits are absent from both sums.
- A cross account is liquidated as one unit when combined equity is less than or equal to combined maintenance margin. Before opening the transaction, the worker groups candidate legs by symbol and performs exactly one cached ticker read per symbol; all same-symbol legs receive that same mark. Any unavailable symbol skips the entire account and reschedules its affected positions.
- Once triggered, the transaction locks every account position and the one `(user_id, margin_asset)` margin wallet. It sets only `margin_wallet_accounts.available` to zero, preserves `frozen` and `locked`, and writes exactly one account ledger whose delta is `-available_before` (including one zero-delta audit row if the bucket was already zero). It never touches spot, another asset, or an isolated position.
- Every cross liquidation record/event has `payout_amount = 0`. Positive residual account equity is consumed rather than credited; bad debt is exactly `max(-account_equity, 0)`. The terminal cross-account row retains the same pre-liquidation equity/risk snapshot that triggered the decision and stores that snapshot's bad debt rather than replacing `last_equity` with the post-clear wallet balance. Wallet mutation, one ledger, all position records/terminal transitions, and account bad debt commit atomically, and replay of terminal positions performs none of them again.
- The margin wallet response includes `cross_accounts[]` with `equity`, `unrealized_pnl`, `interest_amount`, `maintenance_margin`, and optional `margin_ratio` from the latest worker snapshot.
- Cross interest is accrued per position and aggregated into `margin_cross_accounts.last_interest_amount`; close and liquidation deduct the position's accrued interest from settlement.
- Bulk actions have no silent 100-row cap, reuse single-item idempotent transactions, continue after failures, and return `failures`.

### 4. Validation & Error Matrix

- Missing/stale/non-positive ticker on open/close -> `VALIDATION_ERROR`, no financial mutation.
- Anonymous access to the product catalog is allowed; anonymous access to wallets, settings, risk, open, close, cancel, bulk actions, or transfer remains `UNAUTHORIZED`.
- Anonymous access to position close history -> `UNAUTHORIZED`; an admin token
  on the user route remains forbidden, and a foreign or absent position ->
  `NOT_FOUND`.
- Unsupported leverage or margin mode -> `VALIDATION_ERROR`.
- Unknown `order_type`, a market request with `price`, a limit request without positive `price`, or either type with `trigger_price` -> `VALIDATION_ERROR`.
- Limit `price` exceeds pair `price_precision` -> `VALIDATION_ERROR`; never round or rewrite the client's trigger boundary.
- Unknown margin mode -> `VALIDATION_ERROR`; `cross` is accepted only when the product includes it in `margin_modes`.
- Transfer source equals target or account name is unsupported -> `VALIDATION_ERROR`.
- Transfer amount non-positive or exceeds asset precision -> `VALIDATION_ERROR`.
- Active asset has `margin_transfer_enabled = FALSE` on a new spot-to-margin request -> `VALIDATION_ERROR`, with no transfer record, balance mutation, or ledger entry.
- Insufficient source available balance -> `VALIDATION_ERROR`, no opposite-side credit.
- Same idempotency key with different request -> `CONFLICT`.
- Same margin idempotency key with changed `order_type` or `limit_price` -> `CONFLICT`, with no additional collateral debit.
- Explicit close percentage outside `1..=100`, missing/oversized close
  idempotency key, or an unrepresentable nonzero slice -> `VALIDATION_ERROR`
  before wallet mutation.
- Same close key with a different position or percentage -> `CONFLICT`, with no
  additional execution, ledger, wallet delta, or remainder update.
- Unknown `wallet_scope` on close/cancel/liquidation -> `VALIDATION_ERROR`; never default to spot.
- Missing/stale/non-positive ticker for any pair in a cross risk account -> `VALIDATION_ERROR` for the entire account snapshot; never return an aggregate with one leg omitted.
- One bulk item fails -> include its id/code/message and continue later items.

### 5. Good/Base/Bad Cases

- Good: margin-funded position closes back into `margin_wallet_accounts` with a margin ledger row.
- Good: a long limit at 90 while the server ticker is 100 reserves collateral with null entry price; an accepted ticker at 89 fills once at 89, moves `opened_at` to that fill time, starts interest then, inserts one commission, and emits one event.
- Base: the same pending-limit request/key replays the original row, and a cancel racing the trigger wins or loses under the position lock without double refund/fill.
- Good: reverse transfer replay after asset disable returns original snapshots and creates no extra ledgers.
- Good: disabling margin transfer rejects a new inbound request while the user's existing wallet row stays visible and can transfer back to spot.
- Base: an enabled asset without a user margin-wallet row appears in `/margin/wallets` with three zero buckets; the read does not create a database row.
- Base: a second close/cancel sees the terminal position and does not credit twice.
- Good: a 37% isolated close credits only that slice's nonnegative equity,
  records one immutable execution, and leaves 63% of all four exposure amounts
  opened; retrying the same key returns the same result without another ledger.
- Good: a partial cross close applies the slice's signed equity and refreshes
  account risk from the reduced remaining row; it never treats a negative
  slice as zero.
- Good: a hedged cross account whose old position-equity settlement would increase `available` instead sets it to zero, records one `-available_before` ledger, and stores zero payout on every leg.
- Good: partial same-pair hedges return a finite conditional account boundary while exact or near-exact hedges return an explicit null-bearing estimate status.
- Bad: opposite transfer directions lock wallets in different orders; this creates a deadlock window.
- Bad: filling at the client limit, an order-book BBO, a stale Redis frame, or a depth snapshot instead of the CAS-accepted server ticker.
- Bad: including `entry_price IS NULL` rows in dashboard open-position counts, interest summaries, realized-return aggregates, isolated liquidation, cross-account expansion, or commission generation.
- Bad: evaluating a cross position independently can liquidate one position while leaving the shared account under-collateralized.

### 6. Tests Required

- Fresh/stale/missing ticker open tests assert zero position, wallet, and ledger mutations on failure.
- Order tests cover market field rejection, required/positive/precision-safe limits, long/short equality boundaries, immediate trusted-ticker fill, pending persistence, and changed-type/price idempotency conflicts.
- Pending integration tests assert collateral reservation and original-scope cancel refund while interest timestamp, cross account, commission, fill event, and liquidation records remain absent.
- Trigger integration tests assert accepted-ticker fill price, fill-time `opened_at` and interest start, exactly one commission/event, repeat-ticker idempotency, and cancel/fill race safety.
- Transfer tests cover both directions, precision, insufficient balance, same-key replay, changed-request conflict, asset-disable replay, inbound eligibility rejection, outbound-after-disable, and ledger counts.
- Wallet-list tests cover an enabled asset before lazy account creation, an existing wallet after the flag is disabled, backend Logo passthrough, and `margin_transfer_enabled` serialization.
- Close/cancel tests assert balance, ledger, status, and idempotent retry for both wallet scopes.
- Partial-close tests cover isolated and cross settlement, 1/37/50/100 percent
  allocations, cumulative realized PnL, exact remainder preservation, same-key
  replay, changed-request conflict, validation, concurrent retries, one
  execution/ledger per committed request, and legacy body-less full close.
- Isolated liquidation worker tests assert payout uses recorded `wallet_scope` and otherwise remains unchanged.
- Cross liquidation worker tests cover a hedged long/short account that previously could mint available balance, zero post-settlement available, unchanged frozen/locked and spot balances, one negative account ledger, zero payout on every position, all account positions terminal, exact bad-debt policy, same-symbol shared marks, and replay idempotency.
- Bulk tests process more than 100 rows, retain prior successes/events, report a failed row, and continue to later rows.
- Settings tests cover user isolation, leverage round-trip, mode round-trip, and cross acceptance for a product configured with `cross`.
- Route tests prove the product catalog works without a bearer token while private margin routes still reject anonymous callers.
- Position read-model tests cover Unix-millisecond `opened_at`/`created_at`,
  anonymous rejection, user isolation with indistinguishable NotFound,
  owned-position empty history, deterministic execution ordering, exact
  DECIMAL-string serialization, and unchanged positions, wallets, ledgers, and
  execution counts after reads.
- Risk tests cover the legacy PnL alias, new unrealized PnL, quantity, return/margin ratios, isolated liquidation price/distance, and omitted cross object for isolated mode.
- Cross route tests cover exact account-row scope, all-or-nothing unique-pair marks, multi-pair aggregation, min/max mark times, partial-hedge solving, exact-hedge null status, and null legacy isolated estimate fields.
- Domain tests cover combined PnL/interest/maintenance arithmetic, the equality liquidation boundary, named near-zero threshold, partial long/short hedge roots, exact/near hedge nulls, already-triggered/non-positive/wrong-direction statuses, and full-consumption settlement/bad-debt arithmetic.

### 7. Wrong vs Correct

#### Wrong

```rust
// Trusts client price and lets a pending order enter risk accounting.
credit_spot_wallet(...);
position.entry_price = request.price;
accrue_interest(position);
```

#### Correct

```rust
credit_margin_position_amount(tx, user_id, asset_id, &position.wallet_scope, amount, change_type, position.id).await?;
ensure_supported_user_margin_mode(&requested_mode)?;
// cross risk is evaluated by (user_id, margin_asset), never by position alone;
// each unique pair contributes one shared cached mark.
evaluate_cross_margin(&wallet_equity, &position_margin, &position_risks);
let boundary = estimate_cross_margin_conditional_price(
    reference_pair_id,
    reference_mark,
    &(account_risk.equity - account_risk.maintenance_margin),
    price_precision,
    &positions,
)?;
// limit price only decides the boundary; the accepted server ticker is the fill.
if margin_limit_order_is_triggered(direction, limit_price, accepted_ticker)? {
    mark_margin_limit_position_filled(tx, position.id, accepted_ticker).await?;
}
```

Settlement follows the recorded funding scope, and unsupported risk semantics fail explicitly.

## Scenario: Margin Wallet Transfer Idempotency

### 1. Scope / Trigger

- Trigger: transferring funds between the shared spot wallet and a margin wallet.
- Applies before wallet locks, risk validation, balance changes, and ledger insertion.

### 2. Signatures

```text
required request field: idempotency_key
scope: (user_id, idempotency_key)
fingerprint: SHA-256(asset_id + direction + normalized amount + normalized margin mode/account scope)
```

### 3. Contracts

- Missing, blank, or overlong keys are rejected before opening a financial transaction; the backend never replaces them with UUIDs.
- The transfer request row is the durable command receipt. It stores the canonical request fingerprint and the first committed result snapshots.
- Same user/key/fingerprint replays the first result without locking or moving funds again.
- Same user/key with different asset, direction, amount, mode, or account scope returns 409.
- Different users may reuse the same key string.
- Receipt, both wallet mutations, paired ledger entries, post-transfer risk validation, and response snapshots commit atomically.
- The client freezes one key with the confirmed intent and reuses it after an uncertain response; editing any transfer parameter starts a new intent.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Missing key | 4xx, no receipt or wallet mutation |
| Same key and identical intent | Exact response replay |
| Same key and changed amount/direction/scope | 409 Conflict |
| Twenty concurrent same-key requests | One receipt and one paired transfer |
| Commit succeeds but response is lost | Retry reconstructs first committed result |

### 5. Tests Required

- MySQL tests cover missing key, replay, conflict, cross-user reuse, and at least twenty concurrent identical requests.
- Assert one transfer receipt, exactly one debit/credit pair, and unchanged final risk invariants.
- Client contract tests assert an uncertain retry reuses the same key.

## Scenario: Directional User Leverage Settings

### 1. Scope / Trigger

- Trigger: migrating, reading, or updating one user's default leverage for a margin product.
- The setting applies only to later order drafts/opens. It never rewrites leverage, notional, collateral, or liquidation state on an existing position.

### 2. Signatures

```text
migration columns: long_leverage DECIMAL(18,8) NULL, short_leverage DECIMAL(18,8) NULL
legacy PATCH: { "leverage": DECIMAL }
directional PATCH: { "long_leverage": DECIMAL, "short_leverage": DECIMAL }
GET/PATCH response: product_id, margin_mode, leverage, long_leverage, short_leverage
```

### 3. Contracts

- The additive migration backfills both directional columns from every non-null legacy `leverage` value and adds independent positive-or-null checks. Existing migration files remain immutable.
- PATCH accepts exactly one payload shape. Legacy input sets `leverage`, `long_leverage`, and `short_leverage` to the same value. Directional input requires both values and stores legacy `leverage = long_leverage`.
- Mixed legacy/directional fields, either missing directional peer, an empty object, and explicit null values return `VALIDATION_ERROR` before a settings transaction or write begins.
- Both normalized values must be positive and independently match an exact decimal entry in the locked active product's single `leverage_levels` list. No rounding, nearest-level selection, or direction-specific synthetic level list is permitted.
- Product locking, validation against that product version, the one-row three-column upsert, readback, and commit share one transaction. If either direction is unsupported, none of the three leverage columns changes.
- A mode-only update preserves all three leverage columns. A leverage-only update preserves `margin_mode`.
- GET and both PATCH responses expose all three leverage fields. The compatibility `leverage` response is always derived from the stored/fallback long value so old clients remain consistent.
- Rows and lookups remain scoped by `(user_id, product_id)`; one user's request cannot create, read, or overwrite another user's setting.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Legacy value exactly matches a product level | All three leverage columns become that value |
| Both directional values match product levels | Long/short persist independently; legacy equals long |
| Mixed, partial, empty, or null-bearing shape | `VALIDATION_ERROR`, no setting write |
| Either value is non-positive or absent from levels | `VALIDATION_ERROR`, previous row remains unchanged |
| Mode-only update after directional leverage | Mode changes; all leverage values remain unchanged |

### 5. Tests Required

- Migration tests execute the exact `0120` SQL against a legacy fixture and assert nullable `DECIMAL(18,8)` metadata, exact backfill, and both positive checks.
- Route round-trip tests cover legacy and directional PATCH, GET readback, legacy-long synchronization, invalid-shape pre-write rejection, one-invalid-side atomic rollback, mode preservation, and user isolation.
- When MySQL is unavailable, integration branches may skip explicitly, but the migration contract and route target must still compile.

## Scenario: Admin Liquidation Evidence Projection

### 1. Scope / Trigger

- Trigger: changing the Admin margin-liquidation list/detail DTO or the query
  used by `/admin/api/v1/margin/liquidations`.
- This projection is read-only evidence after liquidation. It must not rerun
  risk evaluation, settlement, wallet mutation, or position transitions.

### 2. Signatures

```text
GET /admin/api/v1/margin/liquidations?user_id&email&pair_id&position_id&limit&offset
GET /admin/api/v1/margin/liquidations/:id

AdminMarginLiquidationResponse {
  id, position_id, user_id,
  email: string | null,
  product_id, pair_id,
  symbol: string,
  ...immutable liquidation snapshot fields
}
```

### 3. Contracts

- List and detail share one DTO and one row query, so `email`, `symbol`, IDs,
  amounts, reason, and timestamps cannot drift between the two endpoints.
- `email` comes from the record's user foreign key and remains null when the
  user has no email. Never synthesize a phone, user ID, or placeholder in the
  API response.
- `symbol` comes from the record's trading-pair foreign key and is non-null.
  Do not reconstruct it from current asset-directory requests in the client.
- The additive fields do not remove `id`, `position_id`, or `user_id`; Admin
  row actions and existing integrations may still require those identifiers.
- Qualify liquidation columns after joining `users` and `trading_pairs`.
  COUNT uses the same user/email/pair/position predicates and pagination still
  orders by `liquidation.id DESC` before applying offset/limit.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| User email is null | Serialize `email: null`; retain the record |
| Pair relation resolves | Serialize the stored pair `symbol` |
| Requested liquidation ID is absent | Existing `NOT_FOUND` behavior |
| Database/JOIN mapping fails | Existing database error; never return a partial fabricated row |
| One filter is present | Apply it identically to rows and total |
| No filter is present | Stable `id DESC` pagination with the full matching total |

### 5. Good / Base / Bad Cases

- Good: an administrator sees a nullable email and `BTC-USDT` while hidden IDs
  remain available to open the exact detail record.
- Base: old consumers ignore the two additive fields and continue reading the
  unchanged snapshot and identifier fields.
- Bad: remove IDs from the response because the table hides them; this breaks
  detail actions and compatibility.
- Bad: fetch users and trading pairs once per rendered row; this introduces
  N+1 requests, cache races, and display drift.

### 6. Tests Required

- MySQL route tests seed records with and without email, assert list/detail
  equality, email/symbol values, old IDs, every existing filter, total,
  offset/limit, and deterministic `id DESC` ordering.
- Admin row-contract tests require both keys while accepting `email: null`.
- Render tests assert the two business columns, hidden internal-ID headers,
  shared null presentation, and detail lookup through the hidden record ID.

### 7. Wrong vs Correct

#### Wrong

```sql
SELECT liquidation.* FROM margin_liquidation_records liquidation;
-- The client then performs per-row user and pair lookups.
```

#### Correct

```sql
SELECT liquidation.id, liquidation.position_id, liquidation.user_id,
       liquidation_user.email, liquidation.pair_id, liquidation_pair.symbol, ...
FROM margin_liquidation_records AS liquidation
JOIN users AS liquidation_user ON liquidation_user.id = liquidation.user_id
JOIN trading_pairs AS liquidation_pair ON liquidation_pair.id = liquidation.pair_id
ORDER BY liquidation.id DESC;
```
