import assert from 'node:assert/strict'
import test from 'node:test'
import { chartPoints, mountMarketChart } from './helpers/market-chart-renderer.ts'
import { captureMarketChartLogicalViewport, resolveMarketChartLogicalRange } from '../src/core/marketChartRuntime.ts'

const points = chartPoints(160)

test('actual renderer subscribes to logical ranges; initial fit and programmatic range events never demand history', async () => {
  const chart = mountMarketChart({ points })
  assert.equal(chart.rangeListenerCount, 1)
  chart.notifyRange({ from: 0, to: 30 })
  chart.flushFrames()
  assert.deepEqual(chart.emitted, [])
  chart.theme()
  chart.flushFrames()
  await chart.update({ points: [...points.slice(1), ...chartPoints(1, points.at(-1)!.time + 60)] })
  chart.flushFrames()
  assert.deepEqual(chart.emitted, [])
  chart.unmount()
  assert.equal(chart.rangeListenerCount, 0)
})

for (const gesture of ['pointermove', 'touchmove', 'wheel']) {
  test(`actual renderer emits one history demand after ${gesture} moves toward the oldest edge`, () => {
    const chart = mountMarketChart({ points })
    chart.setRange({ from: 30, to: 60 })
    chart.gesture(gesture, { buttons: 1 })
    chart.notifyRange({ from: 15, to: 45 })
    chart.flushFrames()
    assert.deepEqual(chart.emitted, ['load-history'])
    chart.notifyRange({ from: 0, to: 30 })
    chart.flushFrames()
    assert.deepEqual(chart.emitted, ['load-history'], 'kinetic/programmatic movement without new gesture does not auto-drain')
    chart.gesture(gesture, { buttons: 1 })
    chart.notifyRange({ from: -2, to: 28 })
    chart.flushFrames()
    assert.equal(chart.emitted.length, 2, 'another deliberate edge movement can request the next page')
    chart.unmount()
  })
}

test('hover, old-edge departure, distant movement, double click and settled gestures never leak a demand', async () => {
  const chart = mountMarketChart({ points })
  chart.setRange({ from: 20, to: 60 })
  chart.gesture('pointermove', { buttons: 0 })
  chart.notifyRange({ from: 0, to: 40 })
  chart.flushFrames()
  chart.gesture('wheel')
  chart.notifyRange({ from: 2, to: 42 })
  chart.flushFrames()
  chart.setRange({ from: 90, to: 120 })
  chart.gesture('pointermove', { buttons: 1 })
  chart.notifyRange({ from: 70, to: 100 })
  chart.flushFrames()
  chart.notifyRange({ from: 0, to: 30 })
  chart.gesture('dblclick')
  chart.flushFrames()
  assert.deepEqual(chart.emitted, [])
  chart.gesture('wheel')
  await chart.update({ points: [...points] })
  chart.notifyRange({ from: -10, to: 20 })
  chart.flushFrames()
  assert.deepEqual(chart.emitted, [], 'data writes disarm prior gesture demand')
  chart.unmount()
})

test('range reads at gesture RAF catch library movement before deferred native draw notifications', () => {
  const chart = mountMarketChart({ points })
  chart.setRange({ from: 30, to: 60 })
  chart.gesture('wheel')
  chart.setRange({ from: 10, to: 40 })
  chart.flushFrames()
  assert.deepEqual(chart.emitted, ['load-history'])
  chart.unmount()
})

test('prepending keeps timestamp/zoom without rehydration fit or history auto-drain, and a newer gesture wins', async () => {
  const chart = mountMarketChart({ points })
  chart.setRange({ from: 30, to: 60 })
  chart.gesture('pointermove', { buttons: 1 })
  chart.notifyRange({ from: 10.5, to: 40.5 })
  chart.flushFrames()
  const range = chart.range!
  const anchor = captureMarketChartLogicalViewport(points, range)!
  const older = [...chartPoints(100, points[0].time - 6000), ...points]
  await chart.update({ points: older })
  chart.flushFrames()
  assert.deepEqual(chart.range, resolveMarketChartLogicalRange(older, anchor))
  assert.equal(chart.fits, 1)
  assert.deepEqual(chart.emitted, ['load-history'])
  await chart.update({ points: [...chartPoints(10, older[0].time - 600), ...older] })
  chart.gesture('wheel')
  chart.setRange({ from: 70, to: 85 })
  chart.flushFrames()
  assert.deepEqual(chart.range, { from: 70, to: 85 })
  chart.unmount()
  assert.equal(chart.rangeListenerCount, 0)
  assert.equal(chart.pendingFrames, 0)
})

test('initial loading, dataset replacement and unmount cancel an armed history gesture', async () => {
  for (const boundary of ['loading', 'symbol', 'interval', 'unmount']) {
    const chart = mountMarketChart({ points })
    chart.setRange({ from: 25, to: 55 })
    chart.gesture('wheel')
    chart.setRange({ from: 5, to: 35 })
    if (boundary === 'unmount') chart.unmount()
    else if (boundary === 'loading') await chart.update({ historyLoading: true })
    else await chart.update({ [boundary]: boundary === 'symbol' ? 'ETHUSDT' : '5m' })
    chart.flushFrames()
    assert.deepEqual(chart.emitted, [], boundary)
    if (boundary !== 'unmount') chart.unmount()
  }
})

test('older response anchors the latest gesture made while its request was pending, never the demand-time viewport', async () => {
  const chart = mountMarketChart({ points })
  chart.setRange({ from: 30, to: 60 })
  chart.gesture('wheel')
  chart.notifyRange({ from: 10, to: 40 })
  chart.flushFrames()
  assert.deepEqual(chart.emitted, ['load-history'])
  chart.gesture('pointermove', { buttons: 1 })
  const latestRange = { from: 60.5, to: 80.5 }
  chart.notifyRange(latestRange)
  chart.flushFrames()
  const latestAnchor = captureMarketChartLogicalViewport(points, latestRange)!
  const older = [...chartPoints(100, points[0].time - 6000), ...points]
  await chart.update({ points: older })
  chart.flushFrames()
  assert.deepEqual(chart.range, resolveMarketChartLogicalRange(older, latestAnchor))
  assert.equal(chart.range!.to - chart.range!.from, 20)
  assert.equal(chart.fits, 1)
  chart.unmount()
})
