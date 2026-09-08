# Backend Mutation Audit

## Scope and evidence

Read-only audit of ordinary Admin asset/user/market/convert/risk mutation paths.
No production/database/credential access, no SQL mutations, and no production or
test source changes. New-coin financial design and root-owned settings/config
changes were excluded. Locations below are from the pre-repair working tree.

The three findings are confirmed by complete validator -> application -> storage
or consumer call chains. Database response/rounding scenarios are reproduction
instructions, not a claim that an integration test was executed during this audit.

## 1. P1: Manual recharge accepts amounts outside the asset precision

- Primary location: `src/modules/admin/application/users.rs:183-190`.
- Supporting locations:
  - `src/modules/admin/service/users.rs:11-20`: only positive amount, asset ID,
    reason and idempotency-key validation.
  - `src/modules/admin/infrastructure/wallet_assets.rs:134-149`: asset read
    returns only symbol/status; precision is never loaded or validated.
  - `src/modules/admin/infrastructure/wallet_assets.rs:895-905`: raw requested
    amount is added to the wallet and supplied to the ledger.
  - `src/modules/admin/application/users.rs:225-233`: response `amount` remains
    the raw request while wallet values are reread from SQL.
  - `web/src/admin/resources/actions/users.tsx:74,121-128`: the actual Admin
    form accepts positive DecimalText without checking the asset precision.
- Reproduce: use an active asset with `precision_scale=2` and an existing user;
  POST `/admin/api/v1/users/{id}/recharge` with a fresh idempotency key, a valid
  reason and `amount="0.001"`. This passes every current guard and writes 0.001
  to the DECIMAL(38,18) wallet/ledger, despite the asset permitting only cents.
  Existing route fixtures use precision 18 and miss this case.
- Additional storage-boundary risk (needs MySQL verification): an amount with
  more than 18 meaningful decimal places passes application validation; MySQL
  may round it while the response and receipt snapshot retain the unrounded
  request. A sufficiently small positive request may report success with no
  wallet increase. Do not describe this extra consequence as runtime-proven.
- Minimal compatible fix: read authoritative `precision_scale` with the active
  asset and reject, not round, fresh requests that fail the existing shared
  `amount_fits_asset_precision` rule before receipt/ledger writes. Guard invalid
  stored precision as an internal data-contract error. Preserve the existing
  first-receipt replay path and normalized fingerprint behavior. If adding a
  locking asset read, audit user/asset/wallet lock ordering rather than casually
  inserting an inverse lock order.
- Existing exact helpers: `src/modules/wallet/domain.rs:118`
  `amount_fits_asset_precision(&BigDecimal, i32)` checks normalized meaningful
  fractional scale and returns false outside 0..=18; it is publicly re-exported
  by `src/modules/wallet/mod.rs`. `asset_amount_fractional_scale` and
  `MAX_ASSET_PRECISION_SCALE` are adjacent shared sources.
- Frontend helpers: `web/src/shared/decimal.ts:86,107` exports
  `canonicalDecimalText` and `compareDecimalText`; use exact string comparison
  for spread/fee boundaries, not `Number`/floating point.
- Closest contract: `.trellis/spec/backend/wallet-amount-precision.md:12-22`.
- Closest tests: `tests/admin_routes.rs:6033`
  (`admin_recharges_user_wallet_with_ledger_and_audit`), plus standalone Admin
  service unit tests. Add 2-decimal rejection with zero receipt/ledger/audit,
  18-decimal overflow rejection, accepted trailing-zero input, precise success,
  and unchanged same-key concurrent replay.

## 2. P2: Admin can enable a convert pair that can never produce a quote

- Primary location: `src/modules/admin/service/convert.rs:59-71`.
- Supporting locations:
  - `src/modules/admin/application/convert.rs:78-121,155-212`: create and merged
    update both rely on this validator and persist its accepted values.
  - `src/modules/convert/service.rs:177-180`: actual quoting accepts only the
    `fixed` and `market` pricing modes.
  - `src/modules/convert/service.rs:240-253`: effective rate is
    `rate * (1 - spread_rate)`; non-positive target amount is rejected.
- Reproduce A: create or PATCH an existing pair to `pricing_mode="markte"` with
  otherwise valid values and a reason. Admin validation only checks nonempty
  text, so the enabled pair is saved/audited, but all quote requests fail with
  `unsupported convert pricing_mode`.
- Reproduce B: set `pricing_mode="market"` and `spread_rate="1"` (or `"1.1"`).
  Admin allows every nonnegative spread, but every positive quoted amount has a
  zero/negative effective output and fails. This is a deterministic arithmetic
  contradiction, not a request for a new pricing policy.
- Minimal compatible fix: after the existing trim semantics, require
  `pricing_mode` to be exactly `fixed|market`; require `0 <= spread_rate < 1`
  just as the fee-rate validator already does. Reuse one validator for both
  create and the locked merged update. Preserve fixed/market rate sourcing,
  target amount defaults, DecimalText and same-transaction audit behavior.
- Closest tests: `tests/admin_routes.rs`
  `admin_convert_detail_routes_require_admin_scope_mysql_and_reason`,
  `admin_convert_pair_routes_create_list_update_and_audit` and the existing
  create/update audit-rollback tests; Admin service private tests provide a
  no-DB boundary matrix. Assert invalid create rejects before MySQL acquisition,
  invalid update leaves pair/audit unchanged, and 0 / a value just below 1 plus
  whitespace-normalized supported modes remain valid.

## 3. P2: Malformed known risk fields are saved as enabled but silently ignored

- Primary location: `src/modules/admin/service/risk_security.rs:26-28`.
- Supporting locations:
  - `src/modules/admin/application/risk_security.rs:108-142`: validated target
    plus arbitrary non-null JSON is stored and audited with enabled=true.
  - `src/modules/risk/service.rs:262-267,299-310`: invalid unsigned threshold
    values become no rate-limit candidate, not a rejected Admin write.
  - `src/modules/risk/service.rs:275-278,314-323`: non-array blocklists become
    no blocklist; non-string elements are silently discarded.
  - `src/modules/risk/service.rs:99-108,139-171`: absent valid thresholds leave
    a permissive default policy.
- Reproduce: POST an enabled global risk rule with valid type/target/reason and
  `config_json={"max_requests":-1,"window_seconds":60}`. Creation accepts it;
  `resolve_risk_policy` ignores the negative threshold, so the enabled rule
  contributes no limit. Similarly, an operator entering
  `{"blocked_operations":"wallet.withdrawal.create"}` instead of an array
  receives a successful enabled rule that blocks nothing. Any non-null JSON
  scalar also passes creation and supplies no recognized thresholds.
- Minimal compatible fix: add a pure write-time validator for object shape and
  *present recognized fields*: supported number/string representations, u32
  bounds, nonnegative decimal values, and arrays made only of valid strings.
  Keep the legacy read/runtime parser tolerant; apply strict checks only to new
  writes (and re-enable only with deliberate compatibility handling). Do not
  silently coerce malformed fields or alter the permissive no-rule baseline.
- Compatibility/design boundary: unknown keys are intentionally preserved for
  forward compatibility and `rule_type` is a category, not runtime dispatch.
  Existing `daily_limit` fixtures and unscoped `max_amount` are broader semantic
  issues: the runtime deliberately ignores ambiguous amount units. Rejecting
  unknown keys, inventing daily aggregation, or changing historical runtime
  behavior is **not** part of this minimal finding/fix and needs explicit scope.
- Closest tests: `tests/unit_src/src_modules_admin_service_tests.rs:303`
  (`risk_rule_targets_are_normalized_and_reject_unusable_shapes`),
  `tests/unit_src/src_modules_risk_mod_tests.rs`, and
  `tests/admin_routes.rs:7231` (`admin_manages_risk_rules_and_lists_events`).
  Add write rejection for malformed known fields before MySQL, valid typed
  field acceptance, and a policy-resolution assertion proving accepted known
  fields actually produce limits; keep unknown-field/read compatibility tests.

## Not promoted as defects in this slice

- General full-form last-write-wins edits: no existing expected-version API
  contract was identified for ordinary assets/pairs; adding one is larger than
  a confirmed local validation repair.
- Lists can read rows and counts at different snapshots. Bounded reads and
  ordinary concurrent pagination drift alone are not enough to justify
  transactional list redesign.
- Asset precision reduction with existing balances is explicitly documented as
  not recalculating historical balances; this is a separate business/migration
  decision, not the recharge-input defect above.
- Root was informed about security policy loading `before` outside its write
  transaction; that root-owned settings path is intentionally not duplicated.

## Validation

- Inspected validators, request DTOs, application transactions, SQL writes,
  downstream consumers, schema column definitions and nearest test fixtures.
- No database mutation or production/credential inspection was performed.
- `cargo test --lib risk_policy -- --nocapture`: 5 passed, no DB.
- `cargo test --lib risk_rule_targets_are_normalized_and_reject_unusable_shapes
  -- --nocapture`: 1 passed, no DB. An earlier invocation with `--exact` and
  the short name selected 0 tests; the corrected command above ran the test.
- These are baseline behavior checks, not newly failing regression proofs.
- Report-only `git diff --check` returned clean; because this is an untracked
  file, a separate report text whitespace check was also applied.
- No new failing regression was added in this read-only audit.
