# Directory Structure

> How backend code is organized in this project.

---

## Overview

Backend business code is organized by bounded context under `src/modules/`.
The project uses DDD responsibilities to prevent transport, persistence,
provider I/O, and business rules from collapsing into large route handlers.
DDD is a responsibility model, not a six-file checklist: a context declares
only the layers it currently needs.

Existing contexts migrate incrementally. Keep public routes and compatibility
re-exports stable while moving implementation behind the correct boundary.
When splitting a legacy `mod.rs` or `<context>.rs`, the root module may remain a
facade for real callers, but it must not declare empty modules merely to make a
directory look complete.

---

## Directory Layout

```text
src/
├── modules/
│   ├── <context>.rs                 # legacy single-file context entry, if present
│   └── <context>/
│       ├── mod.rs                   # declares only layers with real responsibilities
│       ├── domain.rs                # optional: entities, value objects, invariants
│       ├── repository.rs            # optional: persistence ports/contracts
│       ├── service.rs               # optional: reusable domain/business services
│       ├── application.rs           # optional: use cases and transaction orchestration
│       ├── infrastructure.rs        # optional: SQLx, Redis, Mongo, provider adapters
│       ├── presentation.rs          # optional: transport DTOs and mapping
│       └── routes.rs                # optional: Axum router and thin handlers
├── infra/                           # cross-context infrastructure
├── workers/                         # scheduling, batching, retries, metrics
└── architecture.rs                  # layer traits implemented by real responsibility types

tests/
├── *_routes.rs                      # integration-style route tests
├── *_services.rs                    # public service/domain tests
├── backend_architecture.rs          # executable dependency and structure guard
└── unit_src/                        # unit bodies extracted from production modules
```

Omitting a layer is valid. Declaring a layer whose only content is comments or
a `*LayerMarker` is invalid. Add the layer when a real rule, contract, use case,
adapter, or DTO exists; delete the file and its `mod.rs` declaration when that
responsibility does not exist.

---

## Layer Responsibilities And Direction

| Layer | Responsibility | Forbidden dependency / ownership |
|---|---|---|
| `domain` | Entities, value objects, pure rules, invariant checks | Axum, SQLx, Redis, MongoDB, Reqwest, `presentation` |
| `repository` | Domain-facing persistence traits and read/write contracts | Concrete SQL queries or `QueryBuilder`; HTTP DTO ownership |
| `service` | Reusable cross-entity business services | `application`, `routes`, transport parsing, concrete persistence |
| `application` | Use cases, transaction/lock order, repository/service coordination | HTTP extraction or router registration |
| `infrastructure` | SQLx/Redis/Mongo access, third-party provider clients, concrete adapters | Transport policy or business decisions that can be pure |
| `presentation` | Request/response DTOs, header/multipart normalization, API mapping | Wallet mutation, settlement/risk decisions, provider I/O |
| `routes` | Router registration, auth/input extraction, one application call, response return | Raw SQL, transaction ownership, direct `infrastructure`, Reqwest/provider workflows |

The intended flow is transport → application → service/domain/repository
ports → infrastructure adapters. Compatibility facades may re-export real
symbols, but must not create reverse dependencies.

### Compatibility Façades And File Size

Compatibility façades may preserve stable import paths while implementation is
split into responsibility-focused child modules. A façade must only declare and
re-export real implementation; it must not duplicate SQL, transactions, or
business policy from its children.

No production Rust file under `src/` may exceed 2,000 lines. Split before the
limit is reached, preferably keeping a focused implementation module near or
below 1,200 lines. File size is not the architecture goal by itself: boundaries
must follow real responsibilities such as cache, persistence, provider adapter,
order repository, settlement, account ledger, deposit, or withdrawal.

Architecture dependency exceptions have been eliminated. New broad or
file-specific allowlists are forbidden; fix the boundary instead. In
particular, events operations use application use cases and presentation
responses, while auth Turnstile policy lives in application/domain and
Siteverify I/O lives in infrastructure.

---

## Module Declarations And Markers

- Search for callers before deleting a layer or compatibility re-export.
- Delete both a pure shell file and its `mod.rs` declaration in the same change.
- Any identifier ending in `LayerMarker` is forbidden; do not use a struct,
  alias, import, or re-export to claim that a layer exists.
- Real domain, repository, service, application, infrastructure, and
  presentation types may continue implementing the matching traits from
  `src/architecture.rs`.
- Layer comments do not count as implementation. The architecture guard looks
  for real functions, types, traits, constants, or statics.

Compatibility facade example:

```rust
pub mod domain;
pub mod infrastructure;

pub use domain::{MarketTickerSnapshot, ValidatedMarketSymbol};
pub use infrastructure::{adapters, market_ticker_redis_key};
```

If `repository` has no contract, omit it instead of adding
`RepositoryLayerMarker`.

---

## Comments

Use Chinese comments for non-obvious business rules and risk-sensitive
invariants, especially wallets, ledgers, settlement, liquidation, idempotency,
authorization boundaries, external side effects, and transaction/lock order.
Public risk-sensitive functions should use Chinese `///` contracts that state
responsibility, preconditions, replay behavior, financial invariants, and side
effects where applicable.

Correct:

```rust
/// 扣减余额与追加流水共用同一事务；幂等重放不得产生第二笔流水。
```

Wrong:

```rust
// Set amount and call the next function.
```

---

## Tests

Do not place test bodies inside production modules.

- Prefer public integration/domain tests under `tests/*.rs`.
- A test that needs private access lives in `tests/unit_src/*.rs`; production
  code keeps only adjacent `#[cfg(test)]`, `#[path = "...tests/unit_src/..."]`,
  and `mod tests;` declarations.
- When a private helper becomes stable domain behavior, expose the smallest
  suitable `pub(crate)` boundary and test it from a normal integration file.
- `tests/backend_architecture.rs` must keep the optional-layer, no-empty-shell,
  no-marker, test-location, dependency-direction, and 2,000-line contracts
  executable.

---

## Naming Conventions

Files use Rust snake_case. When a context needs a standard layer, use these
exact names: `domain.rs`, `repository.rs`, `service.rs`, `application.rs`,
`infrastructure.rs`, `presentation.rs`, and `routes.rs`. Do not introduce
synonyms such as `repo.rs`, `storage.rs`, `manager.rs`, or `controller.rs`
unless an explicit migration requires compatibility with an existing module.
