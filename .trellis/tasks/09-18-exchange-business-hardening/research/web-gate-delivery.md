# Final Web Gate Triage

## Scope

User assigned final full Web gate triage after accepting E05. Preserve concurrent
feature work and unrelated baseline behavior. Do not raise test timeouts, weaken
financial contracts, edit PROGRESS, commit, deploy, or access production.

## Tracking

- [x] Run the complete Web suite and record exact failing test names.
- [x] Reproduce and fix introduced failures within changed surfaces.
- [x] Confirm baseline label/locator issues before any minimal correction.
- [x] Recheck E05 version/lease atomicity, mutation receipts and seconds deep link.
- [x] Run typecheck, lint, production policy, coverage, build and bundle budget.
- [x] Record intermediate results, remaining baseline failures and coordination notes.

## Run Notes

- Initial command uses `--maxWorkers=1 --no-file-parallelism` with unchanged
  timeouts. The entire suite runs; no test selection or retry masking.
- Machine-readable initial results:
  `/private/tmp/rust-chain-web-gate.json`.
- Shared navigation/access changes will be announced before editing. E02
  snapshot and E08 refund owners retain their feature implementations.
- This is an intermediate integration gate while refund/snapshot feature owners
  are still editing. Main owns the final post-freeze gate; a passing result here
  is not a frozen-tree certification.

## Initial Full Run

2026-09-19 00:48:53 Asia/Hong_Kong, duration 413.69s:
92 files, 765 passed / 5 failed, no timeout. Exact failures:

| File | Test | Diagnosis |
| --- | --- | --- |
| `src/shared/adminPresentation.test.tsx` | `covers every registered navigation category and API resource column with readable labels` | Existing `marginLiquidations.bad_debt_amount` missing shared Chinese label |
| `src/admin/resources/resourceConfigs.test.tsx` | `opens a seconds contract pair creation modal from the seconds product page` | New explicit nullable `open_payout_capacity` missing from expected payload |
| Same | `settles and rejects agent commissions from row actions with a dropdown status filter` | New `reversed` filter option missing from expected list |
| Same | `opens margin liquidation record details from hidden row IDs` | Fixture omitted required bad debt field and row-wide `-` matched both email and amount |
| Same | `opens seconds contract product details, edits products, and updates product status from row actions` | New explicit nullable `open_payout_capacity` missing from expected payload |

The first and fourth are confirmed baseline label/fixture issues explicitly
authorized for minimal correction. Other three are expectations lagging changed
feature contracts. No business behavior, timeouts or resource configurations
were changed to make these pass.

## Changes

- `web/src/shared/adminResponseFieldLabels.ts`: add only the missing
  `bad_debt_amount` Chinese label; preserve other owners' new labels.
- `web/src/admin/resources/resourceConfigs.test.tsx`: reflect explicit nullable
  seconds capacity and the reversed commission option; complete liquidation
  fixtures and assert the nullable email in its named column.
- `web/src/admin/governance/financialRetriesApi.ts`: reject active-lease
  submission locally and fail closed on inconsistent mutation receipts.
- `web/src/admin/governance/FinancialRetriesPage.test.tsx`: test receipt
  version/outcome/lease/attempt identity, no false success or automatic retry,
  invalid/stale seconds detail identity, link permission and confirmation
  invalidation when a worker replaces the displayed schedule.
- `tests/unit_src/src_modules_admin_financial_retries_tests.rs`: assert HTTP
  receipt equality with atomic audit `after_json`, changed version, cleared
  lease and unchanged attempt count.
- `.trellis/spec/backend/financial-worker-retries.md`: retain owner/deadline,
  metadata version, schedule version, lease and no-funds-change controls; add
  exact HTTP/audit snapshot and Web receipt contracts.
- This delivery document only; no global spec index or PROGRESS edits.

## Intermediate Checks

All times below are 2026-09-19 Asia/Hong_Kong; parallel feature work continued.
Times without seconds are approximate execution windows.

| Started | Command | Result |
| --- | --- | --- |
| 01:02:24 | `npm --prefix web run test -- src/admin/governance/FinancialRetriesPage.test.tsx src/shared/adminPresentation.test.tsx src/admin/resources/resourceConfigs.test.tsx --maxWorkers=1 --no-file-parallelism --reporter=default --reporter=json --outputFile=/private/tmp/rust-chain-web-gate-focused.json` | 3 files, 141/141; includes the five original failures and 44 E05 tests, before the final confirmation-invalidation test |
| 01:03 | `npm --prefix web run typecheck` / `npm --prefix web run lint` | Passed |
| 01:04:03 | `npm --prefix web run test:production-policy -- --maxWorkers=1 --no-file-parallelism` | 4 files, 15/15 |
| 01:04:03 | `npm --prefix web run test:coverage -- --maxWorkers=1 --no-file-parallelism` | 4 files, 23/23; configured coverage: statements 85.61%, branches 81.78%, functions 85.33%, lines 92.87% |
| 01:04 | `npm --prefix web run build` | Passed; existing lottie direct-eval and large-chunk warnings remain |
| 01:05 | `npm --prefix web run budget` | Passed: initial gzip 460540 bytes, largest async 60078, CSS 73435, total JS 742146 |
| 01:05 | `npm --prefix web run typecheck` / `npm --prefix web run lint` | Passed again after the final E05 confirmation-invalidation test |
| 01:06–01:10 | `FINANCIAL_RETRIES_TEST_DATABASE_URL=mysql://root@127.0.0.1:13316/mysql cargo test --lib financial_retries -- --nocapture` | 6/6 including isolated real MySQL and the new HTTP/audit receipt equality assertion; fixture runtime 6.85s after compilation/build-lock wait |
| 01:05:11–01:12:09 | `npm --prefix web run test -- --maxWorkers=1 --no-file-parallelism --reporter=default --reporter=json --outputFile=/private/tmp/rust-chain-web-gate-after.json` | **92 files, 788/788 passed, 0 failed, 0 skipped, no timeout**, duration 417.22s |

- E05 real isolated MySQL regression passed 6/6 before adding the receipt
  equality assertion. Re-run with the new assertion encountered a concurrent
  missing test module at `src/modules/margin/infrastructure/close_executions.rs:166`
  (`tests/unit_src/src_modules_margin_close_execution_concurrency_tests.rs`).
  No margin changes were made; the file subsequently arrived and the rerun
  passed as recorded above.
- Targeted `rustfmt --edition 2024 --check
  tests/unit_src/src_modules_admin_financial_retries_tests.rs` passed.
- `git diff --check` passed after initial fixes.
- No new live-browser claim: this slice changes validation/tests/a field label,
  not layout. No dev server or production API was accessed.
- Post-fix full-suite report:
  `/private/tmp/rust-chain-web-gate-after.json`, serial execution with unchanged
  test timeouts. JSON timestamps are `2026-09-18T17:05:11.901Z` through
  `2026-09-18T17:12:09.125Z` (the next calendar day in Asia/Hong_Kong).
- A post-run comparison of report file names with all current `web/src`
  test/spec files found 92 files and zero uncollected files. E05 accounts for
  45 passing tests, including the final confirmation-invalidation case.
- All five initial failures are fixed; no remaining failed baseline test was
  observed in this run. Existing Semi React19/createRoot, `rangeSeparatorNode`,
  repeated form-validation and React `act` warnings were not suppressed.

## Handoff Boundary

This is a passing **intermediate** integration gate, not a frozen-tree claim.
Refund and reconciliation snapshot owners were still developing during these
runs. Shared route/access/navigation files were inspected but not changed in
this triage slice. Existing E02 read-only wiring and E08 reversal permissions
remain intact; later feature wiring and new tests require their own validation.

Main must rerun the full Web gate after all feature changes are frozen.
No remaining E05 owner/deadline/manual-review or receipt/lease test gap was
identified in this slice. Browser/live API validation was not repeated.
No workers, money rules, resource configurations, production systems,
PROGRESS, commits or deployment were touched.

## Snapshot Permission Follow-up

New assignment after the intermediate full gate: wire the E02 snapshot and
follow-up API permissions in backend and Web, without default grants or changes
to the child implementation. Earlier full-suite results precede this change.

- [x] Add exact GET/read and POST/operate mapping for collection, detail and follow-ups.
- [x] Test canonical positive-u64 IDs, methods, malformed paths and role isolation on both sides.
- [x] Run focused Rust/Web tests and record validation and any concurrent blockers.

Owned edits in this follow-up:

- `src/modules/admin/service/access_control.rs`
- `tests/unit_src/src_modules_admin_service_tests.rs`
- `web/src/admin/access.tsx`
- `web/src/admin/access.snapshots.test.tsx`
- This delivery note. No child snapshot implementation, routes, navigation,
  permission catalog, role defaults, money logic or migrations changed here.

Exact API contract, beneath `/admin/api/v1/financial-reconciliation`:

| Path | GET | POST |
| --- | --- | --- |
| `/snapshots` | `governance.financial.read` | `governance.financial.operate` |
| `/snapshots/{id}` | `governance.financial.read` | Unmapped |
| `/snapshots/{id}/follow-ups` | `governance.financial.read` | `governance.financial.operate` |

New endpoints reject HEAD/OPTIONS/PATCH/PUT/DELETE in this mapping. The existing
report root retains its previous GET/HEAD/OPTIONS behavior. IDs must be canonical
positive decimal u64, including exact support for `18446744073709551615`; zero,
leading zeros, signs, decimals/exponents, whitespace/newlines, encoded segments,
non-ASCII digits, overflow and extra/trailing segments remain unmapped.
Web uses BigInt rather than Number and verifies full regex consumption.
Read/write/review/settle roles cannot substitute for operate. No grant is added.

Follow-up validation, after the full-suite run above:

- `cargo test --lib financial_reconciliation_snapshot_permissions -- --nocapture`:
  **3/3**, exact path/method matrix, malformed paths and role isolation.
- `cargo test --lib admin_permission_mapping_is_fail_closed_and_action_aware -- --nocapture`:
  **1/1**, existing permission regression.
- `npm --prefix web run test -- src/admin/access.snapshots.test.tsx src/admin/access.test.tsx src/admin/routes.test.tsx src/admin/governance/FinancialRetriesPage.test.tsx --maxWorkers=1 --no-file-parallelism`:
  **118/118**, 4 files, started 01:20:27 HKT; includes 9 new snapshot access tests.
- `npm --prefix web run test:production-policy -- --maxWorkers=1 --no-file-parallelism`:
  **15/15**, started 01:21:02 HKT.
- `npm --prefix web run test -- src/admin/governance/FinancialReconciliationPage.test.tsx --maxWorkers=1 --no-file-parallelism`:
  **26/26**, started 01:21:28 HKT.
- Web typecheck passed at 01:21; targeted Rust formatting, `git diff --check`
  and final Web lint recheck passed.

The initial combined check at 01:18:11 HKT observed concurrent E02 work:
unsupported `SideSheet.destroyOnClose` and `AdminAccessGate is required` in
`先选择币种，只发 GET，五个证据视图均显示口径且没有资金动作`,
`刷新失败保留旧快照并显式提示，不显示新的正常结论`, and
`目录分页使已加载的资产选择失效`. The child owner corrected these during this
slice; subsequent typecheck and all 26 page tests passed. Those child files were
not edited here. These times correspond to 2026-09-18 17:18–17:22 UTC.

The 788-test result still predates snapshot permission wiring. Main owns the
post-freeze complete Web/Rust gates; this follow-up claims focused verification
only, not a new complete-suite certification.

## E05 Clippy Follow-up

Main's `/tmp/hardening-clippy.log` identified two owned warnings. Changed only
the nested kind validation into an equivalent let-chain in
`src/modules/admin/application/financial_retries.rs` and removed the unnecessary
`&format!` borrow in `tests/unit_src/src_modules_admin_financial_retries_tests.rs`.
No lint suppression or behavioral change.

`rustfmt --edition 2024 --check` passed for both E05 files and the shared
`access_control.rs` / Admin service tests. `git diff --check` passed.
`cargo test --lib financial_retries_validate_before_database_and_reject_extra_command_fields -- --nocapture`
passed **1/1** after a 4m15s build-lock/build wait. Main owns the combined full
Clippy rerun. Shared access mappings and their tests are frozen; no duplicate
full Web suite was started while Main runs `/tmp/hardening-web-final.log`.
