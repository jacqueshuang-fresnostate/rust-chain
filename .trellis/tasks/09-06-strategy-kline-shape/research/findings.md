# Confirmed K-line shape findings

## Read-only observations

- The configured public API exposes one strategy market: HIPPO-USDT, price precision 6. Initial public observations used no admin session; later authenticated detail confirmation is recorded below. No strategy mutation was used.
- At 2026-09-06 08:21:50 UTC, 52 public 1m rows since 07:30 contained 37 highs above twice their candle body maximum. Example: O 0.096527, H 0.393109, L 0.096393, C 0.103197. Other rows reach L 0.000001. These values explain the screenshot's needle-like appearance; the renderer did not invent them.
- Initial screenshot/public samples did not establish exact strategy settings. Later authenticated confirmation below resolves the cause, without inferring the intended replacement ratio.

## Source boundary

- `src/modules/market/synthetic.rs`: high/low extension is `body_price * volatility * unit_draw * wick_scale`. Volatility is a dimensionless fraction (`0.01 = 1%`), unlike node tolerance and relative target values, which are percentages divided by 100. Price floor is one pair precision unit. No generator correction is justified without versioned semantics; preserve replay.
- `MarketStrategyForm.tsx` and `MarketStrategyNodeEditor.tsx` label volatility without its unit. Preserve raw request/record values but show explicit ratio examples, live percent interpretation, and a combined wick warning (a multiplier >= 100% can drive lows to the price floor).
- `MarketStrategyPreviewAction.tsx` charts only closes and scales only those closes despite having all OHLC columns. Replace that visual with honest candle bodies/wicks on one high/low range, and identify sparse previews as sampled rather than continuous history.
- `mobile/src/components/LightweightMarketChart.vue` never supplies price format for candle/MA series. Installed lightweight-charts 5.2.0 defaults to precision 2/minMove 0.01; its flat-range fallback extends by five minMove units. Infer display precision from finite OHLC (not volume or computed averages), support scientific notation, and apply the same format to candles/MAs without changing feed data or viewport fitting policy.

## Boundaries and verification

- Do not silently divide existing snapshots, cap wicks, change configured data, or delete/regenerate history. These candles can be financially consequential; operational correction requires the actual settings and an explicit versioned change.
- Prefer exact existing decimal helpers for Admin ratios/percent labels and threshold comparisons. Chart-coordinate arithmetic is display-only.
- Add pure low-price generator contracts, Admin warning/preview tests, and Mobile format/update tests. Validate real local rendering where the browser is available; preserve existing gates and warn separately if a browser runtime fails.
## Authenticated production confirmation — 2026-09-06

- User-provided admin login completed through the normal login UI and automatic site verification. No credentials/session tokens are recorded here.
- Read-only detail panel: strategy 1 / pair 3 / HIPPO-USDT, active version 1, start and target 0.1, no nodes, **volatility `6.00000000`**, generator mean reversion 0.55 / noise 1 / **wick 0.75**. This is a 600% ratio, not 6%, with a maximum random single-side wick factor approaching 450% of its body endpoint. It directly explains the observed ~0.55 highs and minimum tick lows.
- Login/list initially experienced a 15-second read timeout; one UI refresh succeeded. No timeout configuration changed.
- Local Mobile on loopback port 13039 uses the already-configured production API. HIPPO detail renders 1m, real strategy depth, and live six-place prices. This check performs no order or strategy write.
- Asked whether the intended volatility was 6%; no production correction is implied by the source fixes. Historical OHLC remains intact. Changing old generator semantics, clamping plotted highs/lows, or recovering history would conceal or rewrite actual data and is out of scope.

## Bug retrospective

1. **Root cause — B/E (cross-layer contract / implicit assumption):** ratio fields lacked units while neighboring tolerance fields use percent. Low-price chart series inherited a two-place default. **D (coverage gap):** the visual preview ignored valid high/low data entirely.
2. **Why earlier work did not cover this:** the previous task repaired delivery, candle aggregation and scheduling. Successful pushes do not validate an operator's intended unit or the readability of unusually small prices. There is no evidence that the feed fix caused these configured wicks.
3. **Prevention implemented:** one shared global/node ratio field; exact interpretation and shape warning; full OHLC preview with sampling notice; pure low-price/legacy generator tests; explicit shared candle/MA price format and edge-case regressions.
4. **Systematic scope:** both global and node input paths are covered; tolerance is explicitly percent. All four price series share format, while volume remains independent. No snapshot, provider, storage, order or settlement contract changed.
5. **Knowledge captured:** Admin UI, synthetic-market backend, and Mobile integration contracts document these executable boundaries. Operational settings/history changes remain separate.

## Verification

- Red regressions demonstrated the absent OHLC candle preview and missing explicit Mobile price format before implementation.
- Web: final full suite 70 files / **514 tests passed**; closest form/model/preview suite **33 passed**. Lint, typecheck, production-policy 15, coverage 23, final same-origin build and bundle budget passed.
- Mobile: focused chart/viewport suite **16 passed**; final `release:gate` **685/685**, including app/test type-checks, PWA/Tauri builds, artifacts, bundle/source/test-quality budgets.
- Rust: synthetic generator 13, market-details 3, worker 13, architecture 11 and documentation 1 passed (**41 total**); production generator bytes are unchanged. Formatter and source integrity (16 inputs plus 16 gate tests), Trellis 6+6 context, and diff whitespace checks passed.
- Live Mobile chart: candles and all MAs use precision 6 / minMove 0.000001 / base 1000000. Real highs/lows remain intact. A panned logical range 20..50 remained stable across a live close change. Interval switching loaded candles for 1m, 5m, 15m, 1h and 1d.
- Actual Mobile theme-store matrix: 320/390px × light/dark × Chinese/English, all eight cases have zero document overflow, chart within viewport and all four series at precision 6. Actual chart backgrounds change white/black; both light Chinese and dark English screenshots were visually checked.
- Local Admin using the production read model displayed `当前为 600%` and `450%` risk explanation, with active submission disabled and zero document overflow. Early read/preview requests encountered intermittent network/session failures; later authenticated previews succeeded with HTTP 200 and 120 real sample wicks.
- Side-effect-free comparison using the same inherited seed and preview version 2: ratio `6` gives sampled extrema **0.000001..0.850556**; unsaved ratio `0.06` gives **0.095171..0.105052**. Both show 120 samples from 34560 minutes and the non-continuous notice. This is an in-memory next-version preview, not a write or replacement of V1 history. The final detail GET confirmed production **active / V1 / volatility 6.00000000**.
- `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings` passed. No full database-backed suite was needed/run because production Rust/SQL is unchanged.
- Admin baseline: 1728px Dashboard/empty spot/strategy/KYC/Security Policy and 1280px preview/empty spot showed no document overflow. Some earlier reads reported network errors; no error/permission/timeout rule was weakened. Preview chart stayed within 1280px (x444..1241). Final guarded operational write count was zero.
- Cleanup confirmed: browser task space 15 closed (`done: true`); only this task's verified Vite PIDs on 13038/13039 were stopped. Temporary screenshots remain outside the repo for verification; no credentials were written to task files.
