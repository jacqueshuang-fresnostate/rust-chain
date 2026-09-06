# Stabilize the Admin earn-product CI regression

## Goal

Fix the reported 20-second timeout in `resourceConfigs.test.tsx` without weakening behavior assertions or extending timeouts, then commit and push as requested.

## Evidence and scope

- User's CI result: 501/502 passed; the compound earn-product create/detail/edit/disable case exceeded 20000ms.
- Unmodified focused local case: passed in 10.91 seconds (22.07 seconds including environment/imports), already consuming over half the per-test allowance on this host.
- Its list mock always returns the original product. These are independent operations, not a persisted create-then-edit lifecycle; combining them unnecessarily accumulates their cost and obscures the failing action.
- Each character entered into many controlled Semi inputs rerenders the same rich editor/form tree. This test asserts completed field values and request serialization, not keyboard semantics.
- Previous Mobile changes are already committed in `07602a6`; initial working tree was clean and `main` matched its local `origin/main` tracking ref.

## Plan

1. Share earn-product setup within a nested suite and split create, detail, edit, and disable into independently initialized cases.
2. Set non-keyboard fixture values with real DOM change events; keep real Semi/Quill rendering, StrictMode, click/select interactions, confirmation, exact payloads, and refresh assertions.
3. Bound the shared Vitest default to two workers after the full-suite contention reproduction; add a configuration guard while retaining 20000ms. Remove the same per-character fixture overhead in the four other resource tests actually observed timing out (countries, deposit-address creation, assets, seconds products). Compare focused timings, repeat regressions, and run the full Admin gate with the same default test command as CI.
4. Commit only this task's changes and push without rewriting remote history.

Final full-gate refinement: after those fixes, 505/506 passed and only the analogous compound news case exceeded its pre-existing 40000ms allowance. Split its create/detail/edit/publish/archive operations as well, retaining real rich-text upload and legacy translation preservation. Each now uses the unchanged 20000ms default instead of increasing the existing override.

## Full-suite discovery

The default full run (10 available CPUs, Vitest default 9 workers) failed three other resource-form cases at 20000ms and took 266.77 seconds; 502/505 passed including all split earn cases. A two-worker full run still timed out on countries, deposit-address creation, and seconds products (503/506 passed, 537.15 seconds); concurrency alone does not fix expensive fixture entry. Extend scope only to those observed slow form cases, the shared test concurrency setting, and its runtime-configuration guard. Keep clicks, selects, intentional maximum-value clearing, and all payload/refresh assertions. Keep the CI command unchanged and validate the common bounded default.

## Acceptance

- [x] All original multilingual content, form geometry, decimal payload, row-action and refresh contracts remain covered.
- [x] No test skipped, production component mocked away, timeout extended, or retry added to conceal failures.
- [x] Focused tests repeated; full Admin suite, lint/typecheck/policy/coverage/build/budget pass.
- [x] Verification evidence and traceable progress recorded.

Post-validation delivery: commit this scoped fix, archive only this task and record the session, then push to the configured `origin/main` and compare its remote SHA with local HEAD. Report the actual CI state after pushing rather than predicting success.

## Out of scope

No earn business logic, API/database, global timeout, Mobile, Docker build logic, or deployment changes.
