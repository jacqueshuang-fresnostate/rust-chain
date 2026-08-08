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
- Standard resource action columns are fixed right and 216px wide.
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
  fixed-column coverage, row-selection width, prop forwarding, and cleanup.
- `DataTable`: local/server pagination, compact/adaptive modes, selection,
  stable row keys, project resize handles, and numeric scroll-width updates.
- `FilterBar`: controlled-value synchronization, draft submit, blank pruning,
  reset, and loading disablement.
- `ConfirmAction`: trimmed reason, draft reset, ordinary/danger semantics.
- `AdminLayout`: active child group, collapse and restore, navigation.
- KYC / Market Feed / Security Policy: tabpanel semantics and existing API
  payload behavior.

## Browser Assertions

At 1728px:

```text
document horizontal overflow = 0
every named asset-table leaf column has one project resize handle
Semi native .react-resizable-handle count = 0
Pointer and keyboard resizing update the column and numeric scroll width
resource fixed action column = 216px
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
