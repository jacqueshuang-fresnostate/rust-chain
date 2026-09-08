# Risk-rule implementation and verification handoff

## Bounded contracts

- Create and enable validate object JSON plus the six recognized runtime keys.
  Unknown keys, unknown nonblank operation names, empty arrays/objects, optional
  audit reason, nonnegative arbitrary-precision amounts, zero request limits,
  and the existing operation/amount-unit semantics remain supported.
- Reuse the risk resolver's decimal/u32 parsers; no scalar coercion rewrite,
  config editor, cache, target migration or runtime policy change.
- Enable validates the exact row locked by the status transaction. Disable skips
  configuration and target validation, including repeated legacy disable.
- Spot receives numeric and legacy symbol pair scopes. The extra numeric ID is
  resolved by the canonical symbol only, never `symbol = ? OR id = ?`; no other
  pair/rule inventory scan. The prior order replay stays before this guard.
- Stored numeric-only symbols remain namespace-ambiguous with numeric IDs;
  preserve existing matching rather than silently migrating their interpretation.

## Production changes

- `src/modules/risk/service.rs`: one crate-private pure validator; reuses the
  unchanged decimal/u32 readers. Exact raw config is never rewritten.
- `src/modules/admin/service/risk_security.rs`: existing create validation now
  delegates to it before acquiring MySQL.
- `src/modules/admin/application/risk_security.rs`: only enabled=true validates
  `before.config_json` after the row lock and before any status/audit write.
- `src/modules/spot/infrastructure/read_models.rs` and facade: focused
  `load_spot_pair_db_id_by_symbol` performs one exact unique-symbol lookup.
- `src/modules/spot/application/order_creation.rs`: adds numeric pair scope
  alongside the existing canonical symbol, before any financial mutation.
  Existing broad symbol-or-ID helpers and idempotency replay are unchanged.

## Tests added

`tests/unit_src/src_modules_admin_service_tests.rs`:

- `risk_rule_config_rejects_malformed_known_fields_for_both_creation_states`:
  80 JSON configurations across enabled=false/true (160 cases), including all
  present-null known keys, scalar/array top level, invalid string arrays, signed,
  fractional and oversized integers, JSON `1.0`/exponent representation, and
  explicit zero windows.
- `risk_rule_config_preserves_valid_runtime_representations_and_extensions`:
  12 configurations, preserving original JSON, high-precision scientific
  amounts, uppercase/unknown operation names, unknown-null extensions, zero
  requests, maximum u32 and unchanged mixed-unit amount semantics.

`tests/unit_src/src_modules_risk_mod_tests.rs`:

- `risk_policy_numeric_pair_alias_merges_once_without_rewriting_legacy_counter_scope`:
  strictest numeric/symbol merging, order independence, different-pair isolation,
  and unchanged legacy rate-counter scope.
- `risk_policy_keeps_numeric_symbol_legacy_namespace_unchanged`:
  existing numeric-symbol matching is retained, including zero requests and
  the omitted-window default.

- `risk_config_validated_values_keep_runtime_policy_and_original_json`: precise
  scalar policy parity, unchanged operation/amount-unit semantics, zero request
  threshold and continued tolerant legacy runtime parsing.

`tests/admin_routes/risk_validation.rs` (shared existing authenticated fixtures):

1. `risk_config_invalid_create_is_validated_before_mysql` — no configured pool.
2. `risk_config_invalid_create_has_no_rule_or_audit_side_effects` — both enable
   values, invalid recognized fields and valid raw extension JSON preservation.
3. `risk_config_enable_validates_locked_json_but_legacy_disable_is_unconditional`
   — invalid true->true/false->true do not mutate state/audits; scalar/null/array
   and malformed legacy JSON with nonexistent legacy symbol targets can be
   disabled twice with exact before/after JSON, then valid enable still works.
4. `risk_admin_numeric_pair_rule_guards_exact_pair_and_preserves_symbol_rules_and_replay`
   — real numeric-target Admin POST, real symbol/numeric spot POST rejection,
   unchanged wallet/order/ledger counts, existing replay, other-pair allow,
   legacy symbol blocklist, and numeric-symbol collision resolved to the actual
   canonical pair's ID. No Redis is needed.
5. `risk_config_valid_create_and_enable_roll_back_when_audit_insert_fails` —
   valid authenticated requests with an audit trigger scoped to this fixture's
   admin; drops the trigger before assertions, verifies atomic rollback.

## Red evidence

- `cargo test --offline --lib risk_rule_config_ -- --nocapture` — actual
  assertion red: 1 passed, 1 failed. The invalid matrix reports malformed JSON
  accepted in both creation states; the valid representations passed. Log:
  `/private/tmp/risk-config-red.log`.
- Initial compilation attempts first required an explicit u64 test literal and
  then encountered the concurrent Coinbase worker's tuple conversion compile
  error. Both were fixed before the actual red assertion above; compile failures
  are not counted as behavioral red evidence.
- Root executed the numeric pair DB red before any spot production change: 0 passed, 1 failed; a real Admin numeric-target rule was created, but the matching spot order returned HTTP 200 instead of 403. Log: `/private/tmp/risk-pair-red.log`.

## Disposable-DB command handoff

Root owns the disposable schema and its cleanup. Execute serially with other
Cargo workers; the schema must not be production or shared persistent data.

```sh
DATABASE_URL='mysql://root@127.0.0.1:3306/codex_backend_chart_20260907_0040' \
  cargo test --offline --test admin_routes risk_validation:: -- --nocapture --test-threads=1
cargo test --offline --lib risk -- --nocapture
cargo test --offline --test backend_architecture --test backend_documentation
```

Run existing Admin risk CRUD and existing spot risk tests as additional
compatibility evidence. The old price-deviation spot test also needs isolated
Redis; absence of its dependencies causes a skip and must not be called pass.

## Current verification status

Production implementation is present and scoped rustfmt plus `git diff --check` pass.
`cargo test --offline --lib risk -- --nocapture` passed 18/18, including all five
new pure tests, existing target validation and existing risk-policy contracts.
Log: `/private/tmp/risk-lib-green.log`.
Root's first disposable-DB green run passed 4/5, including exact numeric pair
isolation, legacy symbol behavior, replay, and numeric-symbol collision. The
remaining audit rollback fixture encountered MySQL 1295 because trigger DDL was
sent through a prepared statement, before its production call ran. Both CREATE
and DROP now use the repository's existing `sqlx::raw_sql` trigger-fixture
pattern; no production logic changed. Root owns rerunning all five and full gates. No network, production DB, Git metadata, specs or PROGRESS
were modified by this worker.

## Final root verification

After the DDL fixture correction, all five real-MySQL risk route tests pass.
Existing Admin risk CRUD passes 1/1 and existing spot price-deviation/no-freeze
regression passes 1/1 with a dedicated nonpersistent loopback Redis. No skip
messages occurred. Rust lib 348/348, architecture 11/11, documentation 1/1, fmt,
all-target/all-feature check and Clippy `-D warnings` all pass on the final code.
The disposable MySQL schema was dropped; remaining trigger/schema counts are
both 0. Dedicated Redis on port 16385 was shut down without saving.
