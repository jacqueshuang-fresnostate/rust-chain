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

## 2026-08-18 selected contract/margin slice

### Selection evidence

The active Pencil selection contains the following light/dark frames:

| Pencil source | Surface | Production gap |
| --- | --- | --- |
| `by3G9` / `pKHeU` | Contract trade | Main two-column layout exists, but the header omits the API asset mark and opens the generic markets route. Margin mode is hard-coded and leverage mutates immediately instead of opening the selected sheet. |
| `f0L8yf` / `R8t0p` | Leverage sheet | No production sheet. The current control cycles configured levels on every tap. |
| `aNuw6` / `PKAcD` | Margin mode sheet | No production sheet. Mobile API types incorrectly restrict the mode to `isolated` although the backend supports `cross`. |
| `Crw8v` / `YuKtQ` | Contract pair sheet | No production sheet. Real `/margin/products`, ticker logos/prices, favorites and route replacement are available for a truthful implementation. |

### Extracted geometry

- Main frame uses a flat 390×920 white/black canvas. After removing Pencil's mock 28px operating-system status bar, production starts with a 61px sticky Header, a 431px trade module and a 37px position tab rail.
- Trade module padding is `2px 16px 4px`; its columns are exactly `196px + 12px + 150px`. The left form is 425px high and follows `38/36/36/44/40/14/16/15/14/46/46px` visual tracks with 8px vertical gaps.
- The compact book is 372px high: 26px funding header, 13px column labels, six asks, a 38px midpoint, six bids and a 15px bid/ask ratio. The selected frame has no precision dropdown.
- Header controls keep a 44px production hit area around the 40px visual track; the centered pair contains a 24px backend asset mark and a two-line 17px/10px label.
- Leverage, margin-mode and pair sheets are respectively 500px, 446px and 620px high. Their top-level content is start-aligned rather than stretched to the viewport bottom, matching the Pencil child coordinates exactly.
- Leverage content uses a 126px current card, 34px six-column quick rail, 44px scope row, 33px amber notice and 48px confirmation at local Y positions `90/230/278/336/383`.
- Margin mode uses a 45px Header, two 64px options with 10px gap, a 33px notice and 48px confirmation at local Y positions `40/99/251/298`.
- Pair picker uses a 45px Header, 40px search, 22px filters, 322px scroll list and 9px source note at local Y positions `36/91/141/173/505`.
- Selected color language is a pure white/black main canvas, mint `#43efa9`, coral `#ff654a`, thin neutral hairlines and restrained elevation. Sheet surfaces use `#ffffff/#0c100e` with `#f7f9f8/#070a09` inset cards.
- At 320×760, columns contract to `162px + 10px + 124px`; sheet notices become intrinsic-height so wrapped risk copy never overlaps or clips the confirmation action.

### Runtime contracts to preserve

- `/margin/products` is authoritative for enabled products, supported margin modes, leverage levels, maximum leverage and minimum margin.
- `/margin/settings/:product_id` stores a user's selected leverage/margin mode. A missing row returns 404 and means “use product defaults”.
- `/margin/settings/:product_id/leverage` and `/margin/settings/:product_id/mode` are the only settings mutation paths.
- The backend currently accepts market margin positions only. The selected Pencil limit-order sample is visual reference, not permission to expose a non-existent order type.
- Market Store tickers are authoritative for product image, latest price and 24h change; a product without a matching ticker remains visible only with `--`, never with Pencil sample data.
- Pair selection replaces the current route symbol while preserving `mode=contract`; it must not merge spot and margin product lists.
