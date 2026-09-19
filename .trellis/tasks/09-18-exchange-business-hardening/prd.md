# Exchange Business Hardening

## Goal

Implement the eight prioritized findings in the exchange business review, with safe defaults, end-to-end controls, tests and incremental progress records. Preserve all pre-existing dirty changes.

## Execution Plan

1. Withdrawal rate-limit wiring and explicit failure behavior.
2. Market provenance retained through REST/WS and visible on clients.
3. Explicit conditional-order trigger semantics without changing legacy orders.
4. Manual spot-fill actor/reason audit and a controlled admin workflow.
5. Financial exception operations with readonly visibility and audited rescheduling.
6. Per-asset platform journal coverage, reconciliation and exposure reporting.
7. Configurable aggregate withdrawal/loan/exposure limits with transactional enforcement.
8. Explicit, audited commission reversals and refund policy configuration.

## Business Boundaries

- Never pick winners/losers or invent funds to reconcile differences.
- Legacy stop-limit orders retain their stored contract. New explicit trigger directions support rising/falling conditions.
- Do not invent active monetary thresholds, refund deadlines or retroactive commission bases. Configuration requiring business choice stays explicit and inactive until approved.
- A reschedule action never directly pays/refunds an order or steals an active worker lease.
- Unknown chain broadcast outcome never permits a speculative refund.
- External custody, KYC/AML providers and actual deployment/backup certification require operational evidence; do not claim implementation of an external system.
- Optional new product lines, institutional APIs and exchange matching architecture replacement require separate product requirements.
- No production operations, commit, push or deployment.

## Acceptance Criteria

- [x] E01a: Withdrawal rate limits operate before wallet mutation, exact replay bypasses new consumption, configured protection cannot silently fail open.
- [x] E01b: Aggregate allowances, address/security-change cooling and tiered review require explicit configurable business policy.
- [x] E04: Provenance survives REST and WS and is visible; unknown sources are not labeled real.
- [x] E03: Explicit trigger direction and durable activation work without retroactive changes.
- [x] E07: Manual fills require authenticated actor/reason, atomically audited, with no duplicate effects.
- [x] E05: Exception list/filter/pagination and audited safe requeue are available in Admin.
- [x] E02: Financial coverage/reporting is truthful, per-asset and verified for changed paths.
- [x] E06: Aggregate limits and inventory visibility have deterministic concurrency rules and configurable policy.
- [x] E08: Commission reversals preserve original amounts and immutable audit without negative-balance invention.
- [x] Closest unit/integration/frontend checks pass or limitations are recorded precisely.
- [x] Each delivered slice updates PROGRESS.md; remaining policy decisions are not marked complete.

## References

- `.trellis/tasks/09-18-exchange-business-gap-review/research/exchange-business-review.md`
- `.trellis/spec/backend/business-governance-audits.md`
- `.trellis/spec/backend/risk-configuration.md`
- `.trellis/spec/backend/spot-orders.md`
- `.trellis/spec/backend/financial-worker-retries.md`
- `.trellis/spec/backend/wallet-amount-precision.md`

## Final Status

All eight bounded implementation slices are delivered and verified locally.
Final Rust unit/architecture/documentation/OpenAPI gates passed 463/11/1/10;
strict all-target Clippy, fmt and diff checks passed. Web820 plus the later
test-only focused18, Mobile779 release gate and PC110/type/build passed.
Final seconds route suite28 passed with BOTH dedicated database configurations
set, so the principal-refund comprehensive scenario actually ran.

Each slice has its own delivery evidence and PROGRESS entry.
`research/release-checklist.md` records migrations, inactive policy decisions
and operational limits. Isolated MySQL/Redis were shut down after testing;
the pre-existing local Web preview was preserved. No commit, auto-archive
commit, deployment, production operation or financial policy activation.

## Milestone History

E01a delivered: risk unit selection 19/19 and isolated MySQL + Redis route regression 1/1 passed. Remaining slices are in progress or pending; this is not an all-complete status.

E07 delivered with real MySQL 11/11, frontend 12/12 and local mocked-browser
desktop/mobile verification. E04 trade/depth and E06 loan sub-slices delivered;
their broader criteria remain unchecked. E02 seconds/prediction/convert first
journal slice verified, margin and terminal prediction verification in progress.

E05 workbench delivered with owner/deadline/version/lease controls and readonly
seconds manual-review visibility (DB6, Web95). E08 reversal delivered and repeated
real-DB scenario passed with existing commission routes7, units5 and Web14.
Prospective opt-in seconds no-evidence principal refund follow-up is delivered,
default inactive and no backfill for existing orders. Real-DB comprehensive
race/replay/rollback/event checks passed twice; focused Web18 passed including
one additional test after the Web820 gate. Persisted manual report capture and
case follow-up are delivered: real-DB selection8/8, Web42/42, desktop/narrow
mock-browser verified. No historical end-of-day balances are inferred.

E03 delivered: isolated MySQL spot routes 62/62, spot units 26/26, explicit
rising/falling intent and durable activation preserve legacy contracts. PC
1728/1024/390px actual-form mock-browser checks passed after scoped layout repair.

E06 delivered: Loan aggregate principal, Earn principal/gross liability, Convert
explicit inventory budget and Seconds maximum unpaid payout caps are opt-in.
Earn routes 22/22; concurrent budget admission and terminal release tests passed;
actual Admin forms were checked at desktop/390px. This is per-product/pair
admission control, not a cross-product capital pool or custody certification.
