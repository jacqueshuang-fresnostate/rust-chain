# E01b Withdrawal Policy Delivery

Status: E01b implementation and the subsequently assigned focused margin
concurrency fixes delivered with the explicit gaps below. Full wallet routes,
worker checks, both deterministic concurrency cases and parallel partial-close
routes pass. No production operations,
deployment, commit, or live monetary policy has been performed.

## Decisions

- Migration `0134_withdrawal_policy.sql` creates per-asset policy records,
  address registrations, immutable per-withdrawal policy receipts, and distinct
  administrator review votes. No policy is seeded or enabled.
- A missing policy returns revision zero and `enabled=false`. Saves require
  `expected_revision`, authenticated `system.security.write`, and a reason.
  Configuration and shared administrator audit insertion commit together.
- Amount basis is explicitly `principal` or `total_reserved`. Each matching
  user/KYC allowance independently constrains the user's same-asset total.
  User/KYC scopes are intersections, not overrides. Nonmatching scopes impose
  no new allowance. No cross-asset valuation or inferred KYC monetary bands.
- The supported window is explicitly `rolling` seconds, not a calendar day.
  Each allowance explicitly selects `all_outstanding` or `within_window`.
  Pending, approved, broadcasting, unknown-broadcast and manual-review requests
  count; confirmed requests count within the creation-time window. Rejected
  and safely failed requests release their occupancy through existing status
  transitions. No secondary refund or allowance-release command was added.
- Existing asset locks serialize new requests and policy updates. The first
  consistent financial read occurs after that lock; current reads own config
  and credential facts. Any concurrent release can only conservatively
  overestimate occupancy. The original request, receipt, wallet freeze, ledger
  and quote consumption commit atomically. Exact replay precedes new policy
  evaluation and remains compatible with the E01a Redis fail-closed guard.
- Review tiers start explicitly at zero, use inclusive lower bounds and
  nondecreasing reviewer counts. One administrator contributes at most one
  vote; unmet counts retain `pending_review`. Policy receipts freeze review
  requirements for already-created requests. Legacy requests without receipts
  retain the original single-review contract.
- Address maturity is tied to the authenticated user's exact normalized
  network and case-sensitive address identity. Registration reuses the
  withdrawal security verifier, cannot backdate, and is idempotent without
  restarting maturity. This is append-only registration, not a full address
  revocation or administrator-managed allowlist product.
- Credential clocks cover login password/email/phone changes, fund password
  changes, and TOTP secret/enabled/login-enforcement changes via same-row
  database triggers. Normal verification does not extend cooling. Historical
  rows initialize from their existing `updated_at`, a conservative upper
  bound rather than a fabricated exact historical change timestamp.
- Cooling is rechecked at creation, policy review, and the worker's
  `approved -> broadcasting` claim. A cooled approved request is deferred
  without consuming an attempt or releasing funds. In-flight/unknown gateway
  outcomes continue through the existing authoritative reconciliation flow.
- Admin has a real per-asset settings page with exact-decimal fields,
  revision conflict handling, changes/reason confirmation, scope/window
  controls, and independent-review progress in the withdrawal table.

## Changed Files

- `migrations/0134_withdrawal_policy.sql`
- `src/modules/wallet/{domain,application,infrastructure,presentation,routes}/withdrawal_policy.rs`
- Minimal declarations/integration in `src/modules/wallet/{domain,application,infrastructure,presentation,routes}.rs`
- `src/modules/wallet/infrastructure/withdrawals.rs`
- `src/workers/wallet_chain.rs` (only broadcast claim delegation/documentation)
- `src/modules/admin/service/access_control.rs`
- `src/openapi/{withdrawal_policy,wallet}.rs`, `src/openapi.rs`
- `tests/wallet_routes/withdrawal_policy.rs`, `tests/wallet_routes.rs`
- `tests/wallet_chain_worker.rs` (fixture-only, asset-scoped journal cleanup)
- `tests/unit_src/src_modules_wallet_withdrawal_policy_tests.rs`
- `web/src/admin/actions/{WithdrawalPolicyPage.tsx,WithdrawalPolicyPage.test.tsx,withdrawalPolicy.ts,withdrawalPolicy.test.ts}`
- Minimal route/navigation/permission/table integrations in
  `web/src/admin/{routes.tsx,navigation.tsx,access.tsx,resources/resourceConfigs.tsx}`
- `web/src/shared/adminFieldLabels.ts` (withdrawal review count labels only)

The main agent's journal/shared-wallet work is preserved; E01b did not edit
`wallet/infrastructure/shared.rs`, `PROGRESS.md`, or the global spec index.

## Verification

- Isolated `hardening_withdrawal_test` database created on the supplied local
  MySQL port `13316`; HTTP proxy variables cleared for integration commands.
- Final real policy route pass: 2/2, including concurrent cap enforcement,
  replay, distinct reviewers, config snapshot preservation, rejection release,
  address maturity, security cooling and TOTP verification-clock exclusion,
  configuration/review audit-failure rollback, readonly-admin permission
  rejection, and worker cooling without broadcasting or consuming attempts.
- `cargo test --lib withdrawal_policy`: 3/3 passed. An earlier run was blocked
  by concurrent Spot initializer edits; the successful rerun supersedes it.
- `cargo test --test wallet_chain_worker -- --nocapture --test-threads=1`:
  4/4 passed on real MySQL. Unknown-broadcast frozen/reconciliation assertions,
  deterministic rejection refund-once, confirmation idempotency, poison
  deposit and transient-failure assertions remain unchanged.
- `cargo check --all-targets`: passed again after final Rust fixture edits.
- `cargo test --test backend_architecture`: 11/11 passed again.
- Admin policy model/form/access/routes: 68/68 passed. Policy-only model/form
  rerun after narrow-screen wrapping fix: 4/4 passed. Final Web typecheck passed.
  Final `npm --prefix web run lint` passed.
- Earlier whole-Web run: 744 passed / 12 failed. E01b missing Chinese review
  count labels were fixed; subsequent shared presentation failure concerned
  `loanProducts.user_principal_limit`. Other failures were in peer financial
  retry/reconciliation, shared confirmation and commission/margin/earn work.
  Initial build failed on concurrent `resourceConfigs.test.tsx:488-491`
  `.render` union typing; later typecheck and full `npm --prefix web run build`
  passed, as did `npm --prefix web run budget`. The build retains a dependency
  warning about lottie-web eval. No claim of full Web test-suite green.
- Whole-tree `cargo fmt --all -- --check` still detects concurrent changes
  outside E01b, including financial retry unit tests, convert, risk, seconds,
  margin, Spot and E01a tests. Focused `rustfmt --check` on owned policy and
  wallet fixture files passed; `git diff --check` passed.
- Real browser smoke uses an opt-in isolated-DB fixture and the actual router,
  without workers. Default policy was disabled with no amounts; editing a
  disabled policy, required reason/difference confirmation and PATCH save
  succeeded. Temporary session token bypassed password-login testing.
  Desktop 1440x1000 and narrow 390x844 screenshots were inspected; full-page
  width did not overflow. Local header/button wrapping was fixed. The existing
  shared Admin sidebar still occupies 190px on the narrow viewport, limiting
  usable space; no global shell redesign is claimed.
- Browser evidence: `/tmp/e01b-withdrawal-desktop.png`,
  `/tmp/e01b-withdrawal-mobile.png`,
  `/tmp/e01b-withdrawal-mobile-rule.png`. Browser fixture finished 1/1 with
  its own data cleanup; temporary browser space and Vite server were closed.

### Wallet Suite Failures And Authorized Fixture Repairs

All integration commands cleared HTTP(S)/ALL proxy variables and used
`mysql://root@127.0.0.1:13316/hardening_withdrawal_test` plus local Redis port
`16386`, never a production database.

Initial `cargo test --test wallet_routes -- --nocapture --test-threads=1`:
12 passed / 3 failed / 1 ignored. Exact failures:

- `wallet_deposit_observation_credits_once_and_reorg_reverses_once`:
  MySQL 1451 / SQLSTATE 23000, `Cannot delete or update a parent row: a foreign
  key constraint fails`, constraint `fk_platform_financial_journal_asset`,
  `platform_financial_journal.asset_id -> assets.id`.
- `wallet_withdrawal_uses_tiered_withdraw_fee_when_amount_matches`: same
  MySQL 1451 asset-journal cleanup foreign-key error.
- `wallet_today_return_aggregates_realized_sources_and_marks_missing_ticker_partial`:
  MySQL 1406 / SQLSTATE 22001, `Data too long for column 'quote_id' at row 1`.

With explicit owner permission, added `DELETE FROM platform_financial_journal
WHERE asset_id = ?` before fixture asset deletion in the shared wallet cleanup,
deposit cleanup and all four worker test cleanups. Existing confirmation
journals predate this task; this is test lifecycle repair, not a new production
journal regression. All deletions bind the fixture's asset ID. No assertions,
wallet transitions, production journal code or applied migrations were changed.
The two overlong prediction fixture quote IDs retain a 20-character UUID tail
to fit the existing `VARCHAR(64)` schema.

Intermediate rerun: **14 passed / 1 failed / 1 ignored**. The sole failed test was
`wallet_today_return_aggregates_realized_sources_and_marks_missing_ticker_partial`,
at `tests/wallet_routes.rs:1424`: actual `"98.500000000000000000"` versus
expected `"18.500000000000000000"`. This is the seven-day history summary
comparison, NOT the day's amount assertion at line 1402, which passed.
The earlier handoff incorrectly described this as a today-return amount failure.
The proposed TIMESTAMP(0) rounding explanation was investigated and rejected:
information_schema reports `timestamp(6)`/precision 6; the yesterday fixture
retains `2026-09-17 23:59:59.999999` UTC and a direct UTC midnight comparison
correctly excludes it.

Clock/bound diagnosis: `get_today_return_at` derives its lower bound through
`utc_day_start(Utc::now())`; the fixture also uses UTC midnight. The query binds
naive UTC timestamps, `settled_at >= period_start_at AND settled_at <
calculated_at`, and the fixture connection sets `time_zone = '+00:00'`.
At the observed September 19 Asia/Hong_Kong local clock, UTC was still
September 18: the day query starts at September 18 00:00:00 UTC, not local
September 19 midnight. The seven-day query starts six UTC days earlier.
There is no configurable local-day branch in this path. The raw mysql CLI
initially displayed SYSTEM/+08:00 times; explicit UTC inspection was used
before drawing conclusions.

The main owner correctly identified the actual fixture bug: its prior-day
100-stake, 0.8-payout winning Seconds order contributes 80 to seven-day history
but not to today. With subsequent expectation-only authorization, the test
now explicitly expects history amount 98.5, basis 431 and rate
0.228538283062645011; the first five days are zero, yesterday is 80/basis100/
rate0.8, today remains 18.5/basis331, and cumulative total is 98.5.
All timestamps, the today and partial-valuation assertions, and production
return aggregation remain unchanged. Final full-wallet rerun:
**15 passed / 0 failed / 1 ignored** (the opt-in browser fixture).

## Subsequently Assigned Focused Regressions

These are additional main-owner assignments after E01b, not changes to the
withdrawal monetary policy. They use separate local
`hardening_focused_regression_test`, with proxy variables cleared.

- `cargo test --test margin_routes partial_close -- --nocapture --test-threads=1`:
  3/3 passed before the concurrency fix.
- `cargo test --test prediction_commission_routes journal:: -- --nocapture --test-threads=1`:
  `journal::prediction_terminal_journals_match_actual_payout_and_refund_with_replay`
  passed 1/1 without modifying its assertions or journal implementation.
- `cargo test --test openapi_routes openapi_documents_loan_exposure_policy_in_both_aliases -- --nocapture`:
  passed 1/1 without changing OpenAPI production code or the test.
- `tests/margin_routes.rs`: the authorized limit-order fixture reads
  `opened_at`, `interest_accrued_at`, and `created_at` as `DateTime<Utc>`,
  matching their real TIMESTAMP(6) columns. No financial assertions changed.
  Initial reruns were blocked by concurrent Seconds refund / reconciliation /
  agent source-lock compilation changes; final rerun passed 1/1.

### Margin Concurrency Finding And Authorized Fix

The InnoDB trace at September 19 00:37:40 local time showed two actual
`margin_position_close_executions` inserts in the main test database:
users 17/19, positions 7/9, keys `partial-close-*` and `cross-partial-close-*`.
Both held an X gap/supremum lock on `uq_margin_close_executions_user_key` and
waited for an insert-intention lock; transaction 149202 was rolled back.
This was a production missing-key lock hazard, not fixture setup/cleanup.

After reporting it and obtaining explicit production authorization, changed
only the missing-key execution lookup and its caller/export:

- `src/modules/margin/infrastructure/close_executions.rs`
- `src/modules/margin/infrastructure/cross_accounts.rs` (subsequently authorized
  close-only X-lock helper; existing shared creation/worker paths untouched)
- `src/modules/margin/infrastructure.rs`
- `src/modules/margin/application/lifecycle.rs`
- `tests/unit_src/src_modules_margin_close_execution_concurrency_tests.rs`
- `.trellis/spec/backend/margin-trading-actions.md` (close lock/replay contract)

The renamed `load_margin_close_execution_by_key_in_tx` is nonlocking.
The caller guarantees this is the first consistent read after account/
position serialization: isolated uses position FOR UPDATE; cross uses account
no-op upsert + FOR UPDATE and then position FOR UPDATE, with no ordinary
snapshot read before the execution lookup. Existing immutable executions are
visible after waiting for a prior same-position commit. Cross-position
competition is still governed by unique(user,key), inserted before wallet
mutation; duplicate handling rolls back before using the pool. There is no
extra pool checkout while a close transaction is held, no isolation-level
change, and the main owner's journal hooks are untouched.

New tests coordinate two missing-key reads with a barrier before either insert,
and hold the account/position lock until performance_schema confirms both
same-position application calls are blocked. The latter uses explicit 100%
close so a stale snapshot would hit the closed-position guard, rather than
accidentally succeed through duplicate-insert fallback. It covers isolated and
cross replay visibility, changed-percentage/changed-position conflicts, one
wallet credit, one execution and one balanced journal transaction.
The first test run found a missing mandatory margin_modes fixture field;
that fixture was corrected. The missing-key barrier regression subsequently
passed. Intermediate waited-replay runs failed with `Error: Elapsed(())`
in `margin_close_concurrency_waited_isolated_and_cross_replay_see_committed_execution`.
The direct-blocker-only detector missed wait chaining; the later row-key
detector still missed cross upsert waits. The final diagnostic showed two
`margin_cross_accounts` PRIMARY `supremum pseudo-record` waits with
`X,INSERT_INTENTION` (fixture account id33, user35/asset64), not waits whose
LOCK_DATA equals the account row id. Both calls completed after releasing the
blocker; the timeout was the test detector, not a new production failure.
The detector now roots a recursive wait graph at the exact fixture blocker's
connection ID and counts distinct direct/transitive requesting transactions
in the fixture database/table. It neither matches arbitrary table activity nor
assumes a particular physical index record. Both requests must be observed
blocked before release; terminal replay and financial assertions remain intact.

The strengthened run revealed a second real cross-account deadlock:
September 19 01:20:23 local trace, isolated DB user19/asset32, transactions
227296/227297 both held S on `uq_margin_cross_accounts_user_asset` from
INSERT IGNORE and each waited to upgrade to X on SELECT FOR UPDATE. This was
reported before changes. After explicit authorization, added
`claim_cross_margin_account_for_close`: INSERT ... ON DUPLICATE KEY UPDATE
id=id claims X directly, then current SELECT reads state/version. Only the
active close path uses it; shared with_creation, limit fill and workers remain
unchanged. Current-schema information_schema inspection found no account
triggers (rechecked: count0, REPEATABLE-READ, updated_at TIMESTAMP(6) with
ON UPDATE CURRENT_TIMESTAMP(6)). A no-op claim test compares
status/version/last_equity/updated_at exactly,
checks wallet unchanged, then verifies the actual concurrent close increments
version once. Adding account UPDATE triggers in future needs renewed review.

Optional database unit tests now read only `MARGIN_CONCURRENCY_DATABASE_URL`
and `MARGIN_CONCURRENCY_REDIS_URL`, not generic variables mutated by config
unit tests. The MySQL URL must use mysql, loopback host and an `_test` database.
Unset dedicated variables explicitly skip the real-DB cases; the explicit run
supplies the dedicated isolated URLs. This corrects the test-isolation flaw
observed in main's full-lib run (generic DATABASE_URL temporarily became
test@localhost, causing MySQL1045). No route-suite environment changed.

Final original partial-close route regression after both production fixes:
3 passed / 0 failed / 0 ignored, `--test-threads=3`, real isolated MySQL/Redis:

- `margin_partial_close_rejects_invalid_or_unkeyed_percentages_before_money_mutation`
- `cross_margin_partial_close_applies_signed_slice_equity`
- `margin_partial_close_settles_only_selected_slice_and_replays_exactly`

Latest `cargo check --all-targets` passed; `backend_architecture` passed 11/11.
Final deterministic real-DB run: **2 passed / 0 failed / 0 ignored**,
461 unrelated tests filtered out, with the corrected wait graph:

- `margin_close_concurrency_missing_keys_insert_without_gap_deadlock`
- `margin_close_concurrency_waited_isolated_and_cross_replay_see_committed_execution`

The first case coordinates two distinct users (isolated/cross) at the absent-key
barrier before either inserts. The second runs both modes and proves two blocked
100% closes resolve to one non-replay and one same-execution replay: wallet
100 -> 130 exactly once, one execution, one distinct journal transaction with
sum0, terminal closed positions, and conflicts for changed percentage/position.
The cross no-op claim preserves the preexisting inactive state/version/
last_equity/updated_at and balance; after activation the actual close produces
version1, active state and unchanged last_equity. These assertions ran against
real MySQL, not skipped branches or mocks.

Exact final real-DB commands (loopback isolated database only):

```sh
env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy \
  MARGIN_CONCURRENCY_DATABASE_URL=mysql://root@127.0.0.1:13316/hardening_focused_regression_test \
  MARGIN_CONCURRENCY_REDIS_URL=redis://127.0.0.1:16386 \
  cargo test --lib margin_close_concurrency -- --nocapture --test-threads=1

env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy \
  DATABASE_URL=mysql://root@127.0.0.1:13316/hardening_focused_regression_test \
  REDIS_URL=redis://127.0.0.1:16386 \
  cargo test --test margin_routes partial_close -- --nocapture --test-threads=3
```

The lock contract is recorded in `margin-trading-actions.md`, including first
snapshot ordering, close-only X claim, lazy-creation compatibility, trigger
review, unique-receipt-before-money ordering and rollback-before-pool replay.
No shared creation, limit worker, journal hook or monetary rule was changed.
This is a focused gate, not a claim that the full Rust library suite was rerun.
Final `backend_documentation` passed 1/1; focused `rustfmt --check` on the
four margin production files and the concurrency test, plus `git diff --check`,
passed. Assigned margin changes are frozen after this delivery; the main owner
handles PROGRESS and consolidated gates.

## Final Assigned Seconds Route Fixture Repair

The main owner subsequently authorized only two test bodies in
`tests/seconds_contract_routes.rs`. Their previous nonexistent
`admin:999999999` / `user:999999999` identities correctly returned401 before
reaching the intended SQL failure, not the expected500:

- `admin_seconds_contract_product_create_rolls_back_when_audit_fails`
  (former assertion line2101).
- `seconds_contract_open_order_does_not_replay_foreign_key_failures`
  (former assertion line2157).

Both now use real authenticated fixture identities. The audit test injects a
BEFORE INSERT failure scoped to that administrator and product-create action,
then asserts500/DATABASE_ERROR, the injected error marker, zero products and
zero create audits. The order test has a real funded wallet and injects
SQLSTATE23000/MYSQL_ERRNO1452 only for its user/product/idempotency key; it keeps
the original database-error/no-replay assertions and adds zero orders, unchanged
50/0/0 wallet buckets, and zero ledger rows. These are injected SQL failures,
not deleted real identities or weakened authentication.

Both uniquely named temporary triggers are dropped before response errors or
assertions are propagated. No production, Euclid child module, shared helper,
or PROGRESS change was made. `rustfmt --check --config skip_children=true`
for this file and `git diff --check` passed.

The explicitly authorized full-suite command was:

```sh
env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy \
  DATABASE_URL=mysql://root@127.0.0.1:13316/hardening_test \
  REDIS_URL=redis://127.0.0.1:16386 \
  cargo test --test seconds_contract_routes -- --nocapture --test-threads=1
```

Result: **28 passed / 0 failed / 0 ignored / 0 filtered**, including both repaired
tests executing against real MySQL. Important limitation: the reported28
includes `principal_refund::prospective_policy_refund_real_database_contract`,
which prints `skipped: SECONDS_REFUND_TEST_DATABASE_URL absent` and returns
success. That separately owned fixture hard-codes `hardening_commission_test`;
it was not redirected or executed outside this assignment's authorized
`hardening_test` scope. Thus this run does not claim28 real-DB scenarios or
fresh verification of the prospective refund contract. Changes frozen again
after this scoped delivery.

## Explicit Gaps / Business Approval

- No live cap, cooling duration, KYC tier schedule, required reviewer threshold,
  timezone or actual enablement is selected. Business must approve and save
  these values before use.
- Calendar-day/other-timezone reset semantics, scope overrides, counting by
  confirmation time, post-approval vote invalidation, and emergency retroactive
  tightening of pending requests are not implicitly implemented.
- PC/Mobile address-registration screens are not included in this delegated
  wallet/Admin slice. Do not enable address cooling in a client release until
  those clients expose the registration workflow or an approved equivalent.
- Address validation follows the existing generic address contract; no
  provider-backed address screening, custody signature controls or external
  KYC/AML integration is claimed.
- Trigger creation privileges, migration lock/duration on production-sized
  tables, and backup/deployment certification remain operator responsibilities.
