# Findings

- The original test performs four separate operations in one async body, includes 17 per-character `user.type()` calls and multiple clears, and renders two real Quill editors for multilingual creation.
- `AdminTextInput` forwards the DOM input change to its controlled string onChange; `fireEvent.change` is already used in this same test file for fixture values. Real select and confirmation interactions remain necessary.
- The common resource create suite resets mocks/session state and supplies browser polyfills in beforeEach. A nested earn-product beforeEach can isolate identical response fixtures without changing other tests.
- `web/vite.config.ts` already has a 20000ms default; `vitest.setup.ts` has a 5000ms async-query allowance. Keep both timeouts unchanged; bound concurrency separately after the full-suite reproduction.
- The P0 script invokes `npm --prefix web test` without worker overrides. Final full-suite verification must use that same invocation rather than only the previous `--maxWorkers=2` local path.

## Full-suite reproduction and cause refinement

- `npm --prefix web test` with no CLI overrides completed in 266.77s: 3 timed-out resource cases (country create/edit/status, deposit-address-pool create, asset filter/detail/edit), 502 passed including all four new earn cases. No assertion mismatch was reported.
- Installed Vitest resolves run-mode default workers to `availableParallelism() - 1`; this host reports 10 CPUs, hence 9 workers. Transform/setup/import/environment sums were 96.53/125.42/595.68/208.95 seconds.
- Previous locally passing full validation used an explicit `--maxWorkers=2` override that the CI script did not share. The common config should own this resource budget instead of relying on a one-off local flag.
- A second full run with the shared two-worker budget still timed out on countries, deposit-address creation, and seconds products (503/506 passed, 537.15s). All earn cases passed. Concurrency alone was not sufficient; avoid presenting it as the complete root cause or an established speedup.
- Restrict further fixture-entry optimization to the four other resource cases actually observed timing out across those runs. These assert completed input values and serialization, not per-key behavior. Retain actual clicks/selects, intentional clearing of seconds maximum stake, every API payload and refresh assertion, and the unchanged 20000ms deadline.
- The isolated country run also exposed an order-dependent lazy-loading race: the list row appears before its separately imported action buttons. Reuse `waitForLazyResourceActions()` before synchronous button assertions; do not depend on another test warming the action module. The other seven affected form cases and the configuration guard passed in that run.
- The next full run passed all previously fixed regressions (505/506 total) but exposed the same accumulated-runtime issue in the compound news case at its existing 40000ms limit. Split its five independent operations, reuse reset fixtures, keep upload/Quill/layout/legacy-translation assertions, and replace the aggregate refetch lower bound with one exact refresh per mutation. Remove the 40000ms override; each isolated case uses the unchanged 20000ms default.
- AST comparison for the four additional optimized cases retained every original expectation in order: countries 15, deposit-address creation 8, assets 19, seconds products 17. Only ordinary input fixture entry changed; the seconds maximum-stake clear remains a real `user.clear` interaction.
- News AST comparison retained all 61 original expectation statements (normalizing only the stronger per-case refetch count), adding nine isolation/refetch/API-count assertions. Fresh structured clones keep the shared expected news-content baseline separate from rows supplied to each rendered form.

## Assertion and runtime evidence

- TypeScript AST comparison retained all 57 original expectation statements (normalizing only per-case refetch baseline deltas). New independent-operation/read-only-detail/content-retention checks increase this to 63.
- First split run: create 3477ms, detail 265ms, update 1407ms, disable 587ms; total test time 5.74s versus the original single case 10.91s. Real Quill, StrictMode and every original payload/style check remained active.
- Added the shared-config guard before configuring workers: it failed on missing maxWorkers as expected while the original 20000ms deadline assertion stayed present.
- Final focused run after fixing the cold-load race passed all 9 selected regressions with no deadline overrides: country 7996ms, address creation 2484ms, assets 7238ms, seconds products 6152ms; earn create/detail/update/disable 8726/994/5021/1910ms, configuration guard 1ms. Full suite and production checks are recorded separately in the final review. Wall-clock comparisons across these runs are affected by host load and are evidence of deadline headroom, not a universal speedup guarantee.

## Prevention

- Root-cause category: missing test-design contract. Independent mutations accumulated one deadline, while completed fixture values triggered per-character renders in real Semi/Quill trees.
- Contributing verification gap: a local-only worker override did not exercise the same default command as CI. Shared configuration now owns the worker budget, but fixture and lazy-load fixes remain necessary independently of it.
- Isolation exposed an additional assumption: a loaded list does not imply its lazy action module has mounted. Wait on the actual action-loading boundary before reading buttons.
- Keep these practices in the Admin UI spec and guard the common worker/deadline configuration. Test-only changes leave production form behavior, APIs, money handling, and Docker build logic unchanged.
