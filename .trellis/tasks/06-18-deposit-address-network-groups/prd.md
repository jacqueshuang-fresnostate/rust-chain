# Deposit Address Network Groups

## Goal
Align recharge address assignment with network address classes: ETH/Base share EVM addresses, while BTC, Tron, and Solana use their own address classes.

## Scope
- Backend wallet deposit address assignment should search compatible network address groups.
- Admin deposit address pool should reject unsupported asset/network combinations.
- PC recharge network choices should expose BTC for USDT according to the requested flow.
- Add focused tests and progress record.

## Out of Scope
- Database schema migration.
- External chain address generation or on-chain monitoring changes.
