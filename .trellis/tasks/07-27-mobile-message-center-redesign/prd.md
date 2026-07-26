# Redesign Mobile Secondary Pages

## Goal

Transform the prototype's sparse secondary pages into a coherent, production-
grade mobile system. Deeply redesign Message Center, Loan, and Security Center,
then apply the same visual hierarchy, touch, status, and action contracts across
all 39 secondary routes without changing their route or local workflow behavior.

## What I Already Know

- Message Center contains only two tabs, two oversized actions and two terse
  rows with no message body or useful destination.
- Loan flattens product choice, disclosures, amount entry and order lifecycle
  into generic blocks. Raw backend-style statuses are exposed to the user.
- Security Center stacks TOTP and password fields without a protection overview,
  task priority, device visibility, or clear separation between security flows.
- The remaining secondary routes share a generic header and basic component
  styles, so improving the shared surface can materially lift all 39 routes.
- The backend exposes public news list/detail APIs and rich announcement
  categories, but no user notification inbox or read-state API.
- The mobile and PC clients expose announcements but do not implement a user
  notification center.
- The prototype must therefore keep user-specific notifications deterministic
  and local while linking announcements to the existing news-detail route.
- The existing Signal Theatre visual system uses square corners, strong rules,
  Lucide icons, vivid green/coral/cyan signals, and restrained operational
  density. It must stay useful and scannable rather than becoming a marketing
  landing page.

## Requirements

### Shared secondary-page system

- Replace route-sequence and prototype-placeholder header text with a meaningful
  route-group label and relevant route context.
- Establish consistent section labels, separators, operational metrics, list
  rows, status badges, form blocks, and action hierarchy for every secondary
  route.
- Redesign text, numeric, password, search, select, checkbox, and unit-bearing
  fields with explicit focus, invalid, disabled, help, and completion states.
- Introduce one reusable accessible mobile confirmation sheet for consequential
  button actions, with overlay dismissal, Escape support, a compact review
  summary, and explicit primary/secondary commands.
- Keep all touch targets at least 44px, prevent horizontal overflow at 390px,
  and maintain equal clarity in light and dark themes.
- Use Lucide icons only, never emoji, and preserve square or lightly framed
  operational surfaces without nested cards.
- Preserve all route IDs, fallback behavior, validation, single-flight guards,
  and deterministic local-only side effects.

### Message Center

- Add an inbox summary with total and unread counts.
- Add category filters for all, account, funds, trading and announcements.
- Add a compact unread-only toggle and mark-all-read command.
- Replace generic rows with typed messages containing icon, category, title,
  summary, timestamp and visible unread state.
- Group list content into recent time sections.
- Open a complete in-page message detail view with a contextual destination.
- Keep announcements connected to the existing `news-detail` route.
- Provide meaningful filtered-empty and all-read states.

### Loan

- Add a borrowing-power overview, product comparison, and compact loan terms.
- Add amount presets and live principal, interest, total repayment, and due-date
  estimates without weakening existing amount or collateral validation.
- Give credit and collateralized loans visibly distinct requirements.
- Redesign the order lifecycle as readable active/history records with localized
  statuses and clear cancel or repay actions.
- Preserve guest authentication flow, duplicate-application prevention,
  collateral limits, and local-only records.

### Security Center

- Add a protection score and prioritized security checklist.
- Separate two-factor setup, password change, and funds protection into distinct
  tasks with clear completion status.
- Add deterministic recent-device/session visibility and a local device-revoke
  interaction.
- Preserve the existing TOTP, password validation, copy, toggle, and feedback
  behaviors while removing the impression of a real backend mutation.

## Acceptance Criteria

- [ ] All 39 secondary routes keep their existing typed route and fallback
  contracts while using the upgraded shared header and surface system.
- [ ] Message Center shows at least four categories; unread counts, filters,
  detail view, contextual destinations, and recovery states all work.
- [ ] Loan shows product differences, live repayment estimates, validated
  collateral requirements, localized lifecycle states, and working actions.
- [ ] Security Center shows a protection overview, prioritized tasks, working
  TOTP/password controls, and deterministic device management.
- [ ] Shared fields provide visible focus, invalid, disabled, unit, hint, and
  selection affordances without layout shift.
- [ ] Consequential secondary-page actions use an accessible confirmation sheet
  with keyboard dismissal, overlay dismissal, reduced-motion support, and clear
  cancel/confirm actions.
- [ ] No user-facing raw English statuses, prototype-clearing controls, emoji,
  non-Lucide inline icons, or external side effects are introduced.
- [ ] Light and dark themes remain legible at 390x844 and wide desktop without
  overflow, clipping, incoherent overlap, or blank surfaces.
- [ ] All visible controls meet the 44px touch target contract.
- [ ] Lint, production build, tests and browser console checks pass.
- [ ] The exact validated source is deployed to the existing public Sites URL.

## Out Of Scope

- Adding a backend user-notification API or persistent read state.
- Push-notification permissions or delivery.
- Changing backend APIs, public news APIs, or route inventory.
- Adding real credit decisions, loans, device sessions, or security mutations.
- Reworking root pages, bottom navigation, or trading behavior.
- Changing the global header bell entry.

## Technical Notes

- Primary files:
  - `mobile/sites-prototype/app/secondary-pages.tsx`
  - `mobile/sites-prototype/app/globals.css`
  - `mobile/sites-prototype/tests/rendered-html.test.mjs`
- Keep message and device fixtures typed and outside render branches.
- Remove user-facing prototype-only controls such as `清空演示`.
- Preserve component ownership boundaries and typed route contracts.
- Treat shared CSS as a system layer; add final overrides in the existing
  secondary-surface contract instead of scattering route-specific patches.
