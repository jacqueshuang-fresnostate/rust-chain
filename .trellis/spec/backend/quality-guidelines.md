# Quality Guidelines

> Code quality and executable architecture standards for backend development.

---

## Overview

Backend changes must keep financial and authentication behavior explicit,
testable, and recoverable. Prefer focused migrations that preserve routes,
JSON, SQL semantics, ledger metadata, provider payloads, and transaction
behavior. Directory shape is not evidence of architecture quality; executable
responsibility and dependency guards are.

---

## Required Patterns

- Keep handlers thin: authenticate/extract input, create a transport context,
  call an application use case, and return its presentation response.
- Put pure business rules in `domain` so they run without I/O SDKs.
- Create `repository` only when a real domain-facing persistence contract
  exists. Concrete SQL belongs in `infrastructure`.
- Put reusable business rules in `service`; application owns use-case and
  transaction orchestration.
- Put SQLx, Redis, MongoDB, and provider clients in `infrastructure`.
- Put transport DTOs and header/multipart normalization in `presentation`.
- Use Chinese `///` contracts for every visible bounded-context responsibility
  in domain, repository, service, application, and infrastructure. Non-trivial
  entries document applicable input, transaction, lock, replay, side-effect,
  and failure semantics; public worker/cross-context infrastructure entries use
  the same standard.
- Preserve externally observable behavior unless the task explicitly changes
  a contract.

---

## Forbidden Patterns

- Full test bodies inside `src/**/*.rs`.
- A DDD layer file containing only comments, imports, or a `*LayerMarker`.
- Any identifier ending in `LayerMarker`; architecture traits are implemented
  only by real responsibility-bearing types.
- Routes containing raw SQL/query builders, `.begin()` transaction ownership,
  direct context `infrastructure`, or Reqwest/provider HTTP workflows.
- Domain importing Axum, SQLx, Redis, MongoDB, Reqwest, or `presentation`.
- Repository executing concrete SQL or owning a `QueryBuilder`.
- Service depending on `application`, `routes`, `AppState`, context
  `infrastructure`, SQLx, Redis, MongoDB, or Reqwest. Inject adapter-neutral
  ports and assemble concrete adapters in application/runtime boundaries.
- Wallet/ledger mutation without the required transaction and audit record.
- Mechanical comments that repeat syntax instead of explaining policy,
  invariants, lock order, replay, and side effects.
- Bulk template documentation reused across unrelated non-trivial entries.
  Within one responsibility file, four or more visible functions of at least
  six lines must not share an identical full doc block; write contracts that
  distinguish the actual input, state transition, failure, and side effects.
- `allow(unused_imports)` or `expect(unused_imports)` in production Rust.
  Remove stale compatibility re-exports; if an import exists only for an
  adjacent standalone test module, gate that exact import with `cfg(test)`.
- Editing an applied migration; add a new immutable migration instead.

`events/routes.rs` and `auth/routes.rs` must not receive exceptions for the
route boundary. Events list/requeue handlers call application use cases and
return presentation DTOs. Turnstile provider I/O belongs in auth
infrastructure; enable/enforcement policy belongs in domain/application.

---

## Architecture Exception Policy

The dependency guard has no legacy allowlist. Broad wildcards, context-wide
exceptions, and file-specific exceptions are forbidden. When a violation is
found, move transport, orchestration, domain policy, persistence, or provider
I/O to its owning layer instead of weakening the guard.

---

## Architecture Guard Contract

`tests/backend_architecture.rs` enforces:

- standard DDD layers are optional;
- every declared layer contains a real function/type/trait/constant/static;
- pure shell/marker layers remain at zero;
- every `*LayerMarker` definition, import, use, or re-export is rejected;
- source test bodies live under `tests/unit_src`;
- route, domain, repository, and service dependencies follow the rules above;
- production code contains no unused-import warning suppression;
- P1 hotspot compatibility façades and their responsibility-focused child
  implementation files remain below 1,200 lines;
- no production Rust file under `src/` exceeds 2,000 lines;
- bounded-context responsibility docs satisfy the executable Chinese contract
  gate, including the same-file repeated-template check for non-trivial visible
  entries.

When extending the guard, inspect current violations first. Never make a new
failure green with a broad substring or directory whitelist.

---

## Testing Requirements

For backend architecture changes, run at minimum:

```bash
cargo fmt --manifest-path Cargo.toml --all -- --check
cargo test --manifest-path Cargo.toml --test backend_architecture
```

Also run the closest changed context tests. Run
`cargo check --manifest-path Cargo.toml --all-targets` when production module
declarations, imports, or public compatibility re-exports move. If a command is
blocked by a concurrent build lock or an unavailable external service, record
the exact limitation and run the strongest independent check available.

When moving tests, prove both conditions:

```text
src/**/*.rs contains no `mod tests {`
every `#[cfg(test)] mod tests;` in src points to an existing tests/unit_src file
```

---

## Code Review Checklist

- Is every declared layer needed and non-empty?
- Did the change avoid introducing a marker?
- Did routes become thinner, with no SQL/transaction/infrastructure/Reqwest?
- Are domain and repository independent from concrete SDK/persistence work?
- Does service avoid reverse orchestration dependencies?
- Is the architecture guard still free of dependency exceptions?
- Are production files below 2,000 lines and split by real responsibility?
- Do Chinese contracts explain risk rather than syntax?
- Are test bodies standalone?
- Are API, SQL, ledger, provider, and transaction contracts unchanged?
- Were format and architecture tests run after the final edit?
