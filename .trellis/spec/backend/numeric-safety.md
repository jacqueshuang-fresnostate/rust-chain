# Numeric Safety Contract

## Scope

Applies to numeric request boundaries, financial persistence, configuration,
client adapters and time arithmetic. Numeric types follow their business role;
money, identifiers, timestamps and chart geometry are not interchangeable.

## Shared APIs

```rust
numeric::ensure_decimal_storage(value, precision, scale, label) -> AppResult<()>
numeric::ensure_amount_storage(value, label) -> AppResult<()> // DECIMAL(38,18)
numeric::parse_decimal_input(text) -> Result<BigDecimal, String>
time::checked_expiry(now, ttl_seconds, label) -> AppResult<DateTime<Utc>>
time::ensure_timestamp_storage(&datetime, label) -> AppResult<()>
```

`BigDecimal` request fields use the shared `deserialize_decimal`,
`deserialize_optional_decimal` or `deserialize_patch_decimal` adapter.
Optional decimal lists use `deserialize_optional_decimals`; collection wrappers
must not bypass the same per-element validation.
Optional fields retain `serde(default)`; PATCH fields distinguish absence,
explicit null and an actual decimal. Response types and JSON serialization
are not changed by request guards.

## Representation and Bounds

| Role | Representation | Boundary |
| --- | --- | --- |
| Money, price, quantity | Decimal text / BigDecimal | 20 integer digits, 18 fractional digits; asset precision may be stricter |
| Stored rates | Decimal text / BigDecimal | Actual schema, usually DECIMAL(18,8), plus existing product rules |
| Source numeric text | ASCII decimal, optional exponent | 256 input bytes, exponent within -256..256, then exact storage validation |
| IDs and counts in JS | Safe integer or existing exact string contract | Reject unsafe Number values before display as an identity or mutation |
| Backend IDs/counts | Existing unsigned/signed integer types | Checked conversions and arithmetic; SQL errors must not become successful operations |
| UTC timestamps | Existing integer milliseconds | Checked conversion, valid timestamp range; no seconds/milliseconds multiplication overflow |
| Expiry/TTL | Positive checked duration | Current TIMESTAMP(6) schema range; reject values beyond the 2038 limit, not silently clamp |
| Charts/geometry | Finite bounded Number | Presentation only; never reused as execution authority |

Trailing zeros do not count as effective fractional precision. Storage checks
do not rescale values, expand exponents, or format invalid values in errors.
Coefficient/exponent capacity arithmetic uses a wider integer to avoid overflow
even when handed an extreme already-constructed BigDecimal.

## Financial Invariants

- Reject unrepresentable input before any wallet mutation.
- Quantize generated amounts once according to the owning product's asset
  rule. Share the resulting value across order, debit/credit, ledger and journal.
- Do not round available/frozen buckets independently, erase historical dust,
  or rewrite balances when precision metadata changes.
- Explicit normalized quote previews retain their existing rules: the user
  reviews the authoritative returned amount and commits that same quote.
- Final write guards check storage capacity but never silently clamp a balance.
- Preserve unknown, invalid and absent data as errors/unavailable states, not
  fabricated zero balances, free fees or zero debt.
- Display digit limits do not change arithmetic. Tiny nonzero money must remain
  visibly nonzero and exact source text must remain available at confirmation.
- Existing product limits, fee percentages and historical accounting policies
  must not be changed merely to make a numeric test pass.

## Error Matrix

| Input or computation | Required result |
| --- | --- |
| `1e-18`, supported 18-place asset | Exact acceptance |
| `9007199254740993.000000000000000001` as decimal text | Preserve exactly |
| 21 integer digits or 19 effective fraction digits for a stored amount | Reject |
| Trailing zeros beyond scale | Accept when normalized value fits |
| NaN, Infinity, boolean, malformed decimal | Reject, never zero fallback |
| Huge exponent/oversized numeric text | Reject before expensive decimal expansion |
| Nullable numeric PATCH omitted/null/value | Preserve all three semantics |
| JS numeric ID beyond safe integer | Fail closed; do not act on rounded ID |
| Generated rounded-to-zero execution | Apply the product's explicit rejection rule |
| Duration conversion/addition overflow | Return validation/configuration error, never panic or infinite lifetime |

## Regression Requirements

- `tests/numeric_contract.rs` inventories typed financial inputs using Rust AST
  and rejects missing bounded deserializers, including nested fee/policy/default
  market configurations. Cache/response structs are intentionally not request
  DTOs; their read boundaries are reviewed separately.
- Shared unit tests cover signs, maximum integer/fraction precision, zero,
  trailing zeros, exact numeric JSON literals, exponent/length limits and
  missing/null PATCH handling.
- Product tests cover partial execution, cancellation, replay and rollback with
  the same monetary amount across all persisted legs.
- Client behavioral tests load real adapters/forms and cover >2^53 decimal
  values, small nonzero fees and unsafe IDs. A source grep alone is not
  behavioral proof.
- Timer tests exercise u64/i64 extremes and maximum representable deadlines.
- Timestamp capacity follows the actual existing columns, not Chrono or
  DATETIME's broader range. A future post-2038 schema migration is separate work.
- No missing-environment early return may be reported as database validation.

## Examples

Wrong: parse an amount through Number and recover it with String, independently
round wallet buckets, or cast unsigned TTL to signed seconds.

Correct: retain source decimal text, validate both storage and asset precision,
derive one settlement amount, then use checked time and integer operations.
