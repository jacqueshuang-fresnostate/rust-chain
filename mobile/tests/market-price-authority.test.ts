import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  applyLiveMarketTickerUpdate,
  mergeMarketTickerSnapshots,
} from '../src/core/marketTickerFreshness.ts'
import type { MarketTicker } from '../src/core/types.ts'

const tradeSource = readFileSync(new URL('../src/views/TradeView.vue', import.meta.url), 'utf8')
const detailSource = readFileSync(new URL('../src/views/MarketDetailView.vue', import.meta.url), 'utf8')
const homeSource = readFileSync(new URL('../src/views/HomeView.vue', import.meta.url), 'utf8')
const marketsSource = readFileSync(new URL('../src/views/MarketsView.vue', import.meta.url), 'utf8')
const marketStoreSource = readFileSync(new URL('../src/stores/market.ts', import.meta.url), 'utf8')

function ticker(overrides: Partial<MarketTicker> = {}): MarketTicker {
  return {
    symbol: 'BTC/USDT',
    base: 'BTC',
    quote: 'USDT',
    lastPrice: 63_500,
    openPrice: 64_000,
    highPrice: 64_500,
    lowPrice: 63_000,
    volume: 100,
    changePercent: -0.78125,
    observedAt: 1_786_480_000_000,
    ...overrides,
  }
}

test('newer Bitget ticker frame atomically replaces all dynamic 24h fields', () => {
  const current = ticker()
  const next = applyLiveMarketTickerUpdate(current, {
    symbol: 'BTCUSDT',
    lastPrice: 63_700,
    highPrice: 64_700,
    lowPrice: 62_900,
    volume: 125.75,
    changePercent: -0.46875,
    observedAt: 1_786_480_001_000,
  })

  assert.notEqual(next, current)
  assert.equal(next.lastPrice, 63_700)
  assert.equal(next.highPrice, 64_700)
  assert.equal(next.lowPrice, 62_900)
  assert.equal(next.volume, 125.75)
  assert.equal(next.changePercent, -0.46875)
  assert.ok(Math.abs(next.openPrice - (63_700 / (1 - 0.46875 / 100))) < 1e-9)
  assert.equal(next.observedAt, 1_786_480_001_000)
})

test('last-price-only compatibility frame preserves the last authoritative 24h percentage', () => {
  const current = ticker()
  const next = applyLiveMarketTickerUpdate(current, {
    symbol: 'BTCUSDT',
    lastPrice: 63_700,
    observedAt: 1_786_480_001_000,
  })

  assert.equal(next.lastPrice, 63_700)
  assert.equal(next.openPrice, 64_000)
  assert.equal(next.changePercent, -0.78125)
})

test('older ticker frame cannot move the home market price backwards', () => {
  const current = ticker({ lastPrice: 63_700, observedAt: 1_786_480_002_000 })
  const next = applyLiveMarketTickerUpdate(current, {
    symbol: 'BTCUSDT',
    lastPrice: 63_100,
    observedAt: 1_786_480_001_000,
  })

  assert.equal(next, current)
})

test('late REST refresh keeps the newer WebSocket ticker snapshot as one coherent unit', () => {
  const current = ticker({
    lastPrice: 63_700,
    openPrice: 64_100,
    highPrice: 64_700,
    lowPrice: 62_900,
    volume: 110,
    changePercent: -0.62402496099844,
    observedAt: 1_786_480_002_000,
  })
  const incoming = ticker({
    lastPrice: 63_600,
    highPrice: 64_500,
    lowPrice: 63_000,
    volume: 120,
    changePercent: -0.625,
    observedAt: 1_786_480_001_000,
    iconUrl: '/uploads/markets/btc.png',
  })
  const [merged] = mergeMarketTickerSnapshots([current], [incoming])

  assert.equal(merged.lastPrice, 63_700)
  assert.equal(merged.openPrice, 64_100)
  assert.equal(merged.highPrice, 64_700)
  assert.equal(merged.lowPrice, 62_900)
  assert.equal(merged.observedAt, 1_786_480_002_000)
  assert.equal(merged.volume, 110)
  assert.equal(merged.changePercent, -0.62402496099844)
  assert.equal(merged.iconUrl, '/uploads/markets/btc.png')
})

test('trade, market detail, and home use the Bitget ticker as visible-price authority', () => {
  assert.match(tradeSource, /const currentPrice = computed\(\(\) => ticker\.value\?\.lastPrice \?\? 0\)/)
  assert.doesNotMatch(tradeSource, /const currentPrice = computed\(\(\) => trades\.value\[0\]\?\.price/)
  assert.match(detailSource, /const latestPrice = computed\(\(\) => ticker\.value\?\.lastPrice \?\? 0\)/)
  assert.match(homeSource, /formatPrice\(ticker\.lastPrice\)/)
  assert.match(tradeSource, /await marketStore\.refresh\(\)[\s\S]*marketStore\.startLiveUpdates\('trade'\)/)
  assert.match(tradeSource, /marketStore\.stopLiveUpdates\('trade'\)/)
  assert.match(detailSource, /await marketStore\.refresh\(\)[\s\S]*marketStore\.startLiveUpdates\('market-detail'\)/)
  assert.match(detailSource, /marketStore\.stopLiveUpdates\('market-detail'\)/)
})

test('ticker stream remains leased while route transition consumers overlap', () => {
  assert.match(marketStoreSource, /const liveConsumers = new Set<string>\(\)/)
  assert.match(marketStoreSource, /liveConsumers\.add\(consumer\)[\s\S]*if \(stopLive \|\| !tickers\.value\.length\) return/)
  assert.match(marketStoreSource, /liveConsumers\.delete\(consumerId\.trim\(\)\)[\s\S]*if \(liveConsumers\.size\) return[\s\S]*stopLive\?\.\(\)/)
  assert.match(homeSource, /if \(viewActive\) marketStore\.startLiveUpdates\('home'\)/)
  assert.match(homeSource, /viewActive = false[\s\S]*marketStore\.stopLiveUpdates\('home'\)/)
  assert.match(marketsSource, /if \(viewActive\) marketStore\.startLiveUpdates\('markets'\)/)
  assert.match(marketsSource, /viewActive = false[\s\S]*marketStore\.stopLiveUpdates\('markets'\)/)
})
