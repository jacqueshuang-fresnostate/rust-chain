# Margin Trading Action Contracts

## Scenario: Safe Margin Actions And Wallet Settlement

### 1. Scope / Trigger

- Trigger: opening, closing, canceling, bulk-operating, transferring funds, liquidating a position, or reading/updating user margin settings.
- Applies to user margin routes, margin wallet/ledger persistence, and liquidation/interest workers.

### 2. Signatures

- Open/position routes: `/api/v1/margin/positions`, `/close`, `/close-all`, `/cancel`, `/cancel-all`.
- Transfer: `POST /api/v1/margin/transfers` with asset, `from`, `to`, `amount`, and optional `idempotency_key`.
- Settings: `GET /api/v1/margin/settings/{product_id}` plus leverage/mode PATCH routes.
- Persistence: `margin_positions.wallet_scope` and `margin_transfers(user_id, idempotency_key, transfer_id, request fields)`.
- Cross account persistence: `margin_cross_accounts(user_id, margin_asset)` stores the latest account-level equity, PnL, interest, maintenance margin, ratio, status, and version.
- Market cache: `market:ticker:{SANITIZED_SYMBOL}`, positive price observed within 60 seconds.

### 3. Contracts

- Opening and closing require a fresh positive server ticker. A missing opening ticker must fail before position insertion or collateral debit.
- Legacy `entry_price = NULL` rows remain cancelable; API-created positions must always have an entry price.
- `wallet_scope` snapshots whether collateral came from spot or margin. Close, cancel, and liquidation return funds to that same scope.
- Position state, wallet balance, and ledger entry commit in one transaction.
- Transfers lock spot then margin wallet in both directions, update both balances and ledgers atomically, and validate asset precision.
- Same user/key/request replay returns the original `transfer_id` and original post-transfer ledger snapshots without moving funds again, even if the asset later becomes inactive.
- Same key with different asset, direction, or amount returns conflict.
- User leverage must be a configured product level. Persisted settings are readable through the GET route.
- Product listing returns a capability envelope. Implemented values are `order_types=["market"]` and `margin_modes=["isolated", "cross"]`; PC, mobile, and admin configuration must not advertise limit or trigger behavior.
- `cross` accounts are scoped by `(user_id, margin_asset)`. All open cross positions using that asset share wallet equity, initial margin, unrealized PnL, and accrued interest.
- Cross equity is `wallet_equity + sum(open_position.margin_amount) + sum(unrealized_pnl) - sum(interest_amount)`; maintenance margin is the sum of each position's notional times its configured maintenance rate.
- A cross account is liquidated as one unit when combined equity is less than or equal to combined maintenance margin. The worker locks all account positions in one transaction, settles each payout, writes each liquidation record, and closes every open cross position in that account.
- The margin wallet response includes `cross_accounts[]` with `equity`, `unrealized_pnl`, `interest_amount`, `maintenance_margin`, and optional `margin_ratio` from the latest worker snapshot.
- Cross interest is accrued per position and aggregated into `margin_cross_accounts.last_interest_amount`; close and liquidation deduct the position's accrued interest from settlement.
- Bulk actions have no silent 100-row cap, reuse single-item idempotent transactions, continue after failures, and return `failures`.

### 4. Validation & Error Matrix

- Missing/stale/non-positive ticker on open/close -> `VALIDATION_ERROR`, no financial mutation.
- Unsupported leverage or margin mode -> `VALIDATION_ERROR`.
- `order_type` other than `market`, or a market open request that carries `price`/`trigger_price` -> `VALIDATION_ERROR`.
- Unknown margin mode -> `VALIDATION_ERROR`; `cross` is accepted only when the product includes it in `margin_modes`.
- Transfer source equals target or account name is unsupported -> `VALIDATION_ERROR`.
- Transfer amount non-positive or exceeds asset precision -> `VALIDATION_ERROR`.
- Insufficient source available balance -> `VALIDATION_ERROR`, no opposite-side credit.
- Same idempotency key with different request -> `CONFLICT`.
- Unknown `wallet_scope` on close/cancel/liquidation -> `VALIDATION_ERROR`; never default to spot.
- One bulk item fails -> include its id/code/message and continue later items.

### 5. Good/Base/Bad Cases

- Good: margin-funded position closes back into `margin_wallet_accounts` with a margin ledger row.
- Good: reverse transfer replay after asset disable returns original snapshots and creates no extra ledgers.
- Base: a second close/cancel sees the terminal position and does not credit twice.
- Bad: opposite transfer directions lock wallets in different orders; this creates a deadlock window.
- Bad: evaluating a cross position independently can liquidate one position while leaving the shared account under-collateralized.

### 6. Tests Required

- Fresh/stale/missing ticker open tests assert zero position, wallet, and ledger mutations on failure.
- Transfer tests cover both directions, precision, insufficient balance, same-key replay, changed-request conflict, asset-disable replay, and ledger counts.
- Close/cancel tests assert balance, ledger, status, and idempotent retry for both wallet scopes.
- Liquidation worker test asserts payout uses recorded `wallet_scope`.
- Bulk tests process more than 100 rows, retain prior successes/events, report a failed row, and continue to later rows.
- Settings tests cover user isolation, leverage round-trip, mode round-trip, and cross acceptance for a product configured with `cross`.
- Domain tests cover combined PnL/interest/maintenance arithmetic and the equality liquidation boundary; worker integration tests must assert all same-asset cross positions close in one account liquidation.

### 7. Wrong vs Correct

#### Wrong

```rust
// Always credits spot and mislabels cross as supported.
credit_spot_wallet(...);
let mode = "cross";
```

#### Correct

```rust
credit_margin_position_amount(tx, user_id, asset_id, &position.wallet_scope, amount, change_type, position.id).await?;
ensure_supported_user_margin_mode(&requested_mode)?;
// cross risk is evaluated by (user_id, margin_asset), never by position alone.
evaluate_cross_margin(&wallet_equity, &position_margin, &position_risks);
```

Settlement follows the recorded funding scope, and unsupported risk semantics fail explicitly.
