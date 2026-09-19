import assert from 'node:assert/strict'
import test from 'node:test'
import { mountMobileMarketChart } from './helpers/mobile-market-chart.ts'
import { chartPoints } from './helpers/market-chart-renderer.ts'
import type { KlinePoint } from '../src/core/types.ts'
import { mapMarketTicker } from '../src/core/marketMapper.ts'

test('actual chart labels mixed evidence, unknown receipt time, stale/future observations and releases timer', async (context) => {
  const now = 1_790_000_000_000
  context.mock.timers.enable({ apis: ['Date', 'setInterval'], now })
  const points = chartPoints(3).map((point, index) => ({
    ...point,
    ...(index === 0 ? { source: 'external', provider: 'htx' } : index === 1 ? { source: 'generated', provider: 'strategy' } : {}),
  }))
  const wrapper = mountMobileMarketChart(async () => [], points)
  try {
    const ticker = mapMarketTicker({ symbol: 'BTCUSDT' }, { last_price: '10', source: 'external', provider: 'htx', observed_at: now })
    await wrapper.update({ ticker })
    assert.match(wrapper.text(), /外部参考/)
    assert.match(wrapper.text(), /平台生成（模拟）/)
    assert.match(wrapper.text(), /来源未知/)
    assert.doesNotMatch(wrapper.text(), /已陈旧/)
    context.mock.timers.tick(61_000)
    await wrapper.flush()
    assert.match(wrapper.text(), /已陈旧/)
    await wrapper.update({ ticker: { ...ticker, sourceObservedAt: now + 120_000 } })
    assert.match(wrapper.text(), /已陈旧/)
    await wrapper.update({ ticker: { ...ticker, sourceObservedAt: undefined } })
    assert.match(wrapper.text(), /时间未知/)
    assert.doesNotMatch(wrapper.text(), /已陈旧/)
  } finally {
    wrapper.unmount()
  }
  context.mock.timers.tick(1000)
  assert.equal(wrapper.root.children.length, 0)
})

function fixture() {
  const requests: Array<{ before: number, resolve: (points: KlinePoint[]) => void, reject: (reason: Error) => void }> = []
  const points = chartPoints(160)
  const wrapper = mountMobileMarketChart((_symbol, _interval, before) => new Promise((resolve, reject) => {
    requests.push({ before, resolve, reject })
  }), points)
  function demand() { (wrapper.chart().props.onDemand as () => void)() }
  return { wrapper, points, requests, demand }
}

test('actual MobileMarketChart template bridges renderer demand into history without changing initial loading', async () => {
  const { wrapper, points, requests, demand } = fixture()
  assert.equal(wrapper.root.children[0].props['data-history-state'], 'idle')
  assert.equal(requests.length, 0)
  demand()
  demand()
  await wrapper.flush()
  assert.equal(requests.length, 1)
  assert.equal(requests[0].before, points[0].time * 1000)
  assert.equal(wrapper.chart().props.historyLoading, false, 'older loading never rearms the initial hydration fit')
  assert.equal(wrapper.root.children[0].props['data-history-state'], 'loading')
  assert.ok(wrapper.text().includes('正在加载更早 K 线'))
  const status = wrapper.all().find((node) => node.props.role === 'status')!
  assert.equal(status.props['aria-live'], 'polite')
  assert.equal(status.props['aria-busy'], true)
  requests[0].resolve(chartPoints(100, points[0].time - 6000))
  await wrapper.flush()
  assert.equal((wrapper.chart().props.points as KlinePoint[]).length, 260)
  assert.equal(wrapper.root.children[0].props['data-history-state'], 'idle')
  assert.equal(requests.length, 1)
  await wrapper.update({ points: chartPoints(160, points[0].time + 60) })
  assert.equal((wrapper.chart().props.points as KlinePoint[]).length, 261)
  demand()
  requests[1].resolve([])
  await wrapper.flush()
  assert.equal(wrapper.root.children[0].props['data-history-state'], 'exhausted')
  assert.ok(wrapper.text().includes('暂无更早的 K 线'))
  assert.equal(wrapper.all().some((node) => node.type === 'button'), false)
  wrapper.unmount()
})

test('actual wrapper exposes localized accessible retry, retains chart and catches failure without implicit retry', async () => {
  const { wrapper, requests, demand } = fixture()
  demand()
  requests[0].reject(new Error('offline'))
  await wrapper.flush()
  assert.equal(wrapper.root.children[0].props['data-history-state'], 'error')
  assert.equal((wrapper.chart().props.points as KlinePoint[]).length, 160)
  assert.ok(wrapper.text().includes('更早 K 线加载失败'))
  assert.equal(wrapper.find('button').props['aria-label'], '重试加载更早 K 线')
  demand()
  await wrapper.flush()
  assert.equal(requests.length, 1)
  wrapper.i18n.global.locale.value = 'en'
  await wrapper.flush()
  assert.ok(wrapper.text().includes('Earlier candles failed to load.'))
  assert.equal(wrapper.find('button').props['aria-label'], 'Retry loading earlier candles')
  ;(wrapper.find('button').props.onClick as () => void)()
  await wrapper.flush()
  assert.equal(requests.length, 2)
  assert.equal(wrapper.root.children[0].props['data-history-state'], 'loading')
  requests[1].resolve([])
  await wrapper.flush()
  assert.ok(wrapper.text().includes('No earlier candles available'))
  assert.equal(wrapper.all().some((node) => node.type === 'button'), false)
  wrapper.unmount()
})

test('actual wrapper invalidates an old request on same-dataset reload and isolates a new symbol', async () => {
  const { wrapper, requests, points, demand } = fixture()
  demand()
  await wrapper.update({ loading: true })
  requests[0].resolve(chartPoints(100, points[0].time - 6000))
  await wrapper.flush()
  assert.equal((wrapper.chart().props.points as KlinePoint[]).length, 160)
  assert.equal(wrapper.chart().props.historyLoading, true)
  await wrapper.update({ loading: false, points: [...points] })
  demand()
  await wrapper.update({ symbol: 'ETHUSDT' })
  assert.equal((wrapper.chart().props.points as KlinePoint[]).length, 0)
  requests[1].resolve(chartPoints(100, points[0].time - 6000))
  await wrapper.flush()
  assert.equal((wrapper.chart().props.points as KlinePoint[]).length, 0)
  await wrapper.update({ points: chartPoints(10) })
  assert.equal((wrapper.chart().props.points as KlinePoint[]).length, 10)
  demand()
  wrapper.unmount()
  requests[2].reject(new Error('unmounted'))
  await wrapper.flush()
  assert.equal(wrapper.root.children.length, 0)
})
