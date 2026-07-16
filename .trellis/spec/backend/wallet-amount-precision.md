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
