# Wallet Amount Precision Contract

## Scenario: Calculated Wallet Amounts

### 1. Scope / Trigger

- Trigger: A route or worker calculates a wallet amount from rate, price, fee, payout, APR, or other decimal arithmetic before writing `wallet_accounts` or `wallet_ledger`.
- Applies to calculated amounts crossing API, Redis/cache, MySQL persistence, and wallet ledger snapshots.

### 2. Source Of Truth

- `assets.precision_scale` is the asset amount precision contract.
- Valid `precision_scale` range is `0..=18`.
- `wallet_accounts` and `wallet_ledger` stay `DECIMAL(38,18)` for storage compatibility; calculation code must still quantize business amounts before writing.

### 3. Contracts

- User-submitted source amounts must fit the source asset `precision_scale`; trailing zeros do not count as extra precision.
- Generated target amounts must be truncated toward zero to the target asset `precision_scale` before they are returned to the user, cached, inserted into order/quote tables, and written to wallet balances.
- Fee amounts denominated in the source asset must be truncated toward zero to the source asset `precision_scale`.
- Tiered agent commission must quantize cumulative payout amounts to the stored `payout_asset_id` precision before deriving each level's differential amount; do not quantize independently calculated differential rates.
- Wallet ledger `amount`, `balance_after`, and account snapshot fields must match the quantized wallet account values for the affected asset.

### 4. Wrong vs Correct

#### Wrong

```rust
let to_amount = (from_amount * effective_rate).with_scale(18);
```

#### Correct

```rust
let to_amount = truncate_amount_to_asset_precision(&raw_to_amount, to_asset.precision_scale);
```

### 5. Tests Required

- Regression tests for any path that uses division or fee/rate arithmetic and then credits an asset wallet.
- Tests should use a target asset with `precision_scale = 8` and an arithmetic result that would naturally produce more than 8 fractional digits.

## Scenario: Tiered Withdrawal Fees

### 1. Scope / Trigger

- Trigger: asset withdrawal fee configuration, user withdrawal asset listing, or user withdrawal request creation.
- Applies to `assets.withdraw_fee`, `assets.withdraw_fee_tiers_json`, `/api/v1/wallet/withdraw-assets`, and `/api/v1/wallet/withdrawals`.

### 2. Source Of Truth

- Fixed fallback fee: `assets.withdraw_fee`.
- Tiered fee rules: `assets.withdraw_fee_tiers_json`.
- Rule shape: `{ min_amount, max_amount, fee_rate_percent }`.

### 3. Contracts

- `fee_rate_percent` is a human percent value, so `1` means `1%`.
- `max_amount = null` means no upper bound.
- Tier matching uses `min_amount <= amount < max_amount`; this allows adjacent ranges such as `1-100` and `100-500` without double matching at `100`.
- Tier arrays are normalized by ascending `min_amount` and rejected if ranges overlap or an open-ended range is not last.
- If no tier is configured or no tier matches the amount, the backend uses fixed `withdraw_fee`.
- Calculated withdrawal fees are truncated to the asset `precision_scale` before storage.

### 4. Tests Required

- Unit-test range matching, boundary behavior, fallback behavior, and overlap rejection.
- Route-test that a withdrawal request stores the server-calculated tiered fee, not the client-submitted `fee`.
- Frontend tests should cover PC fee preview and admin asset payload round-trip.

## Scenario: UTC Realized Today Return

### Contracts

- `GET /api/v1/wallet/today-return` requires `UserAuth` and derives the user ID
  only from the token.
- Aggregate UTC-day Seconds win/loss, Prediction payout/refund net of stake and
  fee, Margin `closed` and `liquidated` rows as
  `realized_pnl - interest_amount`, and Earn `earn_redeem` credit minus
  principal. Exclude deposits, withdrawals, transfers, canceled/open margin
  positions, spot cost basis, and unrealized PnL.
- Basis is Seconds stake, Prediction stake plus fee, Margin margin amount, and
  Earn subscribed principal. A refunded Prediction order keeps its original
  stake-plus-fee basis; refund and fee-refund change return amount, not the
  capital basis.
- Report in USDT. USDT, USDC, and USD use parity; every other non-zero
  amount/basis requires the current `{ASSET}USDT` Redis ticker.
- A current ticker must have the expected `{ASSET}USDT` symbol, a positive
  decimal `last_price`, and numeric `observed_at` within 60 seconds of the
  calculation cut-off. Missing, malformed, mismatched, non-positive, stale, or
  future tickers produce `status=partial` plus sorted
  `missing_price_assets`.
- Join Earn redemption ledger rows by user, asset, ref type, and subscription
  id. If historical data contains duplicate `earn_redeem` rows for one
  subscription, only the earliest authoritative row contributes so principal
  and yield are not counted twice.
- No realized activity is `status=complete` with true zero amount, basis, and
  rate even when Redis is absent.
- Reporting amounts and rate are truncated toward zero to 18 digits; rate is
  `amount / basis_amount` for positive basis, otherwise zero.

### Tests Required

- Authentication/current-user scope for every union branch, UTC boundaries,
  positive/negative/zero, Prediction refunds, manual close and liquidation
  interest, Earn duplicate-ledger isolation, stablecoin parity, fresh/invalid/
  stale Redis valuation, partial status, and excluded cash flows.

## Scenario: UTC Realized Return History

### Contracts

- `GET /api/v1/wallet/return-history?days=1|7|30|180` requires `UserAuth`;
  missing, malformed, or non-whitelisted `days` is HTTP 400.
- Reuse the Today formulas and exclusions, but aggregate the four terminal fact
  sources by UTC day and asset. Return exactly N ascending UTC-day points and
  fill inactive days with complete 18-place zero decimals.
- USDT, USDC, and USD use parity. A past non-stable activity uses only the
  exact `{ASSET}USDT` Mongo `1d` candle whose `open_time` is that UTC midnight;
  current-day activity uses only the existing strict 60-second Redis ticker.
  Inactive days do not read either price source.
- A missing required price makes that point's amount, basis, and rate null.
  Cumulative amount is null from the first partial point onward; any partial
  point makes the top summary null and status partial.
- Quantize every valued daily amount, daily basis, cumulative amount, summary,
  and rate toward zero to 18 places. The complete summary amount must equal the
  final cumulative amount exactly.

### Tests Required

- Whitelist/auth, exact N-day UTC continuity, all four fact formulas and
  exclusions, historical close/current ticker separation, no-activity zero,
  missing-price propagation, 18-place serialization, and first-partial
  cumulative invalidation.
