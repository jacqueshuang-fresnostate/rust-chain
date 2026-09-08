import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import ts from 'typescript'
import { mapMarketKlines, DEFAULT_MARKET_KLINE_INTERVAL, DEFAULT_MARKET_KLINE_LIMIT } from '../src/api/marketSocketProtocol.ts'
import { asNumber, normalizeSymbol } from '../src/core/format.ts'
import type { KlinePoint } from '../src/core/types.ts'

const NOW = 1_780_000_000_000
type Params = { interval: string; start?: number; end: number; limit: number }
const row = (time: number) => ({ open_time: time, open: '10', high: '12', low: '9', close: '11', volume: '4' })

function apiHarness(respond: (params: Params) => unknown | Promise<unknown>) {
  const source = readFileSync(new URL('../src/api/market.ts', import.meta.url), 'utf8')
  const names = ['fetchKlines', 'fetchOlderKlines', 'fetchKlinePage', 'intervalDuration']
  const functions = names.flatMap((name) => {
    const match = source.match(new RegExp(`(?:export )?(?:async )?function ${name}\\([\\s\\S]*?\\n}`))
    return match ? [match[0].replace(/^export /, '')] : []
  }).join('\n')
  const requests: { url: string; params: Params }[] = []
  const bindings = {
    DEFAULT_MARKET_KLINE_INTERVAL, DEFAULT_MARKET_KLINE_LIMIT, mapMarketKlines, normalizeSymbol, asNumber,
    Date: { now: () => NOW },
    requestUrl: (path: string) => `/api/v1${path}`,
    publicApiRequestConfig: (config: unknown) => config,
    client: { get: async (url: string, { params }: { params: Params }) => {
      requests.push({ url, params })
      return { data: await respond(params) }
    } },
  }
  const code = ts.transpileModule(functions, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } }).outputText
  const api = new Function('bindings', `${Object.keys(bindings).map((key) => `const ${key} = bindings.${key};`).join('\n')}\n${code}\nreturn { fetchKlines, fetchOlderKlines: typeof fetchOlderKlines === 'function' ? fetchOlderKlines : undefined };`)(bindings) as {
    fetchKlines(symbol: string, interval?: string, limit?: number): Promise<KlinePoint[]>
    fetchOlderKlines(symbol: string, interval: string, before: number, limit?: number): Promise<KlinePoint[]>
  }
  return { ...api, requests }
}

test('latest K-line page keeps sparse real history instead of applying a synthetic lookback window', async () => {
  const sparse = [row(NOW - 800 * 60_000), row(NOW - 400 * 60_000), row(NOW - 60_000)]
  const api = apiHarness(({ start, end, limit }) => sparse.filter((row) => row.open_time <= end && (start === undefined || row.open_time >= start)).slice(-limit))
  const points = await api.fetchKlines('btc/usdt')
  assert.equal(points.length, 3)
  assert.deepEqual(api.requests[0], { url: '/api/v1/markets/BTCUSDT/klines', params: { interval: '1m', end: NOW, limit: 100 } })
})

test('historical pages use exclusive millisecond cursor and omit start across sparse gaps', async () => {
  const stored = Array.from({ length: 276 }, (_, i) => row(NOW - (276 - i) * 600_000))
  const api = apiHarness(({ start, end, limit }) => {
    assert.equal(start, undefined)
    assert.equal(limit, 100)
    return { klines: stored.filter((row) => row.open_time <= end).slice(-limit) }
  })
  const received = await api.fetchKlines('ETH-USDT', '5m')
  while (true) {
    const before = received[0]!.time
    const older = await api.fetchOlderKlines('ETH-USDT', '5m', before)
    assert.equal(api.requests.at(-1)!.params.end, before - 1)
    if (!older.length) break
    received.unshift(...older)
    assert.ok(api.requests.length <= 4, 'pagination must progress')
  }
  assert.equal(api.requests.length, 4)
  assert.deepEqual(received.map(({ time }) => time), stored.map(({ open_time }) => open_time))
  assert.equal(new Set(received.map(({ time }) => time)).size, 276)
})

test('K-line request limits reflect the backend cap without changing bounded sparkline requests', async () => {
  const api = apiHarness(() => [])
  for (const [requested, expected] of [[24, 24], [300, 100], [0, 1], [-1, 1], [8.8, 8], [NaN, 100], [Infinity, 100]]) {
    await api.fetchKlines('BTCUSDT', '15m', requested)
    assert.equal(api.requests.at(-1)!.params.limit, expected)
  }
})

test('invalid or exhausted historical cursors never dispatch a request', async () => {
  const api = apiHarness(() => assert.fail('no HTTP for invalid cursor'))
  for (const before of [NaN, Infinity, -1, 0, 1, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    assert.deepEqual(await api.fetchOlderKlines('BTCUSDT', '1m', before), [])
  }
  assert.equal(api.requests.length, 0)
})

test('older K-lines preserve actual OHLC, accept both envelopes and exclude boundary/future rows', async () => {
  for (const envelope of [false, true]) {
    const rows = [row(NOW - 60_000), row(NOW), row(NOW + 60_000)]
    const api = apiHarness(() => envelope ? { klines: rows } : rows)
    assert.deepEqual(await api.fetchOlderKlines('BTCUSDT', '1m', NOW), [{ time: NOW - 60_000, open: 10, high: 12, low: 9, close: 11, volume: 4 }])
  }
})

test('history transport failure stays observable so the chart can offer retry', async () => {
  const failure = new Error('history transport unavailable')
  const api = apiHarness(() => Promise.reject(failure))
  await assert.rejects(api.fetchOlderKlines('BTCUSDT', '1m', NOW), (error) => error === failure)
})
