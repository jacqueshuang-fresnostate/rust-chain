# Spot Order Contracts

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
- `price: decimal string`
- `quantity: decimal string`
- `idempotency_key?: string`

### 3. Contracts

- Stop-limit orders must include both `trigger_price` and `price`.
- Store `trigger_price` in `spot_orders.trigger_price`; include it in user/admin order responses.
- Freeze wallet balances at order creation using the limit `price`, the same reserve asset rules as normal limit orders.
- Do not execute a stop-limit order immediately unless the cached market price already satisfies both trigger and limit conditions.
- Buy condition: execute only when `market_price <= trigger_price` and `market_price <= price`.
- Sell condition: execute only when `market_price >= trigger_price` and `market_price >= price`.
- Idempotency comparison must include `trigger_price`.
- Once triggered, reuse the existing triggered spot execution path and settlement accounting.

### 4. Tests Required

- Unit tests for buy/sell trigger predicates requiring both trigger and limit prices.
- Adapter tests confirming PC `STOP_LIMIT` maps to backend `stop_limit` with `trigger_price`.
- Existing limit and market order tests must continue to pass.
