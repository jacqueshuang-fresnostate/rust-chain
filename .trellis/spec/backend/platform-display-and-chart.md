# PC Display and Chart Contracts

## Scenario: Globally Configurable PC K-line Renderer

### 1. Scope / Trigger

- Trigger: an administrator needs to select the PC K-line renderer without a frontend deployment.
- Scope: `platform_brand_configs`, the public platform display endpoint, the admin PC display page, and the PC `MarketChart` wrapper.
- The renderer choice applies to spot, margin, seconds-contract, and launchpad trading pages.

### 2. Signatures

- Migration: `migrations/0081_platform_chart_provider.sql` adds:
  ```sql
  platform_brand_configs.chart_provider VARCHAR(32) NOT NULL DEFAULT 'klinecharts'
  ```
- Public endpoint: `GET /api/v1/platform/brand` returns `PlatformBrandResponse` with `chart_provider`.
- Admin endpoints:
  - `GET /admin/api/v1/platform/brand`
  - `PATCH /admin/api/v1/platform/brand`
- Accepted `chart_provider` values:
  - `klinecharts`
  - `tradingview`

### 3. Contracts

- `PlatformBrandResponse.chart_provider` is always a non-empty, normalized string.
- `SavePlatformBrandRequest.chart_provider` is optional for compatibility with old admin clients. When absent, the application layer keeps the published provider instead of resetting it.
- The admin save use case writes the provider and its before/after values into the existing `platform_brand_config` audit record in the same transaction.
- The PC store normalizes an unknown or missing response value to `klinecharts`.
- `MarketChart.vue` selects either the existing KLineCharts renderer or `TradingViewChart.vue`; both receive the same internal REST history and public WebSocket K-line stream.
- Do not use a hosted TradingView widget for platform-owned symbols. It does not receive this application's market-data contract. `TradingViewChart.vue` uses the official `lightweight-charts` package and displays the required TradingView attribution link.
- Both renderers are lazy-loaded so a user downloads only the selected chart library.

### 4. Validation & Error Matrix

- `chart_provider = klinecharts` or `tradingview` -> save succeeds and the public endpoint returns the saved value.
- Unsupported or blank non-empty provider -> `400 BAD_REQUEST` with a validation error.
- Omitted `chart_provider` in an otherwise valid legacy admin request -> retain the current provider.
- Platform configuration request fails in the PC -> retain the currently hydrated/default `klinecharts` renderer.
- Database migration not applied -> platform configuration query fails; deployment must run migration `0081` before enabling this release.

### 5. Good / Base / Bad Cases

- Good: admin selects `tradingview`; the audit record contains `klinecharts` before and `tradingview` after; a fresh PC chart mount renders TradingView Lightweight Charts using the exchange's own candle data.
- Base: no chart provider was selected before the migration; the migration backfills `klinecharts` and existing charts continue to render.
- Bad: a frontend hard-codes `tradingview` or fetches candles from a public TradingView widget, causing the chart price to diverge from the internal order book.

### 6. Tests Required

- `tests/admin_routes.rs`: assert default provider, invalid-provider rejection, save response, and audit before/after values.
- `tests/user_routes.rs`: assert the public PC display endpoint returns the configured provider.
- `tests/openapi_routes.rs`: assert `PlatformBrandResponse.chart_provider` is documented.
- `web/src/admin/actions/PlatformBrandPage.test.tsx`: select TradingView and assert the PATCH payload contains `chart_provider`.
- `pc/tests/chart-provider.test.ts`: verify provider normalization and that all PC K-line pages use `MarketChart`.
- `pc/tests/kline-data.test.ts`: assert history and realtime K-line normalization, topic substitution, timestamp conversion, sorting, and duplicate replacement.
- Run `cargo fmt --check`, `cargo check --all-targets`, PC and web type-checks, and the targeted tests above.

### 7. Wrong vs Correct

#### Wrong

```typescript
import TradingViewChart from './TradingViewChart.vue'
import TVChart from './TVChart.vue'

const chartComponent = computed(() => settingStore.chartProvider === 'tradingview' ? TradingViewChart : TVChart)
```

This eagerly bundles both chart libraries into every trading page.

#### Correct

```typescript
const KlineChartsChart = defineAsyncComponent(() => import('./TVChart.vue'))
const TradingViewChart = defineAsyncComponent(() => import('./TradingViewChart.vue'))

const chartComponent = computed(() =>
  chartProvider.value === 'tradingview' ? TradingViewChart : KlineChartsChart
)
```

This preserves the backend-controlled selection while code-splitting the renderer payloads.
