# Admin Recharge and Convert Configuration Validation

## 1. Scope / Trigger

Apply when changing manual wallet recharge, its recoverable Admin form, or
convert pair creation/update. Validation must describe values that actually
survive storage and execute at runtime, not merely parse successfully.

## 2. Signatures

- `POST /admin/api/v1/users/{id}/recharge`: decimal-text `amount`, `asset_id`,
  `reason`, `idempotency_key`; authoritative asset `precision_scale` is 0..18.
- `POST /admin/api/v1/convert/pairs`, `PATCH /admin/api/v1/convert/pairs/{id}`:
  `pricing_mode`, `spread_rate`, `fee_rate`, source/target amount ranges, `enabled`.
- Convert ratios are `DECIMAL(18,8)`; wallet/ledger storage is `DECIMAL(38,18)`.
- Web: `decimalFitsPrecision(value, precision)` and read-only
  `FinancialCommandIntentStore.hasPending(scope, values)`.

## 3. Contracts

- Recharge checks the original receipt first. After acquiring the user lock,
  recheck the receipt in that transaction before current asset validation.
  Then lock asset status/precision using `FOR SHARE`, validate, reserve receipt,
  update wallet, append ledger/audit and response snapshot in one transaction.
  The shared lock blocks concurrent asset edits without serializing all users
  of the asset or conflicting with wallet foreign-key shared locks.
- Submitted amounts are never rounded. Trailing zeros do not consume precision.
  Original successful receipts remain replayable after precision/status changes;
  they return the original response, not a new balance snapshot.
- UI asset options carry optional authoritative precision, never a guessed
  default. New intents fail before HTTP if metadata is unavailable or the amount
  exceeds precision. Perform this check after reason entry so an exact unresolved
  session/user/asset/amount/reason intent can still reuse its original key.
  Reading pending status must not allocate or renew a lease. Server validation
  remains authoritative if cached metadata changes or a retry never committed.
- Convert modes are `fixed|market` (backend trims whitespace); spread and fee
  are ratios in `[0,1)` with at most 8 meaningful decimal places. Source/target
  minimums are nonnegative and optional maximums are not below minimums.
- Validate merged locked update values, not only supplied fields. The sole
  exception is explicit `enabled=false` with no configuration fields: permit
  stopping a legacy invalid row while preserving its stored values. Enabling,
  edits, and explicit nullable bound changes remain strict and audited.

## 4. Validation & Error Matrix

| Condition | Required result |
|---|---|
| Amount exceeds authoritative asset precision | 400, no receipt/wallet/ledger/audit writes |
| Invalid stored precision outside 0..18 | Internal configuration error, no writes |
| Original successful receipt, now stricter precision/disabled asset | Original successful response, no second credit |
| Unknown mode or ratio outside `[0,1)` | 400 before create I/O or update writes |
| Ratio `0.999999999` | 400, never let storage round it to 1 |
| Legacy invalid pair, pure explicit disable | Disable and audit without changing configuration |
| Legacy invalid pair, enable/edit | 400 and preserve row/audit state |

## 5. Good / Base / Bad Cases

- Good: precision 2 accepts `1.2300` and stores exact `1.23`.
- Base: spread `0.99999999` or fee `1e-8` fits storage unchanged.
- Bad: validating only positivity, coercing money through JS `Number`, or
  rounding a ratio/amount to make it fit silently changes the command.

## 6. Tests Required

- Real isolated MySQL routes: precision 0/2/18, invalid stored precision,
  trailing zeros, exact ledger/balance, concurrent precision edit, replay after
  configuration change, and no partial writes including strict SQL overflow.
- Existing 20-concurrent identical recharge and convert CRUD/audit rollback.
- Unit/no-DB route mode and ratio bounds/storage-precision matrix.
- Real Semi form checks: invalid config, unchanged valid payload units,
  new overprecision recharge rejected before HTTP, equivalent original retry
  after precision tightening uses the same key; changed reason does not.

## 7. Wrong vs Correct

```ts
// Wrong: rounding changes the request and may make an existing retry a new credit.
const amount = Number(input).toFixed(asset.precision_scale);
// Correct: preserve exact intent; only genuinely new commands use current UI checks.
if (!store.hasPending(scope, intent) && !decimalFitsPrecision(intent.amount, precision)) {
  throw new Error('充值金额超过资产精度');
}
await runRecoverableFinancialCommand({ scope, values: intent, store, request });
```
