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
  fee, immutable Margin partial-close executions as slice
  `realized_pnl - close_interest_amount`, terminal Margin rows for only the
  remaining unrecorded slice, and Earn `earn_redeem` credit minus principal.
  An opened position with a committed partial-close execution is realized
  activity even though its remainder stays open. Exclude deposits, withdrawals,
  transfers, canceled/pending margin positions, spot cost basis, and unrealized
  PnL.
- Explicit terminal closes are owned by their `fully_closed` execution and must
  not also contribute their terminal position row. A legacy close or liquidation
  after earlier partial executions contributes cumulative terminal PnL minus all
  earlier execution PnL and the remaining position interest, with the remaining
  position margin as basis. This keeps each slice on its actual UTC execution or
  terminal day and prevents duplicate amount or basis.
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
- Reuse the Today formulas and exclusions, but aggregate the five auditable fact
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

- Whitelist/auth, exact N-day UTC continuity, all five fact formulas and
  exclusions, opened partial-close inclusion, explicit-terminal deduplication,
  legacy terminal-remainder attribution, historical close/current ticker
  separation, no-activity zero, missing-price propagation, 18-place
  serialization, and first-partial cumulative invalidation.

## Scenario: Categorized Wallet Ledger Query

### Contracts

- `GET /api/v1/wallet/ledger` accepts optional `category` values exactly:
  `funding`, `spot`, `margin`, `seconds`, `convert`, `earn`, `new_coin`, `loan`,
  `prediction`, and `other`. Omission preserves the existing all-row behavior;
  existing exact `change_type` and the other filters remain compatible and may
  be combined with category.
- The same endpoint reads both `wallet_ledger` and
  `margin_wallet_ledger` through one `UNION ALL` read model. Every row returns
  authoritative `account_type=spot|margin`, and the optional account filter is
  exactly `all|spot|margin`; omission means `all`.
- Account source and business category are independent dimensions. A
  `margin_*` row may belong to either account, so neither backend nor clients
  infer account source from `change_type`.
- Validate `account_type` before acquiring the MySQL pool, just like category.
  List and COUNT apply the same account/category/field predicates to the
  combined source.
- Apply pagination only after global deterministic ordering by
  `created_at DESC`, account type, and row ID descending. The two physical
  tables have independent ID sequences, so consumers use `(account_type, id)`
  as the row identity.
- Parse and whitelist category before acquiring the MySQL pool so malformed
  input deterministically returns HTTP 400 even when database state is absent.
- Funding is exact `deposit`, `admin_recharge`, or `quick_recharge`, plus
  `deposit_` and `withdrawal_` prefixes. The other named categories use their
  audited case-sensitive prefixes; `other` is the exact negation of every
  named rule.
- List and COUNT queries call the same category/filter predicate builder. The
  Rust response classifier uses the same exact/prefix rule table, and every
  entry returns one authoritative category; future unknown values return
  `other`.
- Category filtering and response classification do not alter ledger
  `amount`, `balance_after`, or `fee` decimal serialization.

### Tests Required

- Whitelist and before-database validation for both category and account,
  exact `change_type` compatibility, exact/prefix/other classification, shared
  row/COUNT predicate construction, two-table union, globally stable ordering,
  overlapping numeric IDs, combined filters, filtered total/page metadata,
  authoritative response category/account source, and unchanged 18-place
  decimal serialization.

## Scenario: Precision-Aware Wallet Ledger Filtering

### 1. Scope / Trigger

- Apply this contract when changing `GET /api/v1/wallet/ledger`, its query DTO,
  the combined spot/margin ledger read model, or the asset precision returned
  to clients. It prevents page-local filtering, row/count drift, implicit MySQL
  time conversion, and display precision inferred from `DECIMAL(38,18)` text.

### 2. Signatures

```http
GET /api/v1/wallet/ledger?asset_symbol=BTC&direction=credit&start_time=2026-09-01%2000:00:00.000&end_time=2026-09-01%2023:59:59.999&limit=30&offset=0
```

```rust
struct WalletLedgerFilter {
    asset_symbol: Option<String>,
    direction: WalletLedgerDirection, // all | credit | debit
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    limit: u32,
    offset: u32,
    // existing account/category/reference filters remain composable
}

struct WalletLedgerEntryResponse {
    symbol: String,
    precision_scale: i32,
    amount: BigDecimal,
    balance_after: BigDecimal,
    fee: BigDecimal,
    // existing authoritative fields remain unchanged
}
```

- The row query joins `assets` and selects `a.precision_scale`; both row and
  count queries use the same `WalletLedgerFilter` and shared predicate builder.

### 3. Contracts

- `direction` is exactly `all|credit|debit`. Credit means `amount > 0`, debit
  means `amount < 0`, and zero-valued rows are visible only for `all`.
- `start_time` and `end_time` are inclusive UTC boundaries. Accept RFC3339 or
  `YYYY-MM-DD HH:MM:SS[.fraction]`, parse before pool access, then bind typed
  `DateTime<Utc>` values through SQLx.
- Asset, direction, time, account, category, change-type, and reference filters
  compose with AND semantics and execute before global ordering and pagination.
- List and COUNT must use the same combined ledger source and the same predicate
  builder. Empty results return `total_pages=1`; otherwise total pages are the
  ceiling of filtered rows divided by page size.
- Every response row carries the joined asset's authoritative
  `precision_scale` in `0..=18`. An out-of-range stored value is an internal
  data-contract failure, not a value to clamp or infer.
- This read model never rounds or recalculates `amount`, `balance_after`, fee,
  or bucket snapshots; precision metadata is additive presentation context.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Omitted direction/time | Normalize to `all` and unbounded time |
| Direction outside `all|credit|debit` | HTTP 400 validation error before pool access |
| Empty optional time text | Normalize to no boundary |
| Malformed time text | HTTP 400 validation error before pool access |
| `start_time > end_time` | HTTP 400 validation error before pool access |
| Invalid asset symbol | HTTP 400 validation error before pool access |
| Stored `precision_scale < 0` or `> 18` | Internal data-contract error; emit no malformed row |
| Filtered result has zero rows | Empty `entries`, `total_elements=0`, `total_pages=1` |

### 5. Good / Base / Bad Cases

- Good: combine BTC, credit, UTC range, account, and category filters; SQL
  returns only matching rows, COUNT describes the same set, and each row carries
  BTC's stored precision.
- Base: omit all optional filters; return the existing globally ordered combined
  spot/margin ledger without changing decimal values.
- Bad: fetch an unfiltered page and remove debit rows in Rust or the client,
  because page totals and later offsets then describe a different collection.
- Bad: derive display precision by counting the 18 fractional storage digits or
  silently clamp a damaged `precision_scale`.

### 6. Tests Required

- Unit-test direction whitelist/defaults, both supported time syntaxes, empty
  optional values, malformed/reversed boundaries, and validation before pool
  acquisition.
- Assert generated row and COUNT SQL share asset/direction/time predicates and
  use typed time binds; cover positive, negative, and zero amount semantics.
- Assert response mapping preserves exact BigDecimal text, emits stored precision
  for both ledger sources, and rejects precision outside `0..=18`.
- Integration-test combined filters, filtered pagination metadata, and the empty
  page contract against MySQL when `DATABASE_URL` is available.

### 7. Wrong vs Correct

#### Wrong

```rust
let rows = fetch_page_without_direction(pool, offset, limit).await?;
let entries = rows.into_iter().filter(|row| row.amount > 0).collect();
let precision_scale = 18; // inferred from storage schema
```

#### Correct

```rust
push_wallet_ledger_filters(&mut row_query, &filter);
push_wallet_ledger_filters(&mut count_query, &filter);
let precision_scale = row.precision_scale;
if !(0..=18).contains(&precision_scale) {
    return Err(AppError::Internal("invalid asset precision".into()));
}
```
