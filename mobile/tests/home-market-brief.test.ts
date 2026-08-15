import assert from 'node:assert/strict'
import test from 'node:test'
import { buildHomeMarketBrief } from '../src/core/homeMarketBrief.ts'
import type { MarketTicker } from '../src/core/types.ts'

function ticker(overrides: Partial<MarketTicker> = {}): MarketTicker {
  return {
    symbol: 'BTC/USDT',
    base: 'BTC',
    quote: 'USDT',
    lastPrice: 63_000,
    openPrice: 62_000,
    highPrice: 64_000,
    lowPrice: 61_000,
    volume: 1_000,
    changePercent: 1.2,
    observedAt: 1_786_800_000_000,
    ...overrides,
  }
}

test('首页市场简报使用真实有效交易对计算涨跌数量和上涨占比', () => {
  const brief = buildHomeMarketBrief([
    ticker(),
    ticker({ symbol: 'ETH/USDT', base: 'ETH', lastPrice: 1_900, changePercent: -0.5, volume: 800 }),
    ticker({ symbol: 'SOL/USDT', base: 'SOL', lastPrice: 145, changePercent: 0, volume: 600 }),
  ])

  assert.deepEqual(
    brief && {
      total: brief.total,
      rising: brief.rising,
      falling: brief.falling,
      unchanged: brief.unchanged,
      advancingPercent: brief.advancingPercent,
      tone: brief.tone,
    },
    { total: 3, rising: 1, falling: 1, unchanged: 1, advancingPercent: 33, tone: 'neutral' },
  )
})

test('首页市场简报固定优先展示 BTC 并按涨跌幅选择表现最佳交易对', () => {
  const brief = buildHomeMarketBrief([
    ticker({ symbol: 'ETH-USDT', base: 'ETH', lastPrice: 1_900, changePercent: 2.5, volume: 2_000 }),
    ticker({ symbol: 'BTC_USDT', changePercent: 0.4, volume: 1_000 }),
    ticker({ symbol: 'SOL/USDT', base: 'SOL', lastPrice: 145, changePercent: 1.1, volume: 3_000 }),
  ])

  assert.equal(brief?.focusTicker.symbol, 'BTC_USDT')
  assert.equal(brief?.topMover.symbol, 'ETH-USDT')
  assert.equal(brief?.tone, 'positive')
})

test('首页市场简报去重时保留较新的 ticker，且忽略无效行情', () => {
  const brief = buildHomeMarketBrief([
    ticker({ lastPrice: 62_000, changePercent: -2, observedAt: 1_786_800_001 }),
    ticker({ lastPrice: 63_500, changePercent: 1.5, observedAt: 1_786_800_002_000 }),
    ticker({ symbol: 'BAD/USDT', base: 'BAD', lastPrice: 0, changePercent: 99 }),
    ticker({ symbol: 'NAN/USDT', base: 'NAN', lastPrice: 1, changePercent: Number.NaN }),
  ])

  assert.equal(brief?.total, 1)
  assert.equal(brief?.focusTicker.lastPrice, 63_500)
  assert.equal(brief?.rising, 1)
})

test('没有有效 ticker 时不生成伪造的市场简报', () => {
  assert.equal(buildHomeMarketBrief([]), null)
  assert.equal(buildHomeMarketBrief([ticker({ lastPrice: 0 })]), null)
})
