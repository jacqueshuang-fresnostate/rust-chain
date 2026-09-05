# Confirmed configuration gaps

- `actions.tsx` gates editing only on form syntax. The backend `update_admin_market_strategy` locks and rejects `active`; the UI has no explanation and no pause command, only disable. Keep backend authoritative and add explicit pause-before-edit UI.
- `model.ts::isMarketStrategyNodeSubmittable` only checks nonempty target text/type/mode, unlike backend strict enums and positive resolved target. Invalid text, zero absolute price and percent <= -100 pass the button gate.
- `presetNodes` silently skips collisions after minute rounding. This can discard relative-to-previous nodes and change all subsequent target meanings. Reject an unrepresentable preset instead of applying a subset.
- Form uses a boolean validity gate without a visible reason. Reuse a single Chinese validation result for preview, save and payload construction.
- Date parsing uses permissive Date normalization; reject invalid calendar dates and UTC-minute misalignment without rounding. Keep valid payload decimal strings and existing immutable seed/version behavior unchanged.

# Scope and verification

Frontend-only repair. No backend status/price/seed generation changes. Add model regressions, real Semi action tests for invalid/valid save, active detail despite stale paused list, explicit pause with a reason, failure preservation. Run all Web gates; do not operate live strategies.

## Browser-discovered inaccessible actions

At 1728px the existing fixed action cell spanned x=1407..1695 (288px), but
the Edit button was at x=1711..1755. `elementFromPoint` hit the page behind it,
not the button. The generic max-content/nowrap group clipped edit and status
actions even before the newly added pause command. The repair wraps whole
buttons only within `.admin-market-strategy-row-actions`, constrains its parent
Space to the cell, and leaves generic table behavior unchanged. Post-fix all
six active-row buttons fit inside the cell and pass center-point hit testing.

## Reproduction and verification evidence

- Before production edits: model regressions had 9 failures (7 invalid target /
  enum cases, impossible calendar date, silently dropped preset node).
- Existing Rust Admin strategy tests: 4/4 passed against a fresh local MySQL
  schema, not skipped. No backend production code was changed.
- Browser uses only schema `codex_market_strategy_20260906_0349`, a temporary
  JWT test session, loopback API 18085 and Vite 13035; no workers are running.
