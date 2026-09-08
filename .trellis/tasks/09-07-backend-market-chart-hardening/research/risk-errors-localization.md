# Admin risk validation error localization

## Scope

Additive changes only in `web/src/shared/adminErrorMessage.ts` and its adjacent
test file. Existing recharge/convert localization, fallback diagnostics,
credential sanitization and previous uncommitted edits are retained. No backend
behavior, transport error identity, request data, specification or PROGRESS file
was changed by this follow-up.

The seven literal messages come from
`src/modules/risk/service.rs:34-83` (`validate_risk_rule_config`). Explicit
dictionary entries were added rather than a permissive config-field regex.
The Chinese messages retain complete `config_json` field paths so an operator
can identify the affected JSON entry:

| Field | Chinese guidance contract |
| --- | --- |
| `config_json` | JSON object required |
| `config_json.operations` | String array; each item nonempty and without leading/trailing whitespace |
| `config_json.blocked_operations` | String array; each item nonempty and without leading/trailing whitespace |
| `config_json.max_amount` | Nonnegative decimal number or numeric string |
| `config_json.max_price_deviation_bps` | Integer or integer string in 0..=4294967295, expressed in basis points |
| `config_json.max_requests` | Integer or integer string in 0..=4294967295 |
| `config_json.window_seconds` | Integer or integer string in 1..=4294967295, expressed in seconds |

The integer wording matches `u32_field`, which accepts either a JSON integer
or a parsed integer string. Array guidance applies to each item and does not
incorrectly prohibit an empty array. No unit conversion or stronger input
restriction was introduced by presentation text.

## Regression evidence

Tests exercise the public `adminErrorMessage` function with actual `ApiError`
objects for all seven literal messages, both bare and with the existing
`validation error:` prefix. They assert the exact Chinese result and unchanged
original message, status and code. An additional unknown config field with the
same numeric-bound wording remains an original diagnostic, demonstrating that
the mappings do not infer translations for future rules.

- Before mappings: `npm --prefix web run test --
  src/shared/adminErrorMessage.test.ts` -> **7 failed / 15 passed**. All seven
  new localization regressions failed against the untranslated diagnostics.
- After mappings: the same command -> **22 passed**.
- `npm --prefix web run typecheck` -> **passed**.
- `npm --prefix web run lint` -> **passed**.
- Scoped `git diff --check` for both changed Web files -> **passed**.

No full Web test suite/build/browser run, Cargo invocation, database operation
or external network request was performed for this narrow follow-up. Root owns
the overall task's full gates and progress/specification updates.
