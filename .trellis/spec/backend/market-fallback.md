# External Market Fallback Time Windows

## 1. Scope / Trigger

Apply to Coinbase REST candle config, retry/reconnect and actual HTTP dispatch.
Provider interval expansion and history/cache ingestion are separate contracts.

## 2. Signatures

- `coinbase_rest_kline_url_at(configured_url, interval, now) -> AppResult<String>`.
- `fetch_rest_fallback_frames_with_clock(config, client, now)` reads the clock
  immediately before each Coinbase K-line request; production supplies `Utc::now`.

## 3. Contracts

Config and `url()` preview values remain immutable. Each actual request replaces
all `start/end` query pairs with one pair each: `end = now.timestamp()` and
`start = end.saturating_sub(existing_interval_seconds * 300)`. Keep the endpoint,
port, proxy path, granularity, other query values/repetitions and fragment.
Bitget/HTX and Coinbase ticker URLs bypass this step byte-for-byte. Preserve
timeouts, generation fences, request ordering and single-request failure isolation.

## 4. Validation & Error Matrix

| Case | Result |
|---|---|
| Long-lived config reused after hours | Send the current rolling window |
| Several queued Coinbase candle requests | Read clock at each dispatch, not once per batch |
| Invalid configured URL | Record that request's error; skip HTTP and continue |
| HTTP failure after refresh | Failure context records actual attempted URL |
| Other provider/channel | Do not consult window clock or rewrite URL |

## 5. Good / Base / Bad Cases

- Good: a reconnect reuses config but materializes a new time window.
- Base: 300-candle widths use the existing interval mapping.
- Bad: updating only startup config; using one timestamp for a long serial queue;
  dropping custom proxy query values; weakening Redis CAS to fill history gaps.

## 6. Tests Required

Use explicit fixed clocks at T and T+12h, recording mock HTTP clients and real
fallback dispatch. Cover duplicate start/end, preserved custom fields, two
different dispatch times, malformed URL and HTTP isolation, non-Coinbase byte
identity, and unchanged ingested payload expectations. No live provider required.
Existing unsupported/legacy 4h mapping is not repaired by this time-window fix.

## 7. Wrong vs Correct

```text
Wrong: worker starts -> freeze start/end -> reuse frozen URL on every retry
Correct: worker starts -> keep config -> for each HTTP dispatch refresh start/end
```
