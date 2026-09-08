# Frontend Resource Lifecycle Implementation

## Implemented scope

- `web/src/admin/resources/useResourceDetail.ts`: one page-owned abort-controller
  identity for every asynchronous detail loader. A newer request, static detail,
  close, resource/list-context replacement, local paging or unmount invalidates
  the previous owner. Controller identity protects against transports that resolve
  after abort. Only a current failure surfaces a localized toast; rejected
  loaders are consumed rather than escaping Semi button handlers. Existing
  column/detail field metadata merging is preserved.
- `AdminResourcePage.tsx`: records the exact loaded list context and derives
  readiness during render, before a replacement request effect runs. Batch
  helpers/checkbox state expose no stale IDs while a replacement is pending or
  failed; each list request clears selection. Export is gated by that same
  readiness. Selection respects existing per-row eligibility.
- `shared/DataTable.tsx`: optional pagination-change notification only, preserving
  both existing controlled server paging and local slicing; the resource page
  clears selection and detail ownership for local paging as well.
- `actions/shared.tsx` and only `openUserAssets` in `actions/users.tsx`: delegate
  standard and custom detail GETs to the shared loader and pass its AbortSignal.
  `RowActionHelpers.loadDetail` is required so callers cannot silently bypass
  page ownership. Existing loan/new-coin test helpers have the new operation.
- `actions/agents.tsx`: pins target IDs/status at dialog opening, permanently
  invalidates that confirmation if the selected scope changes, and requires
  explicit cancel/reopen rather than silently replacing targets. Final submit
  checks scope, nonempty/bounded IDs and a synchronous ref lock. Cancel, Escape,
  trigger changes and reason editing are blocked while POST is pending. Failure
  retains the original scope and reason for explicit retry. Existing batch
  partial-success summary, endpoint and payload shape remain intact.

No changes to shared ConfirmAction, permissions, raw filter values, financial
DTOs, live browser/API/credentials, database, specs, PROGRESS or commits here.
Root owns financial validators and asset precision in shared files.

## Regression evidence

Real React/Semi components are used; only list/HTTP boundaries and toast output
are mocked. New focused suites:

- `AdminResourcePage.lifecycle.test.tsx`: 19 cases for inverted detail responses,
  close/static replacement, reload/resource/unmount cancellation, obsolete vs
  current errors, custom user-assets ownership; pending and failed page,
  page-size, filter, toolbar and reload selection/CSV state; local paging; and
  an already-open real commission confirmation spanning reload.
- `actions/agents.test.tsx`: 4 cases for pinned/invalidated targets, single-flight
  and pending cancel/reason lock, explicit retry with preserved reason, and
  empty/oversized selection.

Before production edits, the first lifecycle run failed all 14 original cases;
13 exposed the intended stale ownership/selection defects, while page-size had
an additional jsdom animation-event gap subsequently corrected. Batch red run
exposed scope retargeting and pending cancellation; its failure-toast assertion
also needed to respect the existing localized diagnostic format. No timeout was
increased. Native Pagination Select changes commit at its CSS leave callback;
the test dispatches actual animation-end events. Explicit retry advances Semi
Modal's existing 100 ms debounce using fake timers instead of sleeping.

## Verification

- Initial six-suite focused run: existing AdminResourcePage, DataTable, loan and
  new-coin suites all passed (40 cases); new suites reached 22/23 after harness
  corrections. Final page-size-only run passed **1/1** (9.11 s) after dispatching
  React/jsdom's WebKit-prefixed animation-end event as well; all 23 new cases
  therefore have passing focused evidence, with a combined final run left to root.
- `npm --prefix web test -- src/admin/resources/resourceConfigs.test.tsx`:
  **76/76 passed**, 114.45 s. Existing detail GET assertions now require a signal;
  editing/configuration loads and HTTP mutation expectations are unchanged.
- `npm --prefix web run typecheck`: passed on initial implementation.
- `npm --prefix web run lint`: passed on current shared implementation.
- `git diff --check`: passed.
- Independent read-only explorer reviewed detail context/abort lifecycle,
  render-time readiness, local/server paging and pinned batch scope; no concrete
  correctness findings. No edits or extra tests by reviewer.
- Root will run final full Web quality gates after all workers finish. No
  production browser or real API mutation was performed by this worker.
