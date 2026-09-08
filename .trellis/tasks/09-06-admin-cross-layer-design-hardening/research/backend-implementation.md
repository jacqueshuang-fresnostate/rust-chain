# Backend implementation: Admin financial validation

## Implemented scope

- Manual recharge now uses a dedicated `lock_admin_recharge_asset_in_tx` (`FOR SHARE`) to read and
  hold authoritative asset status and precision in the same transaction as the
  receipt/wallet/ledger/audit. Precision outside 0..=18 is an internal stored-data
  error. `wallet::amount_fits_asset_precision` rejects excessive meaningful
  fractions before receipt or wallet creation; input is never rounded.
- The original receipt fast path stays before current asset validation. After
  obtaining the user lock, a second receipt read observes the successful request
  that might have committed while this request waited; replay stays independent
  of later asset precision/status changes and later wallet balances. The existing
  duplicate-key rollback/replay path stays intact. The loader and replay helper
  accept a SQLx executor so this recheck uses the transaction connection rather
  than acquiring another pool connection while holding a user lock.
- Convert create and merged update accept only case-sensitive `fixed|market`
  after existing whitespace trimming, and spread/fee ratios in `[0,1)`. Both must
  have at most 8 meaningful fractional places for `DECIMAL(18,8)`; otherwise
  `0.999999999` could round into the non-executable spread `1`. Trailing-zero
  equivalents remain accepted, with no percent conversion or rounding.
- By root agreement, only explicit `enabled=false` without *any* configuration
  fields may bypass complete configuration validation to stop legacy invalid
  rows. This preserves the original pricing text as well as numeric fields and
  writes `convert_pair.update_status` in the same locked/audited transaction.
  Enabling, config changes (including an explicit nullable bound), and requests
  with no explicit disable all remain strict.

## Files

- `src/modules/admin/application/users.rs`
- `src/modules/admin/application/convert.rs`
- `src/modules/admin/infrastructure/financial_idempotency.rs`
- `src/modules/admin/service/convert.rs`
- `tests/unit_src/src_modules_admin_service_tests.rs`
- `tests/admin_routes/financial_validation.rs`
- `tests/admin_routes.rs` (dedicated regression module declaration only)
- `src/modules/admin/infrastructure/wallet_assets.rs` (shared asset-lock helper
  and existing asset row extended with precision; unlocked new-coin active-asset
  validation remains unchanged)
- `src/modules/admin/application.rs` (new helper import only)

## Lock and storage assessment

- Lock order remains user -> asset (shared) -> receipt -> wallet. The shared
  asset lock prevents concurrent configuration changes without serializing all
  users of the same asset or conflicting with asset foreign-key shared locks
  held by other wallet operations. Asset update locks only
  the asset for configuration/audit and does not lock users; deletion checks
  references before deleting wallets. Asset creation's users-to-wallet backfill
  is for a new uncommitted asset, not a visible existing recharge target.
- The in-transaction replay read is the first consistent snapshot read after
  acquiring the user lock. Asset loading itself uses a current `FOR SHARE`
  read, so an in-flight precision update is observed after its commit even
  though the receipt query established a consistent snapshot.
- `DECIMAL(38,18)` has 20 integer digits. The shared wallet precision helper
  covers fractional precision, not this integer limit. Existing decimal storage
  validators in earn/seconds/margin are private and have different tail-zero
  semantics; extracting or broadening them is outside this repair. Under the
  configured MySQL strict mode, receipt integer overflow and resulting wallet
  balance overflow already fail SQL and roll back the complete transaction.
  Dedicated route regressions cover both; this is not a newly introduced
  request-size or database-error mapping policy.

## Red regression evidence (actually executed, no DB)

- `cargo test --lib admin_convert_pair -- --nocapture`: original validator had
  2 passing and 2 failing tests; failures were `markte` and `spread_rate=1`.
- `cargo test --lib admin_convert_pair_rejects_ratios_that_would_be_rounded_in_storage
  -- --nocapture`: original validator accepted the 9-place ratio; test failed.
- `cargo test --test admin_routes
  admin_convert_create_rejects_invalid_financial_config_before_mysql -- --nocapture`:
  original route returned 500/MySQL-not-configured for `markte` rather than 400;
  test failed before implementation.
- Recharge database red cases were written before implementation but were not
  executed by this worker: root owns disposable MySQL setup. Do not describe
  them as an observed red runtime.

## Verification and root handoff

- After implementation: 5 focused convert unit tests passed and the no-DB
  create-route rejection matrix passed. These first green runs exposed an unused
  old asset symbol field; the final dedicated shared-lock helper reuses that
  existing row rather than suppressing the warning or renaming new-coin code. Final broader unit and format verification follows below.
- Root should run against a dedicated migrated local MySQL schema with strict
  SQL mode (never production):
  - `cargo test --test admin_routes financial_validation -- --nocapture`
  - `cargo test --test admin_routes admin_recharges_user_wallet_with_ledger_and_audit
    -- --nocapture` (existing 20-concurrent identical request contract)
  - `cargo test --test admin_routes admin_convert_pair_ -- --nocapture`
    (existing create/update/list/audit and audit-failure rollback)
- `DATABASE_URL` is required; the existing `mysql_pool()` runs repository
  migrations. Missing `DATABASE_URL` causes database tests to return early, so
  a pass without a connected dedicated schema is not integration evidence.
- New `financial_validation` tests cover precision 0/2/18 rejection, invalid
  stored -1/19, trailing-zero acceptance, exact 18-place credit and ledger,
  replay after precision tightening/status disable, locked concurrent precision
  update, no receipt/wallet/ledger/audit on rejection, receipt/balance storage
  overflow rollback, invalid create before MySQL, invalid update atomicity,
  legacy stop/re-enable/config-edit rules, and repaired enabled config.
- Worker did not connect to a database/network, modify Web/specs/PROGRESS,
  commit, deploy, create another task, or archive anything.

### Final worker verification

- `cargo test --lib modules::admin::service::tests:: -- --nocapture`: 18/18
  passed after the final `FOR SHARE` helper change, with no warnings.
- `cargo test --test admin_routes
  admin_convert_create_rejects_invalid_financial_config_before_mysql -- --nocapture`:
  1/1 passed and compiled all 5 new route tests. This last route run preceded
  the final shared-lock helper; root's disposable-DB run will rebuild the final
  route target and validate the actual MySQL locking semantics.
- `cargo fmt --all -- --check`: passed on final production/test sources.
- `git diff --check`: passed on the final tracked changes; new test/report files
  were separately checked for trailing whitespace.
- Cargo is handed back to root; this worker has no running Cargo command or
  disposable service to clean up. Root owns full architecture/check/clippy and
  real database integration gates.

## Follow-up: repair legacy audit rollback fault injection

Root's actual disposable-MySQL run passed all five new financial validation
cases and the existing 20-concurrent recharge case. Two older convert audit
rollback tests returned 401 because they used tokens for nonexistent admins;
they no longer reached the intended audit foreign-key failure after current
authentication validation. This was a stale test fault-injection assumption,
not a production rollback defect.

Only those two test bodies in `tests/admin_routes.rs` were repaired: each now
creates a real permitted admin and a UUID-named `BEFORE INSERT` trigger matching
only that admin and the expected convert audit action. The trigger signals a
specific database error at the actual audit insertion. It is dropped before
propagating the request result or making assertions; test rows are cleaned
before outcome assertions, including unexpected pair/audit rows on a rollback
regression. Original pair-count-zero/enabled-unchanged assertions remain, with
additional no-audit and exact injected-database-error assertions. Production
code did not change.

`rustfmt --edition 2024 tests/admin_routes.rs` and scoped `git diff --check`
passed. No Cargo or database command was run by the worker in this follow-up;
root owns the ongoing Cargo gate and reruns
`cargo test --test admin_routes admin_convert_pair_ -- --nocapture` on the
isolated disposable schema.
