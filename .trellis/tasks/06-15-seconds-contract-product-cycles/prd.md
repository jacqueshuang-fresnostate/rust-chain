# 秒合约产品多周期配置

## Background

Seconds contract products currently expose a single duration, payout rate, minimum stake, and maximum stake on the product itself. Product operations now require one trading pair to support multiple selectable periods, where every period can have its own odds and stake range.

## Requirements

- A seconds contract product can contain multiple cycle configurations.
- Each cycle contains `duration_seconds`, `payout_rate`, `min_stake`, and optional `max_stake`.
- The backend create and update APIs accept cycle arrays while preserving backward compatibility with existing single-cycle payloads.
- Product list/detail responses include the cycle array for admin and PC consumers.
- Order creation validates the requested duration against the selected product cycle and uses that cycle's payout and stake limits.
- Admin product create/edit UI lets operators add, remove, and edit multiple cycles in one form.
- Admin list/table displays cycle information clearly without product ID or trading pair ID clutter.
- Existing seconds contract behavior and settlement remain compatible with existing orders.

## Acceptance

- Database migration adds a child table for seconds product cycles and backfills existing products.
- Creating/updating a product with 60s, 120s, and 180s cycles persists all independent rates and limits.
- Creating an order for an unsupported duration is rejected.
- Creating an order outside the selected cycle's stake range is rejected.
- Existing single-cycle payloads still work.
- Backend tests and admin frontend tests cover the new cycle shape.
