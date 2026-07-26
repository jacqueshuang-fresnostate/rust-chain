# Remove Web3 Wallet From Mobile Prototype

## Goal

Remove the Web3 wallet product-mode entry from the HIPPO mobile Sites prototype
and simplify the Home header so the exchange experience starts directly with
search and account information.

## Requirements

- Remove the `交易所 / Web3 钱包` mode switch from Home.
- Remove Web3 wallet copy, click behavior, and unused mode-switch styles.
- Keep exchange wallet capabilities such as Assets, Deposit, Withdraw, and
  Transfer unchanged.
- Preserve the six-column bottom navigation and all Spot/Contract behavior.
- Keep Lucide icons, no-emoji, responsive, and public Sites requirements.

## Acceptance Criteria

- [x] No `Web3` or `产品模式` text remains in UI source or rendered HTML.
- [x] Search is the first Home control below the global header.
- [x] Home spacing remains balanced at 390px with no horizontal overflow.
- [x] Six-column navigation and Spot/Contract tests continue to pass.
- [x] Lint, build/tests, browser checks, and public Sites deployment succeed.

## Out Of Scope

- Removing exchange wallet/account features.
- Modifying production `mobile/src`.
- Changing Spot, Contract, Assets, or Profile workflows.

## Technical Notes

- UI source: `mobile/sites-prototype/app/page.tsx`.
- Styling: `mobile/sites-prototype/app/globals.css`.
- Regression tests: `mobile/sites-prototype/tests/rendered-html.test.mjs`.
