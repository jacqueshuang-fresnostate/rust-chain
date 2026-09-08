# Mobile chart history pagination and seamless expand control

## User request

1. Dragging toward the oldest visible K-lines must load earlier real history.
2. market-detail__chart-toggle should not have a visible border/frame.

## Plan

1. Inspect actual backend history range/limit semantics and Mobile stream/renderer retention.
2. Implement guarded backward pagination shared by the mobile charts: near-left-edge trigger,
   one request at a time, symbol/interval ownership, exclusive cursor, deduplication,
   failure retry/no request loops, live candle authority and viewport preservation.
3. Remove toggle frame without losing keyboard focus visibility or hit area.
4. Add executable regressions; run Mobile release gate and bounded real-browser checks.
   Change backend only if its existing API cannot correctly return prior pages.
5. Update specs and PROGRESS; preserve all existing dirty work. No commit/push/deploy.

## Acceptance

- [x] Dragging near loaded history start loads older available data, including second page.
- [x] Prepending history preserves viewed timestamps/zoom and never replaces live candles.
- [x] Switching symbol/period, failures, empty pages and repeated events remain bounded.
- [x] Toggle has no visible border/shadow frame; focus and controls still accessible.
- [x] Closest tests and package gates pass, limitations documented.

## Constraints

Keep default 1m, hidden wicks and actual OHLCV. Do not fabricate missing history or
change historical storage/CAS as a shortcut. Inspect current TradeView hard size
budget before adding any logic; extract shared behavior rather than raising it.

## Converged implementation

- Existing backend GET is sufficient: end-inclusive Unix ms, last100 then ascending.
- Root API uses end-only pages, strict earlier cursor and shared1..100 cap.
- Shared wrapper owns on-demand retention; renderer emits gesture/range intent.
- One active request, explicit error retry and empty/no-progress terminal notice.
- No backend, TradeView or view-script changes; toggle CSS only in Detail view.
- Focused API19 and chart60 suites passed; final package/browser results in review.md.
