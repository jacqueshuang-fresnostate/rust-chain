# Seconds Contract Contracts

## Scenario: Order Timestamps

### 1. Scope / Trigger

- Trigger: seconds contract order list/detail payloads are consumed by the PC active/history position UI.
- This is a cross-layer contract because database timestamps, backend JSON, PC adapter fields, and table display must stay aligned.

### 2. Signatures

- DB order fields:
  - `seconds_contract_orders.created_at`: order creation / open timestamp.
  - `seconds_contract_orders.expires_at`: expected settlement deadline.
- Backend `SecondsContractOrderResponse` must expose both fields as unix millis:
  - `created_at`
  - `expires_at`
- PC `BackendSecondsOrder` accepts `created_at`, with `opened_at` / `time` as compatibility fallbacks.
- PC `SecondOrder.createTime` is the field rendered by the history table time column.

### 3. Contracts

- Every SQL query that maps into `SecondsContractOrderResponse` must select `orders.created_at` together with `orders.expires_at`.
- User order list, admin order list, detail, idempotency replay, and settlement lock/read paths must expose the same timestamp field set.
- PC adapter must map `created_at` to `createTime`; it must not hard-code `createTime: 0`.
- PC history positions render `formatTime(order.createTime)`.

### 4. Validation & Error Matrix

- Missing `created_at` in any `SecondsContractOrderResponse` query -> runtime mapping error or blank PC time.
- `created_at = 0` from a backend row -> PC displays `--`; investigate data quality, do not silently replace it with `expires_at`.
- Legacy payload with only `opened_at` or `time` -> PC adapter may use that value as a compatibility fallback.

### 5. Good/Base/Bad Cases

- Good: a closed order returns `created_at = 1717170880000`, PC maps `createTime` to that value, and the history row shows local datetime.
- Base: open orders also carry `created_at`; active countdown still uses `expires_at`.
- Bad: relying on `expires_at - duration_seconds` to reconstruct order time; duration snapshots can change and this hides backend response regressions.

### 6. Tests Required

- Route test: user seconds order list includes numeric `created_at`.
- Route test: admin seconds order list/detail includes numeric `created_at`.
- PC adapter test: `created_at` maps to `createTime` and `expires_at` maps to `endTime`.

### 7. Wrong vs Correct

#### Wrong

```typescript
createTime: 0
```

This makes the PC history time column display `--`.

#### Correct

```typescript
createTime: Number(order.created_at ?? order.opened_at ?? order.time ?? 0),
endTime: Number(order.expires_at ?? 0),
```

The adapter preserves backend timestamps and remains compatible with older payload names.

## Scenario: User Order History Pagination

### 1. Scope / Trigger

- Trigger: changing `GET /api/v1/seconds-contracts/orders`, its user-scoped SQL,
  or a client that incrementally loads seconds-contract order history.
- This is a cross-layer contract because query normalization, deterministic SQL
  ordering, the response continuation signal, and client page advancement must
  agree exactly.

### 2. Signatures

```text
GET /api/v1/seconds-contracts/orders?limit=<1..100>&offset=<0..100000>
Authorization: Bearer <user token>

200 {
  "orders": SecondsContractOrderResponse[],
  "has_more": boolean
}
```

The existing request without `offset` is equivalent to `offset=0`.

### 3. Contracts

- The authenticated token is the sole user identity source. Neither query
  parameter may select another user.
- Normalize `limit` with `route_limit` and `offset` with `route_offset` before
  entering persistence.
- SQL orders globally by `orders.created_at DESC, orders.id DESC` before applying
  `LIMIT` and `OFFSET`; the unique ID tie-breaker is mandatory for stable pages.
- Application requests one row beyond the normalized page size. It returns at
  most `limit` rows and sets `has_more=true` only when that extra row exists.
  Do not run a COUNT query for each mobile history page.
- Pagination is read-only. It never settles an expired order, mutates a wallet,
  or writes a ledger/event.
- Existing clients that read only `orders` remain compatible with the additive
  `has_more` field.

### 4. Validation & Error Matrix

| Condition | Required result |
|---|---|
| `limit` is absent, zero, or above 100 | Apply the shared default/clamp policy |
| `offset` is absent | Read the first page from offset zero |
| `offset` exceeds 100000 | Clamp to 100000 before SQL |
| A page has more than `limit` matching rows available | Return `limit` rows and `has_more=true` |
| A page is terminal or empty | Return the remaining/empty rows and `has_more=false` |
| Another user's newer order exists | Exclude it before sorting and pagination |
| Database read fails | Return the existing database error; do not fabricate an empty terminal page |

### 5. Good / Base / Bad Cases

- **Good**: two rows for the current user requested with `limit=1`; offset zero
  returns the newest row with `has_more=true`, offset one returns the older row
  with `has_more=false`.
- **Base**: an old client sends only `limit=100`, receives the first page and
  ignores the additional boolean.
- **Bad**: apply offset before `WHERE user_id = ?`; another user's rows change
  page boundaries and can hide current-user history.
- **Bad**: query exactly `limit` rows and infer `has_more` from page fullness;
  an exact-multiple terminal page then causes an unnecessary extra request.

### 6. Tests Required

- Route integration test with at least two current-user rows and one other-user
  row; assert two limit-one pages, order, `has_more`, timestamps, and isolation.
- Preserve unauthenticated rejection and the existing response field tests.
- Run seconds-contract route tests, `cargo fmt --all -- --check`, backend
  architecture tests, and `cargo check --all-targets` after production changes.

### 7. Wrong vs Correct

#### Wrong

```rust
SELECT ... FROM seconds_contract_orders
WHERE user_id = ?
ORDER BY created_at DESC
LIMIT ?
```

This cannot fetch a second page and is unstable when timestamps are equal.

#### Correct

```rust
SELECT ... FROM seconds_contract_orders
WHERE user_id = ?
ORDER BY created_at DESC, id DESC
LIMIT ? OFFSET ?
```

The application supplies `limit + 1`, truncates the response page, and derives
the continuation signal without an additional count query.

## Scenario: Safe Order Opening And Settlement Precision

### 1. Scope / Trigger

- Trigger: a user opens a seconds-contract order or manual/worker settlement credits a winning payout.
- Applies to Redis market data, product/pair/assets, wallet debit/credit, order snapshots, and ledger precision.

### 2. Signatures

- Open API: `POST /api/v1/seconds-contracts/orders` with `product_id`, optional `duration_seconds`, `direction`, `stake_amount`, and `idempotency_key`.
- Cache key: `market:ticker:{SANITIZED_SYMBOL}` with `last_price` and `observed_at`.
- DB snapshot: non-null `seconds_contract_orders.entry_price`; settlement persists `settlement_price`.
- Asset rule: `assets.precision_scale` controls stake and payout fractional digits.

### 3. Contracts

- Opening requires a positive Redis ticker observed within the last 60 seconds; missing Redis or a stale ticker is rejected before wallet debit and order insertion.
- The product, trading pair, stake asset, and both pair assets must all be active.
- `stake_amount` must fit the stake asset `precision_scale` in addition to the configured cycle limits.
- Manual and worker settlement use the same payout function.
- Winning payout (`stake + stake * payout_rate`) is truncated to the stake asset precision before wallet and ledger updates.
- Settlement persists the exact price used to determine the result in `settlement_price`.

### 4. Validation & Error Matrix

- Redis not configured, ticker missing, or ticker stale -> `VALIDATION_ERROR`, no order/wallet/ledger mutation.
- Ticker price zero or negative -> `VALIDATION_ERROR`.
- Product, pair, stake asset, base asset, or quote asset inactive -> `NOT_FOUND` before mutation.
- Stake exceeds asset precision or selected-cycle bounds -> `VALIDATION_ERROR`.
- Same idempotency key with different request -> `CONFLICT`.
- Legacy due order missing `entry_price` -> worker records a failed attempt and reschedules; it must not settle against an invented price.

### 5. Good/Base/Bad Cases

- Good: a 2-decimal stake asset pays `18.88` rather than storing `18.888...` in the wallet.
- Base: identical idempotent replay returns the existing order without requiring a current ticker or debiting again.
- Bad: inserting an opened order with `entry_price = NULL` creates an order the worker can never settle safely.
- Bad: calculating manual and worker payouts with different rounding rules creates wallet drift.

### 6. Tests Required

- Open tests cover missing Redis, missing/stale/non-positive ticker, and assert zero database mutations.
- Inactive pair/base/quote/stake asset tests reject before debit.
- Stake precision test rejects excessive decimals.
- Manual and worker settlement tests assert the same truncated payout, wallet balance, and ledger amount.
- Worker retry tests keep legacy missing-entry orders unsettled and scheduled for a later attempt.

### 7. Wrong vs Correct

#### Wrong

```rust
let entry_price = cached_price(redis).await.ok();
insert_open_order(entry_price).await?;
debit_wallet(stake).await?;
```

#### Correct

```rust
let entry_price = cached_entry_price(redis, product.pair_id, &product.symbol).await?;
validate_product_stake(&stake_amount, &product)?;
// Insert order, debit wallet, and ledger in one transaction only after validation.
```

## Scenario: Product Cycles

### 1. Scope / Trigger

- Trigger: seconds contract products now support multiple cycle configurations under one product.
- This is a cross-layer contract because the database, admin API, PC adapter, and order validation all share the cycle shape.

### 2. Signatures

- DB table: `seconds_contract_product_cycles(product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order)`.
- DB order snapshot: `seconds_contract_orders.duration_seconds`.
- DB order settlement snapshot: `seconds_contract_orders.settlement_price`.
- Admin create/update product API accepts either legacy single-cycle fields or `cycles`.
- User open order API accepts `product_id` and optional `duration_seconds`.

### 3. Contracts

- Product response must include legacy default fields plus `cycles`.
- Order response must include both numeric IDs and display symbols:
  - `pair_id` and `symbol`.
  - `stake_asset` and `stake_asset_symbol`.
- Admin order responses must include `email`, `symbol`, and nullable `settlement_price`; admin list UI must not expose `id`, `user_id`, or `product_id` as visible columns.
- Worker settlement must persist `seconds_contract_orders.settlement_price` from the cached ticker exit price before publishing the settlement event.
- `cycles[]` fields:
  - `duration_seconds`: positive integer seconds.
  - `payout_rate`: `DECIMAL(18,8)` compatible, non-negative.
  - `min_stake`: `DECIMAL(38,18)` compatible, positive.
  - `max_stake`: optional `DECIMAL(38,18)`, `null` means unlimited.
- The first normalized cycle is mirrored to the legacy product columns for old clients.
- PC order requests must send `product_id` plus `duration_seconds` when choosing a cycle from `cycles`.
- PC current-position filtering depends on `order.symbol`; do not return orders with only `pair_id`.
- PC seconds pair selector must use `/api/v1/seconds-contracts/products` as the tradable-pair source. It may enrich those symbols through per-symbol market ticker endpoints, but must not load `/api/v1/markets` to build the seconds pair list.
- `GET /api/v1/seconds-contracts/products` is a public active-product catalog so guest mobile/PC views can render truthful products and market data. User authentication remains mandatory for listing or opening orders; admin product and settlement routes retain admin scope.

### 4. Validation & Error Matrix

- Missing/empty `cycles` when no legacy fields exist -> validation error.
- Duplicate `duration_seconds` in one product payload -> validation error.
- `duration_seconds = 0` -> validation error.
- `max_stake < min_stake` -> validation error.
- Order `duration_seconds` not configured for product -> not found.
- Stake below/above the selected cycle limits -> validation error.

### 5. Good/Base/Bad Cases

- Good: one BTC-USDT product has 60s, 120s, and 300s cycles with independent payout and stake ranges.
- Base: an old client sends `duration_seconds`, `payout_rate`, `min_stake`, and `max_stake`; backend stores one cycle.
- Bad: creating separate products for 60s and 120s under the same trading pair; this makes PC cycle selection ambiguous.

### 6. Tests Required

- Route test: admin create/update accepts `cycles` and response includes all cycles.
- Route test: open order with `duration_seconds` uses the selected cycle payout and stake limits.
- Route test: order list/open responses include `symbol` and `stake_asset_symbol`.
- Route test: admin order list/detail responses include `email`, `symbol`, and `settlement_price`.
- Worker test: auto settlement stores and broadcasts `settlement_price`.
- Route test: unsupported cycle duration is rejected.
- Route test: the active-product catalog reaches the product use case without an Authorization header, while order routes still reject unauthenticated requests.
- PC adapter test: product `cycles` flatten to selectable PC cycles with `productId` preserved.
- PC adapter test: seconds products map to unique PC ticker rows; duplicate cycle products collapse to one symbol, disabled products are ignored, and product `logo_url` becomes ticker `icon`.
- Admin UI test: create/edit submits one product request with a `cycles` array.

### 7. Wrong vs Correct

#### Wrong

```json
[
  { "pair_id": 1, "duration_seconds": 60 },
  { "pair_id": 1, "duration_seconds": 120 }
]
```

```typescript
// PC seconds pair selector: wrong source, includes non-seconds spot markets.
export const fetchSecondSnapshot = fetchMarketSnapshot
```

#### Correct

```json
{
  "pair_id": 1,
  "cycles": [
    { "duration_seconds": 60, "payout_rate": "0.80000000", "min_stake": "10" },
    { "duration_seconds": 120, "payout_rate": "0.90000000", "min_stake": "20" }
  ]
}
```

```typescript
// PC seconds pair selector: product source controls which pairs are tradable.
const products = await GET("/api/v1/seconds-contracts/products")
const secondsPairs = unique(products.map(product => product.symbol))
```

## Scenario: Shared Spot Wallet Funding

### 1. Scope / Trigger

- Trigger: changing seconds-contract balances, opening debit, settlement credit, PC balance display, or adding any transfer-like action.
- Applies to Rust order application/infrastructure, manual and worker settlement, `wallet_accounts`, `wallet_ledger`, and the PC seconds API/Store.

### 2. Signatures

- Balance API: `GET /api/v1/wallet/accounts`.
- Open API: `POST /api/v1/seconds-contracts/orders`.
- Wallet persistence: `wallet_accounts(user_id, asset_id, available, frozen, locked)`.
- Ledger references: `ref_type = "seconds_contract_order"`.

### 3. Contracts

- Seconds contracts do not have a separate wallet or balance namespace.
- Opening debits the stake asset directly from the user's shared spot `wallet_accounts.available`.
- Winning manual and worker settlement credits the same shared wallet account.
- Wallet mutation, ledger entry, and order state must remain in one transaction.
- PC reads the shared wallet through `/wallet/accounts`.
- No seconds-contract transfer endpoint, transfer request type, Store method, or transfer UI may be introduced.

### 4. Validation & Error Matrix

- Shared wallet account missing -> wallet error and no order mutation.
- Shared wallet available balance below stake -> wallet error and no order/ledger mutation.
- Idempotent replay -> returns the existing order without a second debit.
- Any proposed SPOT/SECOND transfer request -> unsupported by design; remove the caller instead of adding a backend route.

### 5. Good/Base/Bad Cases

- Good: a user with `100 USDT` opens a `10 USDT` order and the shared account becomes `90 USDT`.
- Base: a losing order creates no settlement credit; a winning order credits the calculated payout to the same account.
- Bad: adding `seconds_wallet_accounts` creates duplicate balances and requires unnecessary reconciliation.
- Bad: retaining a client method that always rejects implies a transfer workflow that the product does not support.

### 6. Tests Required

- Route tests assert opening debits `wallet_accounts` once and writes `seconds_contract_open`.
- Manual and worker settlement tests assert payout credits `wallet_accounts` and writes the settlement ledger entry.
- PC contract tests assert `/wallet/accounts` remains the balance source and transfer symbols are absent.

### 7. Wrong vs Correct

#### Wrong

```typescript
await transferSecondFunds({ from: "SPOT", to: "SECOND", amount });
```

#### Correct

```typescript
const accounts = await GET("/api/v1/wallet/accounts");
await POST("/api/v1/seconds-contracts/orders", order);
```

## Scenario: Event-time Settlement Capability And Exception State

### 1. Scope / Trigger

- Trigger: activating a seconds product, opening an order, archiving a market tick, or retrying an expired order settlement.
- Applies to external-feed and strategy/internal trading pairs.

### 2. Signatures

```text
authoritative history: market_price_ticks
settlement evidence: one deterministic tick inside the order event-time window
exception terminal state: manual_review
```

### 3. Contracts

- Product activation and order opening call the same settlement-history-capability check.
- External pairs must be covered by an enabled feed configuration with a supported provider. Strategy/internal pairs must have a current runnable strategy, active immutable version, non-blank lease owner, and an unexpired archival-writer lease.
- Capability failure rejects activation/opening before the shared wallet is debited.
- Settlement reads append-only event-time snapshots. It never substitutes the current Redis ticker, processing time, a generated candle, or a guessed price for a missing historical tick.
- Tick selection is deterministic for an order window and records tick id, source, observed time, and price on the order.
- After the configured maximum wait, a still-unprovable order transitions atomically from `opened` to `manual_review`, records a stable failure code/time/window, and appends one settlement-exception record.
- `manual_review` is an operational terminal state for automatic settlement. It causes no guessed payout and no implicit refund. A future refund command must define commission reversal and strong idempotency separately.
- Repeated scans and worker restart cannot settle or exception the same order twice.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Pair lacks archival capability | Product activation/opening fails closed |
| Strategy run has no owner or an expired lease | Product activation/opening fails closed before debit |
| Valid event-time tick exists | Deterministic normal settlement |
| No tick but maximum wait not reached | Bounded retry, order remains opened |
| No tick after maximum wait | One transition to manual_review and one exception record |
| Redis has a current price but MySQL history is absent | Do not settle |
| Worker restarts after settlement/manual review | No duplicate wallet or audit effect |

### 5. Tests Required

- Route tests cover capability checks at both activation and opening, and prove failure occurs before debit.
- Worker tests cover deterministic tick choice, restart replay, normal win/loss settlement, bounded missing-tick retry, and one-time manual-review transition.
- Database assertions cover settlement provenance and the append-only exception record.
