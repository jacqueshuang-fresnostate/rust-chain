# Current position chip and live PnL audit

## Visual source

- Pencil source `mobile/pencil/hippo-mobile-uiux.pen`, current-position frame `R2wmF0`.
- Position chip frame `iYlbS` uses a 7px gap.
- Each chip uses 4px vertical / 7px horizontal padding and 6px corner radius.
- Chip text uses Noto Sans SC, 13px, weight 650. Production currently uses weight 400 and styles anonymous descendant spans.

## Current data flow

1. `useTransactionRecords().load('positions')` reads `/margin/wallets` and then one `/margin/positions/{id}/risk` snapshot per eligible open position.
2. `OrdersView.positionRecord()` renders `risk.unrealizedPnlText`, `risk.returnRateText`, and `risk.markPriceText` without a later projection.
3. The same view already starts the process-wide market ticker lease. `market.tickers` is reactive and updates from the shared public WebSocket, but only asset latest-price rows consume it.
4. Therefore the connection is live while the position PnL remains frozen at the initial REST observation.

## Backend formula and authority boundary

- `margin_mark_pnl` calculates `notional * directional(mark - entry) / entry`, scaled to 18 decimal places.
- `margin_position_display_metrics` calculates return rate as `unrealized_pnl / margin_amount`, also at scale 18.
- A live presentation helper can reproduce only these same-time display fields with `DecimalText`.
- Maintenance margin, isolated/cross liquidation estimates, account equity and liquidation decisions remain server-owned.

## Selected implementation

- Add one pure, reusable Mobile helper that accepts position exact decimals, the latest ticker and the last server risk snapshot.
- Use the ticker only when it has a positive exact `lastPriceText` and its observation is not older than the risk snapshot.
- Return a coherent tuple containing mark price, PnL and return rate; on any invalid boundary return the server tuple unchanged.
- Consume that tuple in `OrdersView.positionRecord()`. Vue/Pinia dependency tracking then updates cards on every ticker frame without extra REST calls or another WebSocket.
- Add a semantic chip class and exact Pencil typography/geometry instead of broad descendant `span` styling.

## Risks and mitigation

- **Mixed observation times:** compare ticker and risk `observedAt`, and derive all three live values from one ticker.
- **Floating-point drift:** calculate exclusively with `DecimalText` bigint-backed helpers.
- **Missing exact ticker text:** do not rebuild from legacy `number`; keep the server snapshot.
- **Scope expansion:** do not project liquidation or account-level risk locally.

