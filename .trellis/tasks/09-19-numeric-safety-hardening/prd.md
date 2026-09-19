# Numeric Safety Across Mobile, Backend and Admin

## Goal

Implement the user's request to prevent precision loss and numeric overflow
throughout Mobile, Rust APIs/workers and the Admin/Agent console. Classify numbers
by meaning instead of converting every numeric value to the same type.

## Contracts

- Money, prices, quantities, rates and balances retain exact decimal text at
  client boundaries and BigDecimal on the server. Source values exceeding the
  field's storage or asset precision are rejected rather than silently rounded.
  Existing explicit quote normalization (for example withdrawal preview) is
  preserved only where the returned authoritative amount is reviewed and the
  committed request is bound to that same quote; it is not an implicit write-time
  rounding permission.
- Generated monetary amounts use explicit asset quantization, with the same
  amount across orders, wallet deltas, ledgers and platform legs. No independent
  rounding of wallet buckets, no historical balance rewriting.
- Database envelopes are enforced before writes: DECIMAL(38,18) for money;
  narrower rate fields retain their existing schema and business bounds.
- Identifiers must never silently pass through an unsafe JS integer. Bounded
  integer fields enforce safe integer, sign and domain ranges. Keep compatible
  API shapes where possible; fail closed rather than act on rounded IDs.
- Timestamps/durations/pagination use checked arithmetic and valid ranges.
- Charts, geometry and non-authoritative approximation may use finite numbers,
  but cannot become authoritative execution values or conceal missing values.
- Formatting retains exact source values and does not display tiny nonzero
  money as zero; display digit caps are not settlement precision.
- Existing product rates, risk limits and policies are not arbitrarily changed.

## Delivery Slices

- [x] N01 Shared backend storage/input bounds and numeric inventory.
- [x] N02 Mobile monetary models, safe numeric boundaries and display.
- [x] N03 Admin/Agent forms, payloads, integer boundaries and exact aggregation.
- [x] N04 Spot reservation/fill/cancel precision and configuration bounds.
- [x] N05 Margin admission/settlement precision and interest remainder.
- [x] N06 Other backend money/configuration/integer arithmetic gaps.
- [x] N07 Cross-layer regression, browser checks, inventory and release notes.

## Delivery

Implementation and bounded verification are complete. See
`research/numeric-inventory.md`, `research/n01-shared.md` through
`research/n07-review.md`, and `research/release-checklist.md`.
Deployment, production migration rehearsal, native-device verification and
the separately discovered spot bootstrap lock-order issue are not claimed
as completed by this task.

## Validation

- Test 1e-18, 19+ decimal digits, 20/21 integer digits, >2^53, unsafe IDs,
  huge exponents, NaN/Infinity, negative and missing values, trailing zeros.
- Test partial fills/cancels, replay, zero-quantized results, interest batch
  equivalence and rollback where relevant.
- Run Rust unit/architecture/docs and targeted isolated database tests when
  available; do not count environment-skipped cases as database verification.
- Run Mobile release gate and Admin type/lint/tests/build; verify changed
  confirmation/form surfaces at desktop and narrow widths with local mocks.
- Maintain a scanned numeric-surface inventory including permitted float
  uses and explicitly unresolved boundaries.
- Update PROGRESS for each delivered slice.

## Scope / Non-Goals

Preserve the extensive existing dirty work. No production operations, policy
activation, history backfill, commits or deployments. PC is not a primary write
scope in this request, but shared API compatibility must not be broken.

## Coordination

Main owns `src/numeric.rs`, root wiring, cross-cutting ingress, inventory and
PROGRESS. Independent implementers own Mobile, Admin Web, Spot, Margin and
remaining backend domains respectively. Do not edit another owner's files.
Use existing decimal helpers, not a new third-party arithmetic library.

Shared backend helper contract to be implemented by Main:

```rust
crate::numeric::ensure_decimal_storage(&BigDecimal, precision: u64, scale: i64, label: &str) -> AppResult<()>
crate::numeric::ensure_amount_storage(&BigDecimal, label: &str) -> AppResult<()> // 38,18
```

Migration slots, only if needed: Margin 0141; Spot 0142. Others coordinate first.
Agents record delivery notes under this task's research directory; Main alone
updates PROGRESS, task metadata and shared spec indexes.
