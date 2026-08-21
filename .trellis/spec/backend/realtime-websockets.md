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

## Scenario: Margin Liquidation Private Refresh Hints

### 1. Scope / Trigger

- Trigger: the asynchronous margin-liquidation worker commits a terminal
  position/wallet settlement that an already-open trading page did not initiate.
- Scope: post-commit publication to the authenticated user's private socket and
  client-side REST reconciliation. The event is a low-latency hint, not a
  financial state snapshot.

### 2. Signatures

- User route: `GET /api/v1/ws/private?token=<user-access-token>`.
- Server-owned channel: `private:user:<user_id>`; the client sends no subscribe
  command and cannot choose another user ID.
- Event discriminator: `type = "margin.position.liquidated"`.
- Payload fields: `position_id`, `product_id`, `pair_id`, `margin_asset`,
  `direction`, `margin_amount`, `notional_amount`, `interest_amount`,
  `entry_price`, `mark_price`, `realized_pnl`, `payout_amount`, `reason`, and
  Unix-millisecond `liquidated_at`.
- Authoritative follow-up: `GET /api/v1/margin/wallets`, then risk reads only
  for positions still present and eligible.

### 3. Contracts

- Authenticate a non-empty user-scope token before upgrade, resolve its exact
  `user_id`, and bind exactly one private channel on the server.
- Publish only after the liquidation transaction commits. A hub failure,
  missing subscriber, lagged receiver, disconnect, or API restart must never
  roll back or alter the committed liquidation.
- Private broadcasts are process-local, lossy, and non-replayed. On socket open
  or reconnect and after a matching event, the client must fetch the REST
  account snapshot; it must also retain bounded periodic REST reconciliation.
- The client may send text `ping` and receive text `pong`. Confirmation, pong,
  error, unknown, and malformed frames do not mutate business state.
- Event amount fields are notification context only. Wallet balances, active
  positions, and risk-cache pruning come from the authoritative REST response.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Missing, revoked, expired, or wrong-scope token | Reject before WebSocket upgrade |
| Subject is not strict `user:<u64>` | Reject before creating a private subscription |
| Liquidation transaction rolls back | Publish no private event |
| Broadcast hub is absent or receiver is lagged | Keep the committed settlement; REST polling converges later |
| Client receives duplicate hints | Coalesce/serialize REST reconciliation; never apply the amount twice |
| Client reconnects after missing hints | Reconcile REST immediately after open |

### 5. Good / Base / Bad Cases

- Good: one committed liquidation publishes to the exact user's channel; the
  connected client immediately reloads wallets/open positions and removes the
  terminal position.
- Base: no client is connected, so the event is dropped and the next account
  REST read still returns the correct committed state.
- Bad: publish before commit, let the client subscribe to an arbitrary private
  channel, or treat `payout_amount` in the event as an instruction to increment
  the displayed wallet.

### 6. Tests Required

- Backend tests assert private-route pre-upgrade authentication, exact-user
  channel isolation, text ping/pong, and no client-selected channel.
- Liquidation worker tests assert one post-commit event with the exact
  discriminator/position ID and no duplicate event on idempotent replay.
- Mobile tests assert that open/reconnect and the matching event trigger REST,
  while protocol, error, unknown, and malformed frames do not mutate state.
- A disconnect/missed-event test must prove the periodic REST path eventually
  removes the liquidated position and refreshes the wallet from one snapshot.

### 7. Wrong vs Correct

```ts
// Wrong: a lossy notification payload becomes financial truth.
wallet.available += Number(event.payout_amount)
positions.value = positions.value.filter((item) => item.id !== event.position_id)

// Correct: the event only accelerates an authoritative account reconciliation.
if (event.type === 'margin.position.liquidated') {
  void reconcileMarginAccountFromRest()
}
```

## Scenario: Upstream Market Feed Liveness and Recovery

### 1. Scope / Trigger

- Trigger: a Bitget, HTX, or Coinbase market WebSocket can remain transport-open
  while delivering no frames, so the provider reconnect loop would otherwise
  wait forever and stop feeding Redis, Mongo, outbox, and public broadcasts.
- Scope: `src/workers/market_feed.rs` provider connection lifecycle. It does
  not change normalized market payloads or downstream subscription commands.

### 2. Signatures

- Bitget application heartbeat: text `ping` every 25 seconds; text `pong` is a
  control frame.
- All-provider inbound idle deadline: 75 seconds from the most recent inbound
  WebSocket frame.
- Connection establishment timeout: 15 seconds.
- Subscription, heartbeat, and protocol-reply write timeout: 10 seconds.
- Recovery owner: the existing provider REST fallback followed by bounded
  exponential WebSocket reconnect.

### 3. Contracts

- Bitget must receive a client text heartbeat independently of market traffic.
  Its plain text `ping`/`pong` frames are handled before JSON decoding.
- Every inbound frame refreshes the idle deadline, including market data,
  subscription acknowledgements, application/protocol ping/pong, and close.
  Outbound heartbeats do not prove that the peer is readable and therefore do
  not refresh the deadline.
- HTX remains server-ping driven and Coinbase remains heartbeat-channel driven;
  neither receives an extra application heartbeat, but both use the same idle
  deadline to detect half-open transport.
- An idle, connect, or write timeout ends only the current provider cycle. The
  failure enters the existing REST fallback and provider-specific reconnect
  loop; another provider continues independently.
- Any Redis/Mongo write, outbox event, or public broadcast completed before the
  failure remains committed and is never replayed or rolled back by liveness
  recovery.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Bitget sends plain `pong` | Ignore as a valid control frame; do not JSON-decode it |
| Bitget sends plain `ping` | Reply with plain `pong` within the bounded write path |
| Any provider emits an inbound frame | Move the idle deadline to `now + 75s` |
| No inbound frame for 75 seconds | Return an internal idle-timeout error and enter fallback/reconnect |
| Connect does not settle in 15 seconds | End the cycle with a connect-timeout error |
| Subscribe/heartbeat/reply write exceeds 10 seconds | End the cycle with an operation-specific write-timeout error |
| One provider is idle or failing | Keep other provider tasks running |

### 5. Good / Base / Bad Cases

- Good: Bitget receives `ping`, answers `pong`, and the connection continues
  even when an illiquid pair has no order-book or trade update.
- Good: a proxy silently drops the upstream path without a close frame; after
  75 seconds the provider cycle exits, REST refreshes the snapshot, and the
  reconnect loop restores all configured subscriptions.
- Base: HTX pings or Coinbase heartbeats arrive normally and merely refresh the
  common idle deadline.
- Bad: treat `WebSocketStream` presence or a successful outbound `send()` as
  proof of peer liveness, or wait only for close/error before reconnecting.

### 6. Tests Required

- Unit-test Bitget heartbeat selection plus plain text ping/pong actions.
- Unit-test that inbound activity deterministically moves the idle deadline.
- Run a pending/silent stream with a short injected deadline and assert the
  idle event wins; separately assert Bitget heartbeat wins before that deadline.
- Make a Bitget heartbeat and market frame ready together and assert the due
  heartbeat wins, preventing sustained high-frequency data from starving it.
- Keep the provider reconnect-loop event/backoff tests and the complete
  `market_feed_worker` integration suite green.

### 7. Wrong vs Correct

```rust
// Wrong: a half-open socket can keep this future pending forever.
while let Some(message) = reader.next().await {
    ingest(message?).await?;
}

// Correct: transport reads compete with protocol heartbeat and an inbound deadline.
match liveness.wait_next(&mut reader).await {
    MarketFeedSocketEvent::Message(message) => handle(message).await?,
    MarketFeedSocketEvent::HeartbeatDue => send_bitget_ping().await?,
    MarketFeedSocketEvent::IdleTimeout => return Err(idle_timeout_error()),
}
```
