# N01 Shared Numeric Boundaries

## Implemented

- `src/numeric.rs` validates exact DECIMAL capacity without rounding, using
  coefficient/exponent arithmetic instead of expanding pathological exponents.
  The money envelope is 38,18; narrower fields additionally validate their
  actual schema. Trailing zeroes do not consume effective fractional scale.
- Raw decimal text is limited to 256 ASCII bytes and exponent -256..256
  before constructing BigDecimal. JSON numeric literals retain their spelling
  through the existing serde_json arbitrary_precision feature.
- 160 typed decimal request/config fields use the required, optional,
  optional-list or double-optional PATCH deserializer. The final two fields are
  Margin leverage lists, whose elements must not bypass scalar bounds.
  Response/cache models are not mechanically
  converted into request types. `tests/numeric_contract.rs` inventories these
  fields with Syn and rejects missing adapters.
- Market provider and synthetic-snapshot decimal ingress use the same bounded
  parser. Recovery input uses the parser as well; integer millisecond alignment
  replaces floating-point timestamp arithmetic, and gap enumeration/deadline
  subtraction use checked date arithmetic.
- Configuration validates timer representability. JWT/refresh/session issuance
  validates positive TTL before side effects and uses checked expiry. The
  expiry must fit the existing TIMESTAMP(6) columns, not merely Chrono's larger
  range. Generic Unix-millisecond serialization remains a separate contract.

## File Inventory

- Shared: `src/{numeric,time,config,lib}.rs`, `src/infra/auth.rs`,
  `src/modules/auth/{mod,service}.rs`.
- DTOs: BigDecimal fields under presentation files in admin, agent, convert,
  earn, loan, margin, new_coin, prediction, quick_recharge, seconds_contract,
  spot and wallet; nested wallet fee/withdrawal policy and default market config.
- Market: `src/modules/market/infrastructure/adapters/provider.rs`,
  `src/modules/market/synthetic_snapshot.rs`, `src/workers/kline_recovery.rs`.
- Tests: `tests/numeric_contract.rs`, `tests/market_adapters.rs`,
  `tests/unit_src/src_{numeric,time,config}_tests.rs`,
  `tests/unit_src/src_modules_auth_mod_tests.rs`,
  `tests/unit_src/src_workers_kline_recovery_tests.rs`.
- Spec: `.trellis/spec/backend/numeric-safety.md` and backend index.

## Validation

- Shared numeric/time/config/JWT selection: 17 passed.
- Expanded selection including recovery: 33 passed.
- Subsequent full `cargo test --lib --quiet`: 500 passed, including the actual
  TIMESTAMP storage-bound tests. Local HTTP mocks required sandbox escalation
  and clearing proxy environment variables. Optional database branches are not
  claimed as database evidence.
- `cargo test --test numeric_contract --test market_adapters`: 1 + 6 passed.
- Architecture 11, documentation 1, numeric contract 1, OpenAPI 10 and market
  adapters 6 passed in the joint integration-test gate.
- Final `cargo test --all-targets --no-fail-fast --quiet -- --test-threads=1`
  passed, including 501 library tests and the additional list deserialization.
  DATABASE_URL/Redis/Mongo were deliberately unset; optional external-service
  branches are not counted as real database verification.
- Final `numeric_contract` reports exactly 160 checked request/config fields.
- Global rustfmt, strict all-target/all-feature Clippy and diff check passed.
- Older default/follow generator tests now explicitly assert rejection during
  deserialization for huge exponents/19-place input, while retaining original
  business-validation checks. A Margin SQL contract test follows the actual
  qualified columns. One Spot test's no-environment path now matches the
  existing optional fixture contract; its full trigger/replay scenario was
  separately rerun against isolated MySQL/Redis and passed.

## Boundaries

- No automatic string-ID wire migration. Clients must refuse unsafe numeric
  IDs; supporting all u64 IDs requires coordinated API evolution.
- This does not migrate existing TIMESTAMP columns past their 2038 limit.
  New generated deadlines outside storage range fail explicitly.
- No rewrite of historical balances, no policy activation and no production
  operation. Source values are rejected, never silently clamped to capacity.
- DTO AST coverage is a regression guard, not proof for untyped JSON fields,
  database-derived computations or all arithmetic. Those require the N02-N06
  product inventory and behavioral tests.
