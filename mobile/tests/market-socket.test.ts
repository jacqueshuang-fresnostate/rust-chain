import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  depthSubscriptionFrame,
  mapMarketDepthSnapshot,
  mapMarketTrades,
  mergeMarketTradeHistory,
  mergeMarketTrades,
  parseMarketSocketFrame,
  tickerSubscriptionFrame,
  tradeSubscriptionFrame,
} from '../src/api/marketSocketProtocol.ts'

const marketApiSource = readFileSync(new URL('../src/api/market.ts', import.meta.url), 'utf8')

test('market WebSocket subscription payloads match every backend public channel contract', () => {
  assert.deepEqual(JSON.parse(tickerSubscriptionFrame('btc/usdt')), {
    op: 'subscribe',
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
})

test('market WebSocket parser preserves confirmations, ticker, and text heartbeat frames', () => {
  assert.deepEqual(
    parseMarketSocketFrame('{"type":"subscribed","channel":"public:depth:BTCUSDT"}'),
    { type: 'subscribed', channel: 'public:depth:BTCUSDT' },
  )
  assert.deepEqual(
    parseMarketSocketFrame('{"type":"subscribed","channel":"public:trade:BTCUSDT"}'),
    { type: 'subscribed', channel: 'public:trade:BTCUSDT' },
  )
  assert.deepEqual(
    parseMarketSocketFrame('{"symbol":"BTC-USDT","last_price":"61234.5","observed_at":1720000000000}'),
    { type: 'ticker', symbol: 'BTC-USDT', lastPrice: 61234.5, observedAt: 1720000000000 },
  )
  assert.deepEqual(parseMarketSocketFrame('pong'), { type: 'pong' })
  assert.equal(parseMarketSocketFrame('{"type":"error","code":"invalid_request"}'), null)
  assert.equal(parseMarketSocketFrame('{"symbol":"BTCUSDT","last_price":null}'), null)
  assert.equal(parseMarketSocketFrame('{"symbol":"BTCUSDT","last_price":true}'), null)
  assert.equal(parseMarketSocketFrame('{"symbol":"BTCUSDT","last_price":"0"}'), null)
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
  const depthSource = marketApiSource.match(
    /export async function fetchOrderBook[\s\S]*?\n}/,
  )?.[0] ?? ''
  const tradesSource = marketApiSource.match(
    /export async function fetchRecentTrades[\s\S]*?\n}/,
  )?.[0] ?? ''

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

test('malformed depth and trade frames are ignored instead of replacing REST fallback state', () => {
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
})
