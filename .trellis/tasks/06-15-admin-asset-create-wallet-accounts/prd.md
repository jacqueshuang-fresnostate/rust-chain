# Admin Asset Creation Wallet Account Backfill

## Problem

When an admin creates a new asset, existing users do not receive matching `wallet_accounts` rows. The PC wallet account endpoint reads real `wallet_accounts` records, so the newly created asset is missing from user asset lists until another operation creates the row.

## Goals

- Creating an asset through `POST /admin/api/v1/assets` must create zero-balance wallet accounts for all existing users in the same transaction.
- Existing wallet accounts and balances must be preserved if a duplicate row already exists.
- The user wallet account list should be able to return the newly created asset because a real `wallet_accounts` row exists.
- Asset update and delete behavior should remain unchanged; an asset with wallet account references is still protected by existing reference checks.

## Non-Goals

- No schema migration.
- No frontend layout changes.
- No change to wallet account listing filters or virtual `include_empty` admin behavior.

## Acceptance Criteria

- Admin asset creation inserts `wallet_accounts` rows with `available = 0`, `frozen = 0`, and `locked = 0` for existing users.
- The insert is idempotent and does not mutate existing wallet account balances.
- Tests cover that users created before the asset receive wallet account rows after admin asset creation.
- Existing asset audit and validation behavior remains covered.
