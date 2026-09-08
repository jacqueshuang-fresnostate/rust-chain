import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'
import { mapMarketTicker } from '../src/core/marketMapper.ts'
import { applyLiveMarketTickerUpdate, mergeMarketTickerSnapshots } from '../src/core/marketTickerFreshness.ts'
import { mountMobileMarketChart } from './helpers/mobile-market-chart.ts'
import { chartPoints } from './helpers/market-chart-renderer.ts'

test('market source comes only from declared backend market_type and survives newer WS price updates', () => {
  for (const marketType of ['strategy', 'internal', 'external']) {
    const snapshot = mapMarketTicker({ symbol: 'TEST_USDT', market_type: marketType }, { last_price: '1', observed_at: 1_788_700_000_000 })
    assert.equal(snapshot.marketType, marketType)
    const live = applyLiveMarketTickerUpdate(snapshot, { symbol: 'TESTUSDT', lastPrice: 2, observedAt: 1_788_700_001_000 })
    assert.equal(live.marketType, marketType)
    assert.equal(live.lastPrice, 2)
    const [merged] = mergeMarketTickerSnapshots([live], [snapshot])
    assert.equal(merged.marketType, marketType)
    assert.equal(merged.lastPrice, 2, 'source metadata never weakens live price authority')
  }
  for (const marketType of [undefined, null, 'unknown', 'STRATEGY']) {
    assert.equal(mapMarketTicker({ symbol: 'TEST_USDT', market_type: marketType }, { last_price: '1' }).marketType, undefined)
  }
})

test('authoritative metadata replacement clears a former generated-source marker without replacing a newer live price', () => {
  const previous = mapMarketTicker({ symbol: 'TEST_USDT', market_type: 'strategy' }, { last_price: '2', observed_at: 1_788_700_001_000 })
  const incoming = mapMarketTicker({ symbol: 'TEST_USDT', market_type: 'external' }, { last_price: '1', observed_at: 1_788_700_000_000 })
  const [merged] = mergeMarketTickerSnapshots([previous], [incoming])
  assert.equal(merged.marketType, 'external')
  assert.equal(merged.lastPrice, 2)
})

test('actual shared chart renders localized source notice only for generated markets without demanding history', async () => {
  let requests = 0
  const wrapper = mountMobileMarketChart(async () => { requests += 1; return [] }, chartPoints(10))
  assert.ok(!wrapper.text().includes('平台生成行情'))
  for (const marketType of ['strategy', 'internal'] as const) {
    await wrapper.update({ marketType })
    assert.ok(wrapper.text().includes('平台生成行情 · 盘口与成交为模拟数据'))
    assert.equal((wrapper.chart().props.points as unknown[]).length, 10)
  }
  wrapper.i18n.global.locale.value = 'en'
  await wrapper.flush()
  assert.ok(wrapper.text().includes('Platform-generated market · Order book and trades are simulated'))
  for (const marketType of ['external', undefined] as const) {
    await wrapper.update({ marketType })
    assert.ok(!wrapper.text().includes('Platform-generated market'))
  }
  assert.equal(requests, 0)
  wrapper.unmount()
})

test('both production chart hosts pass their current ticker metadata to the shared notice', () => {
  for (const name of ['TradeView', 'MarketDetailView']) {
    const source = readFileSync(new URL(`../src/views/${name}.vue`, import.meta.url), 'utf8')
    const chart = source.match(/<MobileMarketChart\b[\s\S]*?\/>/)?.[0]
    assert.ok(chart, `${name} owns a shared chart`)
    assert.ok(chart.includes(':market-type="ticker?.marketType"'), `${name} passes authoritative pair metadata`)
  }
})
