import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  createMarketDetailStreamSession,
  startMarketDetailStream,
  type MarketDetailStreamOptions,
} from '../src/api/marketDetailStream.ts'
import type { KlinePoint } from '../src/core/types.ts'

type SocketEventType = 'open' | 'message' | 'close' | 'error'

class FakeSocket {
  readyState = 0
  sent: string[] = []
  closed = false
  closeCount = 0
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
    this.closeCount += 1
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

function deferred<Value>(): {
  promise: Promise<Value>
  resolve(value: Value): void
} {
  let resolvePromise: ((value: Value) => void) | undefined
  const promise = new Promise<Value>((resolve) => {
    resolvePromise = resolve
  })
  return {
    promise,
    resolve: (value) => resolvePromise?.(value),
  }
}

test('detail stream subscribes depth, trade, and kline, filters symbols, reconnects, and stops idempotently', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const depths: unknown[] = []
  const trades: unknown[] = []
  const stop = startMarketDetailStream({
    symbol: 'btc/usdt',
    interval: '15m',
    url: 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    onDepth: (snapshot) => depths.push(snapshot),
    onTrade: (trade) => trades.push(trade),
    onKline: () => undefined,
  })

  assert.equal(sockets.length, 1)
  sockets[0]?.emit('open')
  assert.deepEqual(sockets[0]?.sent.map((frame) => JSON.parse(frame)), [
    { op: 'subscribe', channel: 'depth', symbol: 'BTCUSDT' },
    { op: 'subscribe', channel: 'trade', symbol: 'BTCUSDT' },
    { op: 'subscribe', channel: 'kline', symbol: 'BTCUSDT', interval: '15m' },
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
    { op: 'subscribe', channel: 'kline', symbol: 'BTCUSDT', interval: '15m' },
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

test('detail stream supports a kline-only channel set without opening unused depth or trade subscriptions', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const depths: unknown[] = []
  const trades: unknown[] = []
  const klines: KlinePoint[] = []
  const stop = startMarketDetailStream({
    symbol: 'BTCUSDT',
    interval: '1m',
    url: 'wss://example.test/api/v1/ws/public',
    channels: ['kline'],
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    onDepth: (snapshot) => depths.push(snapshot),
    onTrade: (trade) => trades.push(trade),
    onKline: (point) => klines.push(point),
  })

  sockets[0]?.emit('open')
  assert.deepEqual(sockets[0]?.sent.map((frame) => JSON.parse(frame)), [
    { op: 'subscribe', channel: 'kline', symbol: 'BTCUSDT', interval: '1m' },
  ])

  sockets[0]?.emit('message', JSON.stringify({
    symbol: 'BTCUSDT',
    bids: [{ price: '100', quantity: '1' }],
    asks: [{ price: '101', quantity: '1' }],
    observed_at: 1_720_000_000_000,
    provider: 'internal',
  }))
  sockets[0]?.emit('message', JSON.stringify({
    symbol: 'BTCUSDT',
    trade_id: 'unused-trade',
    side: 'buy',
    price: '100',
    quantity: '1',
    traded_at: 1_720_000_000_000,
    provider: 'internal',
  }))
  sockets[0]?.emit('message', JSON.stringify({
    symbol: 'BTCUSDT',
    interval: '1m',
    open_time: 1_720_000_000_000,
    open: '100',
    high: '102',
    low: '99',
    close: '101',
    volume: '2',
    observed_at: 1_720_000_001_000,
    provider: 'internal',
  }))
  assert.equal(scheduler.frames.size, 1)
  scheduler.runFrames()
  assert.deepEqual(depths, [])
  assert.deepEqual(trades, [])
  assert.deepEqual(klines.map(({ time, close }) => ({ time, close })), [{
    time: 1_720_000_000_000,
    close: 101,
  }])

  sockets[0]?.emit('close')
  assert.equal(scheduler.timeouts.size, 1)
  scheduler.runNextTimeout()
  sockets[1]?.emit('open')
  assert.deepEqual(sockets[1]?.sent.map((frame) => JSON.parse(frame)), [
    { op: 'subscribe', channel: 'kline', symbol: 'BTCUSDT', interval: '1m' },
  ])

  stop()
  assert.equal(sockets[1]?.closed, true)
  assert.equal(scheduler.timeouts.size, 0)
})

test('detail stream reconnect delay grows exponentially and remains bounded', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const stop = startMarketDetailStream({
    symbol: 'BTCUSDT',
    interval: '15m',
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
    onKline: () => undefined,
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

test('detail stream rejects unsupported intervals before opening a socket', () => {
  let createCount = 0
  const stop = startMarketDetailStream({
    symbol: 'BTCUSDT',
    interval: '4h',
    url: 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      createCount += 1
      return new FakeSocket()
    },
    onDepth: () => undefined,
    onTrade: () => undefined,
    onKline: () => undefined,
  })

  assert.equal(createCount, 0)
  stop()
})

test('detail stream coalesces high-frequency depth snapshots and cancels pending writes on disconnect', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const depths: Array<{ bids: Array<{ price: number }>; asks: unknown[] }> = []
  const stop = startMarketDetailStream({
    symbol: 'BTCUSDT',
    interval: '15m',
    url: 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    onDepth: (snapshot) => depths.push(snapshot),
    onTrade: () => undefined,
    onKline: () => undefined,
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
  const cancelledFrame = [...scheduler.frames.values()][0]
  sockets[0]?.emit('error')
  sockets[0]?.emit('close')
  assert.equal(scheduler.frames.size, 0)
  assert.equal(scheduler.timeouts.size, 1)
  scheduler.runFrames()
  assert.equal(depths.length, 1)

  scheduler.runNextTimeout()
  sockets[1]?.emit('open')
  sockets[1]?.emit('message', JSON.stringify({
    symbol: 'BTCUSDT',
    bids: [{ price: 104, quantity: 1 }],
    asks: [],
    observed_at: 1_720_000_000_104,
  }))
  assert.equal(scheduler.frames.size, 1)
  cancelledFrame?.()
  scheduler.runFrames()
  assert.equal(depths.length, 2)
  assert.equal(depths[1]?.bids[0]?.price, 104)
  stop()
  assert.equal(scheduler.timeouts.size, 0)
})

test('detail stream commits only the latest valid kline per frame and clears pending klines on close or stop', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const klines: Array<{ time: number; close: number }> = []
  const stop = startMarketDetailStream({
    symbol: 'BTCUSDT',
    interval: '15m',
    url: 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    onDepth: () => undefined,
    onTrade: () => undefined,
    onKline: ({ time, close }) => klines.push({ time, close }),
  })
  const candle = (overrides: Record<string, unknown> = {}) => ({
    symbol: 'BTCUSDT',
    interval: '15m',
    open_time: 1_720_000_000_000,
    open: '100',
    high: '110',
    low: '90',
    close: '101',
    volume: '2',
    observed_at: 1_720_000_001_000,
    provider: 'bitget',
    ...overrides,
  })

  sockets[0]?.emit('open')
  sockets[0]?.emit('message', JSON.stringify(candle({ symbol: 'ETHUSDT' })))
  sockets[0]?.emit('message', JSON.stringify(candle({ interval: '1m' })))
  sockets[0]?.emit('message', JSON.stringify(candle({ volume: false })))
  assert.equal(scheduler.frames.size, 0)
  assert.equal(klines.length, 0)

  for (const close of [102, 103, 104]) {
    sockets[0]?.emit('message', JSON.stringify(candle({ close: String(close) })))
  }
  assert.equal(scheduler.frames.size, 1)
  const cancelledFrame = [...scheduler.frames.values()][0]
  sockets[0]?.emit('error')
  sockets[0]?.emit('error')
  sockets[0]?.emit('close')
  assert.equal(sockets[0]?.closeCount, 1)
  assert.equal(scheduler.frames.size, 0)
  scheduler.runFrames()
  assert.equal(klines.length, 0)

  scheduler.runNextTimeout()
  sockets[1]?.emit('open')
  for (const close of [105, 106, 107]) {
    sockets[1]?.emit('message', JSON.stringify(candle({
      close: String(close),
      observed_at: 1_720_000_001_000 + close,
    })))
  }
  assert.equal(scheduler.frames.size, 1)
  cancelledFrame?.()
  assert.equal(scheduler.frames.size, 1)
  scheduler.runFrames()
  assert.deepEqual(klines, [{ time: 1_720_000_000_000, close: 107 }])

  sockets[1]?.emit('message', JSON.stringify(candle({ provider: '', close: '999' })))
  assert.equal(scheduler.frames.size, 0)
  assert.deepEqual(klines, [{ time: 1_720_000_000_000, close: 107 }])

  sockets[1]?.emit('message', JSON.stringify(candle({ close: '108' })))
  assert.equal(scheduler.frames.size, 1)
  stop()
  assert.equal(scheduler.frames.size, 0)
  scheduler.runFrames()
  assert.equal(klines.length, 1)
  assert.equal(scheduler.timeouts.size, 0)
})

test('detail stream stop is idempotent while connecting and late events cannot resubscribe', () => {
  const sockets: FakeSocket[] = []
  const scheduler = new FakeScheduler()
  const stop = startMarketDetailStream({
    symbol: 'BTCUSDT',
    interval: '15m',
    url: 'wss://example.test/api/v1/ws/public',
    createSocket: () => {
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    scheduler,
    onDepth: () => undefined,
    onTrade: () => undefined,
    onKline: () => undefined,
  })

  stop()
  stop()
  assert.equal(sockets[0]?.closed, true)
  assert.equal(sockets[0]?.closeCount, 1)
  sockets[0]?.emit('error')
  sockets[0]?.emit('error')
  assert.equal(sockets[0]?.closeCount, 1)
  sockets[0]?.emit('open')
  sockets[0]?.emit('close')
  assert.equal(sockets[0]?.closeCount, 2)
  assert.deepEqual(sockets[0]?.sent, [])
  assert.equal(scheduler.intervals.size, 0)
  assert.equal(scheduler.timeouts.size, 0)
})

test('detail session executes live/REST races and isolates interval, request, and generation changes', async () => {
  const starts: MarketDetailStreamOptions[] = []
  let stopCount = 0
  let depthPrice = 0
  const tradeIds: string[] = []
  let renderedPoints: KlinePoint[] = []
  const session = createMarketDetailStreamSession({
    getUrl: () => 'wss://example.test/api/v1/ws/public',
    startStream: (options) => {
      starts.push(options)
      let stopped = false
      return () => {
        if (stopped) return
        stopped = true
        stopCount += 1
      }
    },
    onDepth: (_context, snapshot) => {
      depthPrice = snapshot.bids[0]?.price ?? 0
    },
    onTrade: (_context, trade) => {
      tradeIds.push(trade.id)
    },
    onKlines: (_context, points) => {
      renderedPoints = points
    },
  })
  const point = (time: number, close: number): KlinePoint => ({
    time,
    open: 100,
    high: Math.max(110, close),
    low: 90,
    close,
    volume: close,
  })

  const firstContext = session.replace('BTC/USDT', '15m', 1)
  const firstRequest = session.beginKlineRequest(firstContext)
  assert.ok(firstRequest)
  assert.deepEqual(
    starts.map(({ symbol, interval }) => ({ symbol, interval })),
    [{ symbol: 'BTCUSDT', interval: '15m' }],
  )

  starts[0]?.onDepth({ bids: [{ price: 101, quantity: 1 }], asks: [] })
  starts[0]?.onTrade({
    id: 'trade-15m',
    side: 'buy',
    price: 101,
    quantity: 1,
    time: 1_720_000_000_000,
  })
  const liveCurrent = point(1_720_000_060_000, 105)
  starts[0]?.onKline(liveCurrent)
  assert.equal(firstContext.depthReceived, true)
  assert.equal(firstContext.tradeReceived, true)
  assert.equal(firstContext.klineReceived, true)

  const lateRest = deferred<KlinePoint[]>()
  const lateCommit = lateRest.promise.then((rows) => (
    firstRequest ? session.resolveKlineRequest(firstRequest, rows) : null
  ))
  lateRest.resolve([
    point(1_720_000_000_000, 101),
    point(1_720_000_060_000, 102),
  ])
  const reconciled = await lateCommit
  assert.deepEqual(reconciled?.map(({ time, close }) => ({ time, close })), [
    { time: 1_720_000_000_000, close: 101 },
    { time: 1_720_000_060_000, close: 105 },
  ])
  assert.equal(session.currentPoints()[1]?.close, 105)

  const staleRestRequest = session.beginKlineRequest(firstContext)
  assert.ok(staleRestRequest)
  const oldDepth = depthPrice
  const oldTradeIds = [...tradeIds]
  const secondContext = session.replace('BTC/USDT', '5m', 1)
  assert.equal(stopCount, 1)
  assert.equal(session.currentPoints().length, 0)
  assert.equal(renderedPoints.at(-1)?.close, 105)
  assert.equal(depthPrice, oldDepth)
  assert.deepEqual(tradeIds, oldTradeIds)
  assert.deepEqual(
    starts.map(({ symbol, interval }) => ({ symbol, interval })),
    [
      { symbol: 'BTCUSDT', interval: '15m' },
      { symbol: 'BTCUSDT', interval: '5m' },
    ],
  )

  starts[0]?.onDepth({ bids: [{ price: 999, quantity: 1 }], asks: [] })
  starts[0]?.onTrade({
    id: 'stale-trade',
    side: 'sell',
    price: 999,
    quantity: 1,
    time: 1_720_000_001_000,
  })
  starts[0]?.onKline(point(1_720_000_120_000, 999))
  assert.equal(depthPrice, oldDepth)
  assert.deepEqual(tradeIds, oldTradeIds)
  assert.equal(session.currentPoints().length, 0)
  if (staleRestRequest) {
    assert.equal(session.resolveKlineRequest(staleRestRequest, [point(1_720_000_000_000, 999)]), null)
  }

  starts[1]?.onDepth({ bids: [{ price: 102, quantity: 1 }], asks: [] })
  starts[1]?.onTrade({
    id: 'trade-5m',
    side: 'buy',
    price: 102,
    quantity: 1,
    time: 1_720_000_002_000,
  })
  starts[1]?.onKline(point(1_720_000_300_000, 106))
  assert.equal(depthPrice, 102)
  assert.deepEqual(tradeIds, ['trade-15m', 'trade-5m'])
  assert.equal(session.currentPoints()[0]?.close, 106)

  const thirdContext = session.replace('BTC/USDT', '15m', 1)
  starts[0]?.onKline(point(1_720_000_120_000, 998))
  assert.equal(session.currentPoints().length, 0)
  starts[2]?.onKline(point(1_720_000_120_000, 107))
  assert.equal(session.currentPoints()[0]?.close, 107)
  assert.notEqual(thirdContext.generation, firstContext.generation)

  const supersededRequest = session.beginKlineRequest(thirdContext)
  const currentRequest = session.beginKlineRequest(thirdContext)
  assert.ok(supersededRequest)
  assert.ok(currentRequest)
  if (supersededRequest) {
    assert.equal(session.resolveKlineRequest(supersededRequest, [point(1_720_000_000_000, 500)]), null)
  }
  if (currentRequest) {
    assert.equal(session.resolveKlineRequest(currentRequest, [point(1_720_000_000_000, 103)])?.length, 2)
  }

  const symbolSwitchRequest = session.beginKlineRequest(thirdContext)
  assert.ok(symbolSwitchRequest)
  const fourthContext = session.replace('ETH_USDT', '1m', 2)
  starts[2]?.onDepth({ bids: [{ price: 997, quantity: 1 }], asks: [] })
  starts[2]?.onTrade({
    id: 'stale-symbol-trade',
    side: 'sell',
    price: 997,
    quantity: 1,
    time: 1_720_000_003_000,
  })
  starts[2]?.onKline(point(1_720_000_180_000, 997))
  assert.equal(session.currentPoints().length, 0)
  assert.equal(depthPrice, 102)
  assert.doesNotMatch(tradeIds.join(','), /stale-symbol-trade/)
  assert.equal(session.isCurrent(fourthContext, 'ETH/USDT', '1m', 2), true)
  assert.equal(session.isCurrent(fourthContext, 'BTCUSDT', '1m', 2), false)
  assert.equal(session.isCurrent(fourthContext, 'ETHUSDT', '1m', 1), false)
  if (symbolSwitchRequest) {
    assert.equal(session.resolveKlineRequest(symbolSwitchRequest, [point(1_720_000_000_000, 996)]), null)
  }
  starts[3]?.onKline(point(1_720_000_180_000, 108))
  assert.equal(session.currentPoints()[0]?.close, 108)

  session.stop()
  session.stop()
  starts[3]?.onKline(point(1_720_000_240_000, 109))
  assert.equal(stopCount, 4)
  assert.equal(session.current(), null)
  assert.equal(session.currentPoints().length, 1)
  assert.equal(session.isCurrent(secondContext), false)
})

test('MarketDetailView wires the shared interval source and executable detail session without clearing book/trades', () => {
  const source = readFileSync(new URL('../src/views/MarketDetailView.vue', import.meta.url), 'utf8')
  const load = source.match(/async function load[\s\S]*?\n}/)?.[0] ?? ''
  const chooseInterval = source.match(/function chooseInterval[\s\S]*?\n}/)?.[0] ?? ''

  assert.ok(load.indexOf('startLiveDetail(symbol, selectedInterval, version)') < load.indexOf('await Promise.allSettled'))
  assert.match(load, /if \(version !== requestVersion \|\| symbol !== pairSymbol\.value\) return/)
  assert.match(load, /beginKlineRequest\(liveState\)/)
  assert.match(load, /resolveKlineRequest\(klineRequest, restPoints\)/)
  assert.match(load, /if \(!liveState\.depthReceived && !currentDepthReceived\)/)
  assert.match(load, /mergeMarketTradeHistory\(trades\.value, restTrades, 16\)/)
  assert.match(source, /createMarketDetailStreamSession\(\{/)
  assert.match(source, /v-for="item in MARKET_KLINE_INTERVALS"/)
  assert.doesNotMatch(source, /['"]4h['"]/)
  assert.match(chooseInterval, /normalizeMarketKlineInterval\(value\)/)
  assert.ok(chooseInterval.indexOf('startLiveDetail(') < chooseInterval.indexOf('void refreshKlines(liveState)'))
  assert.doesNotMatch(chooseInterval, /bids\.value|asks\.value|trades\.value/)
  assert.doesNotMatch(chooseInterval, /void load\(/)
  assert.match(source, /onUnmounted\(\(\) => \{[\s\S]*stopLiveDetail\(\)/)
})
