# Admin UI System Contract

## Scope

This contract applies to the operations console under `web/src/`. It covers
visual and interaction structure only. UI refactors must not change API paths,
request payloads, authorization scopes, pagination semantics, export behavior,
or destructive-action behavior.

## Brand and Shell

- Reuse tracked HIPPO assets from `web/src/assets/brand/`; do not restore the
  `RC / Rust Chain` placeholder brand.
- User-facing admin decoration is Chinese-only. Keep the HIPPO brand name and
  necessary business abbreviations such as KYC/API/PC, but do not render
  `HIPPO OPERATIONS`, `OPERATIONS`, English environment/security badges, or
  duplicate English section kickers above an existing Chinese heading.
- `AdminLayout` owns the dark graphite navigation rail, sticky operations
  header, current navigation domain/page, production badge, administrator
  identity, and logout action.
- `PageHeader` owns the page-level title, optional description, actions, and
  document title. Page code must not create a competing header layer.
- The content surface must have `min-width: 0`; document-level horizontal
  overflow is forbidden.
- At `max-width: 1400px`, the expanded sidebar is 208px. The collapsed sidebar
  remains 72px. At `max-width: 840px`, use the narrow layout defined by the
  shared stylesheet instead of adding page-local width overrides.

## Resource Page Composition

All standard admin list pages use this order:

```text
PageHeader
AdminResourcePage Card
  data operation section
  optional filter section
  table state or DataTable
DetailDrawer
```

- Keep create, batch, CSV, and refresh actions in the data operation section.
- Keep filters in `FilterBar`; do not build page-specific unlabeled filter rows.
- Filter labels remain visible. Placeholder text is supplementary, not the
  only field name.
- `FilterBar` keeps edits in a local draft until submit, synchronizes when the
  controlled `value` changes, prunes blank values on submit, and immediately
  emits `{}` on reset.
- While `loading` is true, all filter controls, submit, and reset are disabled.
- Empty, loading, and error states use `admin-table-state` and preserve a
  deliberate minimum content area.
- The density button describes the target state: `切换到自适应` or
  `切换到紧凑`.

## Semi Table Contract

### Column and Scroll Rules

Compact admin tables normalize columns without a numeric width to 160px.
Fixed or explicitly sized columns retain their configured width.

```ts
const tableColumns = normalizeTableColumns(columns, 'compact');
const scroll = containedTableScrollForColumns(
  tableColumns,
  rowSelection ? 48 : 0,
);
```

- Use a numeric `scroll.x` equal to the sum of normalized column widths plus
  the optional row-selection column.
- Raw Semi Table rendering is allowed only inside `shared/ResizableTable`.
  Never enable Semi Table's native `resizable` prop: its combination with
  horizontal scrolling causes fixed-column alignment and duplicate-column
  artifacts.
- Every application-declared leaf column uses the project-owned resize handle,
  including fixed action columns and dynamic columns. Numeric declared widths
  are the initial values; missing widths use the shared 160px default.
- `ResizableTable` owns clamped width state for its mounted instance, provides
  Pointer dragging plus separator keyboard controls, and recomputes numeric
  `scroll.x` from all current leaf widths plus visible row-selection and
  dedicated expand utility columns. Semi defaults `hideExpandedColumn` to
  `true`; only `false` creates the extra 48px expand column.
- Column identity prefers a unique `key`, then a unique `dataIndex`. Duplicate
  identities and anonymous leaves use their tree path so one leaf can never
  overwrite another leaf's width. Unchanged declarations keep mounted widths;
  added/replaced declarations start from their own configured width.
- The project handle must expose `role="separator"`, vertical orientation, a
  Chinese accessible name, current/min/max values, and Left/Right/Home/End
  behavior. It must stop header sorting/filtering activation and clean up
  document listeners and the body drag state after pointer up, cancel, or
  unmount.
- Columns with `key: 'actions'` are action-button columns. They receive the
  shared action-column class and a 120px resize minimum; ordinary data columns
  retain the 80px minimum. A business field titled `操作` is not an action-button
  column unless it uses that key.
- Action-column Semi buttons set both `height` and `min-height` to 24px so the
  shell-wide 36px minimum cannot win, use 8px horizontal padding, and keep
  nested button groups and text on one line even when a caller requests wrap.
- Standard resource action columns are fixed right and 288px wide, and their
  button groups stay on one line.
- Keep the fixed-right separator and shadow visible so the action column does
  not appear inserted between business columns.
- Compact and adaptive `DataTable` modes both route through `ResizableTable`;
  adaptive declarations may remain fluid, but the wrapper assigns their
  controlled initial width and numeric horizontal scroll width.
- Continue supplying a stable `rowKey`; pagination and row selection depend on
  it.

### Wrong vs Correct

```tsx
// Wrong: Semi native resizing conflicts with fixed columns and scroll.x.
<Table columns={columns} fixed resizable scroll={{ x: 'max-content' }} />

// Correct: the project wrapper owns widths, handles, and numeric scroll.x.
<ResizableTable columns={columns} rowSelection={rowSelection} />
```

## Forms, SideSheets, and Confirmation

- Shared admin forms use the global adaptive grid. A 720px medium SideSheet
  resolves to two columns; page-local three-column overrides are forbidden.
- Create and edit actions for the same resource reuse one form component;
  endpoint-only controls such as an initial status are explicit parameters,
  not separate page implementations that can drift in layout or validation.
- Asset create/edit forms expose `margin_transfer_enabled` as the Chinese switch
  `允许转入杠杆账户`; create defaults it off and edit initializes it from the
  returned record. The assets table shows the same boolean as `允许转入杠杆`,
  and every create/update payload sends the explicit value so an unchecked
  control can never be confused with an omitted legacy field.
- When a simplified edit form exposes only the primary item from an existing
  structured payload, preserve every unexposed legacy item unchanged in the
  update request. Never rebuild the payload from visible controls in a way
  that silently deletes hidden translations or migration metadata.
- Detail SideSheets use a bounded width rather than `80%` of a large desktop
  viewport.
- Reference-driven full-page editor SideSheets may use `100vw` when their
  composition combines media, metadata, summary, and a full-width document
  editor. Scope the width override to that resource, keep the header stable,
  make media fill its grid cell, collapse the primary grid by 1100px and the
  field grid by 840px, and verify zero horizontal overflow at each breakpoint.
- Keep the SideSheet header stable and let the body own scrolling.
- Ordinary save/submit actions use the primary HIPPO orange treatment.
  Destructive actions use danger styling.
- `ConfirmAction` requires a non-blank reason, trims it before submission,
  clears the draft after cancel/success, and must not close while submitting.
- Explicit `dangerous` overrides are allowed. Otherwise the shared irreversible
  action matcher determines danger styling.
- Modal cancel and confirm controls need stable Chinese accessible names.

## Tabs and Stateful Workbenches

- A visible tab must own exactly one visible `role="tabpanel"` with a matching
  `aria-labelledby`.
- When form sections are rendered outside Semi `Tabs`, keep one shared React
  state model so switching tabs does not discard edits.
- Security Policy displays one section at a time and keeps the save bar
  outside the tab panel so the action remains predictable.
- KYC and Market Feed workbenches preserve their existing API payloads while
  exposing the active panel semantically.
- Repeated row controls need record-specific accessible names.

## Shared Settings Editors and Audit Explorer

- Singleton settings pages use the shared settings editor shell and hooks rather
  than page-local `useEffect` request state. Query keys include the canonical
  API resource, mutations invalidate that resource after success, and retries
  never replay a write mutation automatically.
- The editor shell owns loading, retryable load failure, save failure, success,
  conflict, and dirty state. HTTP 409 is presented as a Chinese concurrent-edit
  conflict with an explicit latest-data reload action; it must never silently
  overwrite the administrator's draft.
- A dirty editor guards both React Router navigation and browser
  refresh/close. The route confirmation is Chinese and offers continue editing
  or discard-and-leave; `beforeunload` is installed only while dirty and is
  removed after save, reset, discard, or unmount.
- Save confirmation renders field-level changes with Chinese labels and
  human-readable values, a concise impact summary, and a required trimmed
  reason. Ordinary saves use the primary semantic treatment; high-risk changes
  use the danger semantic treatment and state their runtime impact explicitly.
- Secret inputs are write-only. Read responses and editor hydration expose only
  masks, `*_set` flags, last validation/rotation metadata, or equivalent safe
  status. Never place a stored secret, password, token, ciphertext, or a
  placeholder pretending to be that secret back into the input value.
- The audit explorer consumes `GET /admin/api/v1/audit-logs`. Its optional
  `created_from` and `created_to` query parameters are inclusive Unix
  millisecond bounds (including database microseconds within the selected end
  millisecond); the backend rejects an inverted range. The UI translates
  known action, target, and field codes to Chinese, shows structured before/after
  differences, and provides a canonical object link where the target type has a
  supported admin route.
- Audit export uses the currently loaded filters and rows, emits a UTF-8 BOM CSV
  with Chinese headers, and runs every serialized field through the same masking
  layer as the visible detail. Export must not bypass pagination silently: its
  label and filename describe that it contains the current result set.
- Unknown audit fields remain visible under a safe fallback label so new
  backend fields are not silently hidden. Recursive values whose key denotes a
  password, secret, token, credential, private key, or ciphertext are masked
  before rendering, even when historical rows predate backend redaction.
  String values under ordinary keys and all exported metadata fields also pass
  through free-text masking so named assignments, quoted JSON credentials, and
  Bearer tokens cannot escape through diagnostics, IDs, or request traces.
- Prediction configuration has one canonical entry at
  `/admin/prediction/settings`: its `assets` tab owns editable wager-asset
  configuration. The legacy `/admin/prediction/assets` URL redirects to
  `/admin/prediction/settings?tab=assets` and must not appear as a second
  sidebar or generic read-only resource page.

## Margin Product Configuration Workflow

### 1. Scope / Trigger

- Applies to create and edit SideSheets for the `marginProducts` resource.
- The workflow follows the operational order `basic -> leverage -> risk -> review`.
  Create and edit reuse one state model and one field component so a product
  cannot be validated or serialized differently between the two actions.

### 2. Signatures

- `支持保证金模式` is an `AdminMultiSelect` over `isolated` (`逐仓`) and
  `cross` (`全仓`).
- `默认保证金模式` is an `AdminSelect` whose options are restricted to the
  currently supported modes.
- Create and update requests send both `margin_mode` and `margin_modes`.
  `margin_mode` is the visible default, and the same value must be the first
  item in the ordered `margin_modes` array.
- The resource table displays separate `默认保证金模式` and
  `支持保证金模式` columns; neither column may substitute for the other.

### 3. Contracts

- Removing the selected default mode immediately falls back to the first
  remaining supported mode. Removing every mode clears the default and blocks
  publication.
- Edit hydration preserves both implemented modes from `margin_modes` and
  reads the default from `margin_mode`. A legacy row without `margin_modes`
  falls back to its `margin_mode`, then to `isolated` only when both fields are
  absent.
- The leverage step combines preset and comma-separated custom levels,
  normalizes equivalent numeric values, removes duplicates, sorts ascending,
  and derives `max_leverage` from the last level.
- The review step summarizes the selected pair and asset labels, supported and
  default modes, leverage levels, risk values, and initial/updated status. It
  states that active publication opens new positions immediately and product
  changes do not rewrite existing positions.
- Tabs remain directly navigable for review, but only one matching
  `role="tabpanel"` is visible. Every panel uses Semi's `semiTab*` /
  `semiTabPanel*` ID relationship. Direct navigation never bypasses final
  validation; the review submit action remains disabled until all prior steps
  pass.

### 4. Validation & Error Matrix

- Missing pair or margin asset -> block next/review submit with a Chinese
  inline workflow error.
- Empty supported modes, or a default outside the supported set -> block.
- Empty leverage set, an empty custom CSV item, a non-decimal level, or a
  level less than or equal to one -> block; never silently discard it.
- `min_margin` missing, non-decimal, or non-positive -> block.
- Non-empty `max_margin` missing decimal syntax, non-positive, or less than
  `min_margin` -> block; an empty value means no upper limit.
- `maintenance_margin_rate` must be a non-negative decimal.
- A non-empty `hourly_interest_rate` must be a non-negative decimal; empty
  means the backend default.

### 5. Good / Base / Bad Cases

- Good: a product supports `[isolated, cross]`, defaults to `cross`, and sends
  `margin_mode: "cross"` plus `margin_modes: ["cross", "isolated"]`.
- Base: a legacy isolated-only product hydrates as supported/default isolated
  and round-trips without introducing cross.
- Bad: filtering edit hydration to isolated or hard-coding
  `margin_modes: ["isolated"]`; editing a cross-capable row would silently
  remove a live business capability.

### 6. Tests Required

- Create coverage selects both modes, changes the default, exercises previous
  and next navigation, reaches the review summary, and asserts the exact
  default-first request body.
- Edit coverage starts with default cross and both supported modes, then
  asserts the same ordered values survive the update request.
- Validation coverage removes the current/default modes, enters invalid custom
  leverage CSV values, inverted margin bounds, and invalid rates; assert the
  Chinese error and disabled navigation/submit state.
- Accessibility coverage asserts one visible tabpanel and matching
  `aria-controls` / `aria-labelledby` IDs. Responsive browser QA asserts no
  document overflow and usable footer controls at desktop and narrow widths.

### 7. Wrong vs Correct

```tsx
// Wrong: hides backend capability and silently downgrades an edited product.
<AdminTextInput readOnly value="逐仓" />
const body = { margin_modes: ['isolated'] };

// Correct: default mode is explicit and owns the first ordered array slot.
<AdminMultiSelect value={values.marginModes} optionList={marginModeOptions} />
<AdminSelect value={values.defaultMarginMode} optionList={supportedOptions} />
const body = {
  margin_mode: values.defaultMarginMode,
  margin_modes: [
    values.defaultMarginMode,
    ...values.marginModes.filter((mode) => mode !== values.defaultMarginMode)
  ]
};
```

## Market Strategy Settings, Versions, Nodes, and Recovery

- Create and edit reuse one market-strategy form and one ordered node editor.
  Each node exposes Chinese labels for target time/type/value, execution mode,
  tolerance, local volatility, and optional paired volume bounds. Use
  `datetime-local` inputs, convert to Unix milliseconds at the API boundary,
  and preserve array order. Add/delete buttons and every repeated field need a
  record-specific Chinese accessible name such as `节点1目标时间` or `删除节点1`.
- The create form loads active trading pairs from the shared admin pair option
  source and renders `交易对ID` as a searchable select whose labels include
  symbol and ID. Only `internal` and `strategy` market types are eligible for a
  synthetic strategy. Render `策略类型` as a select backed by implemented
  strategy types; the current implementation exposes `price_path` as
  `价格路径（OHLCV）`. Do not restore free-text inputs that can submit an
  unsupported pair or inert strategy type. Existing edit records with a legacy
  type may retain that value as an explicitly labelled historical option.
- `/admin/market/strategies` is the single settings entry. It owns list,
  create/edit, presets, OHLCV preview, version history/rollback, node editing,
  status actions, and manual recovery. Do not restore a duplicate navigation
  item or a second `marketStrategyActions` resource config. The legacy
  `/admin/market/strategies/actions` URL may only redirect to the canonical
  route.
- The generator section uses Chinese controls for `行情场景`, `Seed 模式`,
  `固定 Seed`/`当前实际 Seed`, `均值回归强度（0～2）`, `噪声强度（0～5）`,
  `影线强度（0～5）`, and `成交量形态`. Create defaults must match backend
  legacy defaults. Edit initializes every value from detail `generator`, never
  from the list row.
- Presets are loaded once after a create/edit SideSheet opens from
  `GET /admin/api/v1/market-strategies/presets`. An empty or failed response
  must not create a retry render loop; failure stays inline with an explicit
  reload action. Selecting a scenario alone does not mutate price/nodes. The
  administrator must click `应用场景预设`; applying requires valid start price
  and time range, then writes all returned generator fields, target price, and
  relative nodes into ordinary editable form state.
- `生成 OHLCV 预览` is enabled only when the complete form is submittable.
  Create preview omits `strategy_id`; edit preview includes the row strategy
  ID so the backend can use the next version and inherited seed. Display total
  minutes, returned sample count, preview version, actual seed, close-price
  sparkline, and a scroll-contained OHLCV sample grid. State clearly that the
  regenerate-seed preview seed is ephemeral. Preview never submits a reason or
  creates/updates a strategy.
- Every row exposes `版本历史`. The version SideSheet loads newest first,
  marks the active version in text as well as color, and displays Chinese
  scenario/seed mode plus actual seed, effective time, creation time, and
  creator. A non-active version uses `ConfirmAction` with a trimmed reason to
  call the copy-restore endpoint. Never relabel this as direct activation or
  remove/overwrite the old card after success; reload history and the resource
  list instead.
- The strategy list response does not contain nodes. Opening edit must first
  load `GET /admin/api/v1/market-strategies/:id`, sort by `sequence_no`, and
  populate the shared form. Never submit the list row's implicit empty array,
  because that would delete configured nodes. The empty editor must explicitly
  explain that the legacy compatibility endpoint remains in use.
- Keep the row action `检测缺口/补偿K线` small, single-line, and named independently of
  nearby view/edit/status actions. It opens one bounded, body-scrolling
  SideSheet headed `检测缺口与补偿K线`; the sheet must not automatically execute a
  recovery when opened.
- The SideSheet interaction is strictly `detect -> select one gap -> preview ->
  enter reason -> execute`. Show half-open range times, missing 1m count,
  config version, affected aggregate intervals, first/last price, token expiry,
  bounded OHLCV samples, and task history with status, actual/expected 1m
  progress, aggregate count, reason/error, and creation time. A no-gap result
  remains a successful explicit empty state.
- Clear any prior preview when gaps are re-detected or the sheet closes.
  Execute sends only the returned `preview_token` and a trimmed, non-blank
  reason. Disable confirmation without both, lock reason/action controls while
  submitting, set `maskClosable={false}`, and prevent Escape/cancel from
  closing during submission. After success, clear the reason and preview, then
  refresh gaps and history.
- Detect/preview/execute failures remain in the sheet context via the shared
  error/Toast treatment; loading and empty states use `aria-live`, and an
  inline persistent failure state, when present, uses `role="alert"`. Every gap
  preview button includes the range identity in its accessible name. Status
  must not be conveyed by Tag color alone; render the Chinese text label for
  `pending`, `running`, `completed`, or `failed`.
- The wide recovery sheet must fit within the viewport, keep its header stable,
  allow its tables to scroll inside the body, and introduce no document-level
  horizontal overflow. On narrow screens, toolbar and node-editor headings
  stack without hiding focus indicators or reducing action target semantics.
- This UI contract does not duplicate backend recovery semantics: API fields,
  preview-token validity, version/gap conflicts, and the no-Redis/no-WebSocket/
  no-checkpoint boundary are owned by
  [Synthetic Market and K-line Recovery Contracts](../backend/synthetic-market-kline.md).

## Agent And Admin Online Support Workbench

### 1. Scope / Trigger

- Apply when changing `/agent/support`, `/admin/support`, their shared API
  client/workbench, navigation, RBAC, queue pagination, or chat history.

### 2. Signatures

```ts
createStaffSupportApi('agent' | 'admin'): {
  listConversations({ status, unread_only, assigned_agent_id, unassigned, limit, offset })
  getConversation(conversationId)
  getMessages(conversationId, { limit, before_id })
  sendMessage(conversationId, { body, client_message_id })
  markRead(conversationId, messageId)
  setStatus(conversationId, 'open' | 'closed')
}
```

The scope binds both the API prefix and request-client authentication scope.
Pages never concatenate an alternate staff identity or agent ID into a route.

### 3. Contracts

- The agent page explains and presents exact-owner visibility only. It has no
  global/unassigned controls and cannot request another agent's queue.
- The admin route/nav requires `support.conversations.read`; mutation controls
  require `support.conversations.write`. Read-only roles can inspect history
  while reply, read, and status actions remain visibly disabled.
- Queue status and admin-unassigned filters execute server queries. `DataTable`
  uses controlled server pagination: page/page-size map to
  `offset=(page-1)*pageSize` and `limit=pageSize`; changing a server filter
  resets to page 1. Never page only the first fixed 100 rows against a larger
  backend `total`.
- Any keyword filter that is not implemented by the backend must be labelled as
  a current-page filter and must not claim global result coverage.
- Opening a conversation loads its current metadata and newest message page.
  “Load older” uses `next_before_id`, keeps existing messages on failure,
  deduplicates immutable IDs, and preserves chronological order.
- REST reconciliation runs on a bounded interval and ignores stale overlapping
  queue/detail responses. Unmount clears every timer. A failed silent refresh
  keeps cached queue/history visible with an inline retry.
- A failed reply retains one body/client-message-ID attempt. Retry reuses that
  ID; success clears the composer, merges/reloads persisted state, and does not
  fabricate optimistic backend IDs.
- The queue always shows customer identity, current exact agent or `未分配`,
  status, staff unread count, last-message preview, and time. Message sender
  labels distinguish customer, agent, and administrator.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Admin has read but no write permission | Keep workbench readable; disable all mutations with explanatory copy |
| Server reports total beyond current page | Render controlled pagination and request the matching offset |
| Older-message request fails | Keep loaded history and show an older-page retry |
| Silent poll fails with cached content | Keep content and show a non-destructive warning |
| Reply is blank or over 2,000 scalars | Make no request and show Chinese validation |
| Reply request fails | Preserve the immutable attempt for same-ID retry |
| Selected conversation disappears after reassignment/filter change | Clear or reload selection without showing another agent's stale details |

### 5. Good / Base / Bad Cases

- Good: Agent 8 pages to queue page 2, opens one exact-owned conversation,
  loads older history, replies once, and reconciles the committed record.
- Base: an admin opens an unassigned conversation and replies as administrator.
- Bad: an agent UI sends `assigned_agent_id` to choose another owner's queue.
- Bad: a local table shows page buttons backed only by the first 100 rows while
  its summary displays a larger server total.

### 6. Tests Required

- API tests assert scope-specific prefixes/auth scopes, normalized user contact
  fields, exact request bodies, and message/queue pagination parameters.
- Workbench tests cover agent/admin control differences, runtime write
  permission, page-to-offset mapping, filter page reset, current-page keyword
  semantics, older-history merge/retry, unread/status actions, same-ID reply
  retry, stale request suppression, and polling cleanup.
- Route/navigation/access tests prove the admin read permission and agent route
  registration; browser checks cover 1280px, empty/error/long-message states,
  resizable queue columns, no action wrapping, and zero document overflow.

### 7. Wrong vs Correct

```tsx
// Wrong: local pagination can never reach rows after the first fetch.
<DataTable data={firstHundred} />

// Correct: the table controls backend offset/limit.
<DataTable
  data={currentPageRows}
  pagination={{ currentPage, pageSize, total, onPageChange, onPageSizeChange }}
/>
```

Backend ownership, persistence, and exact-agent authorization remain defined by
[Agent-Routed Online Support Contracts](../backend/online-support.md).

## Visual Tokens and Accessibility

- Use the shared warm-white, graphite, HIPPO orange, information blue, success,
  warning, and danger roles in `styles.css`.
- Do not use the bright decorative orange for small body text when contrast is
  insufficient. `--admin-color-primary` and its hover token are the
  contrast-safe action/text roles.
- Keep keyboard focus visible on inputs, selects, buttons, navigation, tabs,
  and switches.
- Loading and empty states use `aria-live`; failures use `role="alert"`.
- Honor `prefers-reduced-motion`.

## Administrator Wallet Recharge Command

### 1. Scope / Trigger

- Trigger: confirming an administrator-initiated user wallet recharge or retrying an uncertain response.

### 2. Contract

- Opening a fresh confirmation creates one client idempotency key and freezes user, asset, normalized amount, and trimmed reason with that key.
- Loading state, timeout, dropped response, and explicit retry reuse the same key. Editing any frozen field creates a new logical confirmation and key.
- The request always submits `idempotency_key`; there is no legacy no-key fallback.
- Same command replay shows the original successful result. HTTP 409 means the key was reused with different parameters and must be shown as a conflict, not retried automatically.
- Success closes the confirmation and refreshes authoritative wallet/user data once. A client never optimistically adds the recharge amount.

### 3. Tests Required

- Resource action tests prove one key per confirmation, reuse after an uncertain response, a new key after intent editing, and 409 conflict presentation.
- The UI tests assert amount is transmitted as a decimal string and reason trimming matches the backend fingerprint contract.

## Required Tests

- `ResizableTable`: a custom handle for every declared leaf, no Semi native
  `.react-resizable-handle`, Pointer and keyboard resizing, min/max bounds,
  fixed-column coverage, `key: 'actions'` classification, row-selection width,
  prop forwarding, and cleanup.
- Standard resource and custom action tables: 24px button height/min-height,
  8px horizontal padding, and computed no-wrap behavior including callers that
  still render a wrapping `Space`.
- `DataTable`: local/server pagination, compact/adaptive modes, selection,
  stable row keys, project resize handles, and numeric scroll-width updates.
- `FilterBar`: controlled-value synchronization, draft submit, blank pruning,
  reset, and loading disablement.
- `ConfirmAction`: trimmed reason, draft reset, ordinary/danger semantics.
- `AdminLayout`: active child group, collapse and restore, navigation.
- KYC / Market Feed / Security Policy: tabpanel semantics and existing API
  payload behavior.
- Market strategy actions: node add/edit/delete and repeated-field names,
  detail-before-edit generator/node preservation, backend preset application,
  generator/seed payloads, create/edit preview context and rendered
  version/seed/OHLCV, immutable history copy-restore with trimmed reason, exact
  millisecond payloads, recovery detect/preview/execute order, submit lock,
  no-gap/error/live states, and status/progress task history.

## Browser Assertions

At 1728px:

```text
document horizontal overflow = 0
every named asset-table leaf column has one project resize handle
Semi native .react-resizable-handle count = 0
Pointer and keyboard resizing update the column and numeric scroll width
resource fixed action column = 288px
fixed action column remains aligned after resizing and horizontal scrolling
medium asset SideSheet = 720px and two form columns
Security Policy visible cards = active tab only
```

At 1280px:

```text
document horizontal overflow = 0
expanded sidebar = 208px
resource filters align as a stable labeled grid
empty state remains deliberate and readable
project resize handles remain keyboard focusable without clipping
```

## Admin Financial Intent, DTO, Permission, Ticker, and Release Contract

### 1. Scope / Trigger

- Trigger: changing an Admin financial command, resource DTO, action control,
  reference selector, live ticker consumer, table, or production bundle policy.

### 2. Signatures

```ts
canonicalRequestIntent(values: Record<string, unknown>): string
runRecoverableFinancialCommand<T>(options: RecoverableFinancialCommandOptions<T>): Promise<T>
apiRequest<T>(path: string, init?: ApiRequestInit): Promise<T>
adminPermissionForRequest(endpoint: string, method: AdminHttpMethod): string | null
subscribeMarketTicker(symbol: string, listener: TickerListener): () => void
useSharedAdminOptionQuery<T>(options): SharedOptionQueryState<T>
```

### 3. Contracts

- Financial command identity includes auth scope, subject, session generation,
  command, user, asset, and canonical intent. Decimal-like fields canonicalize
  as decimal text. The pending/uncertain lease remains in `sessionStorage`
  through timeout, cancellation, response-body loss, and component remount; only
  success or an explicit definitive non-execution failure releases it.
- API requests have one deadline covering fetch and body parsing. Errors are
  typed as timeout, abort, network, HTTP, or contract failures. List DTOs must
  be objects with the declared response key, object rows, non-negative safe
  integer totals, required fields, and decimal text for amount columns.
- Admin read permission is exact per route. Mutation controls resolve the real
  endpoint and HTTP method to exactly one of `write | review | settle | operate`
  and are absent without permission; the backend remains authoritative.
- Shared reference options are cancellable, cached by stable query identity,
  and reused across drawers/pages. Mutations invalidate the owned key rather
  than causing every selector to refetch on mount.
- Ticker consumers share one symbol-normalized connection manager with
  ref-counting, session generation, watchdog, jittered reconnect, and
  `connecting | fresh | stale | offline` diagnostics.
- Every populated table leaf has the project resize handle, pointer/keyboard
  resize, contained horizontal scrolling, and nowrap compact action controls.
  Selected navigation uses an opaque contrast-safe background; a Semi light
  theme background color must not sit behind white selected text.
- Lint, typecheck, all tests, production-policy tests, coverage thresholds,
  production build, and bundle budget all pass before release.

### 4. Validation & Error Matrix

| Condition | Required behavior |
|-----------|-------------------|
| Same financial intent retries after uncertainty | Reuse the exact idempotency key |
| Same key is paired with changed intent | Never send; create a different lease/key |
| Response body drops after server success | Mark uncertain; preserve key for replay |
| DTO row misses a required/decimal field | Raise `ContractError`; do not coerce |
| User has read but lacks mutation action | Render data; omit that action control |
| Reference request unmounts | Abort consumer without committing stale options |
| Last ticker consumer releases | Close socket/watchdog and retain no reconnect timer |
| Selected sidebar item is white on a light inherited background | Release-blocking visual defect |

### 5. Good / Base / Bad Cases

- Good: a recharge times out after the server commits; reload and retry replay
  the same key and display the original result without a second credit.
- Good: nine visible table leaves expose nine resize handles and a pointer drag
  changes the first column while document overflow remains zero.
- Base: a read-only operator can inspect a page but sees no review/settle button.
- Bad: rotate an idempotency key by TTL, coerce `"0.000000000000000001"` through
  `Number`, infer permission from a page label, or open one ticker socket per row.

### 6. Tests Required

- Financial command tests cover canonical decimal identity, timeout, abort,
  body loss, reload recovery, definitive failure, success, and changed intent.
- API contract tests cover required fields, decimal fields, malformed totals,
  error classification, request deadline, and stale-session 401 behavior.
- Access tests cover every route/API mapping and independent action permission.
- Ticker tests cover ref-counting, stale watchdog, backoff, symbol/session change,
  listener isolation, and cleanup.
- Browser checks cover login, selected sidebar contrast, populated table handles,
  real pointer resize, no wrapped action buttons, and 768/1024/1440 overflow.

### 7. Wrong vs Correct

```ts
// Wrong: each retry creates a new command and silently coerces the amount.
await recharge({ amount: Number(amount), idempotency_key: crypto.randomUUID() })

// Correct: canonical intent owns a recoverable key until the outcome is known.
await runRecoverableFinancialCommand({
  scope: financialCommandScopeFromSession(session, 'wallet.recharge', userId, assetId),
  values: { amount: canonicalDecimalText(amount), reason: reason.trim() },
  store: financialCommandIntents,
  request: (key) => recharge({ amount, idempotency_key: key }),
})
```
