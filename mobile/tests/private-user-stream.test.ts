import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  PRIVATE_USER_INBOUND_IDLE_TIMEOUT_MS,
  createPrivateUserStream,
  parsePrivateUserFrame,
  type PrivateUserEvent,
  type PrivateUserSocket,
  type PrivateUserStreamScheduler,
} from '../src/api/privateUserStream.ts'
import {
  createPrivateUserStreamManager,
  eventMatchesTopic,
  type PrivateUserManagerSession,
} from '../src/core/privateUserStreamManager.ts'

const tradeSource = readFileSync(new URL('../src/views/TradeView.vue', import.meta.url), 'utf8')

test('private frame parser ignores protocol traffic and malformed payloads while preserving object events', () => {
  for (const frame of [
    'pong',
    ' PONG ',
    '{',
    '',
    'null',
    '[]',
    '42',
    '{}',
    '{"type":""}',
    '{"type":"pong"}',
    '{"type":"subscribed","channel":"private:user:1"}',
    '{"type":"unsubscribed","channel":"private:user:1"}',
    '{"type":"subscription.confirmed"}',
    '{"type":"subscription_confirmation"}',
    '{"type":"error","code":"invalid_request"}',
  ]) {
    assert.equal(parsePrivateUserFrame(frame), null, frame)
  }
  assert.equal(parsePrivateUserFrame(new Uint8Array([1, 2, 3])), null)

  assert.deepEqual(
    parsePrivateUserFrame('{"type":" margin.position.liquidated ","position_id":"P1"}'),
    { type: 'margin.position.liquidated', position_id: 'P1' },
  )
  assert.deepEqual(
    parsePrivateUserFrame('{"type":"support.refresh","conversation_id":9}'),
    { type: 'support.refresh', conversation_id: 9 },
  )
})

test('private stream makes no connection without a token and reconnect reads the latest token', () => {
  let token = ''
  const scheduler = new ManualScheduler()
  const sockets: MockPrivateSocket[] = []
  const stream = createPrivateUserStream({
    getAccessToken: () => token,
    getUrl: (accessToken) => `wss://api.example.test/api/v1/ws/private?token=${encodeURIComponent(accessToken)}`,
    onEvent: () => undefined,
    createSocket: (url) => {
      const socket = new MockPrivateSocket(url)
      sockets.push(socket)
      return socket
    },
    scheduler,
    reconnectBaseMs: 10,
    reconnectJitterRatio: 0,
  })

  assert.equal(stream.start(), false)
  assert.equal(stream.isRunning(), false)
  assert.equal(sockets.length, 0)

  token = 'TOKEN A/?='
  assert.equal(stream.start(), true)
  assert.equal(stream.start(), true, 'start is idempotent while active')
  assert.equal(sockets.length, 1)
  assert.equal(
    sockets[0]?.url,
    'wss://api.example.test/api/v1/ws/private?token=TOKEN%20A%2F%3F%3D',
  )

  sockets[0]?.serverClose()
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [10])
  token = 'TOKEN B'
  scheduler.runNextTimeout()
  assert.equal(sockets[1]?.url, 'wss://api.example.test/api/v1/ws/private?token=TOKEN%20B')

  sockets[1]?.serverClose()
  token = ''
  scheduler.runNextTimeout()
  assert.equal(sockets.length, 2)
  assert.equal(stream.isRunning(), false)
})

test('private stream filters events, sends heartbeat only on the current socket, and guards late handlers', () => {
  const scheduler = new ManualScheduler()
  const sockets: MockPrivateSocket[] = []
  const opened: string[] = []
  const events: PrivateUserEvent[] = []
  const stream = createPrivateUserStream({
    getAccessToken: () => 'TOKEN',
    getUrl: (token) => `ws://localhost/ws/private?token=${token}`,
    onOpen: () => opened.push(sockets.at(-1)?.url || ''),
    onEvent: (event) => events.push(event),
    createSocket: (url) => {
      const socket = new MockPrivateSocket(url)
      sockets.push(socket)
      return socket
    },
    scheduler,
    reconnectBaseMs: 10,
    reconnectJitterRatio: 0,
    heartbeatMs: 20,
  })

  stream.start()
  const first = requiredSocket(sockets, 0)
  first.open()
  assert.equal(opened.length, 1)
  assert.deepEqual(first.sent, [], 'the private channel is server-bound; no subscribe command is sent')
  const staleHeartbeat = scheduler.latestIntervalCallback()
  scheduler.fireIntervals()
  assert.deepEqual(first.sent, ['ping'])

  first.message('pong')
  first.message('{"type":"subscribed","channel":"private:user:1"}')
  first.message('{"type":"error","code":"invalid_request"}')
  first.message('{broken')
  first.message('{"type":"margin.position.liquidated","position_id":"P1"}')
  assert.deepEqual(events, [{ type: 'margin.position.liquidated', position_id: 'P1' }])

  first.serverClose()
  scheduler.runNextTimeout()
  const second = requiredSocket(sockets, 1)
  second.open()
  assert.equal(opened.length, 2, 'initial open and reconnect both notify the owner')

  first.message('{"type":"margin.position.liquidated","position_id":"STALE"}')
  first.open()
  first.serverClose()
  first.error()
  staleHeartbeat?.()
  assert.equal(opened.length, 2)
  assert.equal(events.length, 1)
  assert.deepEqual(first.sent, ['ping'])
  assert.deepEqual(
    scheduler.pendingTimeoutDelays(),
    [PRIVATE_USER_INBOUND_IDLE_TIMEOUT_MS],
    'late handlers do not replace the current socket watchdog',
  )

  scheduler.fireIntervals()
  assert.deepEqual(second.sent, ['ping'])
  second.error()
  second.serverClose()
  assert.equal(second.closeCount, 1)
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [10], 'error and close schedule one reconnect')

  scheduler.runNextTimeout()
  const third = requiredSocket(sockets, 2)
  stream.stop()
  stream.stop()
  assert.equal(third.closeCount, 1)
  assert.equal(stream.isRunning(), false)
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [])
  assert.equal(scheduler.activeIntervalCount(), 0)

  third.open()
  third.message('{"type":"margin.position.liquidated","position_id":"AFTER_STOP"}')
  third.error()
  third.serverClose()
  assert.equal(opened.length, 2)
  assert.equal(events.length, 1)
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [])
})

test('private reconnect delay grows exponentially and remains bounded', () => {
  const scheduler = new ManualScheduler()
  const sockets: MockPrivateSocket[] = []
  const stream = createPrivateUserStream({
    getAccessToken: () => 'TOKEN',
    getUrl: () => 'ws://localhost/ws/private?token=TOKEN',
    onEvent: () => undefined,
    createSocket: (url) => {
      const socket = new MockPrivateSocket(url)
      sockets.push(socket)
      return socket
    },
    scheduler,
    reconnectBaseMs: 100,
    reconnectMaxMs: 250,
    reconnectJitterRatio: 0,
  })

  stream.start()
  requiredSocket(sockets, 0).serverClose()
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [100])
  scheduler.runNextTimeout()

  requiredSocket(sockets, 1).serverClose()
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [200])
  scheduler.runNextTimeout()

  requiredSocket(sockets, 2).serverClose()
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [250])
  scheduler.runNextTimeout()

  const current = requiredSocket(sockets, 3)
  current.open()
  current.serverClose()
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [100], 'a successful open resets backoff')
  stream.stop()
})

test('private reconnect jitter is deterministic and remains bounded by the configured maximum', () => {
  const scheduler = new ManualScheduler()
  const sockets: MockPrivateSocket[] = []
  const randomValues = [0, 1, 1]
  const stream = createPrivateUserStream({
    getAccessToken: () => 'TOKEN',
    getUrl: () => 'ws://localhost/ws/private?token=TOKEN',
    onEvent: () => undefined,
    createSocket: (url) => {
      const socket = new MockPrivateSocket(url)
      sockets.push(socket)
      return socket
    },
    scheduler,
    random: () => randomValues.shift() ?? 0.5,
    reconnectBaseMs: 100,
    reconnectMaxMs: 250,
    reconnectJitterRatio: 0.2,
  })

  stream.start()
  requiredSocket(sockets, 0).serverClose()
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [80])
  scheduler.runNextTimeout()

  requiredSocket(sockets, 1).serverClose()
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [240])
  scheduler.runNextTimeout()

  requiredSocket(sockets, 2).serverClose()
  assert.deepEqual(
    scheduler.pendingTimeoutDelays(),
    [250],
    'jitter cannot exceed the configured reconnect maximum',
  )
  stream.stop()
})

test('private inbound watchdog rearms on every frame and stale callbacks cannot close a replacement', () => {
  let clock = 100
  const scheduler = new ManualScheduler()
  const sockets: MockPrivateSocket[] = []
  const states: string[] = []
  const inboundFrames: number[] = []
  const stream = createPrivateUserStream({
    getAccessToken: () => 'TOKEN',
    getUrl: () => 'ws://localhost/ws/private?token=TOKEN',
    onInboundFrame: (receivedAt) => inboundFrames.push(receivedAt),
    onStateChange: (state) => states.push(state),
    onEvent: () => undefined,
    createSocket: (url) => {
      const socket = new MockPrivateSocket(url)
      sockets.push(socket)
      return socket
    },
    scheduler,
    now: () => clock,
    reconnectBaseMs: 10,
    reconnectJitterRatio: 0,
    heartbeatMs: 20,
    inboundIdleTimeoutMs: 50,
  })

  stream.start()
  const first = requiredSocket(sockets, 0)
  first.open()
  const staleWatchdog = scheduler.latestTimeoutCallback(50)
  assert.deepEqual(states, ['connecting', 'live'])
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [50])

  clock = 125
  first.message('pong')
  assert.deepEqual(inboundFrames, [125])
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [50])
  staleWatchdog?.()
  assert.equal(first.closeCount, 0, 'a cleared watchdog is inert after a newer inbound frame')
  assert.equal(states.at(-1), 'live')

  scheduler.runTimeoutWithDelay(50)
  assert.equal(first.closeCount, 1)
  assert.equal(states.at(-1), 'stale')
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [10])

  scheduler.runTimeoutWithDelay(10)
  const second = requiredSocket(sockets, 1)
  assert.equal(states.at(-1), 'connecting')
  first.open()
  first.message('{"type":"support.refresh"}')
  first.error()
  first.serverClose()
  assert.equal(second.closeCount, 0)
  assert.equal(states.at(-1), 'connecting')
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [])

  second.open()
  assert.equal(states.at(-1), 'live')
  stream.stop()
  assert.equal(second.closeCount, 1)
  assert.equal(states.at(-1), 'stopped')
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [])
})

test('shared private manager keeps one generation-scoped socket across independent topic leases', () => {
  let clock = 1_000
  let session: PrivateUserManagerSession = {
    accessToken: 'TOKEN-A',
    scope: 'USER-A',
    generation: 4,
  }
  const scheduler = new ManualScheduler()
  const sockets: MockPrivateSocket[] = []
  const marginEvents: string[] = []
  const supportEvents: string[] = []
  const opens: string[] = []
  const manager = createPrivateUserStreamManager({
    readSession: () => session,
    isOnline: () => true,
    openTransport: (context) => createPrivateUserStream({
      getAccessToken: () => context.accessToken,
      getUrl: (token) => `ws://localhost/ws/private?token=${token}`,
      onOpen: context.onOpen,
      onInboundFrame: context.onInboundFrame,
      onStateChange: context.onStateChange,
      onEvent: context.onEvent,
      createSocket: (url) => {
        const socket = new MockPrivateSocket(url)
        sockets.push(socket)
        return socket
      },
      scheduler,
      now: () => clock,
      reconnectBaseMs: 10,
      reconnectJitterRatio: 0,
      heartbeatMs: 20,
      inboundIdleTimeoutMs: 50,
    }),
  })

  const marginLease = manager.acquire({
    topic: 'margin',
    consumerId: 'trade',
    onOpen: () => opens.push('margin'),
    onEvent: (event) => marginEvents.push(event.type),
  })
  const supportLease = manager.acquire({
    topic: 'support',
    consumerId: 'support-chat',
    onOpen: () => opens.push('support'),
    onEvent: (event) => supportEvents.push(event.type),
  })

  assert.equal(sockets.length, 1)
  assert.equal(requiredSocket(sockets, 0).url, 'ws://localhost/ws/private?token=TOKEN-A')
  assert.deepEqual(manager.snapshot(), {
    connection: 'connecting',
    connecting: true,
    live: false,
    stale: false,
    offline: false,
    lastFrameAt: 0,
    consumerCount: 2,
    marginConsumerCount: 1,
    supportConsumerCount: 1,
    sessionGeneration: 4,
  })

  const first = requiredSocket(sockets, 0)
  first.open()
  assert.deepEqual(opens, ['margin', 'support'])
  clock = 1_125
  first.message('{"type":"margin.position.closed","position_id":"P1"}')
  first.message('{"type":"support.refresh","conversation_id":9}')
  first.message('{"type":"wallet.changed"}')
  assert.deepEqual(marginEvents, ['margin.position.closed'])
  assert.deepEqual(supportEvents, ['support.refresh'])
  assert.equal(manager.snapshot().lastFrameAt, 1_125)
  assert.equal(manager.snapshot().live, true)

  marginLease.release()
  assert.equal(first.closeCount, 0, 'releasing one consumer preserves the shared support socket')
  assert.equal(manager.snapshot().consumerCount, 1)
  first.message('{"type":"margin.position.liquidated"}')
  assert.deepEqual(marginEvents, ['margin.position.closed'])

  session = { accessToken: 'TOKEN-B', scope: 'USER-A', generation: 5 }
  manager.synchronizeSession()
  assert.equal(first.closeCount, 1)
  assert.equal(sockets.length, 2)
  assert.equal(requiredSocket(sockets, 1).url, 'ws://localhost/ws/private?token=TOKEN-B')
  assert.equal(manager.snapshot().sessionGeneration, 5)
  assert.equal(manager.snapshot().lastFrameAt, 0)

  first.open()
  first.message('{"type":"support.refresh","conversation_id":"OLD"}')
  first.error()
  first.serverClose()
  assert.equal(manager.snapshot().connecting, true)
  assert.deepEqual(supportEvents, ['support.refresh'])
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [])

  const second = requiredSocket(sockets, 1)
  second.open()
  clock = 1_250
  second.message('pong')
  assert.equal(manager.snapshot().lastFrameAt, 1_250)
  manager.setOnline(false)
  assert.equal(second.closeCount, 1)
  assert.equal(manager.snapshot().offline, true)

  second.message('{"type":"support.refresh","conversation_id":"OFFLINE"}')
  manager.setOnline(true)
  assert.equal(sockets.length, 3)
  assert.equal(manager.snapshot().connecting, true)

  const third = requiredSocket(sockets, 2)
  session = { accessToken: '', scope: '', generation: 6 }
  manager.synchronizeSession()
  assert.equal(third.closeCount, 1, 'logout closes the shared generation even with an active lease')
  assert.equal(manager.snapshot().connection, 'idle')
  assert.equal(manager.snapshot().sessionGeneration, 6)
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [])

  supportLease.release()
  assert.equal(manager.snapshot().consumerCount, 0)
  manager.dispose()
})

test('private manager routes only exact supported topic event types', () => {
  assert.equal(eventMatchesTopic({ type: 'margin.position.liquidated' }, 'margin'), true)
  assert.equal(eventMatchesTopic({ type: 'margin.position.partially_closed' }, 'margin'), true)
  assert.equal(eventMatchesTopic({ type: 'margin.position.closed' }, 'margin'), true)
  assert.equal(eventMatchesTopic({ type: 'margin.position.opened' }, 'margin'), false)
  assert.equal(eventMatchesTopic({ type: 'support.refresh' }, 'support'), true)
  assert.equal(eventMatchesTopic({ type: 'support.refresh.extra' }, 'support'), false)
})

test('TradeView leases the shared margin topic and treats private frames only as REST hints', () => {
  const leaseSetup = sliceBetween(
    tradeSource,
    'usePrivateUserStreamLease({',
    'const { trapFocus:',
  )
  const refreshHint = sliceBetween(
    tradeSource,
    'function requestPrivateMarginReconciliation',
    'function isCurrentTradingBalancesRequest',
  )

  assert.match(tradeSource, /import \{ usePrivateUserStreamLease \} from '@\/composables\/usePrivateUserStreamLease'/)
  assert.match(leaseSetup, /topic: 'margin'/)
  assert.match(leaseSetup, /consumerId: 'trade-margin-account'/)
  assert.match(leaseSetup, /enabled: \(\) => viewActive && session\.isAuthenticated && mode\.value === 'contract'/)
  assert.match(leaseSetup, /onOpen: requestPrivateMarginReconciliation/)
  assert.match(leaseSetup, /onEvent: requestPrivateMarginReconciliation/)
  assert.match(refreshHint, /!viewActive[\s\S]*?!session\.token[\s\S]*?mode\.value !== 'contract'/)
  assert.match(refreshHint, /marginAccountReconciliation\.refreshBackground\(\{ queueIfBusy: true \}\)/)
  assert.doesNotMatch(refreshHint, /marginWallets\.value|marginPositions\.value|marginRiskSnapshots\.value/)

  assert.doesNotMatch(tradeSource, /createPrivateUserStream|privateUserWebSocketUrl|readAccessToken/)
  assert.doesNotMatch(tradeSource, /privateUserStream\.(?:start|stop)\(\)/)
  assert.match(tradeSource, /onBeforeUnmount\(\(\) => \{\s*viewActive = false[\s\S]*?marginAccountReconciliation\.stop\(\)/)

  assert.match(tradeSource, /marginAccountReconciliation\.startPolling\(\)/)
  assert.match(tradeSource, /document\.visibilityState === 'hidden'[\s\S]*?refreshBackground\(\{ queueIfBusy: true \}\)/)
  assert.match(tradeSource, /const margin = await fetchMarginWallets\(\)/)
})

class MockPrivateSocket implements PrivateUserSocket {
  readonly url: string
  readyState = 0
  readonly sent: string[] = []
  closeCount = 0
  private readonly listeners: Record<string, Array<(event: any) => void>> = {
    open: [],
    message: [],
    close: [],
    error: [],
  }

  constructor(url: string) {
    this.url = url
  }

  addEventListener(type: 'open' | 'message' | 'close' | 'error', listener: (event: any) => void): void {
    this.listeners[type]?.push(listener)
  }

  send(data: string): void {
    this.sent.push(data)
  }

  close(): void {
    this.closeCount += 1
    this.readyState = 3
  }

  open(): void {
    this.readyState = 1
    this.emit('open', {})
  }

  message(data: unknown): void {
    this.emit('message', { data })
  }

  serverClose(): void {
    this.readyState = 3
    this.emit('close', {})
  }

  error(): void {
    this.emit('error', {})
  }

  private emit(type: string, event: unknown): void {
    for (const listener of this.listeners[type] || []) listener(event)
  }
}

interface ScheduledTask {
  id: number
  callback: () => void
  delay: number
  cleared: boolean
}

class ManualScheduler implements PrivateUserStreamScheduler {
  private nextId = 1
  private readonly timeouts: ScheduledTask[] = []
  private readonly intervals: ScheduledTask[] = []

  setTimeout(callback: () => void, delay: number): unknown {
    const task = { id: this.nextId++, callback, delay, cleared: false }
    this.timeouts.push(task)
    return task.id
  }

  clearTimeout(handle: unknown): void {
    this.clear(this.timeouts, handle)
  }

  setInterval(callback: () => void, delay: number): unknown {
    const task = { id: this.nextId++, callback, delay, cleared: false }
    this.intervals.push(task)
    return task.id
  }

  clearInterval(handle: unknown): void {
    this.clear(this.intervals, handle)
  }

  pendingTimeoutDelays(): number[] {
    return this.timeouts.filter((task) => !task.cleared).map((task) => task.delay)
  }

  runNextTimeout(): void {
    const task = this.timeouts.find((candidate) => !candidate.cleared)
    assert.ok(task, 'expected a pending timeout')
    task.cleared = true
    task.callback()
  }

  fireIntervals(): void {
    for (const task of this.intervals.filter((candidate) => !candidate.cleared)) {
      task.callback()
    }
  }

  latestIntervalCallback(): (() => void) | null {
    return this.intervals.at(-1)?.callback || null
  }

  latestTimeoutCallback(delay?: number): (() => void) | null {
    return this.timeouts
      .filter((task) => !task.cleared && (delay === undefined || task.delay === delay))
      .at(-1)?.callback || null
  }

  runTimeoutWithDelay(delay: number): void {
    const task = this.timeouts.find((candidate) => !candidate.cleared && candidate.delay === delay)
    assert.ok(task, `expected a pending ${delay}ms timeout`)
    task.cleared = true
    task.callback()
  }

  activeIntervalCount(): number {
    return this.intervals.filter((task) => !task.cleared).length
  }

  private clear(tasks: ScheduledTask[], handle: unknown): void {
    const task = tasks.find((candidate) => candidate.id === handle)
    if (task) task.cleared = true
  }
}

function requiredSocket(sockets: MockPrivateSocket[], index: number): MockPrivateSocket {
  const socket = sockets[index]
  assert.ok(socket, `expected socket ${index}`)
  return socket
}

function sliceBetween(source: string, start: string, end: string): string {
  const startIndex = source.indexOf(start)
  const endIndex = source.indexOf(end, startIndex)
  assert.ok(startIndex >= 0 && endIndex > startIndex, `missing source slice ${start} -> ${end}`)
  return source.slice(startIndex, endIndex)
}
