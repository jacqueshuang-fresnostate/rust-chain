# Quality Guidelines

> Code quality standards for backend development.

---

## Overview

Backend changes must keep financial behavior explicit, testable, and
recoverable. Prefer small, stable migrations over broad rewrites that change API
behavior and structure at the same time.

---

## Forbidden Patterns

- Full test bodies inside production modules. Use `tests/*.rs` or
  `tests/unit_src/*.rs` instead.
- New route handlers that contain raw SQL plus business decisions plus response
  mapping in one long function.
- New SQLx/Redis code in `domain`, `repository`, `service`, `presentation`, or
  route handlers when a context has already migrated that concern into
  `infrastructure`.
- Wallet/ledger mutations outside a transaction when the operation must be
  auditable.
- Mechanical comments that repeat the code instead of explaining business
  intent.
- Editing applied migrations. Add a new migration instead.

---

## Required Patterns

- Keep HTTP handlers thin: auth/extract input, call an application use case,
  map the result.
- Put pure business rules in `domain` where they can be tested without I/O.
- Put repository traits and domain-facing persistence contracts in
  `repository`.
- Put reusable business services in `service`, then let `application`
  orchestrate them into use cases and transaction boundaries.
- Put SQLx/Redis/provider code in `infrastructure`.
- Use Chinese comments for non-obvious domain invariants and transaction
  ordering.
- Preserve public API payloads unless the task explicitly changes the contract.

---

## Testing Requirements

- Run `cargo fmt --manifest-path Cargo.toml --check` before reporting backend
  completion.
- Run `cargo check --manifest-path Cargo.toml --all-targets` for architecture
  refactors.
- Run the closest context-specific tests for changed modules.
- If moving test code from production files, prove no `#[cfg(test)] mod tests {`
  blocks remain in `src/`.

---

## Code Review Checklist

- Does each changed module follow the DDD layer responsibility table?
- Did route handlers become thinner rather than more complex?
- Are repository interfaces/contracts separated from concrete infrastructure
  implementations where the context has migrated that boundary?
- Are Chinese comments placed on business rules, not syntax?
- Are tests in standalone files?
- Did the change avoid unrelated API/schema behavior changes?
