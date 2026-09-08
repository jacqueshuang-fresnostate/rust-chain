# Root financial form implementation and review

## Changes

- Convert create/edit share one exact DecimalText validator: supported mode,
  distinct positive asset IDs, ratio [0,1), at most 8 meaningful fractional
  places, nonnegative and ordered source/target ranges. The request builder
  repeats validation; visible Chinese hints explain ratio units. Optional
  upper bounds retain existing create-omission/update-null contracts.
- Asset options add optional authoritative precision, validated as an integer
  0..18 without filtering assets needed by unrelated forms or guessing defaults.
- Recharge shows precision and rejects genuinely new invalid intents before
  HTTP. Confirmation reason is part of the stable intent; a readonly pending
  lookup precedes current precision validation so original uncertain requests
  remain replayable after metadata changes. Only exact scope/session/intent
  matches qualify; lookup neither allocates nor refreshes keys. The backend
  remains authoritative for cached precision and replays that never committed.
- Shared decimal precision helper never rounds and accepts trailing-zero and
  scientific-notation equivalents. New backend errors have Chinese mappings
  without changing typed error codes used by retry/conflict handling.

## Actual regressions

- Original convert edit form accepted spread=1; new real Semi test failed on
  toBeDisabled before implementation, then passed unchanged after repair.
- Financial UI/model suite 15/15 passed, including just-below-one storage bound,
  negative/reversed ranges, exact valid payload, new overprecision HTTP block,
  trailing zeros, changed-reason rejection and original-key replay after a
  tighter asset precision.
- Shared decimal/intent/error suite 37/37 passed after new error mappings.
- Independent backend explorer reviewed the final Web + backend agreement and
  reported no concrete findings; review was read-only, not extra test evidence.

## Deliberate boundaries

- Do not reject all overprecision forms at the outer button: the reason has not
  been entered yet, so that would block an original idempotent replay after
  asset configuration changes. Validate a new intent in the confirmation handler.
- No production browser, funds, config, strategy data or account mutation.
- No new dependencies or schema changes. Mobile chart request still awaits
  object clarification; its source and available installed options were only
  inspected, not changed.
