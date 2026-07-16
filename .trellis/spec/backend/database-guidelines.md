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

---

## Naming Conventions

<!-- Table names, column names, index names -->

(To be filled by the team)

---

## Common Mistakes

<!-- Database-related mistakes your team has made -->

(To be filled by the team)
