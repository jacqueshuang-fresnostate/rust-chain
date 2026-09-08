# Backend reliability and Mobile market-chart improvements

## Goal and user scope

Audit backend defects worth optimizing and improve the Mobile market chart.
Use concrete source evidence and reproducible tests; do not assume broad
architecture replacement or rewrite business policies. Preserve all previous
uncommitted Admin/Rust work and the user-confirmed wickVisible:false setting.

## Plan

1. Independently review known risk-config validation boundaries and chart
   renderer/runtime correctness; root inspects upstream Mobile chart data and UI
   lifecycle. Assess remaining settings/approval debt only if bounded and safe.
2. Converge on verified high-impact issues; persist evidence and choose a small
   compatible repair slice. Ask only for genuine product-policy decisions.
3. Add regressions first, implement scoped fixes and verify focused plus package
   quality gates. Test money/backend paths only in a disposable local schema.
4. Update executable specs and PROGRESS with actual results and limitations.
   No commit/push/production changes in this request.

## Constraints / evolution boundaries

- Raw OHLCV and authoritative balance/order facts remain unchanged by display
  optimization; hidden wicks do not authorize fabricating high/low prices.
- Preserve price-line, MA/volume semantics, default 1m, gesture/viewport choices
  and existing stream ownership unless a verified bug requires a narrow repair.
- Prefer matching create/update/runtime validation rather than accepting config
  that silently does nothing; preserve legitimate legacy stop/recovery paths.
- Avoid speculative caching/query rewrites without evidence of hot-path cost.
- Approvals, revision protocols and schema migrations require a coherent contract
  rather than a partial UI-only or state-label-only solution.

## Acceptance (refine after evidence)

- [x] Audited defects have source references and reproducible assertion points.
- [x] Selected backend improvements preserve transactions, auth and audits.
- [x] Mobile chart is more stable/usable with real data; wicks stay hidden.
- [x] Closest tests plus relevant gates pass, or limitations are explicit.
- [x] Previous work remains intact; specs and progress updated.

## References

- Prior completed slices: ../09-06-admin-cross-layer-design-hardening/
- research/baseline-files.json records SHA-256 for all pre-existing dirty files.

## Selected repair slice and ownership

- Backend risk: validate known JSON field types/ranges on create and enable,
  preserve unknown fields and legacy disable; match Admin numeric pair targets
  alongside historical symbol targets using authoritative pair identity.
- Backend fallback: regenerate Coinbase rolling candle time bounds per request
  instead of freezing them at worker startup (subject to bounded adapter review).
- Mobile renderer: preserve live-tail follow on rolling/batch replacements and
  correct initial REST history hydration without overriding deliberate history
  browsing. Keep hidden wicks and current density/resize policies.
- Root Mobile requests: independent initial REST channel completion so depth or
  trades latency does not block ready K-lines; interval switches do not discard
  symbol-owned pending order-book/trade snapshots or leave loading stuck.

Do not partially remove Redis latest-CAS guards to repair historical Mongo gaps;
that ingestion/publication issue is documented for a coherent separate repair.
No generic query/cache architecture changes or approval-protocol migration now.

## Review-stage additions

- Separate book/trade loading indicators accompany independent settlement.
- Trade clears prior-period candles on interval switch without clearing the book.
- Seven new backend validation errors use exact Admin Chinese mappings.

See review.md for final gates, scope limits, temporary-service cleanup and the
prior-working-tree preservation/overlapping-commit warning.
