# Risk configuration audit — narrow backend repair

## Scope and recommendation

Read-only review of Admin risk writes, the risk policy resolver and its two real
consumers. Prior recharge/convert work and all pre-existing dirty files were left
untouched. No database access/mutation, external service calls or Cargo suite.

**Recommended slice:** (1) make Admin numeric pair targets match the exact spot
pair while preserving historical symbol targets; (2) reject malformed present
known JSON fields on create and enable, retaining an unconditional legacy disable
path. Do not replace the runtime parser or add a cache/config-edit API.

## Confirmed P1: normal Admin pair-targeted rules never match spot orders

Evidence chain:

- `src/modules/admin/service/risk_security.rs:46-58` requires a positive numeric
  `target_id` for `target_type="pair"` and returns its normalized numeric string.
- `src/modules/admin/infrastructure/risk_security.rs:44-53` verifies this ID in
  `trading_pairs`; create stores that numeric string unchanged at
  `src/modules/admin/application/risk_security.rs:115-120`.
- `src/modules/spot/infrastructure/read_models.rs:119-144` selects the real row ID
  and symbol, but discards `_id` and puts the symbol in `TradingPairRule.pair_id`.
- `src/modules/spot/domain.rs:204-206` preserves that symbol in `NewOrder.pair_id`.
- `src/modules/spot/application/order_creation.rs:157-160` currently supplies only
  `RiskScope::new("pair", new_order.pair_id.clone())`, hence only the symbol.
- `src/modules/risk/service.rs:202-212` compares target strings to supplied scope
  values; `"42"` never equals `"BTC-USDT"`.
- Existing `tests/spot_routes.rs:7536-7541` seeds the *symbol* directly into SQL,
  so its successful rejection never exercises the Admin-created numeric shape.

Reproduction: create an Admin rule targeting pair ID 42, configured with
`{"blocked_operations":["spot.order.create"]}`, then submit a valid order for
that exact pair. The rule is saved enabled/audited but the spot guard does not
match it. Another user/global rule could mask this failure; isolate the fixture.

### Narrow compatible implementation

At the spot guard, supply both the exact authoritative numeric pair ID and the
already-canonical symbol as two `pair` scopes. Keep the user scope unchanged.
The resolver's `.any()` match means each stored rule is still evaluated once,
not once per alias. Do not change stored target values or reinterpret numeric
strings globally.

There is already a focused query adapter:
`src/modules/spot/infrastructure/trade_settlement.rs:142`
`load_spot_pair_db_id(pool, pair_symbol)`. It is already imported in
`order_creation.rs:12` and used later at line 297. For a strict exact-pair guarantee, do not blindly reuse its `symbol = ? OR
id = ? LIMIT 1` lookup: the Admin symbol validator also permits numeric-only
symbols (`admin/service/market.rs:61`), so a symbol can collide with another
row's numeric ID. A focused adapter lookup `WHERE symbol = ? LIMIT 1` using
the already-canonical `new_order.pair_id` resolves only that authoritative
row (the symbol column is UNIQUE). Then add
`RiskScope::new("pair", pair_db_id.to_string())` beside the existing symbol scope.
It queries one pair, never a rule-to-pair scan or unrelated-pair inventory.
Alternatively retain the ID already selected by `load_pair_rule_async`; that
avoids the extra query but changes its return boundary/fixtures. Do not change
the existing broad lookup helper semantics for other call sites in this slice.

The guard stays before order/wallet writes. Lookup failure propagates before
freezing or inserting an order. Existing order-idempotency replay stays ahead
of the guard and must not become subject to newly-added risk rules.

**Compatibility:** historical symbol rules still match and keep their existing
rate-counter scope/key. Numeric and symbol rules for the same pair both
participate in existing strictest-limit merging; do not merge their stored
scope identities or reset Redis counters. No migration or rewrite of rule JSON.

The existing schema has one unavoidable edge ambiguity: a legacy numeric-only
symbol target and a modern numeric-ID target have identical stored strings.
Preserving both aliases preserves that pre-existing numeric-symbol matching;
it cannot distinguish the intended namespace without a new contract. Do not
claim to have repaired that separate schema ambiguity or silently rewrite such
rows. The recommended pair-ID fix covers ordinary nonnumeric pair symbols and
preserves the established symbol behavior.

## Existing write API: create and status only, no merged configuration update

- Routes: `src/modules/admin/routes/risk_security.rs:21-23` registers GET/POST
  `/risk/rules`, and PATCH `/risk/rules/:id/status` only.
- DTOs: `presentation/risk_security.rs:29-43`: create accepts raw `Value`; status
  contains only `enabled: bool` plus optional `reason`. There is **no** merged
  JSON update route, repository function or editable Admin rule form.
- Create: application lines 91-139 validate -> lock active referenced target ->
  insert -> reread -> audit -> commit. Raw non-null JSON is the only current
  configuration check (`service/risk_security.rs:26-28`).
- Status: application lines 157-173 lock rule -> write enabled -> reread -> audit
  -> commit. No configuration or target validation currently occurs.
- Reason remains optional for these existing endpoints. Changing that policy or
  introducing a config-edit/version protocol is not necessary for this slice.

## Exact known runtime schema and proposed write contract

All key names are case-sensitive. `rule_type` is only a display/category label;
`load_enabled_risk_rules` does not even select it. Unknown keys, including the
existing test key `daily_limit`, have no effect. Preserve that forward-compatible
read/write behavior and document it rather than inventing daily aggregation.

| Key | Current resolver behavior | Narrow create/enable check |
| --- | --- | --- |
| top-level JSON | Scalars/arrays expose no fields; effectively no constraints | Require an object. Retain `{}` and objects with only unknown extension keys. |
| `operations` | Array -> keep string members only; absent/non-array/null -> applies to all operations; `[]` -> matches nothing | If present, require an array whose every item is a string; reject empty/whitespace-only or outer-whitespace strings rather than silently trimming them. Preserve an empty array as an explicit no-match selection. |
| `blocked_operations` | Array -> keep strings and deduplicate exact text; absent/non-array/null -> no blocklist | Same strict string-array check; `[]` remains valid and blocks nothing. |
| `max_amount` | String(trimmed) or JSON number parsed through BigDecimal; negative/invalid/null ignored. Only effective when `operations` identifies one known amount unit | If present, require a nonnegative BigDecimal via the same accepted string/number representations. No floating-point conversion, auto-rounding or arbitrary wallet precision limit. Preserve amount-unit semantics separately as noted below. |
| `max_price_deviation_bps` | String(trimmed) parsed as u64 then narrowed to u32, or JSON integer accepted by `as_u64`; invalid/negative/fractional/overflow/null ignored | If present, require exactly those unsigned integer representations and `0..=u32::MAX`. Zero is a valid strict threshold. |
| `max_requests` | Same u32 parser; invalid -> no rate limit; zero is valid (first counted request exceeds it) | Same parser and range, including zero. Do not silently clamp. |
| `window_seconds` | Same u32 parser; absent, invalid or zero -> default 60 seconds. Only used with a valid max_requests | If present, require a valid positive u32; absent retains 60. Reject zero instead of accepting an explicitly configured zero window that silently executes as 60. |

Parser locations: `risk/service.rs:189-195,216-255,262-267,275-324`.
Domain evaluation: `risk/domain.rs:109-141` compares both operation selectors
and blocklists case-insensitively; strings are **not trimmed** at matching time.
Case variants therefore remain valid; preserve rather than normalize persisted
JSON. Unknown nonblank operation strings remain accepted for compatibility.

A present null known value should be a write validation error, distinct from an
absent optional field. Do not retain the malformed-to-absent coercion on new
writes. Continue permitting nulls *under unknown keys*. Numeric strings retain
the runtime parser's accepted whitespace handling. Integer JSON forms such as
`1.5`, `1.0` or exponent notation must not be accepted merely by coercing them
through floating point: reuse the existing `as_u64`/string-u64 parsing contract.

### Amount-unit and operation boundaries (do not silently change)

Only these two operation names are wired into the actual risk guard:

- `spot.order.create`: `user` and `pair` scopes; amount is quote notional,
  prices available when the spot request/execution provides them.
- `wallet.withdrawal.create`: `user` and `asset` scopes; amount is the asset
  withdrawal quantity; no price/reference facts.

`max_amount` is deliberately ignored if `operations` is absent, empty, contains
an unknown operation, or mixes those two different units. Existing unit test
`risk_policy_amount_limit_only_applies_to_matching_operation_unit` explicitly
locks this behavior. For the smallest **malformed-field** repair, keep these
legacy semantics; accepting a valid but unscoped threshold does not mean it is
promised to execute. If the selected goal additionally requires rejecting every
known semantically inert amount configuration, add a separate write-time check
using the existing `rule_amount_unit` helper (valid max_amount requires explicit
homogeneous supported operations), revise that acceptance contract deliberately,
and leave historical runtime behavior untouched. Do not implement a new unit or
infer a withdrawal unit from `rule_type="withdraw_limit"`.

Likewise, `window_seconds` without `max_requests`, unknown operation names, `[]`
and extension-only objects are not malformed types. Rejecting all no-op
configurations is a broader product-policy change than this slice.

## Pure validation placement and legacy state transitions

Prefer one pure known-field validator adjacent to the parser in
`src/modules/risk/service.rs` (or one focused child), with a small stable error
mapped to `AppError::Validation` at the Admin boundary. Reuse the runtime scalar
parsers or shared value checks; do not duplicate a subtly different numeric
syntax implementation in each create/status path. Keep request DTOs in Admin
presentation and SQL/transactions where they are.

1. Create: validate configuration before acquiring MySQL, regardless of requested
   enabled value. New disabled invalid drafts are not necessary: no draft editor
   or merged update endpoint exists.
2. Status enabled=true: lock the row, then validate its exact stored config JSON
   before UPDATE/audit. This includes true->true to avoid acknowledging invalid
   enable commands as valid. Validation failure must leave row/audit unchanged.
3. Status enabled=false: **never** require valid config JSON, an active target or
   newly valid scope shape. Keep scalar/null/malformed historical rows stoppable;
   preserve original JSON in before/after audits. Repeated false remains the
   current successful audited write.
4. No new target revalidation on status is needed for the config defect. Calling
   `validate_create_risk_rule` on a stored legacy rule would wrongly reject old
   symbol pair targets and could prevent stopping/re-enabling legitimate rules.
5. GET/list and runtime reads must not invoke the new strict validator. A single
   legacy malformed row must not turn all trading requests into HTTP 400/500.
   Repair an invalid row by stopping it and creating a corrected one through the
   existing API; do not add a partial editor in this task.

## Runtime/cache implications and explicit deferred behavior

- No rule/config cache exists: `risk/infrastructure.rs:19-24` queries enabled
  rules every guard invocation. Successful status changes affect subsequent
  reads; in-flight evaluations may still finish from their already-read policy.
- Invalid rules currently become no-op fields, while a malformed `operations`
  selector can instead become wildcard and over-apply another valid threshold.
  Strict writes fix both classes without replacing legacy reads.
- MySQL read failures propagate. Redis missing/failure skips the rate dimension
  (`risk/application.rs:77-102`); rejection-event SQL failure logs a warning but
  does not turn a rejection into approval (`:139-152`). Preserve these policies.
- **Documented separate limitation:** withdrawal calls the guard with Redis=None
  at `wallet/application.rs:878-888`. Thus valid withdrawal rate-limit rules are
  presently not enforced, and withdrawal price-deviation fields have no facts.
  JSON validation alone does not solve either limitation. Wiring Redis/altering
  outage behavior is a separate runtime-policy task, not a cache optimization.
- Fixed-window counters use operation/scope/user identity and EXPIRE NX; a
  changed window does not reset a current counter. No TTL/reset redesign needed.

## Executable regression plan

### No database tests

- `tests/unit_src/src_modules_admin_service_tests.rs`:
  table-drive known-field rejection before pool access; valid target behavior
  remains. Keep existing target normalization tests (their unscoped max_amount
  fixture matters if selecting the stricter amount-unit variant).
- `tests/unit_src/src_modules_risk_mod_tests.rs`:
  add tests for numeric plus symbol pair scopes, another pair non-match, combined
  strictest limits, and old symbol counter-scope preservation. Add exact schema
  matrices and prove valid parsed fields generate the same policies as before.
- Boundary matrix: scalar/list/null top-level; null/bool/object known scalars;
  -1, 1.5, >u32::MAX; max_requests=0; window omitted/0/1; numeric strings;
  arrays with non-strings/blank/outer whitespace; lowercase/uppercase operations;
  empty arrays, {}, unknown keys; positive scientific decimal and negative
  decimal, with raw persisted JSON preserved.

### Isolated MySQL integration tests (implementation phase only)

- Add a focused child under `tests/admin_routes/` to avoid growing the existing
  monolith or overwriting its previous financial-validation additions.
- Use actual POST Admin rule creation with numeric pair ID and
  blocked_operations=[spot.order.create], then actual spot order POST. Assert
  403/risk_operation_not_allowed, no order,
  no balance freeze/ledger, and a rejection event. No Redis is needed for the
  blocklist scenario. An unrelated pair remains allowed.
- Keep/reuse `tests/spot_routes.rs:7485`
  `spot_order_risk_rule_rejects_price_deviation_without_freezing_wallet` to prove
  historical symbol-target behavior, or add a Redis-free symbol blocklist case.
- Invalid create (both enabled flags): 400 and no new rule/audit. Invalid enable:
  400 with unchanged row and audit count. Legacy invalid true->false: succeeds,
  same raw JSON, precise status audit. Repeated disable succeeds. Valid enable
  succeeds. Simulate audit failure only with valid authenticated fixtures and a
  scoped trigger, asserting complete status/creation rollback.
- Existing Admin CRUD: `tests/admin_routes.rs:7231`
  `admin_manages_risk_rules_and_lists_events` uses unknown `daily_limit`; retain
  as unknown-key compatibility or add a separate recognized-field fixture.
- Existing scope/no-pool matrix: `tests/admin_routes.rs:2811`
  `admin_core_resource_routes_require_admin_scope_and_mysql` includes an
  extension-only risk JSON fixture; avoid breaking it accidentally.

## Validation performed for this audit

Read exact source/DTO/route/SQL/query and existing tests; checked report
whitespace. No runtime or database reproduction was executed in this read-only
subtask. Conclusions above distinguish source-proven defects from deliberately
deferred business-policy choices. Only this research file was written.
