# 后台前后端系统设计巡检与可靠性改进

## Goal

Continue the Admin system review across frontend and backend; fix concrete,
reproducible operation/design flaws rather than making cosmetic or speculative
architecture changes. Prioritize cross-layer consistency and preventing mistaken
or duplicate operations.

## Confirmed user scope

- Review both Admin frontend and backend, not only market strategies.
- Implement improvements for unreasonable designs found in code.
- Previous task shipped as work commit 80f1030, remote head e0d8f93; clean baseline.

## Working priorities

1. Concurrent/late requests, repeated submission and contradictory UI state.
2. Configuration save/effect semantics, authoritative detail hydration and stale
   edits; validation/status changes consistent with backend rules.
3. Permission and audit boundaries, bounded reads and useful failure behavior.

## Plan

1. Parallel, read-only code audits of backend mutations and frontend resources;
   main audit of request/settings cross-layer flow. Persist evidence and classify
   risk, affected domains and tests.
2. Converge on a small set of confirmed high-impact defects, add regressions,
   and implement compatible repairs without redesigning unrelated flows.
3. Verify affected backend/frontend tests plus quality gates; record deferred
   design questions and rollout implications explicitly.
4. Update project specs and PROGRESS. No auto commit/push for this new task.

## Acceptance (to refine from evidence)

- [x] Evidence-based findings inventory for frontend/backend boundaries.
- [x] Selected defects reproduced in tests and repaired across affected layers.
- [x] Permissions, audit, DecimalText and transactional invariants preserved.
- [x] Relevant automated checks pass; unavailable runtime tests are documented.

## Out of scope

No production configuration/database/financial writes, live strategy manipulation,
credential changes, dependency replacement, schema-history rewrites, speculative
framework migration or unrelated visual redesign. Business-policy changes need
explicit confirmation. Existing production browser connectivity limitations are
not mistaken for application bugs.

## Open decisions

No initial blocking question: use existing API/spec contracts to determine the
first repair slice. Raise actual business-policy choices if source evidence shows
more than one legitimate behavior.

## Selected implementation slice

- Recharge uses the locked asset precision (0..18), rejects excess fractional
  precision without rounding, preserves trailing-zero equivalence and existing
  idempotent replay. UI uses authoritative option precision and explains limits.
- Convert create/update accept only implemented pricing modes and executable
  `[0,1)` spread/fee ratios with at most 8 meaningful decimal places for storage.
  Only explicit configuration-free disable can stop a legacy invalid pair.
  Frontend mirrors exact-decimal checks and validates
  nonnegative, ordered amount ranges without altering payload units.
- Generic resource details are latest-request-owned; old responses/errors and
  closing/unmounting cannot replace/reopen the current drawer.
- Changing list/filter/page/reloading invalidates old selections immediately;
  batch actions cannot submit invisible stale rows while list data is changing.
- Capture approval-executor and singleton-config concurrency debt separately;
  do not silently change those protocols in this bounded repair slice.

## Scope/ownership

Root: matching frontend convert/recharge validation, shared asset option precision,
coordination, tests/gates, specs/progress. Backend worker: recharge and convert
validation/locking plus Rust regressions. Frontend worker: resource/detail/batch
lifecycle and regressions. Shared files use explicitly disjoint sections.

Recharge UI validation occurs after confirmation reason entry. A read-only exact
pending-intent lookup preserves original-key recovery when asset precision
changes; it never allocates a new lease or permits a different reason to bypass
the new-intent check. Backend always remains authoritative.

## Added mobile request (confirmed 2026-09-07)

User also requested hiding the thin line in a mobile chart screenshot. Three
distinct objects are visible (MA curves, candle wicks, current-price dashed line).
The user explicitly selected candle wicks (影线). Hide upper/lower wicks via
`wickVisible: false` on the shared `LightweightMarketChart` candle series.
Preserve candle bodies, raw OHLCV, MA lines, latest-price line, themes, gestures
and viewport behavior. Do not rewrite backend high/low to hide a visual element.

Plan: add a scoped regression and observe failure, change the renderer option,
run focused chart tests plus the Mobile release gate, and update specs/progress.
No commit, push or production data changes without a separate user instruction.

## Refined acceptance

- [x] Excess-precision recharge has no receipt/wallet/ledger/audit side effect;
  valid precision/trailing zeros and duplicate replay remain supported.
- [x] Invalid pricing modes, spread/fee bounds and reversed/negative amount
  ranges are rejected before UI submission and by backend create/update.
- [x] Delayed detail A after B, delayed errors and close/unmount are covered.
- [x] List transitions cannot trigger batch actions against stale selection.
- [x] Existing financial idempotency, audit transactions and DTO contracts pass.

## Verification progress

- Real disposable MySQL: new financial routes 5/5, existing recharge concurrent
  replay 1/1, convert CRUD/audit rollback 5/5 passed. Temporary database and
  triggers removed; remaining schema count verified 0.
- Two legacy rollback tests initially got 401 before their intended audit
  failure. Replaced nonexistent-admin tokens with valid scoped fixtures and
  uniquely named, exact-admin/action audit triggers; final assertions prove
  DATABASE_ERROR at the intended insert and rollback, not mere authentication.
- Rust library 338/338 and architecture 11/11 passed. Initial five mock-provider
  tests needed localhost bind permission; full rerun passed with escalation.
  Final fmt, all-target/all-feature check and Clippy -D warnings passed.
- Final Web all-tests: 78 files / 607 tests passed in 337.85s with one worker and
  unchanged timeouts. Typecheck/lint, production-policy 15/15, coverage 23/23
  (lines 92.87%, branches 81.78%), build (3790 modules) and budget passed.
- Full live browser matrix was not run; no production session was used.
- Backend selected repair slice is ready for user-confirmed commit. Mobile
  candle-wick hiding is now implemented as a separate rendering-only slice;
  18 focused tests and full Mobile release:gate (686 tests) passed. Deferred
  governance improvements remain separate.
