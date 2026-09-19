import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'
import { createPinia, setActivePinia } from 'pinia'
import { useMarketStore, type Ticker } from '../src/stores/market.ts'
import { mapMarketDepthToTradePlate, mapMarketTradeToPcTrade, mapMarketTickerToPcTicker, mapMarketKlinesToPcRows } from '../src/api/backendAdapters.ts'
import { historyKlineBars, parseRealtimeKline } from '../src/components/chart/klineData.ts'
import { mapMarketProvenance, marketSource, marketTradeIdentity } from '../src/api/marketProvenance.ts'

test('PC store keeps price and provenance together across initial REST, late REST and live replacement', () => {
  setActivePinia(createPinia())
  const store = useMarketStore()
  const old: Ticker = { symbol: 'BTC/USDT', icon: '', open: 10, high: 10, low: 10, close: 10, volume: 2, turnover: 20, time: 1790000000000, chg: 0, zone: 0, source: 'external', provider: 'htx' }
  store.setTickers([old])
  assert.equal(marketSource(store.tickers[0]), 'external')
  store.updateTicker({ ...old, close: 11, time: old.time + 1000, source: 'generated', provider: 'strategy' })
  store.setTickers([{ ...old, icon: '/btc.png' }])
  assert.equal(store.tickers[0]!.close, 11)
  assert.equal(store.tickers[0]!.icon, '/btc.png')
  assert.equal(marketSource(store.tickers[0]), 'generated')
  store.updateTicker({ ...old, time: old.time - 1000 })
  assert.equal(store.tickers[0]!.close, 11)
  const { source: _source, provider: _provider, ...withoutEvidence } = old
  store.updateTicker({ ...withoutEvidence, close: 12, time: old.time + 2000 })
  assert.equal(marketSource(store.tickers[0]), 'unknown')
  assert.equal(store.tickers[0]!.close, 12)
})

test('PC REST adapters retain evidence for generated, external and platform rows', () => {
  for (const [source, provider] of [['platform', 'platform'], ['external', 'htx'], ['strategy', 'strategy'], ['default', 'strategy'], ['generated', 'strategy'], ['unknown', 'future']]) {
    const trade = mapMarketTradeToPcTrade({ id: '7', price: '12', amount: '2', time: 1790000000000, source, provider })
    const book = mapMarketDepthToTradePlate({ symbol: 'BTCUSDT', bids: [], asks: [], source, provider })
    assert.equal(trade.source, source)
    assert.equal(trade.provider, provider)
    assert.equal(book.source, source)
    assert.equal(book.provider, provider)
    assert.equal(marketSource(trade), source)
  }
})

test('PC unknown/legacy/conflicting sources never become platform executions', () => {
  for (const evidence of [{}, { provider: 'platform' }, { source: 'platform' }, { source: 'platform', provider: 'strategy' }, { source: 'real' }, { source: 'unknown', provider: 'bitget' }]) {
    assert.equal(marketSource(evidence), 'unknown')
  }
  assert.equal(marketSource({ provider: 'strategy' }), 'generated')
  assert.equal(marketSource({ provider: 'coinbase' }), 'external')
  for (const source of [null, '', ' ', 42, {}]) {
    assert.equal(marketSource(mapMarketProvenance({ source, provider: 'htx' })), 'unknown')
  }
  assert.notEqual(
    marketTradeIdentity({ id: 7, time: 1, source: 'platform', provider: 'platform' }),
    marketTradeIdentity({ id: 7, time: 1, source: 'external', provider: 'htx' }),
  )
})

test('PC depth hosts and individual prints bind current provenance', () => {
  for (const view of ['Trade', 'Contract']) {
    const source = readFileSync(new URL(`../src/views/${view}.vue`, import.meta.url), 'utf8')
    assert.match(source, /<OrderBook\b[^>]*:provenance="depthProvenance"/)
    assert.ok(source.includes('generation === depthGeneration && !liveDepthReceived'))
  }
  const trades = readFileSync(new URL('../src/components/trade/MarketTrades.vue', import.meta.url), 'utf8')
  assert.ok(trades.includes('<MarketProvenanceLabel :provenance="trade" />'))
  assert.ok(trades.includes(':key="marketTradeIdentity(trade)"'))
})

test('ticker updates replace old source and historical/realtime candles retain individual evidence', () => {
  const current = mapMarketTickerToPcTicker(undefined, {
    symbol: 'BTCUSDT', last_price: '10', volume_24h: '5', observed_at: 1790000000000,
    source: 'external', provider: 'coinbase',
  })
  const next = mapMarketTickerToPcTicker(current, {
    symbol: 'BTCUSDT', last_price: '11', volume_24h: '6', observed_at: 1790000001000,
    source: 'generated', provider: 'strategy',
  })
  assert.equal(marketSource(next), 'generated')
  const unknown = mapMarketTickerToPcTicker(next, { symbol: 'BTCUSDT', last_price: '12', volume_24h: '7', observed_at: 1790000002000 })
  assert.equal(marketSource(unknown), 'unknown')
  const candle = {
    symbol: 'BTCUSDT', interval: '1m', open_time: 1790000000000,
    open: '10', high: '12', low: '9', close: '11', volume: '5',
    observed_at: 1790000001000, source: 'generated', provider: 'strategy',
  }
  const history = historyKlineBars(mapMarketKlinesToPcRows([candle]))[0]!
  const live = parseRealtimeKline(candle)!
  assert.equal(marketSource(history), 'generated')
  assert.equal(history.observedAt, candle.observed_at)
  assert.deepEqual(live, history)
})
