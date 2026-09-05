# Project center and five workspaces — 2026-09-05

## Delivered scope

- Five menu categories replace the old seven siblings; lifecycle actions belong
  to the exact-ID project center. Old action/project filters and lock/unlock URLs
  remain compatible, including projects outside the first 100 reference options.
- Project detail exposes full authoritative configuration, supply, order/pending
  counts, issuance editability, next stage and listing blockers. List/count use
  the same symbol/stage/status filters.
- Active, unused preheat projects can edit price/supply with required reason,
  original-value guards, exact asset precision and atomic audit/event records.
- Unlock, fee and post-listing-purchase forms hydrate current values, preserve
  timestamp milliseconds, submit only category fields and use an original
  configuration snapshot. Conflict drafts survive and require explicit reload.
- Subscription rows own final allocation/refund. User/order IDs are fixed;
  active distribution is rechecked on open. Full/partial/zero semantics and
  same-key retry remain intact. Unlinked grants stay in a separate project tab.
- Per-resource tab/menu guards include unlock-only roles; no unauthorized lazy
  references are fetched. Record deep links accept only supported filters.
- Fixed a discovered Semi Space flattening issue with stable stateful action
  keys, and made ConfirmAction retain failed dialog/reason without an unhandled
  Modal promise. This is covered by executable rejection/conflict regressions.

## Verification

- Web final: 67 files, 474/474 tests; production-policy 15/15; coverage gate
  23 tests; typecheck, lint, production build and bundle budget pass.
- Real disposable MySQL: Admin new-coin 15/15, user new-coin 11/11, not skipped.
  Final exact project-center test 1/1 adds read-only GET success/PATCH 403 and
  quote-precision rejection, alongside editability, stale snapshot, audit and
  next-stage assertions. Manual settlement test checks pending summary/listing
  blockers for all three allocation outcomes.
- Final Rust format, all-target/all-feature check, architecture and Clippy results
  are also recorded in PROGRESS after completion of the final commands.
- Ego used the real loopback router with the disposable database. Verified current
  issuance values, fixed timestamp 2026-12-01T18:22:33.25, partial 4 => debit 10 /
  refund 15 from frozen 25, zero => refund 25, empty reason disabled, cancellation,
  pending listing blocker, legacy project redirect and preserved asset filter
  after switching to unlock records.
- Visual matrix inspected at 1728px: login, Dashboard, populated subscriptions,
  empty purchases, KYC, Security Policy and the fully opened 920px SideSheet.
  Empty purchases and project center inspected at 1280px. Checked document
  overflow was zero. No browser settlement/configuration write was confirmed.
  The existing React Router HydrateFallback development warning remains.
- Trellis context validation 14+14 and git diff whitespace checks pass.

## Cleanup and boundaries

Disposable browser session cleared, task space 5 closed; both loopback preview
servers stopped. Temporary Rust example/session file and the exact database
`codex_newcoin_center_20260905_2230` removed. Screenshots/logs remain only as local
verification evidence. No commit/push/deployment or actual project/fund operation.
Previous manual settlement and preheat-shortcut changes plus the user's Mobile
max-width removal are preserved.

This completes the page/operations slice, not the entire lifecycle task:
planned/actual listing and manual/automatic unlock fee consistency are still
pending. Project aggregate refund amounts and directly joined refund values on
receipt tables are not introduced: actual refunds remain on subscription rows.
Locks/unlocks retain existing read-only record APIs; no new Admin release or
pause/cancellation behavior is implied. Task remains in_progress.
