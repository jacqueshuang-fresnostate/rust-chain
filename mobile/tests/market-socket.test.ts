import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  DEFAULT_MARKET_KLINE_LIMIT,
  MARKET_KLINE_INTERVALS,
  depthSubscriptionFrame,
  klineSubscriptionFrame,
  mapMarketDepthSnapshot,
  mapMarketKline,
  mapMarketKlines,
  mapMarketTrades,
  marketUnsubscriptionFrame,
  mergeMarketKlines,
  mergeMarketTradeHistory,
  mergeMarketTrades,
  normalizeMarketKlineInterval,
  parseMarketSocketFrame,
  tickerSubscriptionFrame,
  tickerUnsubscriptionFrame,
  tradeSubscriptionFrame,
} from '../src/api/marketSocketProtocol.ts'

const marketApiSource = readFileSync(new URL('../src/api/market.ts', import.meta.url), 'utf8')

test('market WebSocket subscription payloads match every backend public channel contract', () => {
  assert.deepEqual(MARKET_KLINE_INTERVALS, ['1m', '5m', '15m', '1h', '1d'])
  assert.equal(normalizeMarketKlineInterval('5M'), '5m')
  assert.equal(normalizeMarketKlineInterval('4h'), '')
  assert.deepEqual(JSON.parse(tickerSubscriptionFrame('btc/usdt')), {
    op: 'subscribe',
    channel: 'ticker',
    symbol: 'BTCUSDT',
  })
  assert.deepEqual(JSON.parse(tickerUnsubscriptionFrame('btc/usdt')), {
    op: 'unsubscribe',
    channel: 'ticker',
    symbol: 'BTCUSDT',
  })
  assert.deepEqual(JSON.parse(depthSubscriptionFrame('btc-usdt')), {
    op: 'subscribe',
    channel: 'depth',
    symbol: 'BTCUSDT',
  })
  assert.deepEqual(JSON.parse(tradeSubscriptionFrame('btc_usdt')), {
    op: 'subscribe',
    channel: 'trade',
    symbol: 'BTCUSDT',
  })
  assert.deepEqual(JSON.parse(klineSubscriptionFrame('btc usdt', '15M')), {
    op: 'subscribe',
    channel: 'kline',
    symbol: 'BTCUSDT',
    interval: '15m',
  })
  assert.deepEqual(JSON.parse(klineSubscriptionFrame('btc usdt', '5m')), {
    op: 'subscribe',
    channel: 'kline',
    symbol: 'BTCUSDT',
    interval: '5m',
  })
  assert.throws(() => klineSubscriptionFrame('BTCUSDT', '4h'), /supported kline interval/)
  assert.deepEqual(JSON.parse(marketUnsubscriptionFrame('kline', 'BTCUSDT', '1M')), {
    op: 'unsubscribe',
    channel: 'kline',
    symbol: 'BTCUSDT',
    interval: '1m',
  })
})

test('market WebSocket parser preserves confirmations, complete ticker snapshots, and text heartbeat frames', () => {
  assert.deepEqual(
    parseMarketSocketFrame('{"type":"subscribed","channel":"public:depth:BTCUSDT"}'),
    { type: 'subscribed', channel: 'public:depth:BTCUSDT' },
  )
  assert.deepEqual(
    parseMarketSocketFrame('{"type":"subscribed","channel":"public:trade:BTCUSDT"}'),
    { type: 'subscribed', channel: 'public:trade:BTCUSDT' },
  )
  assert.deepEqual(
    parseMarketSocketFrame(JSON.stringify({
      symbol: 'BTC-USDT',
      last_price: '61234.5',
      high_24h: '62000.25',
      low_24h: '60100.75',
      volume_24h: '1234.567',
      price_change_percent_24h: '-0.79700',
      observed_at: 1_720_000_000_000,
    })),
    {
      type: 'ticker',
      symbol: 'BTC-USDT',
      lastPrice: 61234.5,
      highPrice: 62000.25,
      lowPrice: 60100.75,
      volume: 1234.567,
      changePercent: -0.797,
      observedAt: 1_720_000_000_000,
    },
  )
  assert.deepEqual(
    parseMarketSocketFrame('{"symbol":"BTC-USDT","last_price":"61234.5"}'),
    { type: 'ticker', symbol: 'BTC-USDT', lastPrice: 61234.5 },
  )
  assert.deepEqual(parseMarketSocketFrame('pong'), { type: 'pong' })
  assert.equal(parseMarketSocketFrame('{"type":"error","code":"invalid_request"}'), null)
  assert.equal(parseMarketSocketFrame('{"symbol":"BTCUSDT","last_price":null}'), null)
  assert.equal(parseMarketSocketFrame('{"symbol":"BTCUSDT","last_price":true}'), null)
  assert.equal(parseMarketSocketFrame('{"symbol":"BTCUSDT","last_price":"0"}'), null)
  assert.equal(parseMarketSocketFrame('{"symbol":"BTCUSDT","last_price":"1","high_24h":true}'), null)
  assert.equal(parseMarketSocketFrame('{"symbol":"BTCUSDT","last_price":"1","low_24h":"0"}'), null)
  assert.equal(parseMarketSocketFrame('{"symbol":"BTCUSDT","last_price":"1","volume_24h":"-1"}'), null)
  assert.equal(parseMarketSocketFrame('{"symbol":"BTCUSDT","last_price":"1","price_change_percent_24h":""}'), null)
  assert.equal(parseMarketSocketFrame('not-json'), null)
})

test('depth frames map the verified full snapshot shape, sort both sides, and cap each side to 12', () => {
  const bids = Array.from({ length: 14 }, (_, index) => ({
    price: String(100 + index),
    quantity: String(index + 1),
  }))
  const asks = Array.from({ length: 14 }, (_, index) => ({
    price: 220 - index,
    quantity: index + 1,
  }))
  const frame = parseMarketSocketFrame(JSON.stringify({
    symbol: 'BTCUSDT',
    bids,
    asks,
    observed_at: 1_720_000_000_000,
    provider: 'binance',
  }))

  assert.equal(frame?.type, 'depth')
  if (!frame || frame.type !== 'depth') return
  assert.deepEqual(frame.bids.map((row) => row.price), [
    113, 112, 111, 110, 109, 108, 107, 106, 105, 104, 103, 102,
  ])
  assert.deepEqual(frame.asks.map((row) => row.price), [
    207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218,
  ])
  assert.equal(frame.observedAt, 1_720_000_000_000)
})

test('trade frames map the verified direct payload shape to a normalized trade print', () => {
  assert.deepEqual(
    parseMarketSocketFrame(JSON.stringify({
      symbol: 'BTCUSDT',
      trade_id: 'trade-42',
      side: 'SELL',
      price: '61234.5',
      quantity: '0.125',
      traded_at: 1_720_000_000,
      provider: 'binance',
    })),
    {
      type: 'trade',
      symbol: 'BTCUSDT',
      trade: {
        id: 'trade-42',
        side: 'sell',
        price: 61234.5,
        quantity: 0.125,
        time: 1_720_000_000_000,
      },
    },
  )
})

test('kline frames strictly map the verified direct payload shape with millisecond timestamps', () => {
  assert.deepEqual(
    parseMarketSocketFrame(JSON.stringify({
      symbol: 'BTC-USDT',
      interval: '15M',
      open_time: 1_720_000_000_000,
      open: '61000.5',
      high: '61300',
      low: '60900',
      close: '61234.5',
      volume: '0',
      observed_at: 1_720_000_001_000,
      provider: 'binance',
    })),
    {
      type: 'kline',
      symbol: 'BTC-USDT',
      interval: '15m',
      point: {
        time: 1_720_000_000_000,
        open: 61000.5,
        high: 61300,
        low: 60900,
        close: 61234.5,
        volume: 0,
      },
      observedAt: 1_720_000_001_000,
    },
  )
})

test('shared kline adapters sort, deduplicate, upsert live candles, and retain the newest configured limit', () => {
  const mapped = mapMarketKlines([
    { open_time: 1_720_000_120, open: 102, high: 105, low: 101, close: 104, volume: 3 },
    { open_time: 1_720_000_000, open: 100, high: 103, low: 99, close: 102, volume: 1 },
    { open_time: 1_720_000_000_000, open: 100, high: 104, low: 98, close: 103, volume: 2 },
    { open_time: 1_720_000_060_000, open: 103, high: 102, low: 100, close: 101, volume: 1 },
  ], 2)
  assert.deepEqual(mapped, [
    {
      time: 1_720_000_000_000,
      open: 100,
      high: 104,
      low: 98,
      close: 103,
      volume: 2,
    },
    {
      time: 1_720_000_120_000,
      open: 102,
      high: 105,
      low: 101,
      close: 104,
      volume: 3,
    },
  ])

  const rest = mapMarketKlines([
    { open_time: 1_720_000_000_000, open: 100, high: 103, low: 99, close: 101, volume: 1 },
    { open_time: 1_720_000_060_000, open: 101, high: 104, low: 100, close: 102, volume: 2 },
  ])
  const live = mapMarketKline({
    open_time: 1_720_000_060_000,
    open: 101,
    high: 106,
    low: 100,
    close: 105,
    volume: 4,
  })
  const appended = mapMarketKline({
    open_time: 1_720_000_120_000,
    open: 105,
    high: 107,
    low: 104,
    close: 106,
    volume: 1,
  })
  assert.ok(live)
  assert.ok(appended)
  if (!live || !appended) return

  const reconciled = mergeMarketKlines([live, appended], rest, DEFAULT_MARKET_KLINE_LIMIT)
  assert.deepEqual(reconciled.map((point) => point.time), [
    1_720_000_000_000,
    1_720_000_060_000,
    1_720_000_120_000,
  ])
  assert.equal(reconciled[1]?.close, 105)
  assert.equal(new Set(reconciled.map((point) => point.time)).size, reconciled.length)
  assert.deepEqual(mergeMarketKlines([appended], reconciled, 2).map((point) => point.time), [
    1_720_000_060_000,
    1_720_000_120_000,
  ])

  const overLimit = Array.from({ length: DEFAULT_MARKET_KLINE_LIMIT + 1 }, (_, index) => ({
    time: 1_720_000_000_000 + index * 60_000,
    open: 100,
    high: 101,
    low: 99,
    close: 100,
    volume: index,
  }))
  const capped = mergeMarketKlines(overLimit, [])
  assert.equal(capped.length, DEFAULT_MARKET_KLINE_LIMIT)
  assert.equal(capped[0]?.time, 1_720_000_060_000)
  assert.equal(capped.at(-1)?.time, 1_720_000_000_000 + DEFAULT_MARKET_KLINE_LIMIT * 60_000)
})

test('shared REST adapters filter invalid rows before sorting, dedupe trades, and enforce limits', () => {
  const snapshot = mapMarketDepthSnapshot({
    bids: [
      { price: '99', amount: '1.5' },
      { price: '101', quantity: '0.5' },
      { price: 100, quantity: 2 },
      { price: 0, quantity: 8 },
    ],
    asks: [
      { price: '104', amount: '1' },
      { price: '102', quantity: '2' },
      { price: 103, quantity: 3 },
      { price: 105, quantity: 0 },
    ],
  }, 2)
  assert.deepEqual(snapshot, {
    bids: [
      { price: 101, quantity: 0.5 },
      { price: 100, quantity: 2 },
    ],
    asks: [
      { price: 102, quantity: 2 },
      { price: 103, quantity: 3 },
    ],
  })

  const trades = mapMarketTrades([
    { id: 'older', direction: 'buy', price: '100', amount: '2', time: 1_720_000_000 },
    { id: 'newer', side: 'sell', price: 102, quantity: 1, traded_at: 1_720_000_002_000 },
    { id: 'older', side: 'sell', price: 999, quantity: 1, traded_at: 1_720_000_003_000 },
    { id: 'invalid', side: 'hold', price: 101, quantity: 1, traded_at: 1_720_000_001_000 },
  ], 2)
  assert.deepEqual(trades.map((trade) => trade.id), ['older', 'newer'])
  assert.equal(trades[0]?.price, 999)
})

test('REST market functions preserve endpoint envelopes while delegating to the shared adapters', () => {
  const klineSource = marketApiSource.match(
    /export async function fetchKlines[\s\S]*?\n}/,
  )?.[0] ?? ''
  const depthSource = marketApiSource.match(
    /export async function fetchOrderBook[\s\S]*?\n}/,
  )?.[0] ?? ''
  const tradesSource = marketApiSource.match(
    /export async function fetchRecentTrades[\s\S]*?\n}/,
  )?.[0] ?? ''

  assert.match(klineSource, /requestUrl\(`\/markets\/\$\{encodeURIComponent\(normalizeSymbol\(symbol\)\)\}\/klines`\)/)
  assert.match(klineSource, /\{ params: \{ interval, start, end, limit } }/)
  assert.match(klineSource, /return mapMarketKlines\(rawRows, limit\)/)
  assert.match(depthSource, /requestUrl\(`\/markets\/\$\{encodeURIComponent\(normalizeSymbol\(symbol\)\)\}\/depth`\)/)
  assert.match(depthSource, /return mapMarketDepthSnapshot\(response\.data\)/)
  assert.match(tradesSource, /requestUrl\(`\/markets\/\$\{encodeURIComponent\(normalizeSymbol\(symbol\)\)\}\/trades`\)/)
  assert.match(tradesSource, /\{ params: \{ limit } }/)
  assert.match(tradesSource, /Array\.isArray\(response\.data\.trades\)/)
  assert.match(tradesSource, /return mapMarketTrades\(rows, limit\)/)
})

test('live trade merging prepends unique arrivals, ignores replayed IDs, and caps retained state at 16', () => {
  const current = Array.from({ length: 16 }, (_, index) => ({
    id: `trade-${index}`,
    side: index % 2 === 0 ? 'buy' as const : 'sell' as const,
    price: 100 + index,
    quantity: 1,
    time: 1_720_000_000_000 - index,
  }))
  const replayed = mergeMarketTrades(current, {
    id: 'trade-5',
    side: 'sell',
    price: 999,
    quantity: 3,
    time: 1_720_000_001_000,
  })
  assert.deepEqual(replayed.map((trade) => trade.id), current.map((trade) => trade.id))
  assert.equal(replayed[5]?.price, 105)
  assert.equal(replayed.filter((trade) => trade.id === 'trade-5').length, 1)

  const inserted = mergeMarketTrades(replayed, {
    id: 'trade-new',
    side: 'buy',
    price: 1_000,
    quantity: 4,
    time: 1,
  })
  assert.equal(inserted.length, 16)
  assert.equal(inserted[0]?.id, 'trade-new')
  assert.equal(new Set(inserted.map((trade) => trade.id)).size, 16)
})

test('REST history reconciliation keeps already-rendered live trades first and deduplicates overlaps', () => {
  const live = [{
    id: 'live',
    side: 'sell' as const,
    price: 103,
    quantity: 0.5,
    time: 1_720_000_003_000,
  }]
  const rest = [
    {
      id: 'live',
      side: 'sell' as const,
      price: 999,
      quantity: 9,
      time: 1_720_000_003_000,
    },
    {
      id: 'rest',
      side: 'buy' as const,
      price: 102,
      quantity: 1,
      time: 1_720_000_002_000,
    },
  ]
  assert.deepEqual(mergeMarketTradeHistory(live, rest, 16), [
    live[0],
    rest[1],
  ])
})

test('malformed depth, trade, and kline frames are ignored instead of replacing valid fallback state', () => {
  assert.equal(parseMarketSocketFrame(JSON.stringify({
    symbol: 'BTCUSDT',
    bids: [{ price: 'bad', quantity: 1 }],
    asks: [],
    observed_at: 1_720_000_000_000,
  })), null)
  assert.equal(parseMarketSocketFrame(JSON.stringify({
    symbol: 'BTCUSDT',
    trade_id: 'bad-side',
    side: 'hold',
    price: 100,
    quantity: 1,
    traded_at: 1_720_000_000_000,
  })), null)
  assert.equal(parseMarketSocketFrame(JSON.stringify({
    symbol: 'BTCUSDT',
    trade_id: 'bad-quantity',
    side: 'buy',
    price: 100,
    quantity: 0,
    traded_at: 1_720_000_000_000,
  })), null)
  assert.equal(parseMarketSocketFrame(JSON.stringify({
    symbol: 'BTCUSDT',
    bids: [{ price: true, quantity: 1 }],
    asks: [],
    observed_at: 1_720_000_000_000,
  })), null)
  assert.equal(parseMarketSocketFrame(JSON.stringify({
    symbol: 'BTCUSDT',
    trade_id: 'bad-time',
    side: 'buy',
    price: 100,
    quantity: 1,
    traded_at: true,
  })), null)
  const validKline = {
    symbol: 'BTCUSDT',
    interval: '15m',
    open_time: 1_720_000_000_000,
    open: '100',
    high: '105',
    low: '99',
    close: '103',
    volume: '2',
    observed_at: 1_720_000_001_000,
    provider: 'bitget',
  }
  for (const payload of [
    { ...validKline, interval: 'fifteen-minutes' },
    { ...validKline, interval: '4h' },
    {
      ...validKline,
      interval: '4h',
      trade_id: 'must-not-fall-through',
      side: 'buy',
      price: '103',
      quantity: '1',
      traded_at: 1_720_000_001_000,
    },
    { ...validKline, observed_at: true },
    { ...validKline, observed_at: 1_720_000_001 },
    { ...validKline, open_time: undefined },
    { ...validKline, open_time: 1_720_000_000 },
    { ...validKline, close: false },
    { ...validKline, close: 103 },
    { ...validKline, close: '1e2' },
    { ...validKline, volume: '-1' },
    { ...validKline, high: '102', close: '103' },
    { ...validKline, provider: '' },
    { ...validKline, provider: undefined },
  ]) {
    assert.equal(parseMarketSocketFrame(JSON.stringify(payload)), null)
  }
})
