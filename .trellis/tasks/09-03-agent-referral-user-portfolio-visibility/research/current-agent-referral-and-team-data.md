# Current agent referral and team-data audit

## Existing referral flow

- Agent portal invite codes and user-owned invite codes already share the
  `invite_codes` table and the registration transaction resolves both owner
  types through `prepare_referral_binding_in_tx`.
- Agent portal creation currently emits `AGT` plus a UUIDv7 string, while user
  registration and `/api/v1/referral/my-code` create six-character uppercase
  alphanumeric values.
- Every user gets a user-owned code at registration. Because an `agents` row is
  linked to a normal `users` row, the user account backing an agent still sees
  that personal code in Mobile even after the agent portal creates a separate
  agent-owned code. This is the observable mismatch.
- A user-owned code derives company ownership from the inviter's
  `user_referrals` row. The account backing a root agent commonly has no such
  row, so exposing its personal code is not only confusing but can also be
  unusable for registration.
- The database has one global unique index on `invite_codes.code`; a common
  six-character generator therefore needs duplicate-key retry for both owner
  types. Existing long-form agent codes must remain valid for compatibility.

## Existing authorization model

- Agent authentication resolves `AgentAccessScope` exclusively from the token
  subject and the server-owned `agents.path`.
- Team reads authorize users by joining `user_referrals.root_agent_id` to the
  directly owning agent and requiring its path to equal the current scope path
  or start with `scope.path + '/'`.
- This already gives a parent agent visibility over descendant-owned users and
  excludes parents, siblings and unrelated roots. New financial reads should
  reuse the same predicate and must not accept an agent ID from the client.

## Missing business views

- `/agent/api/v1/users` exposes identity and referral metadata only.
- The agent portal has no route or API for team wallet balances, margin
  positions or seconds-contract orders.
- Admin-only equivalents exist, but they are globally scoped and cannot be
  reused directly without weakening the agent boundary.
- Spot balances live in `wallet_accounts`; margin balances live in
  `margin_wallet_accounts`. A truthful team asset view should identify the
  account scope instead of merging the two balances.
- Margin rows use `opened | closed | canceled | liquidated`. Seconds-contract
  rows include `opened | settled | manual_review`; listing only settled rows
  would omit the user's explicitly requested in-progress orders.

## Recommended implementation

1. Use the same six-character secure generator and bounded duplicate retry for
   new agent-owned invite codes.
2. Make `/api/v1/referral/my-code` resolve the effective code: for a user that
   backs an agent, return the newest active agent-owned code (creating one when
   the active agent has none); for an ordinary user, preserve the current
   user-owned code behavior. Do not invalidate legacy codes.
3. Add user-addressed, read-only agent routes for assets, margin positions and
   seconds-contract orders. Every row query repeats the server-owned subtree
   predicate even after the target-user membership check, so a concurrent
   reassignment cannot expose stale-scope financial rows.
4. Add one agent user detail page reached from the team-user table. Use tabs,
   server pagination and status filters; fetch a tab only when selected. This
   keeps the existing sidebar focused while making the capability discoverable
   from the user that owns the data.
5. Return Decimal values as text, include asset/market display metadata, and
   keep the endpoints strictly read-only: no settlement, risk recalculation or
   wallet mutation is triggered by viewing the page.

