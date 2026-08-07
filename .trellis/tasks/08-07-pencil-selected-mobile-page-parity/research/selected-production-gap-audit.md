# Pencil selected production gap audit

## Selection evidence

- VS Code Pencil canvas reports `102 Selected`.
- The checked-in `.pen` JSON has 103 top-level nodes, so the selection maps to all saved business artboards except `00 / Design System`.
- Existing production screens already declare sources for the earlier route set. The saved design additions at the end of the document are therefore the remaining implementation slice.

## Saved selected artboards and production mapping

| Pencil source | Production surface | Current gap |
| --- | --- | --- |
| `A9It6g` / `h4gfd` | `NewCoinRecordsView` | Real APIs exist, but the page root omits the new source IDs and list geometry does not use the selected 72px/icon-plate language. |
| `v6phV` / `TuWXq` | `AssetsView` transfer modal | Source IDs are already declared; the modal still uses the generic confirmation sheet rather than the selected transfer route/from-to composition. |
| `UouET` / `FM5tp` | New help route from `ProfileView` | No production route/view. The current Help row incorrectly opens Message Center. |
| `e5Qs1` / `hxe8l` | `OrdersView` empty branches | Empty branches are generic one-line states and do not expose the selected icon plate, description, or trade CTA. |
| `Bcug6` / `IVMAO` | `WalletLedgerView` empty branch | Empty branch is a plain paragraph instead of the selected centered visual state. |
| `t7j6n` / `eSMHf` | `MessageCenterView` empty branch | Empty branch shares the compact data-row state instead of the selected larger empty composition; new source IDs are omitted. |
| `CzpTv` / `ZvGMv` | `PredictionView` quote/confirm state | Real quote and confirm workflow exists; new source IDs and selected market/odds/stake/summary visual hierarchy need parity. |
| `nqP6W` / `aXxul` | `EarnView` subscribe state | Real subscribe workflow exists; new source IDs and selected product/amount/rules/CTA hierarchy need parity. |

## Pencil geometry extracted from `.pen`

- App frame: 390×844.
- Secondary Page Header content uses 20px horizontal padding and circular controls.
- Tab rail: 44px high, 16px horizontal padding, active 2px mint underline.
- Standard data row: 64px or 72px with continuous bottom hairline.
- Empty state: 56px circular icon plate, 12px vertical gaps, 48px top/bottom padding, 300px description width.
- Primary action: full width, 50px height, 4px radius, mint fill.
- Transfer sheet: 390×460 bottom sheet, 20px top radius, drag handle, 12px gaps.
- FAQ/contact row: 64px with 18px Lucide icon and 16px trailing chevron.

## Existing runtime contracts to preserve

- `AssetsView`: `fetchWalletAccounts`, `fetchMarginWallets`, and `transferWalletFunds` remain authoritative.
- `OrdersView`: all spot and margin list/cancel/close API paths remain intact.
- `WalletLedgerView`: paginated `fetchWalletLedger` remains authoritative.
- `MessageCenterView`: only real public news data and local read IDs are shown.
- `PredictionView`: config, market, wallet, quote, confirm, and order APIs remain authoritative.
- `EarnView`: product, wallet, subscription, subscribe, and redeem APIs remain authoritative.
- All modals keep focus trapping, Escape dismissal, body scroll lock, and focus restoration.

## Navigation defect

`ProfileView` labels a row as Help & Support but currently calls:

```ts
router.push({ name: 'message-center' })
```

This is a route-intent mismatch. Add a dedicated `help-support` named route and keep Message Center reachable from its existing Home Bell entry.

## Truthfulness decision

The Pencil sample contains a 24/7 claim and a sample support email. Production must not assert service availability or an address unless configured. The production view may mirror the row geometry while rendering configured channels only; otherwise it renders an explicit unavailable state and keeps the control disabled.
