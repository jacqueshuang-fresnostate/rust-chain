# Earn Product Contracts

## Scenario: Configurable Product Categories

### 1. Scope / Trigger

- Trigger: earn product categories change database schema, admin API request/response fields, admin product forms, and product list/detail display fields.
- Applies to: `earn_product_categories`, `earn_products.category`, `/admin/api/v1/earn/categories`, `/admin/api/v1/earn/products`, and `/api/v1/earn/products`.

### 2. Signatures

- DB category fields:
  - `code VARCHAR(64) NOT NULL UNIQUE`
  - `name_json JSON NOT NULL`
  - `sort_order INT NOT NULL DEFAULT 0`
  - `status VARCHAR(32) NOT NULL DEFAULT 'active'`
  - `created_at` / `updated_at`
- Category multilingual JSON shape:

```json
{
  "version": 1,
  "default_locale": "zh-CN",
  "items": [
    { "locale": "zh-CN", "country": "CN", "title": "定期" }
  ]
}
```

- Product responses include `category`, `category_name`, and `category_name_json`.
- Admin category endpoints support list, create, detail, update, and status update.

### 3. Contracts

- `earn_products.category` remains a stable category code string. Do not migrate it to a numeric foreign key unless a separate compatibility plan exists.
- Category `code` is immutable after creation. Admin edits may change multilingual names, sort order, and status.
- Product create/update must validate that the submitted category code exists in `earn_product_categories` and has `status = 'active'`.
- Product list/detail should left join category metadata and fall back to the raw category code when historical products reference missing or disabled categories.
- Admin product forms must load category options from `/admin/api/v1/earn/categories?status=active` instead of hard-coding category labels.
- Seeds must include the legacy category codes `fixed_term`, `flexible`, `structured`, and `staking`, and migrations should backfill distinct existing product category codes.

### 4. Validation & Error Matrix

- Blank category code -> validation error.
- Invalid category code characters -> validation error.
- Duplicate category code -> conflict or validation error.
- Blank multilingual title list -> validation error.
- Product create/update with unknown category code -> validation error.
- Product create/update with disabled category code -> validation error.

### 5. Good/Base/Bad Cases

- Good: admin creates category `premium` with CN and US names, then product creation selects `premium`; product list returns `category = premium`, configured `category_name`, and full `category_name_json`.
- Base: legacy `fixed_term` products continue to display because migration seeds `fixed_term`.
- Bad: frontend keeps a hard-coded category enum and silently diverges from backend-configured category columns.

### 6. Tests Required

- Admin route tests cover category create/list/detail/update/status and audit records.
- Product create/update tests assert active category validation and `category_name` / `category_name_json` response fields.
- Admin resource tests cover the category page, row actions, and product form loading active category options.
- Route/sidebar tests cover the Earn category navigation entry.

### 7. Wrong vs Correct

#### Wrong

```tsx
const earnProductCategoryOptions = [
  { value: "fixed_term", label: "定期" },
  { value: "flexible", label: "活期" },
];
```

This makes the admin UI impossible to keep aligned with configured columns.

#### Correct

```tsx
const categoryOptions = useEarnCategoryOptions();
<AdminSelect field="category" optionList={categoryOptions} filter />;
```

The UI reads active categories from the backend and stores the stable category code on the product.

## Scenario: Product-Level Redemption Fees

### 1. Scope / Trigger

- Trigger: earn product configuration changes database schema, admin API request/response fields, user subscription snapshots, and redemption settlement.
- Applies to: `earn_products`, `earn_subscriptions`, `/earn/products`, `/earn/subscriptions/:id/redeem`, and the earn auto-redemption worker.

### 2. Signatures

- DB product fields:
  - `redemption_fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0`
  - `maturity_profit_fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0`
  - `early_redeem_fee_basis VARCHAR(32) NOT NULL DEFAULT 'none'`
  - `early_redeem_fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0`
- DB subscription snapshot fields use the same names and constraints.
- Admin create/update payloads may include the same four fields. Omitted values default to `0` and `none`.

### 3. Contracts

- Fee rates are decimal fractions from `0` to `1`; `0.02` means 2%.
- `early_redeem_fee_basis` values:
  - `none`: no early redemption fee; backend normalizes `early_redeem_fee_rate` to `0`.
  - `principal`: early fee is `principal_amount * early_redeem_fee_rate`.
  - `profit`: early fee is accrued yield multiplied by `early_redeem_fee_rate`.
- Subscription creation must snapshot product fee fields into `earn_subscriptions`. Later product edits must not change existing subscription settlement.
- Manual redemption and auto maturity redemption must call the same calculation helper.
- `yield_amount` means net profit after profit-based fees, not net wallet delta above principal. `redeem_amount` is the final wallet credit after all fees.

### 4. Validation & Error Matrix

- Missing fee fields -> default `0` / `none`.
- Fee rate `< 0` -> validation error.
- Fee rate `> 1` -> validation error.
- Fee rate scale greater than 8 -> validation error.
- Unknown `early_redeem_fee_basis` -> validation error.

### 5. Good/Base/Bad Cases

- Good: product has `maturity_profit_fee_rate = 0.1`; a matured subscription with `2` gross yield credits `1.8` yield and `21.8` total redeem on `20` principal.
- Base: all fees default to zero; settlement matches the original principal plus full term yield.
- Bad: calculating auto-redemption fees separately from manual redemption; this causes drift between user-triggered and worker-triggered settlement.

### 6. Tests Required

- Admin product create/update/list returns and audits all fee fields.
- User early redemption succeeds and applies principal/profit basis fees from the subscription snapshot.
- Matured redemption preserves zero-fee legacy behavior.
- Auto-redemption applies `maturity_profit_fee_rate`.
- Shared calculation helper has unit tests for maturity profit fees and early redemption fees.

### 7. Wrong vs Correct

#### Wrong

```rust
let product = load_product(subscription.product_id).await?;
let redeem_amount = calculate_with_current_product(product, subscription);
```

This lets later admin product edits alter existing subscriptions.

#### Correct

```rust
let amounts = calculate_earn_redemption_amounts(EarnRedemptionTerms {
    redemption_fee_rate: &subscription.redemption_fee_rate,
    maturity_profit_fee_rate: &subscription.maturity_profit_fee_rate,
    early_redeem_fee_basis: &subscription.early_redeem_fee_basis,
    early_redeem_fee_rate: &subscription.early_redeem_fee_rate,
    ..terms
}, now);
```

Settlement uses the subscription snapshot and stays stable for the user.

## Scenario: Asset Precision And Platform Counter-Legs

### 1. Scope / Trigger

- Trigger: earn subscription or redemption amount enters, or leaves, a user wallet; or earn settlement starts writing the platform book.
- Applies to `earn/service.rs` validation, `earn/redemption.rs` quantization, `earn/journal.rs` leg construction, `earn/application.rs`, and the earn auto-redemption worker.

### 2. Signatures

- `validate_amount_asset_precision(amount, precision_scale) -> AppResult<()>` rejects amounts a lower-precision asset cannot express.
- `redemption_amounts_for_subscription(subscription, now, asset_precision_scale) -> EarnRedemptionAmounts`.
- `earn_subscription_journal_legs(principal) -> Vec<EarnPlatformJournalLeg>`.
- `earn_redemption_journal_legs(principal, gross_yield, fee_amount, redeem_amount) -> Vec<EarnPlatformJournalLeg>`.
- `insert_earn_platform_journal_legs_in_tx(tx, transaction_key, asset_id, subscription_id, legs)`.

### 3. Contracts

- Subscription time: the user-supplied amount must fit `assets.precision_scale` in full. Over-precision input is rejected, never silently truncated, so the submitted amount equals the debited amount.
- Redemption time: calculated amounts are truncated toward zero to `assets.precision_scale` before they reach the wallet, so `wallet_ledger.amount` always equals the real `available` delta.
- Quantized fields keep `redeem_amount == principal_amount + gross_yield_amount - fee_amount` exactly; the zero-floor clamp is the only exception.
- `yield_amount` is display-only and may differ from the fee breakdown in the last digit.
- Every subscription writes `platform_earn_cash_received` (`+principal`) and `earn_principal_payable_open` (`-principal`).
- Every redemption writes `earn_principal_payable_close` (`+principal`), `platform_earn_redemption_cash` (`-redeem_amount`), plus `earn_yield_expense` (`+gross_yield`) and `platform_earn_fee_income` (`-fee_amount`) when non-zero.
- Legs inside one `transaction_key` must sum to zero; zero-amount legs are omitted because the journal table rejects zero amounts.
- `transaction_key` is `earn_subscribe:{subscription_id}` or `earn_redeem:{subscription_id}`.
- Manual redemption and auto redemption must apply the same quantization and write the same legs.
- The fee income leg is capped at `principal + gross_yield`. Stacked fee rates can exceed the whole position, in which case the wallet credit floors at zero and the platform can only recognize fee income up to the value of the position.

### 4. Validation & Error Matrix

- Amount needs more decimals than the asset `precision_scale` -> validation error, no subscription row, no debit.
- `assets` row missing -> not found, transaction aborts.
- Platform leg insert conflict on `(transaction_key, account_code, asset_id)` -> whole transaction rolls back; no user leg is committed alone.
- Zero-amount leg requested -> filtered out before insert, never sent to SQL.
- Legs that do not sum to zero at insert time -> internal error, transaction aborts.

### 5. Good/Base/Bad Cases

- Good: a two-decimal asset redeems `100` principal with `3.333` gross yield and `0.103333` fee; quantized values become `100.00`, `3.33`, `0.10`, and the wallet receives `103.23`.
- Base: zero yield and zero fee produce only the payable-close and cash legs, which still sum to zero.
- Bad: crediting the 18-decimal value to a two-decimal wallet, or writing only the user ledger row without the platform counter-legs.
- Edge: fees of `1200` against `100` principal plus `1000` yield floor the wallet credit at zero, and the capped fee leg still leaves the entry balanced.

### 6. Tests Required

- Unit tests assert leg composition, ordering, zero-leg omission, and a zero sum for both directions.
- Unit test asserts the capped fee keeps the entry balanced when fees exceed the whole position.
- Unit test asserts quantization truncates toward zero and preserves the redeem identity.
- Unit test asserts over-precision subscription amounts are rejected at `precision_scale` 0 and 2.
- Source assertions keep both the application and the auto-redemption worker wired to quantization and platform legs.

### 7. Wrong vs Correct

#### Wrong

```rust
let amounts = calculate_earn_redemption_amounts(terms, now);
credit_wallet_for_redemption_in_tx(&mut tx, &subscription, &wallet, &amounts.redeem_amount).await?;
```

This credits an 18-decimal figure to an asset that may only express two, and leaves the platform book without a counter-leg.

#### Correct

```rust
let precision = load_asset_precision_in_tx(&mut tx, subscription.asset_id).await?;
let amounts = redemption_amounts_for_subscription(&subscription, now, precision);
credit_wallet_for_redemption_in_tx(&mut tx, &subscription, &wallet, &amounts.redeem_amount).await?;
insert_earn_platform_journal_legs_in_tx(
    &mut tx,
    &format!("earn_redeem:{}", subscription.id),
    subscription.asset_id,
    subscription.id,
    &earn_redemption_journal_legs(
        &amounts.principal_amount,
        &amounts.gross_yield_amount,
        &amounts.fee_amount,
        &amounts.redeem_amount,
    ),
)
.await?;
```
