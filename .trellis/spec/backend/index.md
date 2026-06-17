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
| [Earn Product Contracts](./earn-products.md) | Earn product fee snapshots and redemption settlement contracts | Active |
| [Seconds Contract Contracts](./seconds-contracts.md) | Seconds product cycle, order duration, and PC/admin payload contracts | Active |
| [Wallet Amount Precision Contract](./wallet-amount-precision.md) | Asset precision rules for calculated wallet balances and ledger snapshots | Active |
| [Order Identifier Display Contract](./order-identifiers.md) | User-visible order numbers and internal ID separation across admin and PC UI | Active |
| [Prediction Market Contracts](./prediction-markets.md) | Polymarket-sourced prediction market sync, quote, wallet, settlement, and PC/admin display contracts | Active |

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
