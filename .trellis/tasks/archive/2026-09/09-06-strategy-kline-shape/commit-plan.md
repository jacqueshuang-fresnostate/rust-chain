# Work commit plan (confirmed 2026-09-06)

## 1. 修复行情策略单位提示与 K 线预览精度

- `mobile/src/core/marketChart.ts`
- `mobile/src/components/LightweightMarketChart.vue`
- `mobile/tests/market-chart-price-format.test.ts`
- `tests/synthetic_market.rs`
- `web/src/admin/components/MarketStrategyNodeEditor.tsx`
- `web/src/admin/components/MarketStrategyVolatilityField.tsx`
- `web/src/admin/components/MarketStrategyVolatilityField.test.tsx`
- `web/src/admin/resources/actions/marketStrategy/MarketStrategyForm.tsx`
- `web/src/admin/resources/actions/marketStrategy/MarketStrategyPreviewAction.tsx`
- `web/src/admin/resources/actions/marketStrategy/MarketStrategyPreviewAction.test.tsx`
- `web/src/admin/resources/actions/marketStrategy/MarketStrategyPreviewChart.tsx`
- `web/src/admin/resources/actions/marketStrategy/model.ts`
- `web/src/admin/resources/actions/marketStrategy/model.test.ts`
- `web/src/styles.css`
- `.trellis/spec/admin/ui-system.md`
- `.trellis/spec/backend/synthetic-market-kline.md`
- `.trellis/spec/mobile/backend-integration.md`
- `.trellis/tasks/09-06-strategy-kline-shape/**`
- `docs/superpowers/PROGRESS.md`

Unrecognized dirty files: none. All paths above belong to this task.
The user explicitly confirmed commit and push on 2026-09-06. Commit the validated
source first, then archive this task and record its journal; push the completed
commit chain normally to origin/main. Do not force push, deploy, pause/reconfigure
a strategy or modify historical OHLC. Confirmation does not establish the
user's intended volatility or authorize a live ratio change.
