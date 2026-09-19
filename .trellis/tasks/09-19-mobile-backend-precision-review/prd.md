# Mobile and Backend Precision Review

## Goal

Answer whether the current Mobile and Rust backend can lose financial precision.
Distinguish presentation rounding, mutation inputs, arithmetic, and persistence.

## Scope

- Trace Mobile swap and financial request decimal boundaries.
- Inspect backend float conversions, asset quantization and database scale boundaries.
- Reproduce confirmed findings locally without live funds or services.
- Deliver a prioritized evidence report and progress record.

## Acceptance Criteria

- [x] Identify verified losses with exact source references and impact.
- [x] Identify checked protections without claiming exhaustive safety.
- [x] Run focused tests or executable reproductions and record limitations.
- [x] Update progress and task metadata.

## Delivery

See `research/precision-review.md` and its two executable reproduction scripts.
Mobile focused tests: 36/36. Rust selected non-database tests: 61/61.
Review only; no business fixes, database operations, or commits.

## Constraints

Review only: no business code changes, policy changes, deployment or commits.
Preserve unrelated dirty work. Do not restart database services for this review.
The user's question defines the scope; no requirements clarification is needed.
