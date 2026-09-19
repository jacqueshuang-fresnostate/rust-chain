# E02 Reconciliation and Manual Evidence Delivery

## Manual Capture Follow-up Delivered

- [x] Migration 0140: immutable per-asset manual evidence and append-only follow-up.
- [x] Connection reader shared by READ ONLY GET and consistent capture transaction.
- [x] Raw-key/hash/fingerprint replay, atomic audit, optimistic follow-up version.
- [x] Strict history/capture/follow-up UI with partial/capped evidence preserved.
- [x] Isolated DB, UI/type/browser verification and final delivery update.

Shared wiring was completed by E05 Kuhn (agent
`01a0b4fa-2e18-7da1-9650-0b4f5ec8991e`):

- GET `/admin/api/v1/financial-reconciliation/snapshots`
- GET `/admin/api/v1/financial-reconciliation/snapshots/{id}`
- GET `/admin/api/v1/financial-reconciliation/snapshots/{id}/follow-ups`
- POST `/admin/api/v1/financial-reconciliation/snapshots`
- POST `/admin/api/v1/financial-reconciliation/snapshots/{id}/follow-ups`

GET uses `governance.financial.read`; POST uses
`governance.financial.operate`. Positive numeric IDs and exact method/path only;
unsupported methods or extra segments remain unmapped. Child `routes()` export
is unchanged, no navigation export is needed. The requested `multi_agent`
coordination tool was unavailable in this resumed tool set; no thread-message
substitute was used. Main relayed the exact contract to E05. Shared permission
wiring remains owned by E05/main and was exercised by the final real-DB route test.

### Follow-up Behavior

- Migration `0140_financial_reconciliation_snapshots.sql` adds two sidecar tables.
  Snapshots retain asset identity/precision, schema version, server manual-capture
  timestamp, actor, reason, raw key/key hash/request hash, canonical report digest
  and immutable JSON. Follow-ups append versioned owner/deadline/notes, operator,
  reason and replay identity. Four triggers reject UPDATE/DELETE of these rows.
- The existing current-report GET remains explicit repeatable-read consistent
  `READ ONLY`. Connection-based readers are reused by manual capture inside a
  consistent read/write transaction; only the sidecar and standard audit are
  written. Financial source rows are never locked or mutated by this feature.
- Stable replay compares the raw key as well as SHA-256 key/request digests.
  Changed payloads and simulated hash collisions fail with 409. Concurrent
  duplicate capture/follow-up returns the first saved response with one audit.
  On unique insertion races, rollback before rereading the original receipt.
- Report digests use recursively sorted JSON object keys because MySQL normalizes
  key ordering. Saved JSON content, decimal strings, `coverage=partial`, limitations,
  detail limits and exact totals remain unchanged. This is not byte-for-byte
  preservation of input JSON formatting or a signed third-party attestation.
- Follow-up obtains the existing parent snapshot row lock before the transaction's
  first consistent read of immutable child history. It does **not** range-lock a
  missing child record. Same-parent writers serialize; two different parents can
  both receive first follow-ups without next-key insertion deadlocks.
- Owner must be an active administrator. Owner/deadline are explicit nullable
  inputs; notes and reason are required. Optimistic version conflicts preserve the
  original UI draft and version. DateTime(3) deadlines round-trip milliseconds,
  including years 1000/9999, using TIMESTAMPDIFF rather than limited UNIX_TIMESTAMP.
- Capture/follow-up and `financial_reconciliation.capture|followup` audit commit
  atomically. No scheduler, daily close, synthetic historical balance, backfill,
  financial correction, discrepancy-resolution state, or balance mutation exists.
- UI adds manual capture, bounded readonly history, original-evidence drawer and
  append-only follow-up records. Existing ConfirmAction, table, date, dirty-state
  and recoverable session-scoped idempotency helpers are reused. Unknown outcomes
  retain original keys, and receipts must match actor/identity/reason/key/fields.

### Current Verification

Exact real-DB command used in this resumed delivery:

```sh
env RECONCILIATION_DATABASE_URL=mysql://root@127.0.0.1:13316/hardening_reconciliation_test cargo test --lib financial_reconciliation -- --nocapture
```

**8/8 passed, 0 ignored**, including both real MySQL tests, three E05 permission
mapping tests, input validation and OpenAPI alias test. The named isolated DB did
not initially exist on port 13316 and was created there only for these tests.
No other database was used; the test asserts the exact database name. Migration
runner through 0140 is exercised twice; fixtures remain in this dedicated DB.

The earlier accepted core note transcribed a port-3306 command. That connection
was not re-established in this resumed session; it is superseded by the exact
successful port-13316 command above, not claimed as current verification.

Verified in the final DB run:

- All 13 report obligation metrics now have nonzero fixtures across selected
  assets, including new-coin manual frozen quote and pending assigned-asset
  commissions. Terminal/legacy/unknown-asset exclusions remain explicit.
- Original READ ONLY enforcement and consistent report snapshot under concurrent
  wallet changes; per-asset/per-transaction noncancellation and 105/100 cap.
- Concurrent same-key capture and follow-up, changed-payload conflict, simulated
  raw-key/hash collisions for both tables, original receipt after later versions,
  immutable JSON after live-wallet changes, DB UPDATE/DELETE rejection.
- Audit-failure rollback for both commands using a scoped test trigger, removed
  before assertions; inactive/nonexistent-owner validation, explicit owner/deadline
  clears, year-1000/year-9999 deadlines, pagination/latest-version invariants.
- Deterministic distinct-parent empty-history test acquires both parent locks
  before concurrent first inserts, plus concurrent first application follow-ups
  on two fresh snapshots. No deadlock retry masks this test.
- Registered HTTP reads: unauthenticated 401, read-authorized 200, read-only POST
  403, operate-authorized capture/follow-up 200, permission removal 403.
  Unsupported matched methods and malformed numeric IDs fail 403; extra unmatched
  path returns 404 (not a fabricated middleware 403).
- `/openapi.json` equals `/api/openapi.json`; all six operations and actual
  response/request DTO schemas are present, including required nullable inputs.

Additional commands/results in this follow-up:

- `cargo check --lib`: passed. Early `cargo check --all-targets` was blocked by
  contemporaneous non-E02 seconds test imports; main later reported all-targets
  and clippy green. Those main results are integration evidence, not commands
  independently rerun by this sidecar after freeze.
- `cargo test --test backend_architecture --test backend_documentation -- --nocapture`:
  **11 + 1 passed**.
- `rustfmt --edition 2024 --check --config skip_children=true` on all 12 owned
  Rust production/test files: passed after final route/test formatting.
- `npm --prefix web run test -- src/admin/governance/FinancialReconciliationPage.test.tsx src/admin/governance/FinancialReconciliationSnapshots.test.tsx`:
  **42/42 passed** (26 current-report + 16 snapshot/parser/command/UI tests).
- `npm --prefix web run typecheck`, `npm --prefix web run lint`,
  `npm --prefix web run build`, `npm --prefix web run budget`: passed.
- `npm --prefix web run test:production-policy`: **15/15 passed**.
- `npm --prefix web run test:coverage`: **23/23 passed**; configured shared-policy
  coverage, not a claim of E02 branch coverage.
- `git diff --check`: passed. No lint/whitespace suppression was added.
- Main separately reported Web **820/820**, lint/type/build/budget/policy/coverage,
  Mobile **779** release gate, PC **110** plus type/build, and clippy green.
  This sidecar did not rerun those complete client suites.

Initial failing checks were corrected, not hidden: sandbox loopback permissions;
missing isolated DB; MySQL JSON key-order digest mismatch; new-coin supply counter
and commission source-ID fixture requirements; CREATE TRIGGER requiring raw SQL
instead of prepared protocol; repository Axum `:id` dynamic syntax (OpenAPI still
uses `{id}`); and a UI assertion that ambiguously matched both warning/error
alerts. The final DB/UI counts above are after these fixes.

### Browser Evidence

Ego Browser TaskSpace 16 used the actual page, permissions provider, parser,
query client and components with local fixture HTTP responses. Verified manual
capture confirmation and POST identity, immutable-history route loading,
`100 / 105` truncation and exactly one visible drawer tabpanel, owner/nullable
deadline/notes submission, and no document horizontal overflow at
1728/1280/768/390px. The modal was 560px wide on desktop and 358px at 390px, within
the viewport; screenshots were visually inspected.

- `/private/tmp/e02-followup-1728.png`
- `/private/tmp/e02-followup-1280.png`
- `/private/tmp/e02-followup-768.png`
- `/private/tmp/e02-followup-390.png`
- `/private/tmp/e02-capture-history-1280.png`

Browser task finished, temporary `web/e02-snapshot-preview.html` removed and
loopback Vite port 17114 stopped. This is real-component/mock-HTTP browser
verification, **not** authenticated full-shell browser-to-real-API E2E. Existing
Semi DatePicker `rangeSeparatorNode` React warning was observed and not suppressed.

### Follow-up File List

Modified existing E02 files:

- `src/modules/admin/application/financial_reconciliation.rs`
- `src/modules/admin/infrastructure/financial_reconciliation.rs`
- `src/modules/admin/infrastructure/financial_reconciliation/wallets.rs`
- `src/modules/admin/infrastructure/financial_reconciliation/obligations.rs`
- `src/modules/admin/presentation/financial_reconciliation.rs`
- `src/modules/admin/routes/financial_reconciliation.rs`
- `tests/unit_src/src_modules_admin_financial_reconciliation_tests.rs`
- `web/src/admin/governance/financialReconciliationApi.ts`
- `web/src/admin/governance/FinancialReconciliationPage.tsx`
- `web/src/admin/governance/FinancialReconciliationPage.test.tsx`

Added:

- `migrations/0140_financial_reconciliation_snapshots.sql`
- `src/modules/admin/application/financial_reconciliation/snapshots.rs`
- `src/modules/admin/infrastructure/financial_reconciliation/snapshots.rs`
- `src/modules/admin/presentation/financial_reconciliation/snapshots.rs`
- `tests/unit_src/src_modules_admin_financial_reconciliation_snapshot_tests.rs`
- `web/src/admin/governance/financialReconciliationSnapshotsApi.ts`
- `web/src/admin/governance/FinancialReconciliationSnapshots.tsx`
- `web/src/admin/governance/FinancialReconciliationSnapshots.test.tsx`
- `web/src/admin/governance/financialReconciliationSnapshots.fixtures.ts`
- `src/openapi/financial_reconciliation.rs`
- `.trellis/spec/backend/financial-reconciliation.md`

`src/openapi.rs` received only the reconciliation module declaration and document
merge; peer edits were preserved. This delivery note was updated. No PROGRESS,
global spec index, global navigation, money/order execution, market, production,
commit or deployment edits/operations by this follow-up.

## Original Read-Only Slice

- [x] Read PRD, review, backend/Admin specs, codegraph and neighboring contracts.
- [x] Coordinate shared exports/routes/access/navigation through main and E05.
- [x] Implement per-asset consistent read-only evidence and strict Admin parser/page.
- [x] Verify isolated MySQL, focused frontend checks and browser layout.
- [x] Record exact files, commands and remaining gaps.

Contract: `GET /admin/api/v1/financial-reconciliation`, permission
`governance.financial.read`. The original slice had no migration; the approved
follow-up above adds 0140 only. No backfill, balance mutations or production
operations. History coverage is always partial. Journal net movement
is not custody stock, and obligations must not be added to overlapping wallet
buckets or across different assets.

## Implemented Evidence

- Explicit repeatable-read, consistent-snapshot, READ ONLY transaction. Asset
  directory uses deterministic paging and includes disabled assets. No asset is
  implicitly selected, and a missing selected asset returns not-found.
- Journal sums are checked separately by transaction key within the selected
  asset. Opposite bad transactions and opposite legs in different assets cannot
  cancel the anomaly count. Raw context/account net movements are not balances.
- Spot and margin wallets independently use their latest ledger ID's available,
  frozen and locked snapshots. Missing ledger, missing wallet and nonmatching
  buckets are distinct. Comparable deltas exclude missing evidence and never
  replace it with zero. Internal wallets are not mislabeled customer liabilities.
- Thirteen separate current-business metrics: seconds stake/conditional payout,
  prediction stake/conditional payout, earn principal, loan principal receivable
  and unreleased collateral, margin collateral/recorded interest, withdrawal
  reserved amount, manual new-coin frozen quote, assigned-asset pending commission
  and spot unfilled base quantity. These overlap and are never totalled.
- Each difference list and context/account movement list has a 100-row cap with
  an exact total. The UI explicitly identifies truncation.
- Strict parser retains decimal strings, verifies identity/count/null contracts,
  rejects `coverage=complete`, and does not infer a green all-clear state.

## Original Verification History

- Original accepted readonly core: **2/2 passed**, including the real MySQL test
  (not skipped). The earlier port transcription is superseded by the independently
  executed port-13316 command in Current Verification above. The test refuses
  any other database name. Fixtures remain only in that dedicated local database.
  It verifies per-asset and per-transaction isolation, noncancelling anomaly
  counts, latest ledger-ID bucket snapshots, both wallet scopes, missing evidence,
  all 13 obligation SQL paths, then 11 nonzero obligation metrics (now 13 in the
  follow-up), terminal exclusion,
  per-order payout truncation, read repeatability, concurrent snapshot isolation,
  MySQL rejection of UPDATE in the read-only transaction, journal detail 105/100
  truncation, GET 200, missing auth 401, read-only-role POST 403, revoked permission
  403 and no audit write.
- Initial MySQL attempts exposed sandbox loopback restrictions and invalid test
  fixtures (required earn introduction JSON and collateral-product LTV fields).
  These were corrected, together with confirmed-withdrawal chain evidence.
  A POST expectation was corrected from 405 to the authoritative middleware's
  fail-closed 403. Final rerun passed. No business constraints were weakened.
- `cargo check --all-targets`: **passed**.
- `cargo test --test backend_architecture --test backend_documentation -- --nocapture`:
  **11 + 1 passed**.
- `rustfmt --edition 2024 --check --config skip_children=true` over the seven
  owned Rust files: **passed**.
- `cargo fmt --all -- --check`: **failed outside E02 ownership** at the observed
  run: convert/mod, risk/domain, seconds journal/test, margin liquidation test,
  seconds routes test and wallet rate-limit test. No unrelated formatting edits.
- `npm --prefix web run typecheck`: **passed** after final parser edits.
- `npm --prefix web run lint`: **passed**; final focused ESLint over all four
  owned TypeScript/TSX files also **passed**.
- `npm --prefix web run test -- src/admin/governance/FinancialReconciliationPage.test.tsx`:
  **26/26 passed** after final consistency assertions.
- `npm --prefix web run test -- src/admin/governance/FinancialRetriesPage.test.tsx -t 'E02 复用只读权限'`:
  **1 passed, 26 intentionally filtered**, verifying E05's exact E02 wiring.
- `npm --prefix web run test:production-policy`: **15/15 passed**.
- `npm --prefix web run test:coverage`: **23/23 passed**, shared-policy coverage
  85.61% statements / 81.78% branches / 85.33% functions / 92.87% lines.
  This is the repository's configured shared-policy coverage set, not E02 coverage.
- `npm --prefix web run build` and `npm --prefix web run budget`: **passed**,
  including the final build. Existing lottie eval/chunk-size warnings remain.
- `git diff --check`: **passed**.
- `npm --prefix web run test` was attempted but **not completed**. After failures
  in shared Admin presentation, resourceConfigs (commission/margin/earn),
  NewCoinProjectPage and AdminResourcePage (including timeout failures), this run
  was interrupted to stop broad shared-machine contention. It is not a green
  all-Admin gate and those non-E02 failures were not repaired here.
- Follow-up
  `npm --prefix web run test -- src/admin/governance/FinancialReconciliationPage.test.tsx src/shared/adminPresentation.test.tsx src/admin/access.test.tsx src/admin/routes.test.tsx`:
  **109 passed, 1 failed**. The earlier loan label was fixed by peers; the observed
  remaining failure is `spotOrders.trigger_price` missing a shared Chinese label
  in `adminPresentation.test.tsx:31`. Relayed to main/spot owner.
- Ego Browser TaskSpace 11 used the actual E02 page/components with local mock
  HTTP responses, not production or a real browser-to-API session. Checked
  1728/1280/1440/1024/768px: no document horizontal overflow, one visible tabpanel,
  five evidence tabs accessible, real pointer resize changed 120px to 180px.
  Screenshots: `/private/tmp/e02-wallets-1728.png`,
  `/private/tmp/e02-obligations-1280.png`, `/private/tmp/e02-coverage-768.png`.
  Browser task finished; temporary preview HTML removed; loopback Vite 17114 stopped.

## Owned Files

- `src/modules/admin/application/financial_reconciliation.rs`
- `src/modules/admin/presentation/financial_reconciliation.rs`
- `src/modules/admin/routes/financial_reconciliation.rs`
- `src/modules/admin/infrastructure/financial_reconciliation.rs`
- `src/modules/admin/infrastructure/financial_reconciliation/wallets.rs`
- `src/modules/admin/infrastructure/financial_reconciliation/obligations.rs`
- `tests/unit_src/src_modules_admin_financial_reconciliation_tests.rs`
- `web/src/admin/governance/financialReconciliationApi.ts`
- `web/src/admin/governance/FinancialReconciliationPage.tsx`
- `web/src/admin/governance/FinancialReconciliationReportView.tsx`
- `web/src/admin/governance/FinancialReconciliationPage.test.tsx`
- This delivery note.

Shared module declarations, permission mapping, route registration and navigation
were wired by E05 Kuhn via main, not edited by this sidecar. No
PROGRESS/global spec-index edits were made by E02. Follow-up migration 0140 is
listed above; E02 does not consume migration
versions 0138 (reserved for seconds cap) or 0139 (potential refund policy).

## Remaining Product Boundaries

- Current reports and manual immutable captures are diagnostic, not daily
  accounting close or business-to-journal completeness audits. Snapshot-specific
  owner/deadline/notes now exist, but they do not execute or certify financial
  remediation. No automatic midnight close, reconstructed historical end-of-day
  balance, backfill or complete-history start date exists.
- No custody balances, opening equity, proof of reserves or solvency calculation.
  Context/account movements include any newly recorded peer journals without
  treating their earliest entry as coverage certification.
- Current wallet sums include internal/system wallets. They are current wallet
  bucket totals, not independently classified customer liabilities.
- Earn yield/redemption fees, unbooked loan interest, unrealized margin PnL and
  future margin interest are not estimated. Pending loan applications are not
  counted as principal receivables.
- Spot exposure is open/unfilled base quantity, not executable liquidity or
  remaining quote reservation. New-coin data is only manual frozen quote, not all
  token distribution/unlock obligations. Commission rows without payout asset
  identity, and commissions not yet generated, cannot be assigned to an asset.
- The 13 obligation metrics overlap with each other and wallet buckets. There is
  no combined liability total or cross-asset conversion. Conditional payout sums
  do not imply all mutually exclusive outcomes can win together.
- Difference/account movement detail is capped at 100 with an exact count.
  There is no discrepancy drill-through pagination/export in this bounded slice.
- Aggregate SQL scans current rows within a repeatable-read snapshot. No
  production-volume benchmark or new indexing/materialization was performed.
- Browser evidence is page-level real-component/mock-HTTP verification, not
  authenticated full-shell browser-to-real-API E2E or production verification.
- Real-DB nonzero fixtures now cover all 13 displayed metrics, including
  new-coin frozen quote and pending commissions. This proves those query branches,
  not complete business/journal coverage, complete lifecycle permutations or all
  possible obligations.
- Manual snapshot retention has no automatic pruning/export workflow. Evidence
  and follow-up tables are append-only; history pages are bounded, but accumulated
  storage requires operational sizing. No production-volume benchmark was run.
- Report evidence has no external custody verification, cryptographic signature
  or protection against privileged database administrators replacing schema/data.
- All results are point-in-time observations of a concurrently changing worktree.
  Main must run the final integrated gate after peer changes settle. No commits,
  deployment, production operation or extra agent dispatch occurred.
