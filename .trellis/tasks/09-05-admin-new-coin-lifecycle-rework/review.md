# Manual distribution and refund slice — 2026-09-05

## Delivered

- Subscription reserves supply and transfers quote available to frozen only.
- Admin selects a pending manual order, previews exact payment/refund and
  confirms its final allocation with a required reason. Zero fully refunds.
- Settlement, refund, delivery/lock, supply, receipt, event and audit share one
  transaction. Duplicate confirmations and order/quantity mismatches are rejected;
  identical-key replay returns the original receipt, including after listing.
- Pending manual obligations block listing. Legacy orders remain unchanged.
- Additive Admin, Mobile and PC fields show frozen/paid/refunded amounts and
  partial/refunded status. Unlinked free grants are a separate Admin action.
- Immutable migrations 0121/0122 were rehearsed against a disposable MySQL DB.

## Verification evidence

- Rust formatting, all-target/all-feature checking and Clippy `-D warnings` pass.
- Architecture guard: 11 passed. Pure manual settlement: 2 passed.
- Real MySQL `cargo test --test admin_routes admin_new_coin -- --nocapture`:
  14 passed. `cargo test --test new_coin_routes -- --nocapture`: 11 passed.
  Database-backed branches ran; these were not skip-only results.
- Final focused `admin_new_coin_manual_distribution_settles_and_refunds_exactly_once`:
  1 passed after adding a user-scoped failing receipt trigger. Covers full/partial/
  zero, competing same-key confirmations, excess/changed-input/second-key rejection,
  unrelated frozen funds, ledger/supply conservation, listing gates and post-listing
  replay. Injected failure leaves the order, funds, supply and locks unchanged.
- Web: 65 files / 455 tests; production-policy 15 tests; coverage gate, typecheck,
  lint, production build and bundle budget pass.
- Mobile complete `release:gate`: 673 tests plus production/test type checks,
  PWA/Tauri builds, artifact checks, bundle and governance gates pass.
- PC type-check and production build pass. No dedicated PC runtime smoke performed.
- Ego with the real local router and disposable database verified the pending-order
  form, partial payment 10/refund 15 from frozen 25, zero payment/refund 25, required
  reason dialog and cancellation. At 1728px and 1280px there was no horizontal
  overflow. Browser verification submitted no settlement. This was a focused
  action-page check, not the full unrelated Admin visual-page matrix. Existing
  React Router `HydrateFallback` development warning remains unrelated.

## Cleanup and remaining scope

Test-only browser session cleared and task space closed. Both local test servers
stopped, temporary Rust example/session file removed and the exact disposable
database deleted. No production endpoint or live funds were changed; no commit,
push or deployment. Existing Mobile layout edits and the earlier shortcut slice
were preserved.

The task remains in progress: project-center read/configuration hydration,
post-listing purchase administration, planned/actual listing time and manual/worker
unlock consistency are still follow-up work. This review closes only the manual
subscription settlement slice, not the whole lifecycle reorganization.
