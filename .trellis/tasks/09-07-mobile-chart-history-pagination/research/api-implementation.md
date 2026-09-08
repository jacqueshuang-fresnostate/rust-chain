# Mobile history API implementation

Backend inspection confirmed end-inclusive Unix-ms filtering, descending-limit
then ascending output, and a 100-row maximum. No backend code changes are needed.

Root added fetchOlderKlines(symbol, interval, before, limit=100), with exclusive
before converted to end=before-1 and no artificial start. Both latest and older
requests share fetchKlinePage, use existing public-request auth configuration,
normalize the symbol/rows and respect the 1..100 server cap. Existing requested
24-row sparklines keep that limit. Invalid cursors return without I/O; exceptions
propagate to retry UI. Unexpected response rows beyond the requested end are
excluded; raw accepted OHLCV stays unchanged. No TTL/cache or provider calls.

Six executable tests extract the actual production API function bodies, inject
only transport/time/URL-boundary dependencies, and use real symbol/K-line mapping.
The initial sparse-lookback and cap cases both failed before implementation.
The full six pass, including four requests retrieving 276 unique sparse rows,
exclusive boundary, array/wrapped envelopes, invalid cursor, and transport error.
Existing protocol/cache-whitelist tests pass too: combined 19/19. Source envelope
assertions were narrowly moved to the shared fetchKlinePage helper.

All prior dirty work remains. Root owns final package gates, browser validation,
specs/PROGRESS and no-commit handoff. This pagination returns existing stored
candles only; prior ingestion gaps are a separate backend task.
