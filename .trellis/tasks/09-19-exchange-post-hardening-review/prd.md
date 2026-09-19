# Post-Hardening Business Review

## Goal

Review the current uncommitted workspace after the eight delivered hardening
slices. Give the user a prioritized, evidence-based list of remaining business
workflow gaps without re-listing delivered capabilities as missing.

## Scope

- Inspect user-facing withdrawal, Convert, agency accounting, exception
  operations, trading lifecycle and financial reporting workflows.
- Separate confirmed code gaps from product suggestions and external
  operational capabilities requiring independent evidence.
- Record business impact, closest source evidence and acceptance direction.
- Read-only code review; write only task/review/progress documentation.

## Acceptance

- [x] Confirm each priority against current source and prior delivery boundaries.
- [x] Produce a short actionable recommendation and a durable detailed report.
- [x] Validate referenced files/lines and review documentation.
- [x] Update PROGRESS with checks and explicit unverified areas.

## Boundaries

No code fixes, business threshold choices, new active policies, historical
backfill, database/server startup, financial operations, commit or deployment.
Earlier successful tests are historical evidence, not new tests in this review.

## Outcome

Eight findings are recorded in `research/post-hardening-review.md`. The first
two were locally reproduced from the PC Swap handler and amount conversion;
all 36 source references were checked. Current focused Rust tests passed21/21.
No production code, spec contract, policy, database or deployment was changed.
