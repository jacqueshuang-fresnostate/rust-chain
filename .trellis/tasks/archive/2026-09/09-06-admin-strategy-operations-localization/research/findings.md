# Strategy and Admin presentation audit

## Inventory / coverage

AST inventory (`ui-inventory.json`) scans all 111 non-test TS/TSX modules under
Admin, shared UI and layouts: 55 absolute Admin route occurrences, 283 field keys,
all literal select options, and standalone English JSX/label candidates. Routes
include dashboard, config center, support, users/KYC/agents, wallets/deposits,
loans, predictions, spot, new coins, markets, convert, seconds, margin, earn,
news, risk, system settings, account security and audit. Duplicate/legacy routes
remain part of the inventory. Shared rendering paths cover registered resources;
bespoke pages are checked for raw errors, statuses, credentials/settings labels.

## Confirmed strategy defects and resolutions

- `marketStrategy/model.ts::applyPreset` computes target price from start and
  preset percent, and clears seed/mode. Preserve exact manual prices, time range,
  seed mode/value/regeneration command and global boundaries. Only scenario,
  generator shape parameters and explicit nodes come from presets.
- `MarketStrategyForm.tsx`: selection only changes scenario already (keep this);
  applying presets overwrites draft nodes/parameters without confirmation. Add
  replacement summary + confirm/cancel, preserving the current draft on cancel.
- `useMarketStrategyEditor.ts` / `actions.tsx`: edit close discards changes on
  reopen without warning. Add draft baseline, close confirmation and explicit
  reset; prohibit dismissal during submission. Creation keeps its draft on close
  and says so; expired active creation must be rejected while historical draft
  creation and preview remain legal.
- `MarketStrategyVersionSheet.tsx`: active strategies expose restore even though
  `application/market_settings.rs` rejects it. Disable restore with explanation
  for active status; label current configuration separately from running state.
  Restoring copies a version and preserves strategy status, not automatic enable.
- `MarketStrategyRecoverySheet.tsx`: old previews already clear on a new load;
  preserve this. Expired preview is not checked and late requests can repopulate
  a closed sheet. Reject expired execution,
  disable executing while loading, and ignore obsolete responses. No changes to
  immutable history/replay, token verification, permissions or live mutation.
- Active save guard, explicit pause/status reason, preview-only API, non-auto-
  enabling update, authoritative detail hydration and decimal validation already
  correct; preserve these paths. Frontend guards remain advisory; backend locks
  and validations stay authoritative, including concurrent state changes.

## Confirmed localization defects and resolutions

- `DetailDrawer.tsx`: missing recent DTO fields; unconditional status fallback
  translates arbitrary names/titles. Extract shared field/enum presentation;
  translate only declared enum/status keys or explicit typed metadata. Preserve
  usernames, symbols, addresses, content, seed values, unknown codes and JSON
  export. Known nested model fields get Chinese labels as well.
- `StatusTag.tsx`: paused/stopped/live and several business statuses missing;
  share status labels with detail and table/filter rendering, keep explicit
  per-resource overrides for context-sensitive meanings (pending, closed).
- `AdminResourcePage.tsx`: dynamic select labels and plain enum/JSON cells expose
  protocol text. Reuse field-scoped display mapping for labels only; raw filter,
  payload and diagnostic JSON values remain unchanged. CSV retains existing explicit column value-map labels; automatic shared dictionaries do not rewrite raw exports. Preserve strict DTO contract.
- `AdminLayout.tsx`: builtin role code shown directly; translate known system
  roles only, retain custom names. Branded/technical identifiers stay intact.
- Shared and bespoke error paths show raw English backend/network errors. Add
  presentation-only formatter for known errors and field validation templates,
  Chinese fallback with sanitized original diagnostics for unknown errors;
  preserve ApiError status/code/message and secret masking contracts.
- Credential/storage/SMTP forms contain bare English field labels; supply Chinese
  labels with technical names in parentheses. URL, coin examples, brand names and
  editable machine codes are intentionally not translated.
- Audit snapshot already masks recursively; reuse additional field/enum labels
  only without weakening privacy, typed diff or unknown diagnostic contracts.

## Verification strategy

Model regressions first (exact prices, seed, collision atomicity); real Semi UI
preset cancel/confirm, active restore gating, dirty close/reset, expired create
and recovery stale/expiry tests. Shared rendering tests cover all enum domains,
all resource configs/field labels, nested JSON, user content and raw export/filter
values; error tests include credentials and unknown codes. Run all Admin gates
and representative real-browser route/layout checks on a local read-only API
connection. No deployment, production writes or automatic Git push in this task.

Shared route dependencies also inspected: `support/OnlineSupportWorkbench.tsx`
and `auth/LoginPage.tsx`. Their ordinary content and known status labels were
already Chinese; their raw error rendering now uses the common UI formatter.

## Final inventory disposition

The pre-fix Rust DTO scan had 186 missing response-field keys. All 186 now have
shared Chinese labels (623 total known keys); the exact final count and no-gap
result are in `localization-coverage.json`. `ui-inventory.json` and
`dto-label-gaps.json` deliberately retain the original evidence, not a claim of
remaining defects. Runtime coverage tests walk every registered menu category and
resource column. This is source/test coverage, not a claim that every production
page was opened successfully.

Semi's built-in password eye is hard-coded in English even under zh-CN. A shared
Chinese visibility control replaces only that adornment in login and Admin
password fields; native password behavior, form values, authentication and
Turnstile handling stay unchanged. Audit CSV now also uses 请求标识. Prototype-like
unknown codes remain ordinary diagnostics in detail, audit, filters and export;
only explicit string labels are accepted from dictionaries.
