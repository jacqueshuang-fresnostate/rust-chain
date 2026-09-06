# Chinese Admin Presentation Boundary

## Scope and signatures

All Admin navigation categories, resource tables/filters/details, standalone
settings, audit, login and support use Chinese presentation. Technical brands,
asset symbols, addresses, IDs, operator-entered content and unknown machine codes
remain intact. This does not translate API request or response contracts.

- `adminFieldLabel(key): string` resolves known field names, else the original key.
- `adminEnumLabel(key, value, typedStatus?): string | null` only maps declared
  enum fields, aliases or an explicitly status-typed column.
- `displayDetailValue(value, key, meta?)` recursively formats known fields while
  respecting column label/type/value-map overrides.
- `adminErrorMessage(error, fallback?): string` localizes known errors and
  validation templates, sanitizes credentials/stack lines, and includes raw
  diagnostics plus code for otherwise unknown English errors.
- `adminErrorFieldValue(key, value)` applies that policy to explicit error DTO
  fields only, never to ordinary reasons or messages.

## Ownership and data flow

`adminFieldLabels.ts` and `adminResponseFieldLabels.ts` own shared field labels;
`adminEnumLabels.ts` owns field-scoped enum labels; `adminStatus.ts` owns shared
status labels and tag colors. Resource-specific value maps retain priority for
context-sensitive meanings such as pending/closed. Audit retains its stricter
recursive masking and field-word fallback, reusing shared labels for new fields.

API DTO -> strict row validation -> UI-only formatting. Filters send original
option values, never Chinese labels. CSV keeps its existing contract: explicit
column `valueMap` can label values, but automatic shared dictionaries do not
change raw enum/amount/JSON exports. Never rewrite `ApiError.message/status/code`
to localize it, since request decisions and diagnostics depend on the original.

## Validation matrix

| Input | Display / behavior |
|---|---|
| `status: paused`, `run_status: stopped` | Chinese paused/stopped labels |
| `generator.scenario: trend_up`, `seed_mode: fixed` | Chinese labels inside details |
| `username/title/seed: active` | Exact original string, not a status translation |
| Unknown field/value or `constructor` / `__proto__` | Original diagnostic code, no inherited-object lookup |
| Explicit per-column value map | Takes priority over shared labels |
| Known Unix-millisecond timestamp field in details | Localized timestamp, raw export unchanged |
| `VALIDATION_ERROR` with known field requirement/bound | Actionable Chinese validation |
| Unknown English error | Chinese summary + sanitized original diagnostic/code |
| Named credentials or multiline stack in error | Mask secret; show first line only |
| Password visibility | Chinese accessible show/hide actions, never submit the form |

## Good / bad cases

Good: selecting an enum filter displays `暂停` but requests `status=paused`.
Good: `seed: active` stays `active` even beside `status: active` shown as `启用`.
Bad: looking up every string in a global status map, which silently renames a
user called `active` or masks a future backend enum as a known default.
Bad: translating an API error in the transport layer or coercing bad Decimal
wire values to make a table render.

## Required verification

- Cover every navigation branch and registered resource column with readable
  labels, plus all declared enum domains and unknown values/prototype names.
- Exercise nested details, explicit metadata overrides, unchanged raw CSV enums
  and JSON, and filters whose Chinese labels differ from submitted values.
- Preserve audit masking and raw free-text content; test known/unknown API errors
  and named-secret diagnostics without changing error identity.
- Test real Semi password visibility without changing secret values/submitting.
- Run all Admin quality gates and the representative browser matrix in
  `ui-system.md`; distinguish live API checks from local fixtures and report
  unavailable routes honestly.
