# Frontend Resource Lifecycle Audit

Scope: generic Admin resource lists, row-detail requests, shared confirmation/filter/table components, and `api/adminResources.ts`. Read-only production/test audit; no auth/client/settings overlap. Read AGENTS, current PROGRESS, task PRD, Admin index/UI/resource-response contracts. Two selected findings; one optional follow-up. Paths below are repository-relative.

## F1 — High: stale selection remains actionable while a different list request is pending

### Evidence and reproduction

- `web/src/admin/resources/AdminResourcePage.tsx:289-305`: request start only sets loading/error; selection is cleared **after success**, not on request invalidation.
- `AdminResourcePage.tsx:359-374,480-482`: batch helpers derive selected rows from the previous `rows`/keys and continue rendering outside the table loading state.
- `AdminResourcePage.tsx:519-529`: pager changes request parameters immediately; selection remains from the previous page.
- `web/src/admin/resources/resourceConfigs.tsx:850-858`: the affected production batch surface is agent commission settlement/rejection, pending rows only.
- `web/src/admin/resources/actions/agents.tsx:159-179,200-209`: batch buttons only check nonempty IDs; confirm serializes these live IDs without checking list loading/ownership.

Scenario: select commission #1 on page 1, request page 2 (or a new filter/refresh), defer the GET, then click the still-enabled batch settlement/rejection toolbar button. The table is now a spinner and page/filter context has changed, but POST still targets invisible previous-context #1. This is a mistaken operation risk, not a claim that backend pending-state locks are absent.

A read-only Node stdin harness transpiled and executed the **current source** with deterministic React hook/UI stubs, resolved first GET and deferred second GET:

```json
{"selectedBefore":[1],"requestCount":2,"secondRequest":{"limit":50,"offset":50},"loading":true,"batchSelectedIdsWhileLoading":[1]}
```

No test or production file was written for this probe. Existing `AdminResourcePage.test.tsx:798-834` only asserts selection is empty after the second response succeeds, so it misses the entire pending interval.

### Minimal compatible repair

1. In the central resource page, invalidate selection immediately when request ownership changes (page/page-size/filter/toolbar-filter/reload), and ensure batch helpers expose no actionable selected rows while loading/error. Clear selection on failure too. Avoid merely hiding rows while retaining actionable helpers.
2. Batch submit should validate nonempty/in-limit IDs against the current valid selection, and prevent an already-open confirmation from silently changing target IDs on refresh/selection invalidation. Either invalidate/close an unsubmitted dialog or require its pinned target snapshot to still match the valid current selection. Do not replay a write automatically.
3. Keep a synchronous submission lock and prevent cancellation while an actual batch POST is pending; preserve error/reason on failure. Backend payloads, partial-success summary and commission status rules remain unchanged.
4. CSV is also enabled against previous rows while loading (`AdminResourcePage.tsx:484-488`); optionally gate it with the same loaded-context readiness rather than exporting old-filter data under the new filter heading.

Tests: extend `web/src/admin/resources/AdminResourcePage.test.tsx` with deferred GET cases for page, page-size, filter, refresh and rejection; assert no batch IDs during pending/failure, not only after success. Exercise `AgentCommissionBatchActions` through `web/src/admin/resources/resourceConfigs.test.tsx` or a focused new `web/src/admin/resources/actions/agents.test.tsx`: pending GET cannot POST prior IDs; invalidated open confirmation sends no empty/replacement batch; duplicate confirm/cancel while POST pending creates exactly one request; failed POST preserves a retryable dialog/reason.

## F2 — Medium: row detail requests are last-response-wins, not latest-intent-wins

### Evidence and reproduction

- `web/src/admin/resources/actions/shared.tsx:314-319`: `openRecordDetail` awaits an unowned GET, then unconditionally calls `helpers.openDetail`; it displays and rethrows errors to raw Button handlers.
- `web/src/admin/resources/AdminResourcePage.tsx:416-422,537`: opening merely assigns drawer data and closing merely assigns null; neither invalidates outstanding detail requests.
- `web/src/admin/resources/actions/users.tsx:87-94,279-284`: user assets use a parallel custom async detail path with the same defect, and all row buttons stay available while detail GETs load.
- Common helper is used by country, asset, deposit-address, user, news, earn category/product/subscription, loan product/order, margin product/position/liquidation, spot order, market pair/strategy, convert pair/order and seconds product/order actions.

Scenario: click record A detail, then B detail before A resolves; resolve B then A. A replaces the drawer the operator intentionally opened for B. If B is closed before A arrives, A opens a drawer after the operator dismissed it. Same interaction crosses users' detail and assets views. This is stale display/identity confusion, not a demonstrated authorization bypass.

Read-only Node stdin execution of the actual transpiled shared helper with deferred API responses produced:

```json
{"afterB":2,"responseOrder":[2,1],"drawerAfterCloseAndLateA":1}
```

### Minimal compatible repair

Centralize asynchronous detail ownership in `AdminResourcePage` instead of adding per-row independent flags. Add a shared helper operation accepting a detail loader `(signal) => Promise<DetailDrawerData>` (or equivalent). One page-owned generation + AbortController should invalidate on every async request, synchronous detail open, drawer close, resource/request context replacement, and unmount. Only the current generation may set detail or surface errors; stale failures should be quiet. The shared `openRecordDetail` and custom `openUserAssets` should delegate to this owner, passing its signal to `apiRequest`. Ensure errors are consumed at the owner rather than rethrown to fire-and-forget Button handlers. Preserve existing detail field-meta merging and static-row opening.

Tests: extend `web/src/admin/resources/AdminResourcePage.test.tsx` for deferred A/B response inversion, B-close-then-A, list/resource invalidation and unmount, static detail superseding pending network detail, late rejection suppressed and current rejection visibly reported. Update affected action test helper fixtures in `web/src/admin/resources/resourceConfigs.test.tsx` plus focused action suites as needed. Cover the custom user-assets path so it cannot bypass common ownership. Existing detail tests (`AdminResourcePage.test.tsx:508,563,710`) cover immediate details only.

## Optional follow-up — shared ConfirmAction does not revalidate disabled after opening

`web/src/shared/ConfirmAction.tsx:33-42,55,64` checks `disabled` only on the trigger. If parent changes `disabled` from false to true while a confirmation is open, the confirm control and handler remain live. A source-executed hook probe observed `triggerDisabled: true`, `confirmDisabled: false`, `onConfirmCalls: 1`. The handler also lacks a synchronous ref-based single-flight guard. Consider extending `ConfirmAction.test.tsx` with disabled-transition and same-tick repeated invocation regressions; propagate disabled/submitting to final confirmation, freeze reason during submission and guard the handler. Deferred from selected scope because no specific new high-impact async parent transition was traced here; actual Semi Button loading behavior already blocks ordinary post-render repeated clicks.

## Validation limits

These were source-level deterministic probes, not full React/Vitest/browser or backend execution. They made no HTTP calls and wrote no credentials or source/tests. No production/DB changes or commit. Parent owns PROGRESS update and final implementation/verification.
