# Split Spot And Contract Mobile Columns

## Goal

Separate spot and contract trading into independent top-level mobile columns so
users do not have to enter a generic trade page and switch modes inside it.

## What I Already Know

- The Sites prototype currently has five bottom navigation columns and a single
  `trade` view.
- `TradeView` owns an internal `spot | contract` switch even though Home already
  presents Spot and Contract as separate shortcuts.
- The production mobile navigation contract keeps trade mode as explicit context.
- The user explicitly wants Spot and Contract to be separate columns.

## Requirements

- Replace the generic Trade column with independent Spot and Contract columns.
- Use six bottom navigation columns: Home, Markets, Spot, Contract, Assets,
  Profile.
- Home Spot and Contract shortcuts must open their matching columns directly.
- Market selection must preserve the intended destination: the Contract market
  category opens Contract; other market categories open Spot.
- Remove the Spot/Contract mode switch from the order console.
- Give Spot and Contract distinct headings, balance semantics, controls, helper
  text, and simulated order confirmations.
- Keep all existing Lucide icon, no-emoji, responsive, theme, and public Sites
  deployment requirements.

## Acceptance Criteria

- [x] Bottom navigation displays independent Spot and Contract columns.
- [x] Spot and Contract navigation renders different trade surfaces.
- [x] No Spot/Contract mode switch remains inside either order console.
- [x] Home shortcuts and market rows route to the correct trade column.
- [x] Simulated Spot and Contract orders return mode-specific confirmations.
- [x] The 390px layout has no horizontal overflow or bottom-nav text collision.
- [x] Lint, production build, tests, emoji scan, browser checks, and public Sites
      deployment succeed.

## Definition Of Done

- Source is committed and pushed to the existing Sites repository.
- A new saved Sites version is deployed successfully to the public URL.
- Project progress and task metadata are updated.

## Out Of Scope

- Production Vue/Tauri application changes.
- Real order placement, account balances, or backend integration.
- Changing other product columns or backend APIs.

## Technical Notes

- Primary implementation: `mobile/sites-prototype/app/page.tsx`.
- Layout adjustments: `mobile/sites-prototype/app/globals.css`.
- Tests: `mobile/sites-prototype/tests/rendered-html.test.mjs`.
- Navigation reference:
  `.trellis/spec/mobile/navigation-and-localization.md`.
