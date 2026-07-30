# Database Guidelines

> Database patterns and conventions for this project.

---

## Overview

<!--
Document your project's database conventions here.

Questions to answer:
- What ORM/query library do you use?
- How are migrations managed?
- What are the naming conventions for tables/columns?
- How do you handle transactions?
-->

(To be filled by the team)

---

## Query Patterns

<!-- How should queries be written? Batch operations? -->

(To be filled by the team)

---

## Migrations

<!-- How to create and run migrations -->

### Scenario: Immutable SQLx Migrations

#### 1. Scope / Trigger

- Trigger: any change to a migration file under `migrations/`, especially after the migration may already have been run in a local, staging, or production database.
- Rule: once SQLx has applied a migration version, do not edit that migration file's contents. SQLx stores the applied checksum and will fail with `migration <version> was previously applied but has been modified`.

#### 2. Signatures

- Existing migration signature: `migrations/NNNN_description.sql`
- Follow-up migration signature: `migrations/NNNN+1_new_description.sql`
- Validation command: `sqlx migrate run`

#### 3. Contracts

- Existing applied migrations keep their original SQL exactly.
- Schema changes after an applied migration must be represented as a new migration file.
- When adding a `NOT NULL` column to an existing table, use a three-step migration:
  1. `ALTER TABLE ... ADD COLUMN ... NULL`
  2. `UPDATE ... SET new_column = ... WHERE new_column IS NULL`
  3. `ALTER TABLE ... MODIFY COLUMN ... NOT NULL`

#### 4. Validation & Error Matrix

- Edited applied migration -> `sqlx migrate run` fails with checksum mismatch.
- New migration with duplicate version -> SQLx migration ordering/conflict failure.
- `NOT NULL` column added without backfill -> migration fails or existing rows violate the new constraint.

#### 5. Good/Base/Bad Cases

- Good: migration 71 is already applied; create `0072_add_column.sql` to alter the table and backfill data.
- Base: brand-new migration not applied anywhere can still be edited before first use.
- Bad: migration 71 is already applied, then `0071_user_loans.sql` is edited to add a column.

#### 6. Tests Required

- Run `sqlx migrate run` once to apply the new migration.
- Run `sqlx migrate run` again to confirm the migration set is clean and idempotent from SQLx's checksum perspective.
- Run a whitespace/conflict-marker check for new untracked migration files because `git diff --check` does not cover untracked files.

#### 7. Wrong vs Correct

Wrong:

```sql
-- 0071_user_loans.sql was already applied, but is edited later:
ALTER TABLE loan_products ADD COLUMN name_json JSON NOT NULL;
```

Correct:

```sql
-- 0072_loan_product_name_json.sql
ALTER TABLE loan_products ADD COLUMN name_json JSON NULL;
UPDATE loan_products SET name_json = JSON_OBJECT('version', 1) WHERE name_json IS NULL;
ALTER TABLE loan_products MODIFY COLUMN name_json JSON NOT NULL;
```

### Scenario: Repair Binary Metadata Drift for Rust Strings

#### 1. Scope / Trigger

- Trigger: SQLx reports that Rust `String`/SQL `VARCHAR` is incompatible with
  MySQL `VARBINARY`, or `information_schema.COLUMNS` shows binary string
  metadata for a field owned as text by the application contract.
- Repair schema metadata with a new migration. Do not weaken the Rust model to
  `Vec<u8>` and do not add per-query casts that leave the drift in place.

#### 2. Signatures

- Text metadata: `VARCHAR(length) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci`
- Regression fixture: create the expected text table, drift the affected
  columns to both `VARBINARY` and binary-collated `utf8mb4` `VARCHAR`, then
  execute the exact migration file through `include_str!`.
- Auth credential metadata:
  - `users|admin_users|agent_admin_users.password_hash VARCHAR(255) ... NOT NULL`
  - `users|admin_users|agent_admin_users.status VARCHAR(32) ... NOT NULL DEFAULT 'active'`
- Auth credential regression boundary: call the production
  `MySqlAuthRepository` methods rather than copying their SQL into the test.

#### 3. Contracts

- A metadata repair must explicitly restate each affected column's length,
  character set, non-binary collation, nullability, and default.
- Existing bytes must remain valid UTF-8 application text and survive the
  `VARBINARY` to `VARCHAR` conversion unchanged.
- Nullable text remains nullable; required text remains `NOT NULL`.
- The same `ALTER TABLE ... MODIFY COLUMN` migration must also succeed against
  an already-correct `VARCHAR` schema.
- Credential repairs must preserve each exact Argon2 hash and account status;
  they must not reset passwords, reactivate accounts, or change lookup logic.

#### 4. Validation & Error Matrix

- `VARBINARY` selected into Rust `String` -> SQLx `ColumnDecode` error.
- Binary bytes that are not valid in the target character set -> migration
  failure; investigate the data instead of silently replacing it.
- Omitted `DEFAULT` or nullability in `MODIFY COLUMN` -> MySQL may change the
  column contract even when the type conversion succeeds.
- MySQL 8.4 may expose a `VARBINARY` default through
  `information_schema.COLUMNS` as hexadecimal (for example, `'active'` as
  `0x616374697665`); assert the equivalent binary value before repair and the
  canonical text default after repair.

#### 5. Good/Base/Bad Cases

- Good: add an immutable follow-up migration that explicitly restores text
  metadata and preserves every application-owned value.
- Base: the same repair succeeds when the columns are already correct.
- Bad: change Rust fields to `Vec<u8>`, cast individual queries, or replace
  stored text to make decoding pass.

#### 6. Tests Required

- Use a real isolated MySQL database, not a mocked row decoder.
- Assert the pre-migration SQLx `String` decode failure for both `VARBINARY`
  and binary-collated `VARCHAR` metadata.
- Execute the migration with `sqlx::raw_sql(include_str!(...))`.
- Assert post-migration `String`/`Option<String>` decoding, value preservation,
  defaults, lengths, nullability, `utf8mb4`, and the explicit non-binary
  collation.
- Repeat the repair from a binary-collated `utf8mb4` `VARCHAR` fixture,
  including byte preservation and the pre-repair decode failure.
- Execute the same SQL once more after the columns are correct.
- Credential regression tests must exercise
  `find_user_by_email`, `find_user_by_phone`, `find_user_by_username`,
  `find_admin_by_username`, and `find_agent_by_username`, assert the
  pre-repair `ColumnDecode` points to index `1` (`password_hash`), and verify
  the exact hashes with Argon2 after repair.

#### 7. Wrong vs Correct

Wrong:

```rust
// This bypasses the production repository and can drift from the failing query.
sqlx::query_as::<_, (u64, Vec<u8>, Vec<u8>)>(
    "SELECT id, password_hash, status FROM admin_users WHERE username = ?",
);
```

Correct:

```rust
let repository = MySqlAuthRepository::new(pool.clone());
let error = repository
    .find_admin_by_username(username)
    .await
    .expect_err("binary metadata must fail before repair");
assert!(matches!(
    error,
    AppError::Database(sqlx::Error::ColumnDecode { .. })
));

sqlx::raw_sql(include_str!(
    "../migrations/0098_auth_credential_text_metadata.sql"
))
.execute(&pool)
.await?;
```

---

## Naming Conventions

<!-- Table names, column names, index names -->

(To be filled by the team)

---

## Common Mistakes

<!-- Database-related mistakes your team has made -->

(To be filled by the team)
