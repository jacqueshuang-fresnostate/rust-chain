# Admin strategy operations and Chinese presentation

## Goal

Improve strategy editing so scene/preset operations never unexpectedly overwrite
the administrator's chosen target price, audit the strategy operation lifecycle,
and inspect Chinese presentation across the entire Admin console rather than only
the strategy page.

## Confirmed requests

- Changing/applying a market scene must not unexpectedly change the target price.
- Inspect and fix unreasonable strategy operations from create/edit and preview to
  pause/enable, versions and history recovery, based on concrete code evidence.
- Audit all Admin surfaces for missing Chinese field labels, enum/status names,
  details, prompts and error messages. Preserve symbols, addresses, identifiers,
  user content and protocol values; translation is a presentation boundary.

## Starting point

- Previous K-line shape task has been committed and pushed: work `4daab5f`, final
  remote `6fba4a8`; the initial working tree is clean.
- Production strategy volatility remains unchanged. No live strategy, financial
  state or history write is authorized by this UI improvement task.
- Admin has shared resource/detail/formatting paths and multiple bespoke pages;
  an inventory is required to avoid fixing only one screen or replacing raw API
  enum values in payloads.

## Plan

1. Inventory strategy transitions/presets and all Admin presentation paths; save
   actionable findings with file ownership and intended corrections.
2. Add failing regressions for confirmed issues. Preserve manually configured
   endpoints and make destructive draft operations explicit and reversible.
3. Repair shared Chinese presentation and remaining bespoke-page gaps without
   translating user-authored data or silently masking unknown backend contracts.
4. Verify affected models/UI plus full Admin gates and browser state matrix;
   update specs and progress. Request a separate work-commit confirmation.

## Acceptance

- [x] Scene selection and preset application preserve exact manually entered
  start/target prices and timing, with tested explicit draft-operation semantics.
- [x] Strategy lifecycle audit has a documented resolution for each finding;
  active configuration cannot be accidentally saved and confirmation/errors are
  consistent with authoritative backend rules.
- [x] Every Admin route/presentation category is inventoried; missing Chinese
  labels and enum display gaps are fixed centrally where appropriate, including
  nested details. Payload keys/values and meaningful technical identifiers remain
  unchanged and unknown values remain diagnosable.
- [x] Regression tests, lint/typecheck and Admin quality gates pass; available
  live-browser observations and all omissions are explicitly recorded.
- [ ] Complete the remaining representative browser route/layout matrix when
  production API and Turnstile access is stable. No unverified page is marked as
  visually passed; see `research/verification.md`.

## Out of scope

No automatic production configuration/history changes, financial calculations,
permission relaxation, backend wire-format translation, external dependency
replacement or unrelated visual redesign. Do not alter other historical tasks.

## Delivery status

Implementation and automated regression verification are finished. Live strategy
list/detail and Chinese login controls were inspected without business writes;
full browser acceptance is partial due intermittent production/auth-provider
connectivity. The user has now confirmed the work-commit plan and push. No deployment is
authorized by that request. The exact path list is in `review.md`; the remaining
production browser follow-up stays documented separately. Backend and Mobile source remain untouched.

## Confirmed work commit

`80f103042e9ffae89eea5c6187d26596b5c265a5` — 优化行情策略编辑流程并完善后台中文展示. The user explicitly
requested commit and push; implementation is delivered with the remaining live
browser verification follow-up above still open. No production configuration
was mutated. Archiving records code delivery, not a claim of deployment success.
