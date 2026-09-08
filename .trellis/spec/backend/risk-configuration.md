# Risk Configuration and Spot Target Identity

## 1. Scope / Trigger

Apply when changing Admin risk create/status commands, risk JSON parsing or spot
guard scopes. This contract does not change rule priority, operation amount units,
legacy runtime reads or withdrawal rate-limit infrastructure.

## 2. Signatures

- `validate_risk_rule_config(&Value) -> Result<(), &'static str>` in risk service.
- Admin `POST /admin/api/v1/risk/rules`; `PATCH /admin/api/v1/risk/rules/:id/status`.
- Spot `load_spot_pair_db_id_by_symbol(pool, canonical_symbol) -> AppResult<u64>`.
- Persisted `risk_rules.config_json` is not rewritten by validation.

## 3. Contracts

Create validates before acquiring a pool, including disabled creates. Enable
validates the locked row's JSON before status/audit writes in the same transaction.
Disable accepts legacy invalid JSON unchanged; reasons remain optional. Unknown
extension fields and `{}` remain valid, and operation names are not enumerated.

Admin pair targets are numeric IDs. New spot orders match both their authoritative
numeric ID and legacy canonical symbol. Resolve the ID via exact `symbol = ?`,
never `symbol = ? OR id = ?`; rules matching multiple scopes still evaluate once.
Successful idempotent order replay precedes new risk evaluation. Missing pair/DB
failure aborts without order/wallet writes. Numeric-only legacy symbol targets
share the stored namespace with IDs; no implicit data migration is claimed.

## 4. Validation & Error Matrix

| Field present | Accepted | Rejected |
|---|---|---|
| Whole config | JSON object, including empty | null, array, scalar |
| operations / blocked_operations | Array of nonempty strings with no outer whitespace; empty array valid | null, mixed/non-string items, blank/padded names |
| max_amount | Nonnegative decimal string/number accepted by existing BigDecimal parser | null, malformed, negative; no rounding/coercion |
| max_requests / max_price_deviation_bps | Existing u32 string/integer parser, including 0 and u32 max | null, fractional JSON number, negative, overflow |
| window_seconds | Positive u32 string/integer | explicit 0, null, negative, fraction, overflow |
| Optional numeric field absent | Existing runtime default; window is 60 seconds | — |

Validation returns field-specific 400 errors; Admin maps the exact messages to
Chinese. Runtime parsing of already stored rows remains unchanged. Rule and audit
writes roll back together if audit insertion fails.

## 5. Good / Base / Bad Cases

- Good: creating a numeric-pair limit rejects only that pair without freezing funds.
- Base: unknown extension fields survive create, audit and status changes.
- Bad: accepting `window_seconds: 0` but silently running a 60-second window;
  refusing to disable a legacy malformed config; rerunning new risk on a receipt.

## 6. Tests Required

Run pure schema/runtime-parity and multi-scope tests, then the real-MySQL
`admin_routes risk_validation::` suite (5 tests), existing Admin risk CRUD and
spot price-deviation test with disposable Redis. Assert 400/no side effects,
locked enable/legacy disable, exact-ID versus numeric-symbol collision, symbol
compatibility, successful replay and audit-failure atomicity. Trigger DDL uses
`sqlx::raw_sql`, not the MySQL prepared statement protocol.

## 7. Wrong vs Correct

```rust
// Wrong: the Admin's numeric target never appears in spot guard scopes.
let scopes = vec![RiskScope::new("pair", canonical_symbol)];
// Correct: keep the legacy symbol and add the exact authoritative DB identity.
let scopes = vec![RiskScope::new("pair", canonical_symbol),
                  RiskScope::new("pair", pair_db_id.to_string())];
```
