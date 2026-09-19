# E08 Delivery: Explicit Commission Reversal

Follow-up status (2026-09-19): authorized prospective seconds principal refunds
are now delivered separately in `research/e08-refund-delivery.md`. Its source-first
locking and verification supersede the earlier seconds follow-up proposal below;
the original reversal scope and historical verification remain recorded here.

## Scope And Contract

- `POST /admin/api/v1/agent-commissions/:id/reversal` accepts only
  `{ "idempotency_key": "...", "reason": "..." }`. The authenticated Admin is
  the actor; the exact route requires `agents.commissions.settle`.
- A commission must be `settled`, with exactly one matching positive original
  `agent_commission_payout` wallet ledger entry. Original payout asset and amount
  must agree with the commission snapshot. Missing/ambiguous evidence refuses.
- Debit the original ledger beneficiary, not the agent's current user binding.
  Reverse the entire original amount without recalculating the rate, basis,
  outcome or current asset precision.
- Preserve the original commission source/amount/rate/asset and original payout
  ledger. Append a separate receipt containing original commission JSON,
  payout ledger ID, actual recipient, asset, amount, actor, reason, key and time;
  advance only the commission status to `reversed`. No update/delete receipt API
  is introduced.
- Serialize seconds sources by source order, then commission, then wallet;
  other sources retain commission-then-wallet order. The payout ledger is append-only
  evidence and is read without a range lock. A real concurrency regression caught
  a next-key-lock/wallet-lock deadlock; removing that unnecessary ledger range
  lock fixes it without weakening commission replay or wallet serialization.
- Same commission/actor/key/trimmed reason returns the original immutable receipt,
  including after process/pool restart. Any changed actor/key/reason conflicts.
  Reusing an actor's key for another commission also conflicts.
- Keys are nonempty ASCII letters, digits, `_`, `:` or `-`, maximum 128 bytes.
  Migration 0137 keeps raw key text as UTF-8 and indexes a generated SHA-256
  digest. Receipt replay still compares raw keys, actors and normalized reasons;
  a uniqueness collision is a conflict, never evidence for an unrelated replay.
- Insufficient available balance refuses with no financial writes. Frozen/locked
  buckets are untouched; no negative wallet or synthetic debt is created.
- Wallet debit, wallet ledger, receipt, commission status, Admin audit and
  platform journal commit atomically. Duplicate platform legs fail the entire
  transaction.
- Normal payout writes `platform_commission_expense=+amount` and
  `user_commission_wallet_liability=-amount`; reversal uses exact opposite legs.
  Keys are `agent_commission:{id}:payout` and `agent_commission:{id}:reverse`.
  The shared writer is called, not modified. Every transaction/asset is zero-sum.

## Changed Paths

- `migrations/0135_agent_commission_reversals.sql`
- `migrations/0137_commission_reversal_key_metadata.sql`
- `src/modules/admin/application/agents.rs`
- `src/modules/admin/application/agents/reversal.rs`
- `src/modules/admin/infrastructure/agents.rs`
- `src/modules/admin/infrastructure/agents/reversal.rs`
- `src/modules/admin/presentation/agents.rs`
- `src/modules/admin/service/agents.rs`
- `src/modules/admin/routes/users_agents.rs`
- `tests/admin_routes.rs`: child registration and commission-journal fixture cleanup.
- `tests/admin_routes/commission_reversal.rs`
- `web/src/admin/resources/actions/agents.tsx`
- `web/src/admin/resources/actions/commissionReversal.tsx`
- `web/src/admin/resources/actions/commissionReversal.test.tsx`
- `web/src/shared/ConfirmAction.tsx`
- `web/src/shared/ConfirmAction.test.tsx`
- `web/src/admin/resources/resourceConfigs.test.tsx`: one explicitly requested
  `ResourceConfig` test annotation to fix heterogeneous column `render` typing.

Main/E05 owns exact backend/Web permission mapping, removing the enclosing
resource action gate, and the `reversed` label/filter. The owned row component
now gates legacy status buttons by PATCH separately from the new POST reversal;
settle-only access does not implicitly acquire legacy write permission.
Shared `ConfirmAction.modalWidth` is additive and defaults unchanged; only this
reversal uses `min(480px, calc(100vw - 32px))`.

## Verification

- Real MySQL 9.3, isolated **hardening_commission_test**, loopback port 13316,
  proxy variables cleared, escalation approved. No production database used.
- `cargo test --test admin_routes commission_reversal:: -- --nocapture`: **1/1**
  scenario passed after fixing the demonstrated range-lock deadlock.
  Assertions cover concurrent payouts; concurrent identical reversal/replay;
  independent wallet competitors; restart replay; changed actor/reason/key;
  cross-commission and case-distinct keys; original source snapshots; original
  beneficiary despite agent rebinding; missing payout evidence; exact available,
  frozen and locked balances; settle-only/read-only/no-auth; strict payload;
  journal duplicate rollback on both payout/reversal; audit failure rollback;
  repair metadata; one audit/debit; per-asset and per-account conservation.
  A second execution of the current compiled test binary also passed **1/1**,
  including repeat SQLx migration/checksum validation on the same isolated DB.
- `cargo test --test admin_routes agent_commission -- --nocapture`:
  existing admin commission route/rule/batch regressions **7/7** passed.
- `cargo test --lib agent_commission`: **5/5** passed.
- Web focused actions plus shared confirmation: **3 files, 14/14** passed,
  including actual resource row wrapper with settle-only permission, absence of
  legacy write-only buttons, durable unknown-result retry after remount,
  malformed receipt retention, storage failure, required reason and optional
  shared width/default restoration.
- `npm --prefix web run typecheck` and `lint`: passed, including typecheck after
  the final requested test annotation. The focused spot-column render regression
  passed **1/1** (`resourceConfigs.test.tsx -t 'shows spot order user email'`).
- `cargo test --test backend_architecture --test backend_documentation`:
  **11/11 and 1/1** passed. An earlier attempt hit another agent's unfinished
  test-module declaration; it passed after that file was present.
- Focused `rustfmt --edition 2024 --check` for all owned Rust parents and tests:
  passed. Global `cargo fmt --all -- --check` reported other concurrently edited
  files; those were deliberately not formatted by this agent.
- `git diff --check`: passed.
- Ego Browser local component previews: 1440px desktop and 390px narrow.
  Initial fixed-width clipping was reproduced and repaired. Narrow dialog
  geometry is x=16, width=358, right=374 at viewport 390; failure retains reason.
  Screenshots: `/tmp/e08-commission-desktop.png`,
  `/tmp/e08-commission-narrow-fixed.png`,
  `/tmp/e08-commission-narrow-error-fixed.png`.
  This is component/fixture verification, not an authenticated live API E2E run.

## Source Refund Race Assessment

Code-path review, not an additional real-DB prediction race test:

- `admin/service/agents.rs` permits a prediction commission payout only when its
  source order is `settled`; `open`/`pending_confirmation` waits and `refunded`
  is unpayable.
- `prediction/infrastructure.rs::settle_market` locks the market and returns
  without changes for already `settled`/`refunded` markets. It only processes
  `open` orders. Invalid refunds reject pending commission rows and mark orders
  refunded in that same transaction.
- Consequently, current automatic paths do not convert an already-settled,
  payable prediction source into an invalid refund. While an invalid refund
  waits for a commission lock, the payout reader can only see the earlier open
  source and refuses; after refund commit the commission is rejected. Successful
  market settlement and invalid market refund serialize on the market lock.
- This does **not** implement arbitrary post-settlement source cancellation.
  Any future such operation must coordinate source refund and paid-commission
  reversal under one explicit, tested policy/transaction.

## Remaining Policy Boundaries

The seconds proposal in this original slice was subsequently authorized and
implemented as default-disabled, prospective-only policy. See the follow-up
delivery for current contracts, tests and remaining legacy exclusions.

- No refund deadlines, eligibility threshold, retroactive basis changes,
  collection debt, automatic negative balances or automatic clawbacks were
  invented. Missing/insufficient evidence and insufficient available funds
  remain actionable refusals.
- Legacy payments with unique matching wallet evidence can be reversed, but
  historical platform payouts are not fabricated/backfilled. A legacy reversal
  is a new balanced entry, not proof that historical journal coverage is complete.
- Seconds-contract `manual_review` without authoritative event-time evidence
  still cannot settle or refund through this delivery.
- Bounded follow-up proposal only: add an explicitly disabled-by-default,
  optional per-product principal-refund policy with a versioned/audited admin
  configuration. Business must choose eligibility, any waiting period and the
  treatment of existing contracts; no active default deadline should be guessed.
- A later dedicated cancellation command would require authenticated actor,
  reason and stable key; lock the order to exclude evidence-based settlement,
  persist a separate cancellation/refund receipt, return only original debited
  principal (never a fabricated win/profit), reject pending commissions, and
  atomically write wallet, platform journal and audit. Current opening debits
  available principal, so this is a credit refund, not a frozen-bucket release.
  Unexpected already-paid commissions require explicit handling and cannot be
  silently skipped. Worker scans must recognize the terminal cancellation state.
- That follow-up needs product-policy, legacy-policy, refund/settlement race,
  commission refusal and idempotency tests before activation; it was not built here.
- Production MySQL 8.4, full all-product DB regression, complete Web release gates,
  and authenticated live-backend browser E2E are not certified by this slice.
  Apply both 0135 and 0137 in order; 0135 was not edited after its first local
  application. No commits, deployment, PROGRESS or global-index edits by this agent.
