# Agent-Routed Online Support Contracts

## Scenario: Durable One-to-One Support Routed To The Exact Owning Agent

### 1. Scope / Trigger

- Apply this contract when changing support persistence, user/agent/admin
  support routes, referral reassignment, private refresh events, or any support
  client.
- A customer has one durable conversation. Its staff owner is the exact active
  agent stored in the compatibility owner field
  `user_referrals.root_agent_id`; support never inherits the normal agent
  subtree reporting scope.
- MySQL and REST are authoritative. In-process WebSocket broadcasts are only
  low-latency refresh hints and may be lost on disconnect, restart, or another
  API instance.

### 2. Signatures

```text
support_conversations(
  id, user_id UNIQUE, assigned_agent_id NULL,
  status, user_read_message_id NULL, staff_read_message_id NULL,
  last_message_id NULL, last_message_sender_type NULL,
  last_message_sender_id NULL, last_message_preview NULL,
  last_message_at NULL, closed_at NULL, created_at, updated_at
)

support_messages(
  id, conversation_id, sender_type, sender_id,
  client_message_id, body, created_at,
  UNIQUE(conversation_id, sender_type, sender_id, client_message_id)
)
```

```text
GET    /api/v1/support/conversation
GET    /api/v1/support/conversation/messages?limit=&before_id=
POST   /api/v1/support/conversation/messages
POST   /api/v1/support/conversation/read
PATCH  /api/v1/support/conversation/status

GET    /agent/api/v1/support/conversations?status=&unread_only=&limit=&offset=
GET    /agent/api/v1/support/conversations/:id
GET    /agent/api/v1/support/conversations/:id/messages?limit=&before_id=
POST   /agent/api/v1/support/conversations/:id/messages
POST   /agent/api/v1/support/conversations/:id/read
PATCH  /agent/api/v1/support/conversations/:id/status

GET    /admin/api/v1/support/conversations?status=&unread_only=&assigned_agent_id=&unassigned=&limit=&offset=
GET    /admin/api/v1/support/conversations/:id
GET    /admin/api/v1/support/conversations/:id/messages?limit=&before_id=
POST   /admin/api/v1/support/conversations/:id/messages
POST   /admin/api/v1/support/conversations/:id/read
PATCH  /admin/api/v1/support/conversations/:id/status
```

```json
{
  "body": "trimmed text",
  "client_message_id": "mobile-or-web-safe-token"
}
```

```json
{
  "conversation": {},
  "message": {},
  "replayed": false
}
```

### 3. Contracts

- `UserAuth`, `AgentAuth`, and `AdminAuth` derive every actor ID from the
  bearer token. A request never accepts `user_id`, `agent_id`, or staff sender
  identity as writable support data.
- The first user message creates the user's unique conversation. Messages are
  append-only; sending any new message atomically updates last-message metadata
  and reopens a closed conversation.
- Agent access is always `support_conversations.assigned_agent_id =
  token_agent_id`. A parent, child, sibling, or unrelated agent gets the same
  not-found result as an absent conversation.
- An assigned agent is eligible only when its node and every path ancestor are
  active. An unavailable or absent owner resolves to `NULL`; administrators
  retain global access to that conversation.
- Admin support endpoints require runtime
  `support.conversations.read`/`support.conversations.write`. Admin messages
  persist with `sender_type=admin`, never as a forged agent message.
- Admin user reassignment updates referral ownership and every affected
  existing conversation in the same transaction. If a referral subtree is
  migrated, all descendants whose authoritative owner changes are synchronized
  with a set-based query; unrelated users are excluded. A real owner change
  resets only `staff_read_message_id`.
- A send retry with the same conversation, sender type, sender ID, client key,
  and body returns the original message with `replayed=true`. Reusing that key
  with another normalized body returns conflict.
- `user_read_message_id` and `staff_read_message_id` advance monotonically.
  `user_unread_count` counts agent/admin messages after the user cursor;
  `staff_unread_count` counts user messages after the staff cursor.
- Message pages are ordered oldest-to-newest. `next_before_id` is the oldest
  returned ID and the next request uses the strict `id < before_id` boundary.
  Staff queue pages use server `limit`/`offset`; clients must not locally page
  only the first fetched slice when `total` is larger.
- Successful non-replayed commits may publish `support.refresh` to the exact
  user and exact assigned agent private channels. Publishing happens after the
  database commit, contains no message body, and never determines correctness.
- All API timestamps are Unix milliseconds. Text columns use canonical
  `utf8mb4`/`utf8mb4_unicode_ci` metadata so SQLx can decode them as Rust
  `String` values.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Body is empty after trimming | `VALIDATION_ERROR`; no message or key is consumed |
| Body exceeds 2,000 Unicode scalar values | `VALIDATION_ERROR` before database access |
| Client message ID is not 8-64 ASCII alphanumeric/`_`/`-` characters | `VALIDATION_ERROR` |
| Same idempotency key and normalized body | Return the original message with `replayed=true` |
| Same idempotency key and different normalized body | `CONFLICT`; append nothing |
| `before_id=0`, `message_id=0`, or unknown status | `VALIDATION_ERROR` |
| Read target is not in the conversation | `NOT_FOUND`; do not advance either cursor |
| Agent guesses another owner's conversation ID | `NOT_FOUND`; reveal no target metadata |
| Admin combines `assigned_agent_id` with `unassigned=true` | `VALIDATION_ERROR` |
| User has no referral or active owner | Create/use an unassigned conversation; admin remains able to reply |
| Refresh broadcast is dropped | Persisted REST state remains complete; the next poll/reconnect reconciles it |

### 5. Good / Base / Bad Cases

- **Good**: a user owned by Agent 17 sends a message; only Agent 17 lists and
  opens it, while an administrator can supervise it globally.
- **Good**: an admin moves a referral subtree from Agent 17 to Agent 29; all
  affected existing conversations move in the same transaction and each new
  owner starts with a null staff cursor.
- **Base**: an unassigned user sends a message; the conversation remains
  usable and appears in the admin unassigned queue.
- **Base**: a client misses every refresh event, loads the next queue/message
  page through REST, and reconstructs the complete ordered state.
- **Bad**: use `agents.path LIKE ...` in an agent support query. That leaks
  child-owned customer conversations to parent agents.
- **Bad**: update only the directly reassigned user's conversation after the
  referral workflow has migrated descendant `root_agent_id` values.
- **Bad**: fetch `limit=100, offset=0` once and let a local table claim to page
  a backend total larger than 100.

### 6. Tests Required

- Migration tests lock additive DDL, unique keys, foreign keys, status/sender
  checks, canonical text metadata, and read/last-message columns.
- Route integration tests prove exact-agent isolation, parent denial,
  unassigned admin fallback, idempotent replay/conflict, monotonic unread
  cursors, close/reopen, and runtime admin RBAC.
- Reassignment tests cover the direct user and an affected descendant in one
  subtree, assert staff cursor reset, and prove an unrelated conversation does
  not move.
- Pagination tests cover `limit + 1`, strict `before_id`, ordered deduplication,
  older-history loading, and queue page-to-offset mapping.
- Event tests prove an exact agent channel receives the hint while parent and
  unrelated channels do not; recovery tests use REST without any event.
- Web/mobile tests cover loading, empty, cached-refresh error, retry, unread,
  closed/reopen, send-id reuse, older history, polling cleanup, narrow viewport,
  and complete Chinese/English mobile copy.

### 7. Wrong vs Correct

#### Wrong

```sql
-- Reporting scope is too broad for support ownership.
JOIN agents owner ON owner.id = conversations.assigned_agent_id
WHERE owner.path = :agent_path
   OR owner.path LIKE CONCAT(:agent_path, '/%')
```

#### Correct

```sql
-- The authenticated agent can access only its exact queue.
WHERE conversations.assigned_agent_id = :token_agent_id
```

#### Wrong

```rust
// Referral descendants moved, but their existing support snapshots stayed stale.
sync_conversation_assignment_in_tx(tx, reassigned_user_id, new_agent_id).await?;
```

#### Correct

```rust
// In the same reassignment transaction, synchronize the direct user and every
// migrated descendant selected by the old subtree boundary.
sync_reassigned_support_subtree_in_tx(tx, old_path, old_owner, new_owner).await?;
```
