import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'
import { mapMarketDepthSnapshot, mapMarketKline, mapMarketTrades, mergeMarketTradeHistory, mergeMarketTrades, parseMarketSocketFrame } from '../src/api/marketSocketProtocol.ts'
import { mapMarketTicker } from '../src/core/marketMapper.ts'
import { applyLiveMarketTickerUpdate, mergeMarketTickerSnapshots } from '../src/core/marketTickerFreshness.ts'
import { normalizeMarketChartPoints } from '../src/core/marketChart.ts'
import { mapMarketProvenance, marketSource } from '../src/core/marketProvenance.ts'
import zh from '../src/i18n/messages/zh-CN.ts'
import en from '../src/i18n/messages/en.ts'

const print = { id: '7', symbol: 'BTCUSDT', price: '10', amount: '2', direction: 'BUY', time: 1_790_000_000_000 }

test('REST and WS retain source/provider for platform, external and both generator sources', () => {
  for (const [source, provider] of [['platform', 'platform'], ['external', 'coinbase'], ['strategy', 'strategy'], ['default', 'strategy'], ['generated', 'strategy'], ['unknown', 'future-provider']]) {
    const provenance = { source, provider }
    const [rest] = mapMarketTrades([{ ...print, ...provenance }])
    const live = parseMarketSocketFrame(JSON.stringify({ ...print, ...provenance, trade_id: print.id }))
    assert.ok(rest && live?.type === 'trade')
    assert.deepEqual(live.trade, rest)
    assert.equal(rest.source, source)
    assert.equal(rest.provider, provider)
    const depth = { symbol: print.symbol, bids: [], asks: [], ...provenance }
    const restBook = mapMarketDepthSnapshot(depth)
    const liveBook = parseMarketSocketFrame(JSON.stringify(depth))
    assert.ok(liveBook?.type === 'depth')
    assert.deepEqual(restBook.provenance, provenance)
    assert.deepEqual(liveBook.provenance, provenance)
  }
})

test('missing, unknown or conflicting evidence never becomes a platform trade', () => {
  for (const payload of [{}, { provider: 'platform' }, { source: 'real' }, { source: 'platform' }, { source: 'platform', provider: 'strategy' }, { source: 'unknown', provider: 'coinbase' }, { provider: 'future-provider' }]) {
    assert.equal(marketSource(mapMarketProvenance(payload)), 'unknown')
  }
  assert.equal(marketSource({ provider: 'htx' }), 'external')
  assert.equal(marketSource({ provider: 'strategy' }), 'generated')
  for (const source of [null, '', ' ', 42, {}]) {
    assert.equal(marketSource(mapMarketProvenance({ source, provider: 'htx' })), 'unknown')
  }
  for (const key of ['platform', 'external', 'strategy', 'default', 'generated', 'unknown'] as const) {
    assert.ok(zh.marketProvenance[key])
    assert.ok(en.marketProvenance[key])
  }
})

test('same IDs from different providers/sources coexist; replay only deduplicates exact source identity', () => {
  const rows = mapMarketTrades([
    { ...print, source: 'platform', provider: 'platform' },
    { ...print, source: 'external', provider: 'htx' },
    { ...print, source: 'external', provider: 'coinbase' },
  ])
  assert.equal(rows.length, 3)
  assert.equal(mergeMarketTradeHistory(rows, rows).length, 3)
  assert.equal(mergeMarketTrades(rows, rows[0]!).length, 3)
})

test('all market/trade book hosts and per-print rows render provenance without inferring from market type', () => {
  for (const view of ['MarketDetailView', 'TradeView']) {
    const source = readFileSync(new URL(`../src/views/${view}.vue`, import.meta.url), 'utf8')
    const books = source.match(/<OrderBookPanel\b[\s\S]*?\/>/g) ?? []
    assert.ok(books.length)
    assert.ok(books.every((book) => book.includes(':provenance="depthProvenance"')))
    assert.ok(source.includes('<MarketProvenanceLabel :provenance="trade" />') || source.includes(':trade="trade" :format-value="moneyText"'))
    assert.ok(source.includes(':key="marketTradeIdentity(trade)"'))
  }
  const row = readFileSync(new URL('../src/components/MarketTradeRow.vue', import.meta.url), 'utf8')
  assert.ok(row.includes('<MarketProvenanceLabel :provenance="trade" />'))
})

test('ticker source follows authoritative snapshot across late REST and unknown live replacement', () => {
  const now = 1790000000000
  const old = mapMarketTicker({ symbol: 'BTCUSDT' }, { last_price: '10', observed_at: now, source: 'external', provider: 'htx' })
  const frame = parseMarketSocketFrame(JSON.stringify({
    symbol: 'BTCUSDT', last_price: '11', volume_24h: '5', observed_at: now + 1000,
    source: 'generated', provider: 'strategy',
  }))
  assert.ok(frame?.type === 'ticker')
  const live = applyLiveMarketTickerUpdate(old, frame)
  assert.equal(marketSource(live), 'generated')
  const merged = mergeMarketTickerSnapshots([live], [old])[0]!
  assert.equal(marketSource(merged), 'generated')
  assert.equal(merged.sourceObservedAt, now + 1000)
  const withoutTime = mapMarketTicker({ symbol: 'BTCUSDT', logo_url: '/new.png' }, { last_price: '1' })
  const retained = mergeMarketTickerSnapshots([live], [withoutTime])[0]!
  assert.equal(retained.lastPrice, live.lastPrice)
  assert.equal(retained.iconUrl, '/new.png')
  assert.equal(marketSource(retained), 'generated')
  assert.equal(marketSource(applyLiveMarketTickerUpdate(live, { symbol: 'BTCUSDT', lastPrice: 12, observedAt: now + 2000 })), 'unknown')
  const receiptOnly = applyLiveMarketTickerUpdate(live, { symbol: 'BTCUSDT', lastPrice: 12 }, now + 3000)
  assert.equal(receiptOnly.observedAt, now + 3000)
  assert.equal(receiptOnly.sourceObservedAt, undefined)
  assert.equal(mapMarketTicker({ symbol: 'BTCUSDT' }, { last_price: '10' }).sourceObservedAt, undefined)
})

test('candle source and observation survive REST, WS and chart normalization', () => {
  const candle = { symbol: 'BTCUSDT', interval: '1m', open_time: 1790000000000, open: '10', high: '12', low: '9', close: '11', volume: '5', observed_at: 1790000001000, source: 'generated', provider: 'strategy' }
  const rest = mapMarketKline(candle)!
  const live = parseMarketSocketFrame(JSON.stringify(candle))
  assert.ok(live?.type === 'kline')
  assert.deepEqual(live.point, rest)
  const chart = normalizeMarketChartPoints([rest])[0]!
  assert.equal(marketSource(chart), 'generated')
  assert.equal(chart.observedAt, candle.observed_at)
  assert.equal(marketSource(mapMarketKline({ ...candle, source: undefined, provider: undefined })!), 'unknown')
})
