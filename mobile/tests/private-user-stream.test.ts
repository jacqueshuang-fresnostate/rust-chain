import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  createPrivateUserStream,
  parsePrivateUserFrame,
  type PrivateUserEvent,
  type PrivateUserSocket,
  type PrivateUserStreamScheduler,
} from '../src/api/privateUserStream.ts'

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
  assert.deepEqual(scheduler.pendingTimeoutDelays(), [])

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

test('TradeView scopes the private stream to mounted authenticated contract mode and treats events as REST hints', () => {
  const streamSetup = sliceBetween(
    tradeSource,
    'const privateUserStream = createPrivateUserStream',
    'const { trapFocus:',
  )
  const refreshHint = sliceBetween(
    tradeSource,
    'function requestPrivateMarginReconciliation',
    'function handlePrivateUserEvent',
  )
  const eventHandler = sliceBetween(
    tradeSource,
    'function handlePrivateUserEvent',
    'function syncPrivateUserStream',
  )
  const streamSync = sliceBetween(
    tradeSource,
    'function syncPrivateUserStream',
    'function isCurrentTradingBalancesRequest',
  )

  assert.match(tradeSource, /import \{ apiErrorMessage, readAccessToken \} from '@\/api\/client'/)
  assert.match(tradeSource, /import \{ privateUserWebSocketUrl, publicMarketWebSocketUrl \} from '@\/config\/app'/)
  assert.match(streamSetup, /getAccessToken: readAccessToken/)
  assert.match(streamSetup, /getUrl: privateUserWebSocketUrl/)
  assert.match(streamSetup, /onOpen: requestPrivateMarginReconciliation/)
  assert.match(streamSetup, /onEvent: handlePrivateUserEvent/)

  assert.match(eventHandler, /'margin\.position\.liquidated'/)
  assert.match(eventHandler, /'margin\.position\.partially_closed'/)
  assert.match(eventHandler, /'margin\.position\.closed'/)
  assert.match(eventHandler, /\.includes\(event\.type\)/)
  assert.match(eventHandler, /requestPrivateMarginReconciliation\(\)/)
  assert.doesNotMatch(eventHandler, /marginWallets\.value|marginPositions\.value|marginRiskSnapshots\.value/)
  assert.match(refreshHint, /!viewMounted[\s\S]*?!session\.token[\s\S]*?mode\.value !== 'contract'/)
  assert.match(refreshHint, /marginAccountReconciliation\.refreshBackground\(\{ queueIfBusy: true \}\)/)

  assertOrdered(streamSync, [
    'privateUserStream.stop()',
    '!viewMounted',
    '!session.isAuthenticated',
    "mode.value !== 'contract'",
    'privateUserStream.start()',
  ])
  assert.match(tradeSource, /onMounted\(async \(\) => \{\s*viewMounted = true[\s\S]*?syncPrivateUserStream\(\)/)
  assert.match(tradeSource, /watch\(\(\) => \[mode\.value, session\.token\] as const,[\s\S]*?marginAccountReconciliation\.invalidate\(\)[\s\S]*?syncPrivateUserStream\(\)[\s\S]*?flush: 'sync'/)
  assert.match(tradeSource, /onBeforeUnmount\(\(\) => \{\s*viewMounted = false[\s\S]*?privateUserStream\.stop\(\)[\s\S]*?marginAccountReconciliation\.stop\(\)/)

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

function assertOrdered(source: string, fragments: readonly string[]): void {
  let cursor = -1
  fragments.forEach((fragment) => {
    const next = source.indexOf(fragment, cursor + 1)
    assert.ok(next > cursor, `expected ${fragment} after index ${cursor}`)
    cursor = next
  })
}
