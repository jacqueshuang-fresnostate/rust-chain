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

## Market Strategy Settings, Versions, Nodes, and Recovery

- Create and edit reuse one market-strategy form and one ordered node editor.
  Each node exposes Chinese labels for target time/type/value, execution mode,
  tolerance, local volatility, and optional paired volume bounds. Use
  `datetime-local` inputs, convert to Unix milliseconds at the API boundary,
  and preserve array order. Add/delete buttons and every repeated field need a
  record-specific Chinese accessible name such as `节点1目标时间` or `删除节点1`.
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
