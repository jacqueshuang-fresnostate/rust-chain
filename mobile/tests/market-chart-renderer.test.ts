import assert from 'node:assert/strict'
import test from 'node:test'
import { chartPoints, mountMarketChart } from './helpers/market-chart-renderer.ts'
import {
  captureMarketChartLogicalViewport,
  resolveMarketChartLogicalRange,
} from '../src/core/marketChartRuntime.ts'

const previous = chartPoints(300)
const nextBar = chartPoints(1, previous.at(-1)!.time + 60)[0]
const replacements = [
  { name: 'rolling retention', points: [...previous.slice(1), nextBar], shift: 0 },
  { name: 'batch append', points: [...previous, ...chartPoints(2, nextBar.time)], shift: 2 },
  { name: 'last revision plus append', points: [...previous.slice(0, -1), { ...previous.at(-1)!, close: 11.5 }, nextBar], shift: 1 },
]

for (const replacement of replacements) {
  test(`renderer follows an advancing ${replacement.name} at the live tail with identical width and padding`, async () => {
    const chart = mountMarketChart({ points: previous })
    chart.setRange({ from: 249.5, to: 309.5 })
    await chart.update({ points: replacement.points })
    chart.flushFrames()
    assert.deepEqual(chart.range, { from: 249.5 + replacement.shift, to: 309.5 + replacement.shift })
    assert.equal(chart.fits, 1)
    assert.equal(chart.series[0].rows.length, replacement.points.length)
    assert.equal(chart.series[0].rows.at(-1)?.time, replacement.points.at(-1)?.time)
    chart.unmount()
  })

  test(`renderer timestamp-anchors ${replacement.name} while browsing history`, async () => {
    const chart = mountMarketChart({ points: previous })
    const range = { from: 100.5, to: 160.5 }
    chart.setRange(range)
    const anchor = captureMarketChartLogicalViewport(previous, range)!
    await chart.update({ points: replacement.points })
    chart.flushFrames()
    assert.deepEqual(chart.range, resolveMarketChartLogicalRange(replacement.points, anchor))
    assert.ok(chart.range!.to < replacement.points.length - 1)
    assert.equal(chart.fits, 1)
    chart.unmount()
  })
}

test('replacement follows a tail at the viewport edge but not a viewport entirely after it', async () => {
  for (const range of [{ from: 239, to: 299 }, { from: 238.75, to: 298.75 }, { from: 300, to: 360 }]) {
    const chart = mountMarketChart({ points: previous })
    chart.setRange(range)
    await chart.update({ points: replacements[1].points })
    chart.flushFrames()
    const shift = range.from < 299 ? 2 : 0
    assert.deepEqual(chart.range, { from: range.from + shift, to: range.to + shift })
    chart.unmount()
  }
})

test('prepend, older-tail replacement and same-tail corrections do not newly follow', async () => {
  const range = { from: 239.5, to: 299.5 }
  for (const points of [
    [...chartPoints(2, previous[0].time - 120), ...previous],
    previous.slice(0, -2),
    [{ ...previous[0], close: 10.5 }, ...previous.slice(1)],
  ]) {
    const chart = mountMarketChart({ points: previous })
    chart.setRange(range)
    const anchor = captureMarketChartLogicalViewport(previous, range)!
    await chart.update({ points })
    chart.flushFrames()
    assert.deepEqual(chart.range, resolveMarketChartLogicalRange(points, anchor))
    assert.equal(chart.fits, 1)
    chart.unmount()
  }
})

test('same-candle updates remain incremental and leave pan, zoom and pending fit unchanged', async () => {
  const chart = mountMarketChart({ points: previous })
  const range = { from: 90, to: 110 }
  chart.setRange(range)
  const sets = chart.series.map((series) => series.sets)
  await chart.update({ points: [...previous.slice(0, -1), { ...previous.at(-1)!, close: 11.7 }] })
  chart.flushFrames()
  assert.deepEqual(chart.range, range)
  assert.deepEqual(chart.series.map((series) => series.sets), sets)
  assert.deepEqual(chart.series.map((series) => series.updates), [1, 1, 1, 1, 1])
  assert.equal(chart.fits, 1)
  chart.unmount()
})

test('initial REST history fits after 1 → 2 → 3 visible live candles exactly once on settlement', async () => {
  const chart = mountMarketChart({ historyLoading: true })
  for (const count of [1, 2, 3]) await chart.update({ points: previous.slice(-3).slice(0, count) })
  await chart.update({ points: previous, historyLoading: false })
  chart.flushFrames()
  assert.equal(chart.range!.to - chart.range!.from, 300)
  assert.equal(chart.fits, 2, 'first visible live dataset plus authoritative initial history')
  assert.equal(chart.series[0].rows.length, 300)
  const count = chart.fits
  await chart.update({ historyLoading: true })
  await chart.update({ points: replacements[1].points, historyLoading: false })
  chart.flushFrames()
  assert.equal(chart.fits, count, 'same-dataset reload must not rearm initial history fitting')
  chart.unmount()
})

for (const gesture of ['wheel', 'pointermove', 'touchmove', 'dblclick']) {
  test(`initial hydration respects prior user ${gesture} intent`, async () => {
    const chart = mountMarketChart({ historyLoading: true })
    for (const count of [1, 2, 3]) await chart.update({ points: previous.slice(-3).slice(0, count) })
    chart.flushFrames()
    chart.gesture(gesture, { buttons: 1 })
    const range = { from: -4.5, to: 1.5 }
    chart.setRange(range)
    const anchor = captureMarketChartLogicalViewport(previous.slice(-3), range)!
    const fits = chart.fits
    await chart.update({ points: previous, historyLoading: false })
    chart.flushFrames()
    assert.equal(chart.fits, fits)
    assert.deepEqual(chart.range, resolveMarketChartLogicalRange(previous, anchor))
    chart.unmount()
  })
}

test('hover is not viewport intent and delayed loading start still hydrates initially empty charts', async () => {
  const chart = mountMarketChart()
  await chart.update({ historyLoading: true, points: previous.slice(-3) })
  chart.gesture('pointermove', { buttons: 0 })
  await chart.update({ points: previous })
  await chart.update({ historyLoading: false })
  chart.flushFrames()
  assert.equal(chart.range!.to - chart.range!.from, 300)
  assert.equal(chart.fits, 2)
  chart.unmount()
})

test('failed or empty initial history settles once and later real data still receive their first fit', async () => {
  const chart = mountMarketChart({ historyLoading: true })
  await chart.update({ historyLoading: false })
  assert.equal(chart.fits, 0)
  await chart.update({ points: previous })
  assert.equal(chart.fits, 1)
  assert.equal(chart.range!.to - chart.range!.from, 300)
  await chart.update({ historyLoading: true })
  await chart.update({ historyLoading: false })
  assert.equal(chart.fits, 1)
  chart.unmount()
})

test('key-only replacement waits for new points and resets hydration/user intent for the new dataset', async () => {
  const chart = mountMarketChart({ points: previous })
  chart.gesture('wheel')
  chart.setRange({ from: 20, to: 40 })
  const sets = chart.series[0].sets
  await chart.update({ interval: '5m', historyLoading: true })
  assert.equal(chart.fits, 1)
  assert.equal(chart.series[0].sets, sets)
  assert.deepEqual(chart.range, { from: 20, to: 40 })
  await chart.update({ points: previous.slice(-3) })
  assert.equal(chart.fits, 2)
  await chart.update({ points: previous, historyLoading: false })
  assert.equal(chart.fits, 3)
  assert.equal(chart.range!.to - chart.range!.from, 300)
  chart.unmount()
})

test('consecutive replacements before RAF preserve the intended live-tail padding', async () => {
  const chart = mountMarketChart({ points: previous })
  chart.setRange({ from: 239.5, to: 299.5 })
  await chart.update({ points: [...previous.slice(1), nextBar] })
  await chart.update({ points: [...previous.slice(2), ...chartPoints(2, nextBar.time)] })
  assert.equal(chart.pendingFrames, 1)
  chart.flushFrames()
  assert.deepEqual(chart.range, { from: 239.5, to: 299.5 })
  chart.unmount()
})

test('user gestures cancel pending viewport restoration instead of overriding a new pan', async () => {
  const chart = mountMarketChart({ points: previous })
  await chart.update({ points: replacements[1].points })
  assert.equal(chart.pendingFrames, 1)
  chart.gesture('wheel')
  chart.setRange({ from: 50, to: 80 })
  chart.flushFrames()
  assert.deepEqual(chart.range, { from: 50, to: 80 })
  chart.unmount()
})

test('theme restoration followed by an append still follows only a previously visible tail', async () => {
  for (const range of [{ from: 239.5, to: 299.5 }, { from: 50, to: 110 }]) {
    const chart = mountMarketChart({ points: previous })
    chart.setRange(range)
    chart.theme()
    await chart.update({ points: [...previous, nextBar] })
    chart.flushFrames()
    const shift = range.to > 299 ? 1 : 0
    assert.deepEqual(chart.range, { from: range.from + shift, to: range.to + shift })
    assert.equal(chart.fits, 1)
    chart.unmount()
  }
})

test('queued historical replacement then another replacement retains timestamps rather than stale indexes', async () => {
  const chart = mountMarketChart({ points: previous })
  const range = { from: 50.5, to: 110.5 }
  const anchor = captureMarketChartLogicalViewport(previous, range)!
  chart.setRange(range)
  await chart.update({ points: [...previous.slice(1), nextBar] })
  const points = [...previous.slice(2), ...chartPoints(2, nextBar.time)]
  await chart.update({ points })
  chart.flushFrames()
  assert.deepEqual(chart.range, resolveMarketChartLogicalRange(points, anchor))
  chart.unmount()
})

test('runtime explicitly opts into advancing-tail follow without changing default timestamp anchoring', () => {
  const viewport = captureMarketChartLogicalViewport(previous, { from: 239.5, to: 299.5 })!
  assert.deepEqual(viewport.liveTail, { timestamp: previous.at(-1)!.time, rightOffset: .5 })
  assert.deepEqual(resolveMarketChartLogicalRange(replacements[1].points, viewport), { from: 239.5, to: 299.5 })
  assert.deepEqual(resolveMarketChartLogicalRange(replacements[1].points, viewport, true), { from: 241.5, to: 301.5 })
  assert.equal(captureMarketChartLogicalViewport(previous, { from: 30, to: 60 })!.liveTail, undefined)
  assert.equal(captureMarketChartLogicalViewport(previous, { from: NaN, to: 60 }), null)
  assert.equal(resolveMarketChartLogicalRange([], viewport, true), null)
  assert.equal(resolveMarketChartLogicalRange(previous, { ...viewport, rangeWidth: 0 }, true), null)
})

test('theme/locale/resize retain data and viewport; unmount clears observers, frames and gesture listeners', async () => {
  const chart = mountMarketChart({ points: previous })
  const range = { from: 50, to: 80 }
  chart.setRange(range)
  chart.theme()
  chart.flushFrames()
  await chart.update({ locale: 'zh-CN' })
  chart.container.clientWidth = 0
  chart.resize()
  assert.equal(chart.resizeCalls.length, 0)
  chart.container.clientWidth = 700
  chart.resize()
  assert.deepEqual(chart.resizeCalls, [[700, 300]])
  assert.deepEqual(chart.range, range)
  assert.equal(chart.fits, 1)
  assert.equal(chart.series[0].options[0].wickVisible, false)
  assert.equal(chart.series[0].options.some((options) => options.wickVisible === true), false)
  assert.equal(chart.series[0].options[0].priceLineVisible, undefined)
  assert.deepEqual(chart.series[0].rows[0], { time: previous[0].time, open: 10, high: 12, low: 9, close: 11 })
  assert.equal(chart.series[1].rows[0].value, 1)
  assert.deepEqual(chart.series.slice(2).map((series) => series.rows.length), [296, 291, 281])
  assert.equal(chart.series[2].rows[0].value, 11)
  assert.deepEqual(chart.chartOptions.at(-1), { localization: { locale: 'zh-CN' } })
  await chart.update({ points: replacements[1].points })
  assert.equal(chart.pendingFrames, 1)
  chart.unmount()
  assert.equal(chart.pendingFrames, 0)
  assert.equal(chart.listenerCount, 0)
  assert.equal(chart.observersDisconnected, true)
  assert.equal(chart.removed, true)
  chart.flushFrames()
})
