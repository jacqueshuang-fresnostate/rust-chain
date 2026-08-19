# Agent-routed online support

## Goal

Replace the mobile client's external customer-service jump with a durable first-party chat workflow. Route each authenticated customer to the exact agent currently assigned to that customer, while allowing platform administrators to supervise all conversations and handle unassigned customers.

## What I already know

- The current mobile support entry only opens `VITE_SUPPORT_CHAT_URL`.
- User-to-agent ownership is authoritative in `user_referrals.root_agent_id` and is administered by the existing agent-assignment workflow.
- Agent authentication and an agent portal already exist.
- Parent agent subtree access is suitable for reporting but is not the requested customer-service routing rule.
- The event hub is best-effort; durable chat state must live in MySQL and be recoverable through REST.

## Assumptions

- “Which agent the customer is assigned to” means the exact directly owning agent, not every ancestor in the agent tree.
- Platform administrators are the fallback operators for unassigned users or unavailable agent lines.
- Text chat is the MVP. Attachments, voice/video, typing indicators, canned replies, and SLA automation are outside this task.
- One durable conversation per user is sufficient; sending a new customer message reopens a closed conversation.

## Requirements

### Persistence and assignment

- Add immutable migrations for support conversations and messages.
- Persist open/closed status, exact assigned agent, last message, separate user/staff read cursors, and timestamps.
- Persist a client message idempotency key and prevent duplicate sends on retry.
- Resolve assignment only from server-side referral and active agent hierarchy data.
- Synchronize an existing conversation when the admin user-assignment workflow changes the user's owning agent.
- When that workflow migrates a referral subtree, synchronize every existing descendant conversation whose authoritative `root_agent_id` changes in the same transaction; unrelated conversations must remain untouched.
- Reset the staff read cursor when a conversation moves to a different agent.

### User API and mobile client

- An authenticated user can read only their own conversation and messages, send text, mark messages read, and close/reopen the conversation.
- The first message creates the conversation. A new message reopens a closed conversation.
- Mobile `/profile/help` opens a first-party chat route rather than an external URL.
- Guests see a login-required state instead of a dead or disabled channel.
- The chat UI provides loading, empty, sending, retry, unread/read, assignment, and closed states with Chinese and English locale strings.
- Message history must expose bounded “load older” pagination instead of silently truncating conversations after the newest page.

### Agent API and portal

- An authenticated agent can list and open only conversations whose exact `assigned_agent_id` equals the agent identity resolved from the token.
- Parent agents do not automatically receive conversations assigned to child agents.
- Agents can reply, mark read, close, and reopen conversations.
- The agent portal adds an “在线客服” workbench with queue filters, unread indication, message history, composer, and conversation state controls.
- Agent/admin queues must use the backend `limit`/`offset` contract for every page; a local table page must not hide conversations beyond the first fetched slice.

### Admin API and console

- Admins with `support.conversations.read/write` can inspect all conversations, including unassigned ones.
- The admin console adds an “在线客服” workbench and shows the current owning agent or “未分配”.
- Admin replies use the same immutable message model and are visible to the customer.
- Admin support routes are registered in runtime RBAC rather than falling through to `admin.unmapped`.

### Realtime and recovery

- REST is authoritative and clients periodically reconcile state.
- Successful committed messages publish a best-effort private refresh event to the customer and exact assigned agent.
- Disconnects, dropped broadcasts, API restart, or process restart must not lose messages; reconnect triggers REST reconciliation.

## API contract

- User prefix `/api/v1/support`:
  - `GET /conversation`
  - `GET /conversation/messages`
  - `POST /conversation/messages`
  - `POST /conversation/read`
  - `PATCH /conversation/status`
- Agent prefix `/agent/api/v1/support`:
  - `GET /conversations`
  - `GET /conversations/:id`
  - `GET /conversations/:id/messages`
  - `POST /conversations/:id/messages`
  - `POST /conversations/:id/read`
  - `PATCH /conversations/:id/status`
- Admin prefix `/admin/api/v1/support` exposes the same staff operations globally.
- Message body is trimmed, non-empty, and at most 2,000 Unicode scalar values.
- `client_message_id` is required for sends, uses a safe 8-64 character token, and is scoped by conversation plus sender identity.
- List/message pagination is bounded server-side; timestamps are Unix milliseconds and money-like float conversion is not involved.

## Acceptance Criteria

- [x] Customer A assigned to Agent A appears in Agent A's queue and not Agent B's queue.
- [x] A parent agent cannot open a child agent's directly assigned conversation by guessing its id.
- [x] An unassigned customer's conversation appears in the admin queue and remains usable.
- [x] Admin reassignment of a user moves the conversation to the new agent without client-provided assignment data.
- [x] Referral-subtree reassignment moves all affected descendant conversations and resets their staff cursor without moving unrelated conversations.
- [x] Duplicate sends with the same client id create one message.
- [x] User/staff read cursors produce correct unread counts.
- [x] Sending after close reopens the conversation.
- [x] Mobile, agent, and admin interfaces expose usable loading, empty, error, unread, and closed states.
- [x] Mobile and staff interfaces can load message history older than the newest bounded page without duplicates or reordered messages.
- [x] Agent/admin queue pagination requests the matching server offset and can reach records beyond the first page.
- [x] REST polling recovers messages after WebSocket loss/restart.
- [x] Backend, web, and mobile focused tests plus full quality gates pass.

## Definition of Done

- Immutable migration and backend DDD module implemented with detailed Chinese contracts.
- Route authorization, exact-agent isolation, admin fallback, idempotency, read state, and reassignment tests pass.
- OpenAPI declarations match the production routes.
- Agent/admin web workbench and mobile chat are implemented and tested.
- Relevant backend/admin/mobile specs and `docs/superpowers/PROGRESS.md` are updated.
- Rust formatting/check/clippy/tests, web lint/typecheck/tests/build, mobile type-check/tests/PWA/Tauri builds, and `git diff --check` pass.

## Out of Scope

- File/image/audio attachments.
- Voice/video calls and typing/presence indicators.
- Automated bot replies, canned-response management, SLA escalation, or multi-agent round-robin queues.
- Allowing a parent agent to inspect child-agent conversations.
- Replacing the existing user-to-agent assignment business workflow.

## Technical Notes

- Research: [`research/existing-support-and-agent-routing.md`](research/existing-support-and-agent-routing.md).
- Reuse the existing `UserAuth`, `AgentAuth`, `AdminAuth`, agent scope lookup, request clients, private event hub, and UI shell conventions.
- WebSocket notifications are refresh hints only. Do not write a message after broadcasting and do not assume broadcasts are durable.
