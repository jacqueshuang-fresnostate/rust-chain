# Prototype Parity Contract

## Source Of Truth

The v16 public prototype and the checked-in source are the same design target:

- Root structure: `mobile/sites-prototype/app/page.tsx`
- Secondary structure: `mobile/sites-prototype/app/secondary-pages.tsx`
- Geometry and visual states: `mobile/sites-prototype/app/globals.css`
- Route semantics: `mobile/sites-prototype/app/prototype-routes.ts`

The Vue app must port these structures instead of approximating screenshots.

## 390x844 Root Baseline

### Shared Shell

- Mobile canvas: full viewport on phone, maximum width `430px`.
- Sticky root header: `66px`.
- Root navigation: `84px` plus bottom safe area.
- Seven equal navigation destinations remain visible.
- Raised Seconds control: `48px`.
- Header and secondary header remain above route transitions and scrolling content.
- Light theme uses the prototype cool-neutral page/surface family and must not use
  the retired `#0b1811` border family.

### Home

- Header, search/scan row, portfolio, funding actions, 4x2 shortcuts, coral report,
  home markets, and benefits appear in that exact order.
- Portfolio values or placeholders occupy the same boxes.
- API failure belongs inside the market list body and must not move the column header.

### Markets

- `MARKET PULSE` intro and the two-line Chinese headline replace the current generic
  market overview dashboard.
- Search, five category rail, market temperature, table header, and rows are the only
  first-level sections.
- Each row contains favorite, asset orbit, symbol/volume, sparkline, and price/change.

### Spot And Contract

- Both use the prototype `trade-heading`, `trade-quote`, `chart-panel`, and
  `trade-console` structure.
- Spot and contract remain separate routes/surfaces.
- Contract settings precede direction; spot has no contract settings block.
- Existing API-backed balances, order book, chart data, margin products, leverage,
  and submit handlers replace demo values without changing geometry.

### Assets

- Intro, bright asset hero, four actions, allocation, holdings, and accounts remain
  in the prototype order.
- Guest or loading state uses same-size placeholders and entry actions, not a
  different full-page layout.

### Profile

- Member view keeps identity, level, metrics, account matrix, and logout.
- Guest view follows prototype guest identity and dual authentication actions.
- Existing profile/KYC API values replace demo copy when available.

### Seconds

- Seconds is a protected secondary route, not a root page with bottom navigation.
- It contains secondary header, market board, pair selector, direction, duration,
  amount, quick amounts, order summary, confirmation, and records.
- Unauthenticated action routes to login while the workbench geometry remains stable.

## Priority Secondary Pages

- Message Center: summary, equal-width category row, unread tools, grouped timeline.
- Loan: overview, two-column product comparison, cost estimate, lifecycle/orders.
- Security: score, priority checklist, TOTP/password/fund protection, device sessions.
- All secondary pages share the prototype PageShell, field, action, status, and
  confirmation-dialog classes.

## Integration Boundaries

- Do not copy demo mutations from the prototype.
- Keep existing Vue API and store methods as the data/side-effect layer.
- Use prototype fallback values only as non-authoritative visual skeletons. Label
  deterministic demo-only behavior where the backend has no operation.
- Do not read or write browser storage during visual tests. Change language/theme
  through visible UI or deterministic test setup.

## Verification

1. Capture public and local screenshots at identical viewport, theme, language,
   route, scroll position, and auth state.
2. Compare bounding rectangles for shell, header, major sections, and navigation.
3. Inspect browser console errors and horizontal overflow.
4. Repeat at 320 and 448 widths.
5. Run the mobile type-check, test, PWA build, Tauri build, and Android debug build.
