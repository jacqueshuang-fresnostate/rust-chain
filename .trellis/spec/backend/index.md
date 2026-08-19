# Backend Development Guidelines

> Best practices for backend development in this project.

---

## Overview

This directory contains the active, project-specific backend contracts. Read
the structure and quality guides before changing module boundaries, then read
the feature contract for the business context being modified.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | Optional DDD layers, dependency direction, module/test layout | Active |
| [Database Guidelines](./database-guidelines.md) | Immutable migrations and MySQL text-metadata repair; general query guidance remains incomplete | Partial |
| [Error Handling](./error-handling.md) | External-provider error mapping; general error taxonomy remains incomplete | Partial |
| [Quality Guidelines](./quality-guidelines.md) | Executable architecture/documentation guards, adapter-independent services, hotspot limits, and validation | Active |
| [Logging Guidelines](./logging-guidelines.md) | Structured logging and log-level guidance | Draft |
| [Public News Contract](./public-news-contract.md) | Public news API locale, response, and rich-text rendering contract | Active |
| [Spot Order Contracts](./spot-orders.md) | Spot market order reference-price protection, Redis ticker execution, and wallet reservation contract | Active |
| [Margin Trading Actions](./margin-trading-actions.md) | Margin ticker, wallet-scope settlement, transfer idempotency, bulk actions, and supported-mode contracts | Active |
| [Earn Product Contracts](./earn-products.md) | Earn product fee snapshots and redemption settlement contracts | Active |
| [Loan Product Contracts](./loan-products.md) | Admin loan-product filtering, enum validation, and paginated total contracts | Active |
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
| [Agent-Routed Online Support Contracts](./online-support.md) | Durable support messages, exact owning-agent isolation, admin fallback, reassignment, unread, pagination, and refresh contracts | Active |
| [Synthetic Market and K-line Recovery Contracts](./synthetic-market-kline.md) | Deterministic strategy ticker/K-line generation, active-version leases, authoritative 1m aggregation, and admin-only historical recovery | Active |
| [New Coin Mobile Contract](./new-coin-mobile-contract.md) | Public project purchase-pair exposure and authenticated mobile lifecycle actions | Active |
| [Container Delivery Contract](./container-delivery.md) | Backend image entrypoints, GHCR publication, migration gating, and Compose runtime contracts | Active |
| [Market Favorites and Asset Logo Contract](./market-favorites.md) | Authenticated favorites and backend-owned market/convert/wallet logo metadata | Active |

---

## Pre-Development Checklist

1. Read [Directory Structure](./directory-structure.md) before adding, deleting,
   or moving a DDD layer or compatibility re-export.
2. Read [Quality Guidelines](./quality-guidelines.md) before changing routes,
   tests, architecture guards, or legacy exceptions.
3. Read the relevant feature contract for authentication, wallet, trading,
   settlement, or public API work.
4. Search all callers and mirrored declarations before deleting a layer or
   moving a public symbol.
5. Run the closest tests plus the validation commands required by the quality
   guide after the final edit.

---

**Language**: All documentation should be written in **English**.
