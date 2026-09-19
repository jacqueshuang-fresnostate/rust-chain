# E05 Financial Retry Workbench

## Scope and Plan

- [x] Read-only filtered/paginated Admin retry query and source evidence.
- [x] Authenticated, reason-required, atomic audited schedule-only requeue.
- [x] Exact permission mapping, OpenAPI, Chinese Admin workbench and navigation.
- [x] Explicit incident owner/deadline metadata, migration 0131, audited versioned update.
- [x] Readonly seconds manual_review visibility and order link.
- [x] Extended backend/frontend regressions and final validation evidence.

Updated scope: user explicitly requested incident assignment/SLA and reserved
migration 0131. No invented default deadline, automatic refund, financial
decision, business state mutation, worker execution, production access, commit
or deployment. Existing worker contracts remain authoritative. Shared integration
files receive additive wiring only; unrelated concurrent edits are preserved.
E04 ticker/Kline is owned by main and was not started here.

## Implemented

- Read-only snapshot list includes the existing earn/loan/commission retries
  and seconds `manual_review`, with exact kind/outcome filters, pagination and
  matching total/counts. Safe source joins preserve raw Decimal amounts.
- Requeue is reason-required and authenticated, refuses active leases, compares
  the full stored schedule SHA-256 version under lock and changes scheduling
  only. Old command replay cannot shorten a new worker backoff.
- 0131 creates independent owner/deadline metadata; there is no automatic
  assignment or default SLA. PATCH requires explicit nullable fields, reason,
  current metadata version and a valid active Admin owner. Same-transaction
  audit records the actor and before/after snapshots. Clearing retains version
  history. No worker or business contract/schema was changed.
- Seconds rows retain stored failure/time/window evidence; attempt/next-time
  fields are null. They have no requeue action. A separate read-only order
  deep link reuses the existing exact-ID GET and `seconds.orders.read`.
- E02 backend child declarations/merge, Web lazy route/navigation and exact
  existing `governance.financial.read` mapping are wired.
- E08 exact positive-u64 `POST agent-commissions/:id/reversal` mapping uses
  existing `agents.commissions.settle`; no default permission grant.

## Validation

- Latest focused Web tests: 95/95 with `--maxWorkers=1 --no-file-parallelism`,
  typecheck and lint passed.
- Production policy 15/15; configured coverage selection 23/23, thresholds met.
- Web build and budget passed, existing dependency eval/chunk-size warnings remain.
- `cargo check --all-targets` passed after backend feature implementation.
- `FINANCIAL_RETRIES_TEST_DATABASE_URL=mysql://root@127.0.0.1:13316/mysql cargo
  test --lib financial_retries -- --nocapture`: **6/6 passed**, including the
  opt-in real MySQL test. It creates/drops only its own random E05 schema and
  cleans up even when an assertion panics; no default DATABASE_URL is inherited.
  Fixtures cover all three worker source joins and a seconds review order.
  Regressions cover exact permissions, authenticated actor, validation,
  OpenAPI, unchanged repeated GET, kind/outcome counts/pagination, active and
  stale leases, schedule replay/worker-update conflicts, concurrent requeue,
  late-finish fencing, atomic audit rollback, explicit owner/deadline, disabled
  owner refusal, clear/version retention, concurrent assignment, metadata edits
  preserving active leases, and unchanged source states/amounts, wallets and
  wallet/platform journal counts.
- The real query regression found MySQL COALESCE widening a missing metadata
  version to DECIMAL; explicit UNSIGNED projection fixes it and is exercised
  with absent and present metadata in the passing database test.
- Browser fixture-only checks: 1728/1280/768, no document overflow; active
  lease requeue disabled, seconds has no requeue, owner/deadline dialog starts
  blank. Screenshots `/private/tmp/e05-incident-*.png`. This is NOT real API
  browser integration. Vite was explicitly pinned to local API origin.
- Full Rust fmt currently reports unrelated concurrent convert/journal/test
  changes. Own files are individually formatted; no broad formatting applied.
- Scoped rustfmt check and `git diff --check` passed.
- Architecture 11/11 and documentation 1/1 passed.
- Full Web run: 745 passed, 13 failed, one fork worker timeout. Failures include
  known concurrent resource/label changes (trigger_price, commission reversed
  filter, liquidation details), other feature tests and multiple wall-clock
  timeouts. A separate E05 parallel run had one timeout; the serial scoped
  rerun passed all 95 without changing timeouts or weakening assertions.

## Files

- `migrations/0131_financial_retry_incidents.sql`
- `src/modules/admin/{application,infrastructure,presentation,routes}/financial_retries.rs`
- `src/openapi/financial_retries.rs`
- `tests/unit_src/src_modules_admin_financial_retries_tests.rs`
- `web/src/admin/governance/FinancialRetriesPage.tsx`
- `web/src/admin/governance/financialRetriesApi.ts`
- `web/src/admin/governance/FinancialRetriesPage.test.tsx`
- Minimal shared wiring only:
  `src/modules/admin/{application,infrastructure,presentation,routes}.rs`,
  `src/modules/admin/service/access_control.rs`, `src/openapi.rs`,
  `web/src/admin/{access,routes,navigation}.tsx`.
  E02 child implementations and E08 business/action implementations were not edited.
- Feature spec only: `.trellis/spec/backend/financial-worker-retries.md`.

## Handoff

E05 requested functionality is implemented and focused verification passed.
Apply 0131 before enabling this API/page in any future release; no deployment
was performed. Real service validation used the supplied isolated MySQL 9.3,
not production MySQL 8.4. Browser checks used fixtures, not real API login or
write integration. Main should run the final full Web gate after concurrent
resource changes settle. No E05 owner/deadline/manual_review functionality is
being deferred; remaining exclusions below are deliberate policy boundaries.
The temporary fixture browser space and local Vite process were closed.

## Explicit Boundaries

No manual win/loss, unknown-withdrawal refund, automatic financial decision,
new default grants, incident notification scheduler, archival incident search,
PROGRESS edit, commit, deployment or production access. E04 ticker/Kline is
owned by main and was not started here. Main retains refund-policy decisions.
