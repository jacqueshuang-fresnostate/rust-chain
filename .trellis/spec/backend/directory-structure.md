# Directory Structure

> How backend code is organized in this project.

---

## Overview

Backend business code is organized by bounded context under `src/modules/`.
Each bounded context should move toward a DDD-inspired layout. The goal is not
to add ceremony; the goal is to stop large route files from mixing transport,
SQL, wallet mutations, validation, event publishing, repository contracts,
business services, and domain rules.

Existing modules may migrate incrementally. During migration, keep public route
functions stable so `src/lib.rs` does not need a large router rewrite for every
internal extraction.

When splitting a legacy `mod.rs` or `<context>.rs`, prefer keeping the root
module as a compatibility façade that re-exports the moved symbols. This lets
callers keep stable paths such as `modules::market::ValidatedMarketSymbol` or
`modules::market::adapters::*` while the actual implementation moves into the
proper DDD layer file.

---

## Directory Layout

```
src/
├── modules/
│   ├── <context>.rs                 # legacy single-file context entry, if present
│   └── <context>/
│       ├── mod.rs                   # context entry for directory-backed modules
│       ├── domain.rs                # entities, value objects, invariants
│       ├── repository.rs            # repository traits and persistence contracts
│       ├── service.rs               # reusable business services / domain services
│       ├── application.rs           # use cases / transaction orchestration
│       ├── infrastructure.rs        # SQLx, Redis, provider clients, repositories
│       ├── presentation.rs          # HTTP DTO mapping helpers
│       └── routes.rs                # Axum routing and thin handlers
├── infra/                           # cross-context infrastructure
├── workers/                         # background jobs
└── architecture.rs                  # shared architecture markers and docs

tests/
├── *_routes.rs                      # integration-style route tests
├── *_services.rs                    # public service/domain tests
└── unit_src/                        # unit tests extracted from source modules
```

---

## Module Organization

### Layer Responsibilities

| Layer | Responsibility | Must Not |
|-------|----------------|----------|
| `domain` | Business entities, value objects, pure rules, invariant checks | Know Axum, SQLx row types, Redis, HTTP DTOs |
| `repository` | Repository traits, persistence contracts, domain-facing read/write boundaries | Execute SQLx/Redis directly or import Axum DTOs |
| `service` | Reusable business services and cross-entity business rules | Parse HTTP requests, own SQL, or render API responses |
| `application` | Use cases, transaction boundaries, coordination between repositories/services | Parse HTTP requests or render API responses |
| `infrastructure` | SQLx queries, Redis/cache access, third-party providers, concrete repository implementations | Own business decisions that should be testable without I/O |
| `presentation` | Request/response DTO helpers, API shape mapping, transport-specific normalization | Mutate wallets or decide financial/risk outcomes |
| `routes` | Router registration, auth extraction, call one application use case, return JSON | Contain long business workflows or raw SQL unless not yet migrated |

### Comments

Use Chinese comments for non-obvious business rules and risk-sensitive
invariants, especially around wallets, ledgers, settlement, liquidation,
idempotency, permission boundaries, and transaction ordering.

Do not add comments that merely restate Rust syntax.

Correct:

```rust
// 钱包扣减和流水写入必须在同一个事务中完成，避免余额变化没有审计记录。
```

Wrong:

```rust
// 设置变量 amount。
```

### Tests

Do not place full test bodies inside production modules. Put tests in standalone
files:

- Prefer public integration/domain tests under `tests/*.rs`.
- If a test must access private helpers during an incremental migration, put the
  body under `tests/unit_src/*.rs` and include it from the production module
  with a tiny `#[cfg(test)] mod tests;` declaration.
- When a private helper becomes stable domain behavior, prefer moving it into
  `domain`/`application` with `pub(crate)` visibility and test it from a normal
  `tests/*.rs` file.

---

## Naming Conventions

Files use Rust snake_case. Bounded-context layer files use these exact names:

- `domain.rs`
- `repository.rs`
- `service.rs`
- `application.rs`
- `infrastructure.rs`
- `presentation.rs`
- `routes.rs`

Do not create alternative names such as `repo.rs`, `storage.rs`, `manager.rs`,
or `controller.rs` for new code unless an existing context already has that
file and the task explicitly migrates it.

---

## Examples

- `src/modules/spot/mod.rs` already contains domain-ish order types and can be
  used as a migration source for a future cleaner `spot/domain.rs`.
- `tests/unit_src/` contains extracted unit tests that still need private
  helper access during migration.

Compatibility façade example:

```rust
pub mod domain;
pub mod infrastructure;

pub use domain::{ValidatedMarketSymbol, MarketTickerSnapshot};
pub use infrastructure::{market_ticker_redis_key, adapters};
```
