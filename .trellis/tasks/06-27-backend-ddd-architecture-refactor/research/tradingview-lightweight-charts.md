# TradingView Lightweight Charts Research

## Decision

Use TradingView's official `lightweight-charts` npm package for the PC renderer, selected through the platform display configuration.

## Why this integration

- It renders the exchange's own REST history and WebSocket K-line stream, so chart prices stay consistent with order-book and trading data.
- TradingView's hosted widgets and Advanced Charts do not provide this repository's private market-data integration without separate datafeed and licensing work.
- The project keeps the existing KLineCharts renderer as an administrator-selectable fallback.

## Verified API and license points

- Official documentation shows v5's `createChart` plus `chart.addSeries(CandlestickSeries, ...)` API and real-time `series.update(...)` flow.
- The public package version selected is `5.2.0`.
- The Lightweight Charts license requires TradingView attribution and a link, so the renderer includes a visible `TradingView` attribution link.

## Sources

- https://tradingview.github.io/lightweight-charts/docs/5.0
- https://tradingview.github.io/lightweight-charts/docs/api/functions/createChart
- https://www.tradingview.com/charting-library-docs/latest/getting_started/product-comparison/
