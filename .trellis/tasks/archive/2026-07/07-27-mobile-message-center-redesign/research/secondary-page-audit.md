# Mobile Secondary-Page Audit

## Scope

The Sites prototype contains 39 typed secondary routes. The requested redesign
must preserve that route inventory, fallback graph, protected-route behavior,
and deterministic local records.

## Backend and PC Findings

- The backend exposes public news list/detail endpoints and administrative news
  management.
- No user notification inbox, unread-state, device-session, loan-decision, or
  real security-mutation API is available to this prototype.
- User-specific messages, loan records, and recent-device rows must therefore be
  deterministic local fixtures. Announcement destinations should continue to
  use the existing `news-detail` route.

## Current UX Findings

### Message Center

- Two tabs, two oversized actions, and two terse rows.
- No message preview, grouping, category breadth, detail view, or useful
  destination for non-announcement messages.
- `清空演示` is a prototype control rather than a user workflow.

### Loan

- Product choice, disclosures, amount entry, and lifecycle are visually flat.
- No borrowing-power overview or live repayment summary.
- Raw statuses such as `pending`, `disbursed`, and `repaid` are visible.
- Existing validation is useful and must remain: bounds, precision, duplicate
  application prevention, collateral balance/value checks, and single-flight
  mutation guards.

### Security Center

- TOTP setup and password fields are stacked without an overview or priority.
- No protection score, checklist, device/session visibility, or separation of
  account and funds protection.
- Existing copy, six-digit validation, two-factor toggle, password validation,
  and feedback behavior must remain.

### Shared Surface

- Header displays `SCENE nn / 39` and `HIPPO PROTOTYPE`, which provides little
  task context.
- Shared list, metric, field, status, and action styles are functional but do
  not establish enough hierarchy on information-dense secondary pages.
- Input fields are mostly undifferentiated rectangles with weak focus, invalid,
  disabled, unit, and completion affordances.
- Consequential actions currently mutate local state directly or only produce a
  toast. A reusable confirmation sheet is needed for reviewable secondary-page
  actions while keeping the root shell unchanged.
- Final CSS should stay in the existing `Signal Theatre final
  secondary-surface contract` layer to avoid conflicting legacy rules.

## Design Direction

- Quiet, dense operational interface informed by exchange-product patterns.
- Square framing, strong rules, precise mono data, vivid green/coral/cyan
  signals, and Lucide icons only.
- No marketing hero, decorative card nesting, emoji, or speculative backend
  capabilities.
- Use a bottom confirmation sheet as the common button-popup pattern: accessible
  dialog semantics, overlay/Escape dismissal, concise review data, and visible
  primary/secondary actions.
- Deep structural redesign for Message Center, Loan, and Security Center.
- Shared header, section, list, status, form, and responsive upgrades for all
  secondary routes.
