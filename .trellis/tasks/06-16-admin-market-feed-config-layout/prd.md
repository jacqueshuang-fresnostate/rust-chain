# 优化后台行情订阅配置页面

## Goal

Improve the admin market feed configuration page so operators can scan provider status, edit subscriptions, inspect runtime state, and manage credentials with a clearer SaaS-style structure while preserving the existing backend API contract.

## Requirements

- Keep the existing backend endpoints and request payloads unchanged.
- Preserve the single-provider selection behavior for market feed providers.
- Rework the page into a clearer Semi UI layout with an overview area, tabbed modules, grouped form sections, and full-width contained tables.
- Make subscription editing less cramped by separating basic settings, providers, intervals, symbols, and the toggleable subscription list.
- Make runtime and credential information easier to scan without relying on long static helper copy.
- Do not add unrelated custom visual styles or change global admin navigation behavior.

## Acceptance Criteria

- [x] The page still loads saved config, runtime status, and provider credentials from existing APIs.
- [x] Operators can save config with `enabled`, `intervals`, `providers`, `symbols`, and `reason` unchanged.
- [x] Operators can reload market feed subscriptions from the same action.
- [x] Operators can save provider credentials without rendering plaintext secrets after save.
- [x] Provider selection remains single-choice in both form controls and the subscription list.
- [x] Subscription table remains 100% contained in the card/container and does not exceed its parent.
- [x] Existing MarketFeedConfigPage tests are updated and pass.
- [x] Web type-check passes.

## Definition of Done

- Tests updated for the new page structure.
- `npm --prefix web test -- src/admin/actions/MarketFeedConfigPage.test.tsx` passes.
- `npm --prefix web run typecheck` passes.
- `git diff --check` passes for touched files.
- Progress is recorded in `docs/superpowers/PROGRESS.md`.

## Technical Approach

- Use the existing `MarketFeedConfigPage` as the only product-code target.
- Use Semi `Tabs`, `Card`, `Table`, `Space`, `Button`, `Banner`, `Tag`, `Descriptions`-style scanning where available in the installed package.
- Prefer existing shared controls (`AdminSelect`, `AdminTextInput`, `AdminPasswordInput`, `AdminCheckbox`) and shared table layout constants.
- Keep changes scoped to frontend page/test unless type-check reveals a shared helper issue.

## Decision (ADR-lite)

**Context**: The current page already works but lays form controls, runtime state, credentials, and subscription toggles in a dense structure.

**Decision**: Redesign the page as a Semi workbench: compact overview on top, tabbed modules below, and grouped sections inside each tab. Keep all behavior and API contracts stable.

**Consequences**: This improves operator scanability without backend risk. Further admin-wide table/filter redesign remains out of scope.

## Out of Scope

- Backend market-feed provider behavior or validation changes.
- Adding new providers or changing provider credentials schema.
- Admin global layout/navigation changes.
- PC market subscription behavior.

## Technical Notes

- Inspected `web/src/admin/actions/MarketFeedConfigPage.tsx` and `MarketFeedConfigPage.test.tsx`.
- Semi MCP docs consulted for `Tabs`, `Table`, `Card`, and `Tag`.
- Existing recent progress notes show provider selection is intentionally single-provider.
- Implemented with existing frontend-only contracts; no backend endpoint, payload, database, or infra contract changed, so no code-spec update was required.
