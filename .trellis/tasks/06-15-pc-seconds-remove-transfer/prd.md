# PC端移除秒合约页面划转入口

## Goal

Remove the transfer entry and transfer modal from the PC seconds contract trading page.

## Background

The current `SecondOptions.vue` page shows a transfer icon next to the USDT balance in the trading panel. Clicking it opens a transfer modal that calls the seconds store transfer API. The requested product behavior is that the seconds contract page should no longer expose transfer.

## Requirements

- Remove the transfer button from the seconds contract trading panel.
- Remove the transfer modal state, direction switching, amount input, submit handler, and store transfer call from the page.
- Keep the USDT balance display in the trading panel.
- Keep order placement, cycles, current positions, history positions, and settlement result behavior unchanged.

## Out of Scope

- Removing generic transfer APIs, store methods, or i18n keys used by other pages.
- Backend account transfer behavior.
- Other PC pages or admin pages.

## Acceptance Criteria

- `pc/src/views/SecondOptions.vue` no longer contains page-level transfer modal code or `store.transfer(...)` usage.
- The trading panel still displays the user USDT balance.
- A regression test covers that the seconds page does not expose transfer actions.
- PC type-check passes.
