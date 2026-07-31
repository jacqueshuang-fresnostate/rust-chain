import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { startMarketDetailStream } from '../src/api/marketDetailStream.ts'

type SocketEventType = 'open' | 'message' | 'close' | 'error'

class FakeSocket {
  readyState = 0
  sent: string[] = []
  closed = false
  private readonly listeners = new Map<SocketEventType, Array<(event: { data?: unknown }) => void>>()

  addEventListener(type: SocketEventType, listener: (event: { data?: unknown }) => void): void {
    const listeners = this.listeners.get(type) ?? []
    listeners.push(listener)
    this.listeners.set(type, listeners)
  }

  send(data: string): void {
    this.sent.push(data)
  }

  close(): void {
    this.closed = true
    this.readyState = 3
  }

  emit(type: SocketEventType, data?: unknown): void {
    if (type === 'open') this.readyState = 1
    if (type === 'close') this.readyState = 3
    for (const listener of this.listeners.get(type) ?? []) listener({ data })
  }
}

class FakeScheduler {
  private nextId = 1
  readonly timeouts = new Map<number, { callback: () => void; delay: number }>()
  readonly intervals = new Map<number, { callback: () => void; delay: number }>()
  readonly frames = new Map<number, () => void>()

  setTimeout(callback: () => void, delay: number): number {
    const id = this.nextId++
    this.timeouts.set(id, { callback, delay })
    return id
  }

  clearTimeout(handle: unknown): void {
    this.timeouts.delete(Number(handle))
  }

  setInterval(callback: () => void, delay: number): number {
    const id = this.nextId++
    this.intervals.set(id, { callback, delay })
    return id
  }

  clearInterval(handle: unknown): void {
    this.intervals.delete(Number(handle))
  }

  requestFrame(callback: () => void): number {
    const id = this.nextId++
    this.frames.set(id, callback)
    return id
  }

  cancelFrame(handle: unknown): void {
    this.frames.delete(Number(handle))
  }

  runNextTimeout(): void {
    const entry = this.timeouts.entries().next().value as
      | [number, { callback: () => void; delay: number }]
      | undefined
    assert.ok(entry)
    this.timeouts.delete(entry[0])
    entry[1].callback()
  }

  tickIntervals(): void {
    for (const timer of [...this.intervals.values()]) timer.callback()
  }

  runFrames(): void {
    const frames = [...this.frames.values()]
    this.frames.clear()
    for (const callback of frames) callback()
  }
}

test('detail stream subscribes depth and trade, filters symbols, reconnects, and stops idempotently', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const depths: unknown[] = []
  const trades: unknown[] = []
  const stop = startMarketDetailStream({
    symbol: 'btc/usdt',
    url: 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    onDepth: (snapshot) => depths.push(snapshot),
    onTrade: (trade) => trades.push(trade),
  })

  assert.equal(sockets.length, 1)
  sockets[0]?.emit('open')
  assert.deepEqual(sockets[0]?.sent.map((frame) => JSON.parse(frame)), [
    { op: 'subscribe', channel: 'depth', symbol: 'BTCUSDT' },
    { op: 'subscribe', channel: 'trade', symbol: 'BTCUSDT' },
  ])
  assert.equal(scheduler.intervals.size, 1)

  sockets[0]?.emit('message', JSON.stringify({
    symbol: 'ETHUSDT',
    bids: [{ price: 2, quantity: 1 }],
    asks: [{ price: 3, quantity: 1 }],
    observed_at: 1_720_000_000_000,
  }))
  assert.equal(depths.length, 0)

  sockets[0]?.emit('message', JSON.stringify({
    symbol: 'BTCUSDT',
    bids: [{ price: 101, quantity: 1 }, { price: 102, quantity: 2 }],
    asks: [{ price: 104, quantity: 1 }, { price: 103, quantity: 2 }],
    observed_at: 1_720_000_000_000,
  }))
  sockets[0]?.emit('message', JSON.stringify({
    symbol: 'BTCUSDT',
    trade_id: 'trade-live',
    side: 'buy',
    price: 102,
    quantity: 0.5,
    traded_at: 1_720_000_000_001,
  }))
  assert.equal(depths.length, 0)
  assert.equal(scheduler.frames.size, 1)
  scheduler.runFrames()
  assert.deepEqual(depths, [{
    bids: [{ price: 102, quantity: 2 }, { price: 101, quantity: 1 }],
    asks: [{ price: 103, quantity: 2 }, { price: 104, quantity: 1 }],
  }])
  assert.deepEqual(trades, [{
    id: 'trade-live',
    side: 'buy',
    price: 102,
    quantity: 0.5,
    time: 1_720_000_000_001,
  }])

  scheduler.tickIntervals()
  assert.equal(sockets[0]?.sent.at(-1), 'ping')
  sockets[0]?.emit('close')
  assert.deepEqual([...scheduler.timeouts.values()].map((timer) => timer.delay), [1_000])
  assert.equal(scheduler.intervals.size, 0)

  scheduler.runNextTimeout()
  assert.equal(sockets.length, 2)
  sockets[1]?.emit('open')
  assert.deepEqual(sockets[1]?.sent.map((frame) => JSON.parse(frame)), [
    { op: 'subscribe', channel: 'depth', symbol: 'BTCUSDT' },
    { op: 'subscribe', channel: 'trade', symbol: 'BTCUSDT' },
  ])

  stop()
  stop()
  assert.equal(sockets[1]?.closed, true)
  assert.equal(scheduler.timeouts.size, 0)
  assert.equal(scheduler.intervals.size, 0)
  sockets[1]?.emit('message', JSON.stringify({
    symbol: 'BTCUSDT',
    trade_id: 'late',
    side: 'sell',
    price: 99,
    quantity: 1,
    traded_at: 1_720_000_000_002,
  }))
  sockets[1]?.emit('close')
  assert.equal(trades.length, 1)
  assert.equal(scheduler.timeouts.size, 0)
})

test('detail stream reconnect delay grows exponentially and remains bounded', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const stop = startMarketDetailStream({
    symbol: 'BTCUSDT',
    url: 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    reconnectBaseMs: 100,
    reconnectMaxMs: 250,
    onDepth: () => undefined,
    onTrade: () => undefined,
  })

  const delays: number[] = []
  for (let index = 0; index < 4; index += 1) {
    sockets.at(-1)?.emit('error')
    sockets.at(-1)?.emit('close')
    assert.equal(scheduler.timeouts.size, 1)
    const timer = [...scheduler.timeouts.values()][0]
    assert.ok(timer)
    delays.push(timer.delay)
    scheduler.runNextTimeout()
  }
  assert.deepEqual(delays, [100, 200, 250, 250])
  stop()
})

test('detail stream coalesces high-frequency depth snapshots and cancels pending writes on disconnect', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const depths: Array<{ bids: Array<{ price: number }>; asks: unknown[] }> = []
  const stop = startMarketDetailStream({
    symbol: 'BTCUSDT',
    url: 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    onDepth: (snapshot) => depths.push(snapshot),
    onTrade: () => undefined,
  })

  sockets[0]?.emit('open')
  for (const price of [100, 101, 102]) {
    sockets[0]?.emit('message', JSON.stringify({
      symbol: 'BTCUSDT',
      bids: [{ price, quantity: 1 }],
      asks: [],
      observed_at: 1_720_000_000_000 + price,
    }))
  }
  assert.equal(scheduler.frames.size, 1)
  assert.equal(depths.length, 0)
  scheduler.runFrames()
  assert.equal(depths.length, 1)
  assert.equal(depths[0]?.bids[0]?.price, 102)

  sockets[0]?.emit('message', JSON.stringify({
    symbol: 'BTCUSDT',
    bids: [{ price: 103, quantity: 1 }],
    asks: [],
    observed_at: 1_720_000_000_103,
  }))
  assert.equal(scheduler.frames.size, 1)
  sockets[0]?.emit('error')
  sockets[0]?.emit('close')
  assert.equal(scheduler.frames.size, 0)
  assert.equal(scheduler.timeouts.size, 1)
  scheduler.runFrames()
  assert.equal(depths.length, 1)
  stop()
  assert.equal(scheduler.timeouts.size, 0)
})

test('detail stream stop is idempotent while connecting and late events cannot resubscribe', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const stop = startMarketDetailStream({
    symbol: 'BTCUSDT',
    url: 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    onDepth: () => undefined,
    onTrade: () => undefined,
  })

  stop()
  stop()
  assert.equal(sockets[0]?.closed, true)
  sockets[0]?.emit('open')
  sockets[0]?.emit('close')
  assert.deepEqual(sockets[0]?.sent, [])
  assert.equal(scheduler.intervals.size, 0)
  assert.equal(scheduler.timeouts.size, 0)
})

test('MarketDetailView streams alongside REST, protects live state from stale REST, and preserves the socket on interval changes', () => {
  const source = readFileSync(new URL('../src/views/MarketDetailView.vue', import.meta.url), 'utf8')
  const load = source.match(/async function load[\s\S]*?\n}/)?.[0] ?? ''
  const chooseInterval = source.match(/function chooseInterval[\s\S]*?\n}/)?.[0] ?? ''

  assert.ok(load.indexOf('startLiveDetail(symbol, version, liveState)') < load.indexOf('await Promise.allSettled'))
  assert.ok(load.indexOf('stopLiveDetail()') < load.indexOf('startLiveDetail(symbol, version, liveState)'))
  assert.match(load, /if \(version !== requestVersion \|\| symbol !== pairSymbol\.value\) return/)
  assert.match(load, /if \(!liveState\.depthReceived\)/)
  assert.match(load, /if \(liveState\.tradeReceived\)[\s\S]*mergeMarketTradeHistory\(trades\.value, restTrades, 16\)/)
  assert.match(source, /onDepth:[\s\S]*state\.depthReceived = true/)
  assert.match(source, /onTrade:[\s\S]*state\.tradeReceived = true[\s\S]*mergeMarketTrades\(trades\.value, trade, 16\)/)
  assert.match(chooseInterval, /void refreshKlines\(\)/)
  assert.doesNotMatch(chooseInterval, /void load\(/)
  assert.match(source, /onUnmounted\(\(\) => \{[\s\S]*stopLiveDetail\(\)/)
})
