# Admin resource timeout fix — review

## Scope and cause

- The reported earn case shared a deadline across four independent operations and rerendered a real Semi/Quill form for each fixture character. It now has four isolated cases and real change events for ordinary fixture values.
- Full-gate verification exposed the same issue in news (five operations, previously a 40000ms override), plus expensive fixture entry in countries, deposit addresses, assets, and seconds products. Only these observed cases were adjusted. News now uses the common 20000ms default.
- Country/news initialization explicitly waits for lazy actions rather than assuming loaded rows imply mounted buttons. Fresh queries and response fixtures isolate operations; news responses clone the expected content baseline.
- The shared two-worker budget makes local and CI defaults identical. It did not fix the regression by itself and is not claimed as a universal wall-clock speedup.
- No production component, business/API contract, monetary logic, Docker build step, or Mobile behavior changed. The earlier Mobile fixes already exist in remote commit `07602a6`.

## Verification

| Check | Result |
| --- | --- |
| Original focused earn baseline | Passed locally in 10.91s; user CI exceeded 20s |
| Regression for common worker budget | Failed before `maxWorkers: 2`, passed after |
| Final `npm --prefix web test` with no overrides | 68 files, 510/510 passed; 394.14s total |
| Final selected form/config repetition | 14/14 passed; 41.01s total; no retries |
| `npm --prefix web run lint` | Passed |
| `npm --prefix web run typecheck` | Passed |
| `npm --prefix web run test:production-policy` | 15/15 passed |
| `npm --prefix web run test:coverage` | 23/23 passed; statements 85.61%, branches 81.78%, functions 85.33%, lines 92.87% |
| Same-origin production build | Passed with `VITE_API_SAME_ORIGIN=true VITE_API_BASE_URL=`; existing chunk-size advisory remains |
| `npm --prefix web run budget` | Passed; initial JS gzip 445217 bytes, largest async JS gzip 60074 bytes |
| `cargo test --test docker_image_contract` | 5/5 passed |
| `python3 scripts/source_integrity_gate.py` | 16 build source inputs passed UTF-8/source checks |
| Trellis context validation / `git diff --check` | Passed |
| Full-file TypeScript AST assertion multiset comparison | All 775 original expectation statements retained; 790 now, normalizing only independent-operation refresh deltas |

The final repetition measured earn create/detail/update/disable at
2534/225/1440/485ms and news create/detail/edit/publish/archive at
2603/577/2198/717/544ms. These are observed local timings, not CI guarantees.

No skip, retry, increased timeout, fake timer, or production-form mock was
introduced. Focused runs intentionally select a subset; the final full run
has no skipped tests. Real rich-text rendering, uploads, selects, clicks,
country-derived locales, legacy translations, exact decimals, intentional
maximum-stake clearing, confirmations, and per-operation refetches remain.

## Verification boundaries

This is a test/configuration-only change. No browser visual matrix, live API
or money operation, complete Rust/PC/Mobile gate, image publish, or deployment
was run locally. CI execution is a separate post-push result.
