import assert from 'node:assert/strict'
import test from 'node:test'
import { mountMobileMarketChart } from './helpers/mobile-market-chart.ts'
import { chartPoints } from './helpers/market-chart-renderer.ts'
import type { KlinePoint } from '../src/core/types.ts'

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
