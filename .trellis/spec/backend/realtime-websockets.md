# Realtime WebSocket Contracts

## Scenario: Business-Scoped Market WebSocket Endpoints

### 1. Scope / Trigger
- Trigger: PC spot, margin, and seconds trading pages need independent realtime connections so each business can evolve subscription behavior without affecting the others.
- Scope: Backend websocket routes in `src/modules/events/routes.rs` and PC websocket client routing in `pc/src/api/stomp.ts`.
- Compatibility: `/ws/public` remains valid for existing clients.

### 2. Signatures
- Root routes:
  - `GET /ws/public`
  - `GET /ws/spot`
  - `GET /ws/margin`
  - `GET /ws/seconds`
  - `GET /ws/private?token=<user access token>`
- Nested user API routes expose the same public aliases under `/api/v1`, for example `GET /api/v1/ws/spot`.
- Single-channel compatibility routes:
  - `GET /ws/public/:namespace/:topic`
  - `GET /ws/spot/:namespace/:topic`
  - `GET /ws/margin/:namespace/:topic`
  - `GET /ws/seconds/:namespace/:topic`

### 3. Contracts
- Public command payload:
  ```json
  {
    "op": "subscribe",
    "channel": "ticker",
    "symbol": "BTC-USDT",
    "interval": "1m"
  }
  ```
- `op`: `subscribe` or `unsubscribe`.
- `channel`: one of `ticker`, `depth`, `trade`, `kline`.
- `symbol`: required for all public channels. Backend normalizes `BTC-USDT`, `BTC_USDT`, and `BTC/USDT` to `BTCUSDT`.
- `interval`: required only for `kline`; normalized through `KlineUpsertKey`.
- Public subscription confirmation:
  ```json
  {"type":"subscribed","channel":"public:ticker:BTCUSDT"}
  ```
- PC endpoint mapping:
  - `spot` and legacy `market` connect to `/ws/spot`.
  - `margin` and legacy `swap` connect to `/ws/margin`.
  - `seconds` and legacy `second` connect to `/ws/seconds`.

### 4. Validation & Error Matrix
- Missing `symbol` -> JSON error message with `type=error`, `code=invalid_request`; socket stays open.
- `kline` without `interval` -> JSON error message with `type=error`, `code=invalid_request`; socket stays open.
- Unsupported `channel` -> JSON error message with `type=error`, `code=invalid_request`; socket stays open.
- Invalid path segment in single-channel route -> validation error before websocket upgrade.
- Invalid private token -> unauthorized/forbidden response before websocket upgrade.

### 5. Good/Base/Bad Cases
- Good: `/ws/spot` subscribes to `ticker BTC-USDT`, receives broadcasts on `public:ticker:BTCUSDT`, and responds to `ping` with `pong`.
- Base: `/ws/public` continues to use the same command and delivery contract.
- Bad: Repointing all PC businesses to `/ws/public` again removes business isolation and can make future business-specific subscriptions interfere with each other.

### 6. Tests Required
- Backend `tests/events_ws.rs` must assert:
  - `/ws/public`, `/ws/spot`, `/ws/margin`, `/ws/seconds`, and nested `/api/v1/ws/*` aliases are not 404.
  - Business aliases accept the same subscribe command and receive matching broadcast messages.
  - Invalid commands return an error frame without closing the socket.
- PC `pc/tests/stomp.test.ts` must assert:
  - default/spot connects to `/ws/spot`.
  - margin connects to `/ws/margin`.
  - seconds connects to `/ws/seconds`.
  - reconnecting one business client does not reconnect or close the others.

### 7. Wrong vs Correct

#### Wrong
```typescript
function endpointPath(_endpoint: BusinessEndpoint): string {
  return '/ws/public'
}
```

#### Correct
```typescript
function endpointPath(endpoint: BusinessEndpoint): string {
  return `/ws/${endpoint}`
}
```

## Scenario: Exact-Agent Private Support Refresh Hints

### 1. Scope / Trigger

- Trigger: a committed durable support message should prompt a customer or its
  exact assigned agent to reconcile sooner than the polling interval.

### 2. Signatures

- User private route: `GET /api/v1/ws/private?token=<user-token>`.
- Agent private route: `GET /agent/api/v1/ws/private?token=<agent-token>`.
- Channels: `private:user:<user_id>` and `private:agent:<agent_id>`.
- Hint payload:
  `{"type":"support.refresh","reason":"message_committed","conversation_id":1,"message_id":2}`.

### 3. Contracts

- Agent handshake authentication resolves the exact active `agent_id` from the
  agent token and active ancestor chain before upgrade. It never accepts a
  client channel, path, root ID, or subtree subscription.
- Publish only after the support transaction commits and only for a
  non-replayed message. Send the hint to the exact user and, when assigned, the
  exact agent; do not broadcast message text or credentials.
- Hints are process-local and lossy. Every client must poll/reconcile REST on
  startup, reconnect, and normal intervals; no business state may exist only
  in the broadcast hub.

### 4. Validation & Error Matrix

- Missing, revoked, wrong-scope, or inactive-chain agent token -> reject before
  WebSocket upgrade.
- Unassigned conversation -> publish only to the user channel.
- Lagged receiver or restarted API -> drop the hint and recover from REST.

### 5. Good/Base/Bad Cases

- Good: Agent 9 receives a refresh for its customer while Agent 9's parent and
  Agent 10 receive nothing.
- Base: no broadcast hub is configured; the committed message remains fully
  available through REST.
- Bad: publish before commit or treat a delivered hint as proof that a message
  exists.

### 6. Tests Required

- Assert the agent private route is registered and rejects invalid agent
  identity before upgrade.
- Publish one exact-agent message and assert exact delivery plus parent and
  unrelated-channel silence.
- Assert idempotent replay publishes no second hint and REST still returns the
  original message.

### 7. Wrong vs Correct

```rust
// Wrong: subtree fan-out leaks a child's customer activity.
for ancestor in agent_ancestors {
    hub.publish(EventBroadcastMessage::private_agent(ancestor.id, payload.clone()));
}

// Correct: one exact owner hint; REST remains authoritative.
if let Some(agent_id) = conversation.assigned_agent_id {
    hub.publish(EventBroadcastMessage::private_agent(agent_id, payload));
}
```
