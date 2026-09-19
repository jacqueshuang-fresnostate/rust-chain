# Business Hardening Release Checklist

## Status

Implementation is locally verified and uncommitted. All eight bounded feature
slices are delivered, including the final seconds-route fixture corrections.
This document does not authorize deployment, activate a policy or certify
production readiness. The task is marked completed without auto-archiving,
because the archive workflow would create an unrequested commit.

## Migration Order

Verify the deployed schema and existing SQLx migration checksums first.
Versions 0129 (durable financial retries) and 0130 (session generations) are
prerequisites from earlier work, not assumed to be applied in production.
Apply the repository migration chain in order; never edit applied SQL files.

| Version | Purpose |
| --- | --- |
| 0131 | Exception ownership, due dates and optimistic versions |
| 0132 | Nullable explicit spot trigger direction and durable activation |
| 0133 | Loan user/product exposure caps and serialization |
| 0134 | Withdrawal policy, address registration, review receipts and security clocks |
| 0135 | Commission reversal receipts |
| 0136 | Earn capacities and explicit Convert inventory budgets |
| 0137 | Commission reversal idempotency-key metadata |
| 0138 | Seconds unpaid maximum payout capacity |
| 0139 | Prospective seconds principal-refund policies and receipts |
| 0140 | Immutable manual reconciliation evidence and append-only follow-ups |

The migrations were exercised on isolated local MySQL 9.3, not production
MySQL 8.4. Rehearse against the deployed version and realistic table sizes.
Check trigger privileges, DDL lock duration, backups and recovery before release.
Do not start a new backend against an old schema.

## Explicit Configuration

- New monetary caps and refund policies remain disabled/unset. No limits,
  cooling durations, reviewer thresholds or refund waiting periods are guessed.
- Withdrawal rolling windows are not a silently assumed local calendar day.
  Confirm KYC tiers, asset units, review count and cooling policy explicitly.
  Address cooling requires a usable client address-registration workflow or an
  approved operational equivalent; PC/Mobile registration screens are not
  included here. Registration maturity is not a complete revocable allowlist.
- Loan and Earn caps admit new obligations transactionally. Tightening a cap
  does not rewrite or liquidate existing contracts.
- Convert budgets require explicit funding amount, reference, reason and
  revision. They do not prove deposits or custody. Enabled forward-output
  protection rejects reverse output in a different asset; no implicit funding
  from incoming trades or cross-asset netting.
- Seconds payout capacity retains manual-review liabilities. Principal refunds
  require an enabled policy captured when a genuinely new order opens.
  Existing orders are never backfilled into eligibility.
- No refund is an operator choice of win/loss. Evidence, original debit,
  principal amount and commission state must pass the transaction checks.
  Unknown withdrawal broadcast outcomes never use this refund mechanism.
- Review roles before granting access. Read-only reporting does not imply
  operate, settle or manual-fill privileges. No default role grants were added.

## Deployment Compatibility

- Coordinate PC and API release for new stop-limit direction. Existing nullable
  direction and legacy replay fingerprints retain their original meaning.
- Deploy terminal `refunded` display and ledger labels with the refund API.
  Refund is neutral, principal-only, and not a fabricated trading gain.
- Reconciliation captures are manual observations at an explicit time, not
  certified daily closes. They retain partial coverage and capped detail.
  Assignment/follow-up does not alter evidence, wallet balances or journals.
- Journal additions are prospective. Do not fabricate historical opening
  entries, custody balances, cross-asset totals or an all-clear reconciliation.
- Requeue only reschedules eligible work and cannot steal an active lease.
  The original business transaction remains responsible for money idempotency.

## Verification Evidence

See the E01-E08 delivery notes in this directory for exact paths and real-DB
commands. Browser checks use actual components and local mocked API responses;
they are not a live authenticated production end-to-end test.

- Web complete suite: 820/820; one later refund test-only addition passed in
  the focused 18/18 rerun. Typecheck, lint, build and bundle budget passed.
  Production policy: 15/15. Configured shared-policy coverage tests: 23/23,
  statements 85.61%, branches 81.78%, functions 85.33%, lines 92.87%.
  Those percentages are not whole-application coverage.
- PC complete suite: 110/110; typecheck and production build passed.
- Mobile release gate: 779/779; app/test typechecks, PWA/Tauri builds, artifact
  isolation, bundle/source-size budgets and critical-test quality passed.
- Rust all-target/all-feature Clippy with warnings denied passed.
  Library 463/463, architecture 11/11, documentation 1/1 and OpenAPI 10/10 passed.
  Optional database branches in the library suite are not counted as live-DB
  proof; their dedicated explicitly configured runs are documented separately.
- Seconds principal-refund real-DB comprehensive scenario passed twice,
  including settlement/payment/evidence races, atomic rollback and one
  post-commit private refresh event. Full seconds routes then passed 28/28
  with BOTH `DATABASE_URL` and `SECONDS_REFUND_TEST_DATABASE_URL` explicitly
  set to their isolated databases and both Redis URLs supplied. The refund
  scenario actually ran again, not its missing-environment early return.
  Two obsolete nonexistent-identity fixtures now use authenticated actors and
  scoped SQL failure injection; rollback/error assertions remain intact.
- Reconciliation real-DB/input/permission/OpenAPI selection: 8/8, including two
  explicitly configured real-DB tests; current-report/snapshot UI tests: 42/42.
  Covers immutable replay, audit rollback, all 13 nonzero obligation metrics
  and distinct-parent concurrent first follow-ups without deadlock retries.
- Final `cargo fmt --all -- --check`, `git diff --check` and task-context
  validation passed. The strict Clippy gate passed again after the last test edit.

Final full seconds-route command, with proxy environment cleared:

```sh
DATABASE_URL=mysql://root@127.0.0.1:13316/hardening_test \
REDIS_URL=redis://127.0.0.1:16386 \
SECONDS_REFUND_TEST_DATABASE_URL=mysql://root@127.0.0.1:13316/hardening_commission_test \
SECONDS_REFUND_TEST_REDIS_URL=redis://127.0.0.1:16386/11 \
cargo test --test seconds_contract_routes -- --nocapture --test-threads=1
```

After verification, only the task-owned MySQL (13316, exact isolated socket)
and Redis (16386) were shut down; their recorded processes no longer exist.
The pre-existing Web preview at `http://127.0.0.1:5178/` returned HTTP200 and
was preserved. A frontend preview is not proof of a migrated live backend.

## Remaining Operational Boundaries

This release does not implement or certify external custody/key management,
KYC/AML providers, external hedging, a cross-product global liquidity pool,
production backup restoration or on-call delivery. It also does not turn the
current pricing model into an institutional matching engine or add new products.
These need separate operational evidence and business requirements.

No production database/API, live money, commit, push or deployment was used.
