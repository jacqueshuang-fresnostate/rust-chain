# Backend Development Guidelines

> Best practices for backend development in this project.

---

## Overview

This directory contains guidelines for backend development. Fill in each file with your project's specific conventions.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | Module organization and file layout | To fill |
| [Database Guidelines](./database-guidelines.md) | ORM patterns, queries, migrations | To fill |
| [Error Handling](./error-handling.md) | Error types, handling strategies | To fill |
| [Quality Guidelines](./quality-guidelines.md) | Code standards, forbidden patterns | To fill |
| [Logging Guidelines](./logging-guidelines.md) | Structured logging, log levels | To fill |
| [Public News Contract](./public-news-contract.md) | Public news API locale, response, and rich-text rendering contract | Active |
| [Spot Order Contracts](./spot-orders.md) | Spot market order reference-price protection, Redis ticker execution, and wallet reservation contract | Active |
| [Margin Trading Actions](./margin-trading-actions.md) | Margin ticker, wallet-scope settlement, transfer idempotency, bulk actions, and supported-mode contracts | Active |
| [Earn Product Contracts](./earn-products.md) | Earn product fee snapshots and redemption settlement contracts | Active |
| [Seconds Contract Contracts](./seconds-contracts.md) | Seconds product cycle, order duration, and PC/admin payload contracts | Active |
| [Agent Hierarchy Contracts](./agent-hierarchy.md) | Three-level agent organization, subtree scope, and multi-business differential commission contracts | Active |
| [Wallet Amount Precision Contract](./wallet-amount-precision.md) | Asset precision rules for calculated wallet balances and ledger snapshots | Active |
| [Deposit Address Contracts](./deposit-addresses.md) | Configurable deposit network address groups and address-pool allocation contracts | Active |
| [Order Identifier Display Contract](./order-identifiers.md) | User-visible order numbers and internal ID separation across admin and PC UI | Active |
| [Auth Session Contract](./auth-sessions.md) | sa-token-rust Redis session, refresh, extractor, and frontend Bearer compatibility contracts | Active |
| [User Authentication Contracts](./user-authentication.md) | User username configuration, username login policy, and cross-layer login identifier contracts | Active |
| [PC Display and Chart Contracts](./platform-display-and-chart.md) | PC brand configuration, global K-line renderer selection, and TradingView Lightweight Charts data-source boundaries | Active |
| [Prediction Market Contracts](./prediction-markets.md) | Polymarket-sourced prediction market sync, quote, wallet, settlement, and PC/admin display contracts | Active |
| [Realtime WebSocket Contracts](./realtime-websockets.md) | Business-scoped public websocket aliases and PC market subscription routing | Active |
| [New Coin Mobile Contract](./new-coin-mobile-contract.md) | Public project purchase-pair exposure and authenticated mobile lifecycle actions | Active |

---

## How to Fill These Guidelines

For each guideline file:

1. Document your project's **actual conventions** (not ideals)
2. Include **code examples** from your codebase
3. List **forbidden patterns** and why
4. Add **common mistakes** your team has made

The goal is to help AI assistants and new team members understand how YOUR project works.

---

**Language**: All documentation should be written in **English**.
