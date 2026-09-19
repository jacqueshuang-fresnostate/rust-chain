# E07 Manual Spot Fill Delivery

## Delivered Contract

- `POST /admin/api/v1/spot/fills` passes the authenticated `AdminAuth` subject ID to the application; actor IDs are never accepted from request JSON.
- Manual fills require a trimmed nonblank `reason` (at most 512 characters) and a trimmed nonblank request key (at most 255 characters).
- Reuses `insert_admin_audit_log_entry_in_tx`: `action=spot.fill`, `target_type=spot_trade`, target is the committed trade ID. Before/after snapshots contain both orders; after also contains the trade and exact idempotency key. Existing helper retains request ID/IP context.
- Audit INSERT is inside the same existing order/trade/wallet/ledger/commission transaction, after financial writes and before commit. Audit failure rolls back everything.
- Exact replay checks orders/price/quantity plus original actor, normalized reason and exact key. Both ordinary replay and duplicate-key-race replay perform this check. No second audit, financial effect or event.
- Historical/automatic trades without a manual audit cannot be adopted by manual replay. Missing audit or changed actor/reason returns conflict. Automatic matching and its settlement paths are unchanged in E07.
- Existing spot-order resource gains a dedicated permission-gated action using existing `spot.orders.read` and `spot.orders.write`; no permission grants, global navigation or new routes.
- Preview performs only the existing two order-detail GETs; validates response identities, sides and pair, displays both snapshots and exact Decimal strings, and invalidates on edits/close/late response. Confirmation freezes the command, requires reason and shows both IDs/price/quantity.
- Uses the existing recoverable financial-command intent store, scoped to session generation/actor/buyer/pair and full canonical intent. Unknown outcomes preserve the key across remounts; filled-order snapshots do not block an exact uncertain replay. Unavailable storage and malformed success responses fail closed.

## Changed Paths

- `src/modules/spot/routes.rs`
- `src/modules/spot/presentation.rs` (manual-fill DTO only for E07)
- `src/modules/spot/application/settlement.rs`
- `src/modules/spot/infrastructure.rs`
- `src/modules/spot/infrastructure/trade_settlement.rs`
- `tests/unit_src/src_modules_spot_application_tests.rs`
- `tests/spot_routes.rs` (manual-fill fixtures now have real administrators and reasons; isolated actor cleanup)
- `tests/spot/manual_fill.rs`
- `web/src/admin/resources/actions/spotFill.tsx`
- `web/src/admin/resources/actions/spotFill.test.tsx`
- `web/src/admin/resources/resourceConfigs.tsx` (lazy action attachment only)
- This report.

## Verification

- `cargo test --lib manual_spot_fill -- --nocapture`: 2/2 passed (reason/key bounds and actor-zero rejection before database access).
- Real MySQL 9.3 on the provided disposable port 13316, separate `e07_manual_fill_test` schema, Redis port 16386 available. Escalated execution, not skipped:

  ```sh
  DATABASE_URL=mysql://root@127.0.0.1:13316/e07_manual_fill_test \
  REDIS_URL=redis://127.0.0.1:16386 \
  cargo test --test spot_routes -- \
    spot_fill_settles spot_fill_is_idempotent spot_fill_replays \
    spot_fill_allows spot_fill_rejects_user spot_fill_releases \
    spot_fill_rejects_price spot_fill_idempotency spot_fill_concurrent \
    manual_fill_actor --nocapture
  ```

  11/11 passed. New real-DB case covers unauthenticated/user-scope rejection, missing/blank/overlong reason, audit failure injection via a temporary actor-scoped trigger, complete rollback snapshot, ten concurrent exact replays, audit contents/count, changed actor/reason/price/quantity conflicts and refusal to invent missing historical audit.
- Initial DB run caught MySQL JSON extraction being exposed as `LONGBLOB`. Fixed by reading native JSON and comparing its key in Rust, without schema changes or text casts; all 11 then passed.
- `npm --prefix web run test -- src/admin/resources/actions/spotFill.test.tsx`: 12/12 passed, including readonly preview, permission gates, reason validation, uncertain replay/remount, terminal-order replay, stale preview, malformed DTO/result, single-flight, storage failure and changed intent.
- `npm --prefix web run typecheck`: passed after unrelated concurrent work stabilized.
- `npm --prefix web run lint`: passed.
- `git diff --check`: passed at E07 check.
- `cargo test --test backend_architecture --test backend_documentation`: 11 architecture and 1 documentation test passed after the shared build lock cleared.
- Ego Browser used the actual local resource page with explicitly mocked API responses: 1440x1000 two-column preview; 390x844 stacked preview and confirmation; document width 390 and drawer width/scrollWidth 342; no horizontal overflow. Recorded only two order GETs before confirmation, one fill POST after reason, then authoritative list reload. This is not a live backend/browser E2E test.
- Screenshots: `/private/tmp/e07-fill-desktop.png`, `/private/tmp/e07-fill-narrow.png`, `/private/tmp/e07-fill-confirm-narrow.png`.
- Repository-wide `cargo fmt --all -- --check` encountered formatting in other in-progress slices (financial retries, loans, journals). E07-owned Rust files were formatted individually. The E07 DB build also reported an unrelated unused wallet journal re-export while main was implementing that work.

## Integration Needs / Limits

- E07 has no migration. Production MySQL 8.4 was not tested; test server is 9.3.
- Main session owns overall PROGRESS/acceptance and final all-slice architecture, clippy, full Web tests/coverage/build/budget gates. No PROGRESS edit, commit, push, deployment or production operation was performed.
- Existing old clients/scripts calling manual fills must now send a reason. Historic no-audit manual trades fail closed on replay; no fabricated actor or retroactive audit backfill.
- The existing audit-log endpoint can inspect `spot.fill` records. Dedicated audit localization/navigation was deliberately not expanded.
- The preview is an informational read snapshot, not a reservation or execution guarantee; the existing transaction checks remain authoritative.
- Test schema was kept for repeatable integration checks; shared test services were not shut down.
- Subsequent E03 work was explicitly accepted by the main session after E07 stabilization. Its stop-limit changes must be reviewed separately from the manual-fill sidecar.
- Final integrated E03/E02/E07 regression: escalated full `spot_routes` suite
  passed 62/62 without skips using isolated `e03_spot_trigger_test` and local
  Redis, including actor/reason audit rollback and concurrent manual replay.
  E07 component tests were rerun successfully (12/12); no E07 permission grants
  or global navigation expansion was introduced by integration.
