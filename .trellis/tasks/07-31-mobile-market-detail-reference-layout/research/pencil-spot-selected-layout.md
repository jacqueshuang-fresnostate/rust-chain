# Pencil selected Spot production mapping — 2026-08-03

## Source

- Pencil document: `mobile/pencil/hippo-mobile-uiux.pen`
- Light frame: `yzOPc` (`06 / Spot Trading · Light`)
- Dark frame: `bo8k5` (`07 / Spot Trading · Dark`)
- Reference canvas: `390 × 920`

The checked-in `.pen` JSON and the current 2× exports were inspected directly. The
production page before this correction rendered Root Header → quote hero → 292px
chart → 326px market panel → order form, which does not match the selected frames.

## Required default hierarchy

1. 64px secondary trade header without the HIPPO Root Header.
2. Header content: 44px Back, 24px asset mark, pair + live change, 44px favorite,
   44px share.
3. Main module: 16px side padding, 14px column gap, fluid order form plus a 148px
   compact book.
4. Form geometry: 40px side switch, 40px type selector, 44px price, 44px quantity,
   percentage dots, 44px amount, TP/SL inactive row, available balance, 46px pill.
5. Book geometry: two-column heading, five asks, central live price, five bids,
   buy/sell depth ratio, precision control.
6. Below the module: order/position-and-asset navigation, current-pair filter and
   truthful guest/empty/funded asset state.
7. Local chart is collapsed by default behind a pair-labelled 48px entry; expanding
   it may expose the existing intervals, local dual engines and latest trades.
8. Global five-entry Dock remains visible and owns the safe-area bottom spacing.

## Behavior that must remain production-owned

- Existing REST + WebSocket detail session and race handling.
- Existing spot order request, validation, confirmation and balance refresh.
- Existing pair picker, orders, deposit/assets and authentication routes.
- Local favorites persistence and Web Share / clipboard fallback.
- No fabricated depth, trades, balances or stop-loss order behavior.
- No runtime dependency on `mobile/pencil`.
