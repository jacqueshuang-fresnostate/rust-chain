# Root causes and chosen contracts

- Synthetic worker has no depth/trade generation. REST trades reads only real spot_trades; simulated feed must use isolated bounded Redis history and retain strategy provenance, never forge spot executions.
- Higher intervals publish only on complete boundaries: no live higher-timeframe candle. Publish forming aggregates from stored 1m plus current 1m through Redis/WS; REST merges the current cache with Mongo history. Never persist incomplete higher windows as closed history or backfill absent roots.
- Mongo history sorts ascending before limiting to 100 while mobile requests 160: latest 60 roots can be excluded. Select newest N then return ascending.
- Mobile RAF keeps one pending candle regardless of open_time: close-previous/open-next burst loses final previous candle. Coalesce per open_time, reject stale same-slot observations, and retain bounded candle state.
- Mobile REST merge gives all previous REST data permanent precedence over fresh REST. Track actual live points separately so refreshed historical roots can repair old REST-only rows while late REST cannot overwrite live data.
- Redis K-line rejects an identical replay after a partial Redis-success/Mongo-failure; add identical replay classification so same input can repair persistence.
- Mobile detail/trade defaults and API default are 15m. Use shared DEFAULT_MARKET_KLINE_INTERVAL=1m. Markets list 15m sparklines are not interactive chart defaults and remain unchanged.

## Admin expansion
- Activation did not revalidate pair status/type, reject expired ranges, or serialize overlapping strategies on one pair. Create already locks pair; extend status activation to pair→strategy lock ordering and current-read half-open overlap checks.
- Admin response omitted strategy_runs.error_message and last_tick_at; initialized last_generated_at was not evidence of successful publication. Expose real diagnostics and snapshot semantics, not an auto-aging display of unrefreshed list data.


## Final review findings
- Identical Redis K-line replay previously had no repair path after Mongo failure;
  preserved Accepted-only financial callers while explicitly permitting identical
  K-line repair in ingestion. Real integration deletes the Mongo root and replays
  the same observation, proving repair without a duplicate display trade.
- Worker checkpoints previously retained microseconds while archive evidence used
  milliseconds. Normalize at the round boundary before any side effects.
- New diagnostics are a loaded snapshot, explicitly labelled, rather than an aging
  timer that misreports a healthy running strategy from stale list data.
- UI regressions exposed tests assuming the page title meant lazy action imports
  were ready. Updated only strategy tests to await their actual buttons; final full
  suite passes without increasing timeouts. Initial SMTP timeout under concurrent
  compile load disappears in the bounded-worker full run.

- Final Redis review reproduced a conflicting trade being classified as replay
  because identical depth returned before validating the trade. Move the replay
  shortcut after conflict checking; the 105-print bounded queue test now proves
  every same-depth/changed-quantity submission is rejected without writes.
