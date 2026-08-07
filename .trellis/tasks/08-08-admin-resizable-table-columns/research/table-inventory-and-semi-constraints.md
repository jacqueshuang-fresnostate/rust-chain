# Admin table inventory and Semi resize constraints

## Production inventory

- `web/src/shared/DataTable.tsx` is the shared path for standard resource pages,
  agent pages, and agent-management pages. There are eleven production
  `DataTable` call sites.
- Nine production tables bypass `DataTable`: two detail-drawer tables, two KYC
  tables, two market-feed tables, two prediction tables, and one SMTP table.
- The current table contract normalizes compact columns, calculates a numeric
  horizontal scroll width, and deliberately forbids Semi's `resizable` prop.

## Semi Design 2.99.2 findings

- Semi's built-in resizable mode requires every resizable column to have a
  width.
- Semi documents that fixed columns require at least one fluid column and that
  `resizable` is not recommended with `scroll.x` because alignment and duplicate
  fixed-column artifacts can occur.
- This project requires both fixed-right action columns and contained
  horizontal scrolling, so enabling Semi's built-in `resizable` prop globally
  would reintroduce a known rendering defect.

## Chosen approach

Create one project-owned `ResizableTable` wrapper around Semi Table.

- Every declared leaf business column receives a numeric initial width and a
  project-owned header-edge drag handle.
- The wrapper controls widths in React state and recalculates numeric `scroll.x`
  from the current column widths, preserving fixed-left/fixed-right behavior.
- The wrapper never passes Semi's `resizable` prop, so it does not create
  `.react-resizable-handle` nodes.
- Pointer dragging is primary; keyboard Left/Right/Home/End adjustment is also
  provided on an accessible separator handle.
- Widths are clamped to a safe minimum and maximum. They live for the mounted
  table instance only; cross-session persistence is outside this request.
- Semi-generated selection/expand utility columns keep their framework-owned
  width. Every named column supplied by the application, including action
  columns and dynamic detail columns, is resizable.

## Coverage strategy

1. Route `DataTable` through `ResizableTable` so all standard resource and agent
   lists inherit the behavior.
2. Replace every remaining direct production `<Table>` with the shared wrapper,
   forwarding custom `components`, row keys, pagination, loading, fixed columns,
   and styles unchanged.
3. Add a source-level guard that allows raw Semi Table only inside the shared
   wrapper.
4. Update existing tests that previously required zero resize handles to
   require the project-owned handles while still forbidding Semi's native ones.

## Final browser verification

- Local Admin Web: `http://127.0.0.1:3032`, API pointed to the production fixture.
- At 1728×1006, the asset table exposed 15 named project handles and zero
  `.react-resizable-handle` nodes. Dragging the asset-ID edge from 160px to
  256px increased the table width from 2456px to 2552px.
- The fixed-right action header remained a single node with a 0px right-edge
  gap before and after the drag; document horizontal overflow remained 0.
- At 1280×800, keyboard ArrowLeft changed the focused fixed action column from
  216px to 200px after horizontal scrolling. The handle remained visible and
  focused, the action header remained aligned, and document overflow stayed 0.
- The direct market-feed table rendered five named handles, no Semi-native
  handle, and no document-level horizontal overflow.
