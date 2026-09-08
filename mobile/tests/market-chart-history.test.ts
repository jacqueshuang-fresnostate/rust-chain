import assert from 'node:assert/strict'
import test from 'node:test'
import { createMarketChartHistorySession } from '../src/core/marketChartHistory.ts'
import { chartPoints } from './helpers/market-chart-renderer.ts'
import type { KlinePoint } from '../src/core/types.ts'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: Error) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

function fixture() {
  const requests: Array<{ symbol: string, interval: string, before: number, page: ReturnType<typeof deferred<KlinePoint[]>> }> = []
  const session = createMarketChartHistorySession({
    loadOlder: (symbol, interval, before) => {
      const page = deferred<KlinePoint[]>()
      requests.push({ symbol, interval, before, page })
      return page.promise
    },
  })
  const points = chartPoints(160)
  const input = { symbol: ' btcusdt ', interval: '1m', points, loading: false }
  session.sync(input)
  return { session, requests, input, points }
}

test('history paging is demand-only, exclusive, single-flight and advances through sparse real pages', async () => {
  const { session, requests, points } = fixture()
  assert.equal(requests.length, 0)
  const first = session.requestOlder()
  assert.equal(session.snapshot().status, 'loading')
  await session.requestOlder()
  assert.equal(requests.length, 1)
  assert.equal(requests[0].symbol, 'BTCUSDT')
  assert.equal(requests[0].before, points[0].time * 1000)
  const older = chartPoints(2, points[0].time - 60 * 1000)
  requests[0].page.resolve([older[1], older[0], { ...older[0], time: older[0].time * 1000 }, { ...points[0], close: 999 }])
  await first
  assert.equal(session.snapshot().status, 'idle', 'short sparse page is not exhaustion')
  assert.deepEqual(session.snapshot().points, [...older, ...points])
  assert.equal(requests.length, 1, 'settlement does not start another page')
  const second = session.requestOlder()
  assert.equal(requests[1].before, older[0].time * 1000)
  requests[1].page.resolve(chartPoints(1, older[0].time - 60))
  await second
  assert.equal(session.snapshot().points.length, 163)
})

test('history survives bounded live replacements without REST overwriting current or in-flight live rows', async () => {
  const { session, requests, input, points } = fixture()
  const older = chartPoints(2, points[0].time - 120)
  const loading = session.requestOlder()
  const liveOlder = { ...older[0], close: 11.9, high: 12.8 }
  const live = [...points.slice(1), { ...points.at(-1)!, time: points.at(-1)!.time + 60, close: 11.8 }]
  session.sync({ ...input, points: [liveOlder, ...live] })
  requests[0].page.resolve([...older, { ...points.at(-1)!, close: 999 }])
  await loading
  assert.equal(session.snapshot().points.find((row) => row.time === liveOlder.time)?.close, 11.9)
  assert.equal(session.snapshot().points.find((row) => row.time === liveOlder.time)?.high, 12.8)
  session.sync({ ...input, points: live })
  assert.equal(session.snapshot().points.length, 163)
  assert.equal(session.snapshot().points.at(-1)?.close, 11.8)
  assert.equal(session.snapshot().points[0].time, older[0].time)
})

test('ordinary bounded live updates do not accumulate before history browsing', () => {
  const { session, input } = fixture()
  for (let index = 1; index <= 20; index++) {
    session.sync({ ...input, points: chartPoints(160, input.points[0].time + index * 60) })
  }
  assert.equal(session.snapshot().points.length, 160)
})

test('empty, overlapping-only and invalid pages exhaust without request loops', async () => {
  for (const response of [[], chartPoints(160), [{ ...chartPoints(1)[0], time: 0 }]]) {
    const { session, requests } = fixture()
    const request = session.requestOlder()
    requests[0].page.resolve(response)
    await request
    assert.equal(session.snapshot().status, 'exhausted')
    await session.requestOlder()
    await session.retry()
    assert.equal(requests.length, 1)
  }
})

test('failure retains rows and only explicit retry can send the same cursor again', async () => {
  const { session, requests, points } = fixture()
  const request = session.requestOlder()
  requests[0].page.reject(new Error('offline'))
  await request
  assert.equal(session.snapshot().status, 'error')
  assert.deepEqual(session.snapshot().points, points)
  await session.requestOlder()
  assert.equal(requests.length, 1)
  const retry = session.retry()
  assert.equal(requests.length, 2)
  assert.equal(requests[1].before, requests[0].before)
  requests[1].page.resolve(chartPoints(1, points[0].time - 60))
  await retry
  assert.equal(session.snapshot().status, 'idle')
})

test('symbol, interval, reload and unmount reject late page success/failure, including A to B to A', async () => {
  for (const change of ['symbol', 'interval', 'reload', 'unmount', 'roundtrip']) {
    for (const failure of [false, true]) {
      const { session, requests, input, points } = fixture()
      const request = session.requestOlder()
      if (change === 'unmount') session.dispose()
      else if (change === 'reload') session.sync({ ...input, loading: true })
      else if (change === 'roundtrip') {
        session.sync({ ...input, interval: '5m' })
        session.sync({ ...input, interval: '1m', points: [...points] })
      } else session.sync({ ...input, [change]: change === 'symbol' ? 'ETHUSDT' : '5m', points: chartPoints(10) })
      const snapshot = session.snapshot()
      if (failure) requests[0].page.reject(new Error('stale'))
      else requests[0].page.resolve(chartPoints(10, points[0].time - 600))
      await request
      assert.deepEqual(session.snapshot(), snapshot, `${change}, failure=${failure}`)
    }
  }
})

test('key-only replacement waits for replacement rows and initial loading suppresses paging', async () => {
  const { session, requests, input, points } = fixture()
  session.sync({ ...input, symbol: 'ETHUSDT' })
  assert.equal(session.snapshot().points.length, 0)
  await session.requestOlder()
  assert.equal(requests.length, 0)
  session.sync({ ...input, symbol: 'ETHUSDT', points: [...points], loading: true })
  await session.requestOlder()
  assert.equal(requests.length, 0)
  session.sync({ ...input, symbol: 'ETHUSDT', points: [...points], loading: false })
  const request = session.requestOlder()
  assert.equal(requests.length, 1)
  assert.equal(requests[0].symbol, 'ETHUSDT')
  requests[0].page.resolve([])
  await request
})
