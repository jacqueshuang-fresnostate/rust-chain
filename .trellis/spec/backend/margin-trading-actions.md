# Margin Trading Action Contracts

## Scenario: Safe Margin Actions And Wallet Settlement

### 1. Scope / Trigger

- Trigger: opening, ticker-filling, closing, canceling, bulk-operating, transferring funds, liquidating a position, or reading/updating user margin settings.
- Applies to user margin routes, authoritative market ingestion, margin wallet/ledger persistence, and liquidation/interest workers.

### 2. Signatures

- Open/position routes: `/api/v1/margin/positions`, `/close`, `/close-all`, `/cancel`, `/cancel-all`.
- Transfer: `POST /api/v1/margin/transfers` with asset, `from`, `to`, `amount`, and optional `idempotency_key`.
- Transfer eligibility: `assets.margin_transfer_enabled`; new assets default to `FALSE` and the admin asset API owns this flag.
- Margin wallet catalog: `GET /api/v1/margin/wallets` returns `asset_id`, `asset_symbol`, optional `logo_url`, `margin_transfer_enabled`, and the three balance buckets.
- Settings: `GET /api/v1/margin/settings/{product_id}` plus leverage/mode PATCH routes.
- Persistence: `margin_positions.wallet_scope` and `margin_transfers(user_id, idempotency_key, transfer_id, request fields)`.
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
- `wallet_scope` snapshots whether collateral came from spot or margin. Close, cancel, and liquidation return funds to that same scope.
- Position state, wallet balance, and ledger entry commit in one transaction.
- Margin open idempotency compares product, direction, explicit mode, margin, leverage, normalized order type, and optional limit price. Same-key same-request replay returns the original pending or filled row without another debit; any order-type or limit-price change is a conflict.
- Transfers lock spot then margin wallet in both directions, update both balances and ledgers atomically, and validate asset precision.
- A new `spot -> margin` transfer requires an active asset with `margin_transfer_enabled = TRUE`. Disabling the flag blocks only new inbound transfers; an existing margin balance remains visible and may still move `margin -> spot`.
- Margin-wallet reads include every active asset whose transfer flag is enabled, using zero balance buckets when the user has no lazy-created wallet row. They also retain every existing user margin-wallet row after the flag is disabled so configuration changes never hide stored funds.
- Same user/key/request replay returns the original `transfer_id` and original post-transfer ledger snapshots without moving funds again, even if the asset later becomes inactive.
- Same key with different asset, direction, or amount returns conflict.
- User leverage must be a configured product level. Persisted settings are readable through the GET route.
- Product listing returns a capability envelope. Implemented values are `order_types=["market", "limit"]` and `margin_modes=["isolated", "cross"]`; clients render only those advertised order types, while missing-order-type PC requests remain market-compatible.
- `cross` accounts are scoped by `(user_id, margin_asset)`. All open cross positions using that asset share wallet equity, initial margin, unrealized PnL, and accrued interest.
- Cross equity is `wallet_equity + sum(filled_open_position.margin_amount) + sum(unrealized_pnl) - sum(interest_amount)`; maintenance margin is the sum of each filled position's notional times its configured maintenance rate. Pending limits are absent from both sums.
- A cross account is liquidated as one unit when combined equity is less than or equal to combined maintenance margin. The worker locks all account positions in one transaction, settles each payout, writes each liquidation record, and closes every open cross position in that account.
- The margin wallet response includes `cross_accounts[]` with `equity`, `unrealized_pnl`, `interest_amount`, `maintenance_margin`, and optional `margin_ratio` from the latest worker snapshot.
- Cross interest is accrued per position and aggregated into `margin_cross_accounts.last_interest_amount`; close and liquidation deduct the position's accrued interest from settlement.
- Bulk actions have no silent 100-row cap, reuse single-item idempotent transactions, continue after failures, and return `failures`.

### 4. Validation & Error Matrix

- Missing/stale/non-positive ticker on open/close -> `VALIDATION_ERROR`, no financial mutation.
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
- Unknown `wallet_scope` on close/cancel/liquidation -> `VALIDATION_ERROR`; never default to spot.
- One bulk item fails -> include its id/code/message and continue later items.

### 5. Good/Base/Bad Cases

- Good: margin-funded position closes back into `margin_wallet_accounts` with a margin ledger row.
- Good: a long limit at 90 while the server ticker is 100 reserves collateral with null entry price; an accepted ticker at 89 fills once at 89, moves `opened_at` to that fill time, starts interest then, inserts one commission, and emits one event.
- Base: the same pending-limit request/key replays the original row, and a cancel racing the trigger wins or loses under the position lock without double refund/fill.
- Good: reverse transfer replay after asset disable returns original snapshots and creates no extra ledgers.
- Good: disabling margin transfer rejects a new inbound request while the user's existing wallet row stays visible and can transfer back to spot.
- Base: an enabled asset without a user margin-wallet row appears in `/margin/wallets` with three zero buckets; the read does not create a database row.
- Base: a second close/cancel sees the terminal position and does not credit twice.
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
- Liquidation worker test asserts payout uses recorded `wallet_scope`.
- Bulk tests process more than 100 rows, retain prior successes/events, report a failed row, and continue to later rows.
- Settings tests cover user isolation, leverage round-trip, mode round-trip, and cross acceptance for a product configured with `cross`.
- Domain tests cover combined PnL/interest/maintenance arithmetic and the equality liquidation boundary; worker integration tests must assert all same-asset cross positions close in one account liquidation.

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
// cross risk is evaluated by (user_id, margin_asset), never by position alone.
evaluate_cross_margin(&wallet_equity, &position_margin, &position_risks);
// limit price only decides the boundary; the accepted server ticker is the fill.
if margin_limit_order_is_triggered(direction, limit_price, accepted_ticker)? {
    mark_margin_limit_position_filled(tx, position.id, accepted_ticker).await?;
}
```

Settlement follows the recorded funding scope, and unsupported risk semantics fail explicitly.
