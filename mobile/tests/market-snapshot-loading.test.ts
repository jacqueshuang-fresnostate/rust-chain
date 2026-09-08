import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import ts from 'typescript'
import { loadMarketDetailSnapshot } from '../src/api/marketDetailSnapshot.ts'
import { normalizeDecimalText } from '../src/core/decimal.ts'
import { createMarketDetailStreamSession, type MarketDetailStreamOptions, type MarketDetailStreamSessionOptions } from '../src/api/marketDetailStream.ts'
import { mergeMarketTradeHistory, mergeMarketTrades, normalizeMarketKlineInterval } from '../src/api/marketSocketProtocol.ts'
import type { KlinePoint, OrderBookLevel, TradePrint } from '../src/core/types.ts'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
const flush = () => new Promise<void>((resolve) => setImmediate(resolve))
const point = (close: number): KlinePoint => ({ time: 1_720_000_020_000, open: close, high: close, low: close, close, volume: 1 })
const trade = (id: string): TradePrint => ({ id, price: 10, quantity: 1, side: 'buy', time: 1_720_000_020_000 })
type Depth = { bids: OrderBookLevel[]; asks: OrderBookLevel[] }
const book = (price: number): Depth => ({ bids: [{ price, quantity: 1, priceText: normalizeDecimalText(String(price))!, quantityText: normalizeDecimalText('1')! }], asks: [] })

// Execute the real view loader/control flow with deferred transports and the real
// stream session. No browser, backend, or fabricated copy of production decisions.
function harness(view: 'MarketDetailView' | 'TradeView') {
  const source = readFileSync(new URL(`../src/views/${view}.vue`, import.meta.url), 'utf8')
  const detail = view === 'MarketDetailView'
  const names = detail
    ? ['load', 'isCurrentLiveDetail', 'startLiveDetail', 'refreshKlines', 'chooseInterval']
    : ['loadMarketData', 'isCurrentMarketRequest', 'refreshIntervalKlines', 'chooseInterval']
  const functions = names.map((name) => {
    const match = source.match(new RegExp(`(?:async )?function ${name}\\([\\s\\S]*?\\n}`))
    assert.ok(match, `${view}.${name} exists`)
    return match[0]
  }).join('\n')
  const stream = source.match(/const detailStreamSession = createMarketDetailStreamSession\([\s\S]*?\n}\)/)?.[0]
  assert.ok(stream)
  const streams: MarketDetailStreamOptions[] = []
  const klines: ReturnType<typeof deferred<KlinePoint[]>>[] = []
  const depths: ReturnType<typeof deferred<Depth>>[] = []
  const histories: ReturnType<typeof deferred<TradePrint[]>>[] = []
  const state = {
    points: { value: [] as KlinePoint[] }, bids: { value: [] as OrderBookLevel[] }, asks: { value: [] as OrderBookLevel[] }, trades: { value: [] as TradePrint[] },
    pairSymbol: { value: 'BTC/USDT' }, interval: { value: '1m' }, chartLoading: { value: false },
    loading: { value: false }, tradesLoading: { value: false }, depthLoading: { value: false }, klineError: { value: false }, depthError: { value: false }, tradesError: { value: false },
    liveDepthReceived: { value: false }, liveDetailActive: { value: false }, liveDetailUpdatedAt: { value: 0 },
  }
  const bindings = {
    ...state, requestVersion: 0, marketRequestVersion: 0, viewActive: true,
    createMarketDetailStreamSession: (options: MarketDetailStreamSessionOptions) => createMarketDetailStreamSession({
      ...options, startStream: (options) => { streams.push(options); return () => undefined },
    }),
    publicMarketWebSocketUrl: () => 'wss://example.test/ws',
    fetchKlines: () => { const request = deferred<KlinePoint[]>(); klines.push(request); return request.promise },
    fetchOrderBook: () => { const request = deferred<Depth>(); depths.push(request); return request.promise },
    fetchRecentTrades: () => { const request = deferred<TradePrint[]>(); histories.push(request); return request.promise },
    marketStore: { refresh: async () => undefined },
    loadMarketDetailSnapshot, mergeMarketTradeHistory, mergeMarketTrades, normalizeMarketKlineInterval,
  }
  const code = ts.transpileModule(`${stream}\n${functions}`, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } }).outputText
  const api = new Function('bindings', `${Object.keys(bindings).map((key) => `let ${key} = bindings.${key};`).join('\n')}\n${code}\nreturn { load: ${detail ? 'load' : 'loadMarketData'}, chooseInterval, stop() { viewActive = false; detailStreamSession.stop() } };`)(bindings) as {
    load(): Promise<void>; chooseInterval(value: string): void; stop(): void
  }
  return { ...api, ...state, streams, klines, depths, histories }
}

for (const view of ['MarketDetailView', 'TradeView'] as const) {
  test(`${view}: ready K-lines do not wait for order-book or trade REST latency`, async () => {
    const state = harness(view)
    const loading = state.load()
    state.klines[0]!.resolve([point(11)])
    await flush()
    assert.equal(state.points.value[0]?.close, 11)
    assert.equal(state.chartLoading.value, false)
    assert.equal(state.depthLoading.value, true)
    assert.equal(state.tradesLoading.value, true)
    state.depths[0]!.resolve(book(10)); state.histories[0]!.resolve([trade('rest')])
    await loading
    state.stop()
  })

  test(`${view}: interval switch retains pending symbol-owned book/trades without old candles`, async () => {
    const state = harness(view)
    const loading = state.load()
    state.chooseInterval('5m')
    state.klines[1]!.resolve([point(15)])
    state.klines[0]!.resolve([point(1)])
    state.depths[0]!.resolve(book(12)); state.histories[0]!.resolve([trade('rest')])
    await loading; await flush()
    assert.equal(state.points.value[0]?.close, 15)
    assert.equal(state.bids.value[0]?.price, 12)
    assert.deepEqual(state.trades.value.map(({ id }) => id), ['rest'])
    assert.equal(state.chartLoading.value, false)
    assert.equal(state.depthLoading.value, false)
    state.stop()
  })

  test(`${view}: live candles stay visible but do not declare REST history settled`, async () => {
    const state = harness(view)
    const loading = state.load()
    state.streams[0]!.onKline(point(19))
    assert.equal(state.points.value[0]?.close, 19)
    assert.equal(state.chartLoading.value, true)
    state.klines[0]!.reject(new Error('history unavailable'))
    await flush()
    assert.equal(state.chartLoading.value, false)
    assert.equal(state.points.value[0]?.close, 19)
    assert.equal(state.klineError.value, false)
    state.depths[0]!.resolve(book(10)); state.histories[0]!.resolve([])
    await loading
    state.stop()
  })

  test(`${view}: late REST does not overwrite a newer interval's live book or trades`, async () => {
    const state = harness(view)
    const loading = state.load()
    state.chooseInterval('5m')
    state.streams[1]!.onDepth(book(22)); state.streams[1]!.onTrade(trade('live'))
    state.klines[1]!.resolve([point(15)]); state.klines[0]!.resolve([point(1)])
    state.depths[0]!.resolve(book(12)); state.histories[0]!.resolve([trade('rest')])
    await loading; await flush()
    assert.equal(state.bids.value[0]?.price, 22)
    assert.deepEqual(new Set(state.trades.value.map(({ id }) => id)), new Set(['rest', 'live']))
    state.stop()
  })

  test(`${view}: stopped view rejects every pending snapshot`, async () => {
    const state = harness(view)
    const loading = state.load()
    state.stop()
    state.klines[0]!.resolve([point(1)]); state.depths[0]!.resolve(book(1)); state.histories[0]!.resolve([trade('old')])
    await loading
    assert.deepEqual(state.points.value, []); assert.deepEqual(state.bids.value, []); assert.deepEqual(state.trades.value, [])
  })

  test(`${view}: ready book and trades do not wait for history`, async () => {
    const state = harness(view)
    const loading = state.load()
    state.depths[0]!.resolve(book(18)); state.histories[0]!.resolve([trade('ready')])
    await flush()
    assert.equal(state.bids.value[0]?.price, 18)
    assert.equal(state.trades.value[0]?.id, 'ready')
    assert.equal(state.chartLoading.value, true)
    assert.equal(state.depthLoading.value, false)
    assert.equal(state.tradesLoading.value, false)
    state.klines[0]!.resolve([point(1)])
    await loading
    state.stop()
  })

  test(`${view}: live empty book remains authoritative across multiple interval switches`, async () => {
    const state = harness(view)
    const loading = state.load()
    state.chooseInterval('5m')
    state.streams[1]!.onDepth({ bids: [], asks: [] })
    state.chooseInterval('15m')
    state.klines.forEach((request) => request.resolve([]))
    state.depths[0]!.resolve(book(12)); state.histories[0]!.resolve([])
    await loading; await flush()
    assert.deepEqual(state.bids.value, [])
    assert.equal(state.depthError.value, false)
    state.stop()
  })

  test(`${view}: symbol switch and same-symbol retry reject previous load results`, async () => {
    const state = harness(view)
    const oldLoad = state.load()
    state.pairSymbol.value = 'ETH/USDT'
    const nextLoad = state.load()
    const retry = state.load()
    state.klines[2]!.resolve([point(30)]); state.depths[2]!.resolve(book(30)); state.histories[2]!.resolve([trade('current')])
    await retry
    for (const index of [0, 1]) {
      state.klines[index]!.resolve([point(1)]); state.depths[index]!.resolve(book(1)); state.histories[index]!.resolve([trade('stale')])
    }
    await Promise.all([oldLoad, nextLoad])
    assert.equal(state.points.value[0]?.close, 30)
    assert.equal(state.bids.value[0]?.price, 30)
    assert.deepEqual(state.trades.value.map(({ id }) => id), ['current'])
    state.stop()
  })

  test(`${view}: a new interval does not display the previous interval's candles`, async () => {
    const state = harness(view)
    const loading = state.load()
    state.klines[0]!.resolve([point(1)]); state.depths[0]!.resolve(book(10)); state.histories[0]!.resolve([])
    await loading
    state.chooseInterval('5m')
    assert.equal(state.points.value.length, 0)
    assert.equal(state.chartLoading.value, true)
    assert.equal(state.bids.value[0]?.price, 10, 'interval change preserves symbol book')
    state.klines[1]!.resolve([point(5)])
    await flush()
    assert.equal(state.points.value[0]?.close, 5)
    state.stop()
  })

  test(`${view}: individual request failures settle loading without suppressing successful channels`, async () => {
    const state = harness(view)
    const loading = state.load()
    state.depths[0]!.reject(new Error('depth unavailable')); state.histories[0]!.resolve([trade('available')])
    await flush()
    assert.equal(state.depthError.value, true)
    assert.equal(state.depthLoading.value, false)
    assert.equal(state.trades.value[0]?.id, 'available')
    state.klines[0]!.reject(new Error('history unavailable'))
    await loading
    assert.equal(state.chartLoading.value, false)
    if (view === 'MarketDetailView') assert.equal(state.klineError.value, true)
    state.stop()
  })
}

test('views bind book/trade indicators to their own channels', () => {
  const detail = readFileSync(new URL('../src/views/MarketDetailView.vue', import.meta.url), 'utf8')
  const trade = readFileSync(new URL('../src/views/TradeView.vue', import.meta.url), 'utf8')
  assert.match(detail, /:loading="depthLoading"/)
  assert.match(detail, /v-if="tradesLoading && !trades.length"/)
  assert.match(trade, /tradesLoading \? t\('common.loading'\) : t\('trade.noRecentTrades'\)/)
})
