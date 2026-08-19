# Existing Support And Agent Routing Audit

## Current state

- `mobile/src/views/HelpSupportView.vue` only opens `VITE_SUPPORT_CHAT_URL`; there is no first-party conversation or message API.
- The agent portal already has an independent `agent` authentication scope and server-derived agent identity.
- `user_referrals.root_agent_id` is the compatibility column for the user's directly owning agent. The `agents.path` column is used only when a parent agent is intentionally allowed to see a descendant subtree.
- Existing agent business pages use subtree visibility, but the requested customer-service routing is stricter: the conversation must be owned by the exact agent assigned to the customer.
- Admin requests are protected by runtime permission mapping. Unknown paths map to `admin.unmapped`, so support routes need an explicit permission resource.
- The process already exposes a user-private WebSocket and a best-effort in-process broadcast hub. REST remains authoritative because the hub is not durable and does not bridge API instances.

## Recommended shape

1. Persist one durable support conversation per user and immutable text messages in MySQL.
2. Resolve the assigned agent from the server-side referral record; never accept an agent id from the mobile client.
3. Give the exact owning agent access. Do not apply the normal agent-subtree visibility rule to customer-service conversations.
4. Give platform admins global visibility so unassigned users and unavailable agents still have a support path.
5. Keep read state, unread counts, open/closed status, message idempotency, and assignment synchronization in the backend.
6. Use REST polling as the correctness path and private WebSocket notifications only as a low-latency refresh hint.
7. Replace the mobile external link with an authenticated in-app chat page; retain email as a secondary contact channel.
8. Add a shared web support console for both the agent portal and admin console, with scope-specific API clients.

## Key edge cases

- Unassigned customer: conversation remains visible to platform admins and is labelled unassigned.
- Reassigned customer: the open conversation moves to the new direct agent and its staff read cursor resets.
- Disabled agent or disabled ancestor: user-originated synchronization treats the route as unavailable; admins retain access.
- Retried send: a client-generated idempotency key returns the existing message instead of duplicating it.
- Closed conversation: the next user message reopens it.
- WebSocket loss or process restart: clients reconcile from REST and do not rely on event replay.

## Integration findings from final review

- `assign_admin_user_agent` can migrate an entire referral subtree, not only
  the selected user. Support synchronization must therefore use the same old
  path/old owner boundary and update every existing descendant conversation in
  that transaction; syncing only `user_id` leaves stale queues.
- The backend message contract is cursor-paged. A client that fetches only the
  newest 100 records cannot claim complete history; both mobile and staff
  clients need an explicit older-page action that merges immutable IDs.
- `DataTable` defaults to local pagination. A support workbench that fetches
  only `offset=0` while displaying the backend `total` makes rows beyond the
  first slice unreachable; the workbench must supply controlled server
  pagination.
