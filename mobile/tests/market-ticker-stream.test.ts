import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createMarketTickerStream,
  type TickerUpdate,
  type TickerSocket,
  type TickerStreamScheduler,
} from '../src/api/marketTickerStream.ts'

class FakeSocket implements TickerSocket {
  readyState = 0
  readonly sent: string[] = []
  closeCount = 0
  private readonly listeners = new Map<string, Array<(event: { data: unknown }) => void>>()

  send(data: string): void {
    this.sent.push(data)
  }

  close(): void {
    this.closeCount += 1
    this.readyState = 3
  }

  addEventListener(type: string, listener: (event: { data: unknown }) => void): void {
    const current = this.listeners.get(type) ?? []
    current.push(listener)
    this.listeners.set(type, current)
  }

  emit(type: 'open' | 'message' | 'close' | 'error', data: unknown = undefined): void {
    if (type === 'open') this.readyState = 1
    if (type === 'close') this.readyState = 3
    for (const listener of this.listeners.get(type) ?? []) listener({ data })
  }
}

class FakeScheduler implements TickerStreamScheduler {
  readonly timeouts = new Map<object, { callback: () => void; delay: number }>()
  readonly intervals = new Map<object, () => void>()

  setTimeout(callback: () => void, delay: number): object {
    const handle = {}
    this.timeouts.set(handle, { callback, delay })
    return handle
  }

  clearTimeout(handle: unknown): void {
    this.timeouts.delete(handle as object)
  }

  setInterval(callback: () => void): object {
    const handle = {}
    this.intervals.set(handle, callback)
    return handle
  }

  clearInterval(handle: unknown): void {
    this.intervals.delete(handle as object)
  }

  runNextTimeout(): void {
    const entry = this.timeouts.entries().next().value as
      | [object, { callback: () => void; delay: number }]
      | undefined
    assert.ok(entry, 'expected a pending reconnect')
    this.timeouts.delete(entry[0])
    entry[1].callback()
  }

  runTimeoutWithDelay(delay: number): void {
    const entry = [...this.timeouts.entries()].find(([, timer]) => timer.delay === delay)
    assert.ok(entry, `expected a pending ${delay}ms timeout`)
    this.timeouts.delete(entry[0])
    entry[1].callback()
  }
}

test('ticker stream keeps an exact union of view leases and dispatches only requested symbols', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const firstUpdates: string[] = []
  const secondUpdates: string[] = []
  const stream = createMarketTickerStream({
    getUrl: () => 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
  })

  const stopFirst = stream.subscribe(['btc/usdt', 'ETH-USDT'], (update) => {
    firstUpdates.push(`${update.symbol}:${update.lastPrice}`)
  })
  const stopSecond = stream.subscribe(['eth_usdt', 'SOLUSDT'], (update) => {
    secondUpdates.push(`${update.symbol}:${update.lastPrice}`)
  })

  assert.equal(sockets.length, 1)
  sockets[0]?.emit('open')
  assert.deepEqual(commands(sockets[0]), [
    { op: 'subscribe', channel: 'ticker', symbol: 'BTCUSDT' },
    { op: 'subscribe', channel: 'ticker', symbol: 'ETHUSDT' },
    { op: 'subscribe', channel: 'ticker', symbol: 'SOLUSDT' },
  ])

  emitTicker(sockets[0], 'BTC-USDT', 101)
  emitTicker(sockets[0], 'ETHUSDT', 202)
  emitTicker(sockets[0], 'SOL/USDT', 303)
  assert.deepEqual(firstUpdates, ['BTCUSDT:101', 'ETHUSDT:202'])
  assert.deepEqual(secondUpdates, ['ETHUSDT:202', 'SOLUSDT:303'])

  stopFirst()
  stopFirst()
  assert.deepEqual(commands(sockets[0]).at(-1), {
    op: 'unsubscribe',
    channel: 'ticker',
    symbol: 'BTCUSDT',
  })
  emitTicker(sockets[0], 'BTCUSDT', 999)
  emitTicker(sockets[0], 'ETHUSDT', 204)
  assert.deepEqual(firstUpdates, ['BTCUSDT:101', 'ETHUSDT:202'])
  assert.deepEqual(secondUpdates, ['ETHUSDT:202', 'SOLUSDT:303', 'ETHUSDT:204'])

  stopSecond()
  assert.equal(sockets[0]?.closeCount, 1)
  assert.equal(scheduler.timeouts.size, 0)
  assert.equal(scheduler.intervals.size, 0)
})

test('ticker stream rejects late old-connection events and reconnects only current leases', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const updates: number[] = []
  const stream = createMarketTickerStream({
    getUrl: () => 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    reconnectBaseMs: 5,
    reconnectMaxMs: 20,
  })

  const stop = stream.subscribe(['BTCUSDT'], (update) => updates.push(update.lastPrice))
  sockets[0]?.emit('open')
  sockets[0]?.emit('error')
  sockets[0]?.emit('close')
  assert.equal(sockets[0]?.closeCount, 1)
  assert.equal(scheduler.timeouts.size, 1)
  assert.equal(scheduler.intervals.size, 0)

  scheduler.runNextTimeout()
  assert.equal(sockets.length, 2)
  sockets[1]?.emit('open')
  assert.deepEqual(commands(sockets[1]), [
    { op: 'subscribe', channel: 'ticker', symbol: 'BTCUSDT' },
  ])

  emitTicker(sockets[0], 'BTCUSDT', 111)
  emitTicker(sockets[1], 'BTCUSDT', 222)
  assert.deepEqual(updates, [222])

  stop()
  assert.equal(sockets[1]?.closeCount, 1)
  assert.equal(scheduler.timeouts.size, 0)
  assert.equal(scheduler.intervals.size, 0)
})

test('a released lease cannot deliver through its old socket into a new same-symbol lease', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const updates: number[] = []
  const stream = createMarketTickerStream({
    getUrl: () => 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
  })

  const stopOld = stream.subscribe(['BTCUSDT'], () => assert.fail('old lease must be removed'))
  sockets[0]?.emit('open')
  stopOld()
  const stopNew = stream.subscribe(['BTCUSDT'], (update) => updates.push(update.lastPrice))
  sockets[1]?.emit('open')

  emitTicker(sockets[0], 'BTCUSDT', 111)
  emitTicker(sockets[1], 'BTCUSDT', 222)
  assert.deepEqual(updates, [222])

  stopNew()
})

test('ticker stream forwards the complete backend 24h snapshot without recomputing fields', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const updates: TickerUpdate[] = []
  const stream = createMarketTickerStream({
    getUrl: () => 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
  })

  const stop = stream.subscribe(['BTCUSDT'], (update) => updates.push(update))
  sockets[0]?.emit('open')
  sockets[0]?.emit('message', JSON.stringify({
    symbol: 'BTC-USDT',
    last_price: '63700',
    high_24h: '64700',
    low_24h: '62900',
    volume_24h: '125.75',
    price_change_percent_24h: '-0.46875',
    observed_at: 1_786_480_001_000,
  }))

  assert.deepEqual(updates, [{
    symbol: 'BTCUSDT',
    lastPrice: 63_700,
    highPrice: 64_700,
    lowPrice: 62_900,
    volume: 125.75,
    changePercent: -0.46875,
    observedAt: 1_786_480_001_000,
  }])
  stop()
})

test('ticker stream replaces a silent open socket, refreshes on pong, and restores every lease', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const stream = createMarketTickerStream({
    getUrl: () => 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    reconnectBaseMs: 5,
    inboundIdleTimeoutMs: 65,
  })

  const stopBtc = stream.subscribe(['BTCUSDT'], () => undefined)
  const stopEth = stream.subscribe(['ETHUSDT'], () => undefined)
  sockets[0]?.emit('open')
  const staleWatchdog = [...scheduler.timeouts.values()].find((timer) => timer.delay === 65)
  assert.ok(staleWatchdog)

  sockets[0]?.emit('message', 'pong')
  staleWatchdog.callback()
  assert.equal(sockets[0]?.closeCount, 0, 'a superseded watchdog cannot close a live socket')

  scheduler.runTimeoutWithDelay(65)
  assert.equal(sockets[0]?.closeCount, 1)
  scheduler.runTimeoutWithDelay(5)
  assert.equal(sockets.length, 2)
  sockets[1]?.emit('open')
  assert.deepEqual(commands(sockets[1]), [
    { op: 'subscribe', channel: 'ticker', symbol: 'BTCUSDT' },
    { op: 'subscribe', channel: 'ticker', symbol: 'ETHUSDT' },
  ])

  stopBtc()
  stopEth()
  assert.equal(scheduler.timeouts.size, 0)
  assert.equal(scheduler.intervals.size, 0)
})

function emitTicker(socket: FakeSocket | undefined, symbol: string, lastPrice: number): void {
  socket?.emit('message', JSON.stringify({ symbol, last_price: String(lastPrice) }))
}

function commands(socket: FakeSocket | undefined): Array<Record<string, string>> {
  return (socket?.sent ?? [])
    .filter((frame) => frame !== 'ping')
    .map((frame) => JSON.parse(frame) as Record<string, string>)
}
