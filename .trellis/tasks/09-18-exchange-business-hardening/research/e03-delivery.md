# E03 Explicit Stop-Limit Activation

## Accepted Scope And Plan

Explicitly accepted after the E07 delivery; migration 0132 is reserved and
approved by the main session. E02 spot journal integration is completed and
verified before changing the stop-limit implementation.

- [x] Read PRD, spot/quality/database contracts and consult codegraph.
- [x] Add nullable explicit direction and durable activation timestamp without backfill.
- [x] Preserve legacy comparisons and legacy request fingerprint bytes.
- [x] Persist activation independently of later settlement failures; re-lock before fill.
- [x] Carry intent and readback through API, existing PC stop-limit UI and Admin.
- [x] Verify pure contracts, migration, durable activation, replay, cancellation and E07 regression.
- [x] Record changed paths and precise checks; main owns PROGRESS.

## Compatibility Decisions

- NULL direction means the historical conjunction: buy <= trigger and limit;
  sell >= trigger and limit. Do not flip or activate those rows retroactively.
- New stop-limit requests require rising or falling. Runtime activation is not
  client-controlled and is not part of request identity.
- Old request fingerprints remain byte-for-byte unchanged when direction is
  absent; exact existing requests replay before new-order validation.
- Explicit activation is durable even when the limit is not yet satisfied or
  inventory settlement fails; terminal/cancelled orders cannot reactivate.
- Do not change trading fees, risk policy, market provenance or permission grants.

## Delivered Behavior

- Domain enum and API DTO accept explicit `rising`/`falling` only; new
  stop-limit commands require direction independently of side. Direction and
  activation are carried through both repository implementations, row locking,
  SQL candidates, Admin/user read models and private events.
- The database activation timestamp is committed before later fill work and
  survives unmet limits, inventory failures, reconnection and recrossing.
  A second lock checks cancellation/terminal state before any fill.
- Fresh-cache activation at creation is retained when the limit is not yet
  executable; when both conditions hold, immediate settlement retains the
  timestamp and the existing atomic/replay behavior.
- PC requires an explicit selection and includes it in retry identity; order
  history and cancellation intent display it. Admin exposes trigger price,
  direction and activation time, explicitly identifying legacy rows. Shared
  labels cover these fields and stop-limit/rising/falling enum values in
  details and audit, without translating wire values or free text.
- Mobile preserves its existing market/limit creation scope and gains strict
  Decimal/direction/timestamp readback for stop-limit orders created elsewhere.

## Changed Paths

These are E03 paths/hunks, not ownership claims over other contributors'
changes already present in shared frontend files.

- `migrations/0132_spot_explicit_trigger_direction.sql` (approved reserved version).
- `src/modules/spot/{domain,mod,presentation,repository,service}.rs`.
- `src/modules/spot/application/{idempotency,order_creation,triggering}.rs`.
- `src/modules/spot/infrastructure.rs` and
  `src/modules/spot/infrastructure/{common,market_prices,order_repository,read_models}.rs`.
- `tests/spot/explicit_trigger.rs`, `tests/spot_routes.rs`,
  `tests/spot_domain.rs`, `tests/wallet_spot_services.rs`,
  `tests/wallet_spot_sqlx_repositories.rs`,
  `tests/unit_src/src_modules_spot_{service,routes}_tests.rs`.
- `pc/src/api/{backendAdapters,exchange}.ts`,
  `pc/src/components/trade/{OrderForm,OrderHistory}.vue`,
  `pc/src/i18n/index.ts`, `pc/tests/{backendAdapters,idempotency}.test.ts`.
- After main's explicit layout handoff: responsive classes only in
  `pc/src/views/Trade.vue` and `pc/tests/spot-layout.test.ts`. Main's E04
  provenance bindings, imports and callbacks were preserved unchanged.
- `mobile/src/core/spotTrigger.ts`, `mobile/src/api/trading.ts`,
  `mobile/src/views/OrdersView.vue`, `mobile/src/i18n/messages/{en,zh-CN}.ts`,
  `mobile/tests/spot-trigger.test.ts`.
- `web/src/admin/resources/resourceConfigs{.tsx,.test.tsx}`,
  `web/src/shared/{adminFieldLabels,adminEnumLabels}.ts`,
  `web/src/shared/adminPresentation.test.tsx`.
- `.trellis/spec/backend/spot-orders.md` and this report.
- E07 and E02's separate delivery notes identify the manual settlement/audit
  and journal sidecar changes sharing this worktree.

## Verification

- `cargo check --all-targets` passed after repository/read-model/fixture fields
  were connected; main was notified that E03 compile blockers were removed.
- `cargo test --lib modules::spot -- --nocapture`: 26 passed, including the
  final rerun after shared compilation recovered.
- Architecture/documentation guards: 11 + 1 passed before the final fixture edit.
- Final escalated full real MySQL/Redis `spot_routes` run: **62/62 passed,
  zero failures or skips**, schema `e03_spot_trigger_test`, Redis port 16386.
  All five explicit-trigger tests and E02/E07 passed, including immediate
  execution for all four side/direction combinations. Actual test time 16.58s;
  Cargo preparation was 6m23s, including shared build-lock waiting.
- Earlier full runs exposed two old Admin-list fixture assumptions: unreliable
  `LAST_INSERT_ID` after duplicate system-user insertion, and hardcoded
  `admin:1` absent from the isolated schema. The fixture now reads the unique
  system email's actual ID and creates/cleans a real test Admin with existing
  helpers. No production authentication/permission behavior changed.
  Intermediate concurrent compilation blockers were resolved by their owners
  before the final successful run.
- Admin typecheck/lint and PC/Mobile typechecks passed. E07 component 12/12.
  Targeted Admin trigger-label/resource/cancellation tests passed 3/3. PC/Mobile
  focused spot adapter/trigger/retry tests passed 6/6. Prior Mobile transaction
  history tests passed 17/17. Shared label gate now passes all 21 tests after
  its unrelated margin label was supplied by the other owner.
- Browser, local fixtures only and external HTTPS/WSS blocked: missing direction
  sent no POST; rising submitted explicit string prices/quantity/direction;
  uncertain retry retained the key; changing to falling used a different key.
  Desktop 1728px screenshot `/tmp/e03-pc-desktop.png` was inspected.
- The initial browser run found legacy PC layout clipping/overlap, documented
  in `/tmp/e03-pc-1024.png` and `/tmp/e03-pc-390.png`. Main then explicitly
  handed over layout classes for repair. The center column now uses `min-w-0`,
  a nonshrinking 660px stacked height and bounded flex children; the right
  column has independent desktop vertical scrolling. No redesign or Mobile
  source edits were made for this repair.
- Layout AST regression + existing chart-provider/guest regression: 8/8 passed.
  Combined spot adapter/trigger/retry/layout selection: 8/8 passed. PC typecheck
  and `git diff --check` passed after the layout edit.
- Real local browser checks at 1728x963, 1024x900 and 390x844 each selected a
  direction, entered values and clicked the actual submit button, producing a
  matching mock POST. `elementFromPoint` confirmed button hit targets; buttons
  stayed inside viewport horizontally and document overflow was zero.
  Mock OHLCV rendered real candles: primary canvas red/green pixel counts were
  103126 / 21903 / 16825 respectively (not merely a blank chart container).
  At 390px normal scrolling shows the whole form without history overlap.
  Inspected screenshots:
  `/tmp/e03-layout-1728.png`, `/tmp/e03-layout-1024.png`,
  `/tmp/e03-layout-390-chart.png`, `/tmp/e03-layout-390-form.png`.
- These browser checks blocked external HTTPS/WSS and used local in-memory
  API fixtures, not production or a live authenticated backend. During the
  final narrow check, local Vite hot-reload notifications were suppressed in
  that browser tab to avoid unrelated concurrent file writes resetting the
  fixture session; no application or Vite source/config was changed.
- Global fmt check currently reports other owners' convert/risk/seconds/margin/
  wallet files, not spot; no unrelated formatting was applied.
- Scoped `rustfmt --edition 2024 --check` on spot modules and all changed spot
  integration/domain/repository fixtures passed; `git diff --check` passed.

## Integration Needs And Limits

- Spot production files are stable and handed back. No global navigation,
  permissions, market provenance, risk/wallet policy, index or PROGRESS edits
  were made for E03. No commit, deployment or production operation.
- Apply approved migration 0132 before running the new backend. It adds nullable
  columns/constraints only; do not backfill or rewrite legacy stop-limit intent.
  Real migration tests used MySQL 9.3, not production MySQL 8.4.
- Older clients cannot create new stop-limit orders without explicit direction;
  exact legacy retries remain supported. Roll out API and PC contract together.
- A replay returns the original stored response even after activation/fill;
  current order queries are authoritative for subsequent live state.
- Main owns final all-slice compile, architecture, clippy and frontend release
  gates. The PC outer-layout issue is fixed and browser-verified as above.
- Ego task spaces 13 and 15 were finished. Own Admin/PC Vite processes on
  5187/5188, including the restarted responsive-check server, were stopped.
  Shared MySQL/Redis and isolated test schemas were left available.

Final successful isolated integration command:

```sh
DATABASE_URL=mysql://root@127.0.0.1:13316/e03_spot_trigger_test \
REDIS_URL=redis://127.0.0.1:16386 \
cargo test --test spot_routes -- --test-threads=1
```
