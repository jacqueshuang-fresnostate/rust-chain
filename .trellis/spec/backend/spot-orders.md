# Spot Order Contracts

## Scenario: Exact Numeric Reservation And Settlement

- Source quantities must be positive and exactly fit both pair quantity
  precision and the actual base asset `precision_scale`. Source prices,
  reference prices and triggers must be exactly representable, never rounded.
  Pair precision is bounded to `0..=18`; quantity precision cannot exceed the
  base asset precision. Amount storage is `DECIMAL(38,18)`.
- Read actual base/quote asset metadata in the financial transaction and hold
  shared configuration locks while using it. Pair minimum configuration must
  be positive, storage-safe and representable at quote asset precision.
- Generated quote reserve and each actual fill quote amount are truncated
  toward zero to quote asset precision. A positive product that quantizes to
  zero is rejected, as is a result exceeding storage capacity. Do not insert
  zero-value trades or silently round a user's source quantity.
- Buy reservation uses the whole order quantity and limit/reference price;
  market-buy uses the existing maximum reference/execution protection. A fill
  uses its actual price and slice quantity. Sum of per-fill truncations does
  not exceed the truncated whole-order reserve at the limit. This is explicitly
  a **per-execution** contract, not partition-independent cumulative rounding.
- Buyer frozen debit, seller available credit, commission base and platform
  journal all reuse the same generated quote amount. Base debit/credit reuse
  the exact admitted quantity. Never round wallet available/frozen/locked
  separately. Validate the delta and every resulting bucket before SQL writes.
- Partial fills keep all unused reserve frozen. Full fill and cancellation
  release exactly stored reserve minus actual frozen settlement debits and
  prior releases, including dust and price improvement. Do not reconstruct
  actual spent quote using `SUM(price * quantity)` or reprice remaining quantity.
- Existing positive `spot_freeze` frozen ledger evidence takes precedence over
  reservation snapshots. NULL or zero historical snapshots can use this
  evidence without any backfill. Ledger reads are scoped by user, asset and
  numeric order reference. Never claim an entire wallet's frozen bucket as one
  order's reservation.
- Historical release preserves its actual stored precision, even if today's
  asset precision is lower. No migration, balance rewrite or historical amount
  quantization is performed. Without positive reserve evidence, or when partial
  quantity and actual debit/base-credit evidence disagree, fail closed and leave
  order/balances unchanged for investigation. New metadata absence alone is not
  grounds to reject an evidenced historical refund.
- Stable order locks and existing idempotency keys still arbitrate concurrent
  fills/cancel/replay. Remaining-reserve reads occur after trade placeholder
  insertion but before current wallet legs, so the current trade has no debit
  evidence yet and cannot be counted twice.
- The synchronous `SpotService` adapter requires explicit actual asset
  precisions and a transaction-owned remaining-reserve snapshot in its command.
  It must not recompute cancellation reserve from price; production financial
  routes continue to use the atomic asynchronous application path.

### Required Regressions

Cover 8+8 and 10+10 fractional products, `1e-18`, 19 fractional digits,
20/21 integer digits, huge exponents, trailing zeros, invalid asset/pair
precision and generated zero. Real isolated MySQL tests must exercise both
automatic directions, multiple partial fills/full-fill dust release, partial
cancel, concurrent exact fill replay, rollback, per-asset journal zero sums,
legacy NULL/zero snapshots with evidence, stored historical dust and missing
evidence rejection. Environment-skipped database tests are not verification.

## Scenario: Market Order Reference Price Protection

### 1. Scope / Trigger

- Trigger: User-facing `POST /api/v1/spot/orders` accepts market orders with `reference_price`, while backend execution may use Redis cached market ticker `last_price`.
- Applies to spot route code that validates market order slippage, freezes wallet balances, and executes immediate market fills.

### 2. Signatures

- API: `POST /api/v1/spot/orders`
- Request fields involved:
  - `pair_id: string`
  - `side: "buy" | "sell"`
  - `order_type: "market"`
  - `quantity: decimal string`
  - `reference_price: decimal string`
  - `idempotency_key?: string`
- Cache key: `market:ticker:{SANITIZED_SYMBOL}` via `market_ticker_redis_key(symbol)`.
- Cache payload field: `last_price` as a positive decimal string.

### 3. Contracts

- Market orders must include a positive `reference_price`.
- If Redis has a valid ticker `last_price`, the route uses it as execution price.
- Redis must contain a positive ticker observed within the last 60 seconds. The client `reference_price` is only a slippage guard and must never become the execution price.
- Market buy may execute above `reference_price` only within `MARKET_REFERENCE_PRICE_TOLERANCE_BPS`.
- Market sell may execute below `reference_price` only within `MARKET_REFERENCE_PRICE_TOLERANCE_BPS`.
- Market buy wallet reservation must use `max(reference_price, execution_price)` when the order executes immediately, so settlement never exceeds frozen quote funds.
- Store the original request `reference_price` in `spot_orders.request_reference_price`; do not replace it with execution price.

### 4. Validation & Error Matrix

- Missing market `reference_price` -> `VALIDATION_ERROR: reference_price is required for market orders`.
- Buy execution price above allowed reference ceiling -> `VALIDATION_ERROR: market price exceeds submitted reference price; please retry`.
- Sell execution price below allowed reference floor -> `VALIDATION_ERROR: market price is below submitted reference price; please retry`.
- User quote/base balance cannot cover the chosen reservation -> wallet validation error, no order should be inserted.
- Missing Redis/ticker or a stale ticker -> validation error before order insertion or wallet freezing.
- Invalid Redis ticker payload -> internal error, because cached market data is malformed.

### 5. Good/Base/Bad Cases

- Good: buy `reference_price=1717.8`, cached `last_price=1718.0`, tolerance allows it; reserve and settle at `1718.0`.
- Base: no Redis ticker; reject without inserting an order or changing wallet balances.
- Bad: buy cached price moves more than tolerance above reference; reject before freezing.
- Bad: sell cached price moves more than tolerance below reference; reject before freezing.

### 6. Tests Required

- Unit tests for buy/sell tolerance acceptance and rejection.
- Integration test with Redis ticker where buy execution price is slightly above `reference_price`; assert HTTP 200, `reserved_amount`, trade price, wallet available, and frozen balance.
- Missing/stale Redis ticker tests must assert that no order, wallet, or ledger mutation occurs.

### 7. Wrong vs Correct

#### Wrong

```rust
// Rejects any tiny buy uptick and makes normal market orders flaky.
if execution_price > reference_price {
    return Err(AppError::Validation("market price exceeds submitted reference price; please retry".to_owned()));
}
```

#### Correct

```rust
// Allow a small bounded drift, and reserve enough quote for the actual execution price.
ensure_market_price_within_reference(OrderSide::Buy, execution_price, reference_price)?;
let reservation_price = max_reference_or_execution_price(reference_price, execution_price);
```

## Scenario: User Cancel All

### 1. Scope / Trigger

- Trigger: an authenticated user requests server-side cancellation of all currently cancellable spot orders, optionally for one pair.
- Applies to reservation release, order state, private events, and partial-failure reporting.

### 2. Signatures

- API: `DELETE /api/v1/spot/orders?pair_id={optional_symbol_or_id}`.
- Response: `orders[]` for newly canceled orders and `failures[]` containing `id`, `code`, and `message`.
- Cancellable states: `pending`, `open`, and `partially_filled`.

### 3. Contracts

- The endpoint cancels only cancellable orders owned by the authenticated user.
- Optional `pair_id` narrows the operation to one trading pair.
- Every item reuses the single-order cancellation transaction and unfreezes only the remaining reservation.
- A failed item must be returned in `failures` and must not prevent later items from being attempted.
- Repeated requests must not unfreeze balances or publish cancellation events twice.
- User trade queries must always filter by the authenticated user as buyer or seller.

### 4. Validation & Error Matrix

- Missing/invalid user token -> `UNAUTHORIZED`.
- MySQL not configured -> server configuration error.
- Pair filter does not match a user order -> empty success response, no mutation.
- One malformed legacy reservation -> add that order to `failures`; continue remaining orders.
- Already canceled/filled order -> exclude from the cancellable ID set and never unfreeze again.

### 5. Good/Base/Bad Cases

- Good: two matching open orders cancel, each remaining reserve is released exactly once.
- Base: no cancellable order returns empty `orders` and `failures`.
- Bad: loading all market orders without `user_id` leaks or mutates another user's orders.
- Bad: wrapping the whole batch in one transaction causes one broken row to roll back prior successes.

### 6. Tests Required

- Route scope test rejects non-user tokens and missing MySQL.
- Pair-filter test keeps nonmatching orders open and leaves their frozen balance unchanged.
- Idempotency test repeats cancel-all and asserts no extra balance or ledger mutation.
- Partial-failure test asserts a failed row is reported and a later valid row still cancels.
- Trade-list test creates unrelated fills and returns only trades where the current user is buyer or seller.

### 7. Wrong vs Correct

#### Wrong

```rust
for order in orders {
    cancel_order(order).await?;
}
```

The first bad legacy row aborts later valid cancellations.

#### Correct

```rust
for order_id in order_ids {
    match cancel_user_spot_order_with_events(pool, order_id, user_id, hub).await {
        Ok(response) if response.cancelled => orders.push(response.order),
        Ok(_) => {}
        Err(error) => failures.push(batch_failure(order_id, error)),
    }
}
```

## Scenario: Stop-Limit Spot Orders

### 1. Scope / Trigger

- Trigger: User-facing `POST /api/v1/spot/orders` accepts `order_type: "stop_limit"`.
- Applies to route creation, idempotency checks, wallet reservation, order list responses, and market-feed-triggered spot execution.

### 2. Request Fields

- `pair_id: string`
- `side: "buy" | "sell"`
- `order_type: "stop_limit"`
- `trigger_price: decimal string`
- `trigger_direction: "rising" | "falling"` (required for new stop-limit orders)
- `price: decimal string`
- `quantity: decimal string`
- `idempotency_key?: string`

### 3. Contracts

- New stop-limit orders must include `trigger_price`, `price` and an explicit
  `trigger_direction`; direction is independent of buy/sell and never inferred
  by a client. Other order types reject a supplied direction.
- Store and return nullable `trigger_price`, `trigger_direction`, and
  `triggered_at` (Unix milliseconds in API responses). Only the server sets
  `triggered_at`; activation is not part of client request identity.
- Freeze wallet balances at order creation using the limit `price`, the same reserve asset rules as normal limit orders.
- Explicit rising activates at `market_price >= trigger_price`; falling at
  `market_price <= trigger_price`. Equality activates either direction.
- Commit activation under an order lock in its own transaction before attempting
  later settlement. An unmet limit or inventory/settlement failure must not erase
  activation. Re-lock and recheck status before settlement so cancellation and
  terminal states cannot be bypassed.
- After activation, only the usual limit remains: buy `market_price <= price`,
  sell `market_price >= price`. Recrossing the trigger does not deactivate.
- At creation, a fresh cached threshold observation saves activation with the
  order even when the limit is unmet. If both threshold and limit are met,
  creation can use existing atomic immediate settlement; an unsuccessful
  creation transaction creates no order or reservation.
- Legacy NULL direction retains its historical conjunction without persistence
  changes: buy `market_price <= trigger_price && market_price <= price`; sell
  `market_price >= trigger_price && market_price >= price`. Migration 0132 adds
  nullable columns and constraints only, with no backfill or comparison flip.
- Idempotency includes trigger price and explicit direction. NULL-direction
  fingerprints retain their original v1 bytes. Existing exact requests replay
  before new-request direction validation, returning the first stored response
  without a second financial effect; live readback exposes later activation.
- Manual fills cannot bypass an unactivated explicit stop-limit order. Legacy
  manual behavior and ordinary automatic fill rules remain unchanged.

### 4. Tests Required

- Unit tests cover both directions for both sides, equality, durable activation,
  terminal status and original legacy fingerprints/comparisons.
- Real MySQL/Redis tests cover migration preservation, independent activation,
  inventory failure, new-pool readback, price recrossing, concurrent execution,
  cancellation, cached activation, immediate fill, exact replay and direction conflict.
- PC adapters and retry tests include direction in intent; the existing
  stop-limit form requires a selection. Admin/PC/Mobile readback distinguishes
  legacy NULL and explicit activation. Mobile does not gain a new order type.
- Existing limit and market order tests must continue to pass.

## Scenario: Controlled Manual Fills And Actual Fill Journals

- `POST /admin/api/v1/spot/fills` requires the existing spot fill permission,
  authenticated admin ID, bounded nonblank `reason` and stable client
  `idempotency_key`. A body-supplied actor is never trusted.
- Use the shared Admin audit helper within the same transaction as trade,
  order state, wallet changes, ledger, commissions and platform journal.
  Audit or journal failure must roll back the entire fill.
- Exact replay requires the original actor, reason, canonical order IDs, price,
  quantity and key. It produces no duplicate audit, journal, wallet or event
  effect; conflicting reuse is rejected, including concurrent duplicate keys.
  Historical unaudited fills cannot be claimed as a new manual operation.
- Admin exposes a permission-gated action on the existing spot resource, with
  readonly preview of both orders and explicit confirmation. Preview never
  mutates a wallet; uncertain responses preserve the original request identity.
- Journal each actual fill separately per asset using context `spot`, key
  `spot:{trade_id}:fill`, reference type `spot_trade` and the real trade ID.
  Source/target user liability amounts are opposite actual wallet changes.
  The existing prepaid system liquidity account, not an upstream ticker, is
  the `platform_spot_inventory` counterparty. Every asset balances independently.
- Current fills charge exactly zero fees. Do not invent fee income or add a
  debit. A future nonzero fee requires an explicit charged-asset and net-wallet
  settlement contract; the journal fails closed until that contract exists.
- Tests include missing actor/reason, parameter/actor conflicts, concurrent
  replay, audit/journal failure injection, both inventory directions and
  unchanged financial/journal counts after replay.

## Scenario: Client-scoped Order Idempotency

### 1. Scope / Trigger

- Trigger: creating any market, limit, or stop-limit spot order, changing the create-order DTO, or changing retry behavior in PC/Mobile clients.
- Applies before wallet reservation, order insert, matching, or trigger creation.

### 2. Signatures

```text
POST /api/v1/spot/orders
required body field: idempotency_key: nonblank bounded string
scope: (user_id, idempotency_key)
fingerprint: SHA-256(canonical complete order intent)
```

### 3. Contracts

- The client creates one key for one logical confirmation and reuses it after timeout, dropped response, or explicit retry.
- The server never generates a fallback key for a financial command.
- Canonical intent includes pair, side, type, normalized quantity, normalized optional price, normalized optional stop price, and every field that changes reservation or execution behavior.
- Same user/key/fingerprint returns the first stored order response and causes no second reservation, order, trade, or ledger entry.
- Same user/key with a different fingerprint returns HTTP 409. Different users may reuse the same key string.
- Database uniqueness is user-scoped. Historical NULL keys remain read-compatible and are not rewritten as invented client identities; all new API requests require a key.
- Order insert, request fingerprint, wallet reservation, and associated financial rows commit in one transaction.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Missing, blank, or overlong key | 4xx before any financial mutation |
| Same user/key and identical canonical request | Replay first response |
| Same user/key and different request | 409 Conflict |
| Different users use the same key text | Independent orders |
| Twenty concurrent identical requests | One order and one reservation effect |

### 5. Tests Required

- MySQL route tests cover missing key, exact replay, changed parameter conflict, different-user reuse, and twenty concurrent requests.
- Tests assert the final wallet values and counts of orders, reservations/ledger entries, and trades where matching is enabled.
- Client contract tests prove retry reuses the frozen key and editing the order intent creates a new logical confirmation.

### 6. Wrong vs Correct

```rust
// Wrong: absence silently disables idempotency.
let key = request.idempotency_key.filter(|value| !value.trim().is_empty());
```

```rust
// Correct: normalize, require, fingerprint, then let the database receipt arbitrate concurrency.
let key = require_idempotency_key(&request.idempotency_key)?;
let fingerprint = fingerprint_spot_order(&canonical_intent)?;
```
