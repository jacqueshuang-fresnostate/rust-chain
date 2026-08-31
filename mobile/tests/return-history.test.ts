import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  createReturnHistoryRequestLifecycle,
  mapReturnHistory,
  RETURN_HISTORY_DAY_MS,
  RETURN_HISTORY_PERIODS,
  type BackendReturnHistory,
  type ReturnHistory,
  type ReturnHistoryPeriodDays,
} from '../src/core/returnHistory.ts'
import { buildReturnHistoryGeometry } from '../src/core/returnHistoryGeometry.ts'
import {
  decimalAdd,
  decimalCompare,
  decimalDivide,
  normalizeDecimalText,
} from '../src/core/decimal.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const walletSource = readFileSync(new URL('../src/api/wallet.ts', import.meta.url), 'utf8')
const homeSource = readFileSync(new URL('../src/views/HomeView.vue', import.meta.url), 'utf8')
const parityCss = readFileSync(new URL('../src/styles/prototype-parity.css', import.meta.url), 'utf8')
const CALCULATED_AT = Date.UTC(2026, 7, 9, 18)

test('收益历史适配器接受四个白名单周期并严格映射连续 UTC 日', () => {
  for (const period of RETURN_HISTORY_PERIODS) {
    const mapped = mapReturnHistory(historyPayload(period), period)
    assert.equal(mapped.periodDays, period)
    assert.equal(mapped.points.length, period)
    assert.equal(mapped.points[0]?.dayStartAt, mapped.periodStartAt)
    assert.equal(mapped.points.at(-1)?.dayStartAt, Date.UTC(2026, 7, 9))
    assert.equal(mapped.points.at(-1)?.valuedAt, CALCULATED_AT)
    assert.equal(mapped.status, 'complete')
    assert.deepEqual(mapped.summary, { amount: '0', basisAmount: '0', rate: '0' })
  }

  const secondsPayload = historyPayload(1)
  secondsPayload.period_start_at = Number(secondsPayload.period_start_at) / 1000
  secondsPayload.calculated_at = Number(secondsPayload.calculated_at) / 1000
  const point = (secondsPayload.points as Array<Record<string, unknown>>)[0]!
  point.day_start_at = Number(point.day_start_at) / 1000
  point.valued_at = Number(point.valued_at) / 1000
  point.amount = '-0.000000000000000000'
  point.basis_amount = '-0.000000000000000000'
  point.rate = '-0.000000000000000000'
  point.cumulative_amount = '-0.000000000000000000'
  const summary = secondsPayload.summary as Record<string, unknown>
  summary.amount = '-0.000000000000000000'
  summary.basis_amount = '-0.000000000000000000'
  summary.rate = '-0.000000000000000000'
  const mappedSeconds = mapReturnHistory(secondsPayload, 1)
  assert.equal(mappedSeconds.points[0]?.amount, '0')
  assert.equal(mappedSeconds.summary.amount, '0')
})

test('收益历史适配器拒绝周期、数值、点数、UTC、状态与累计合同漂移', () => {
  const base = historyPayload(7)
  assert.throws(() => mapReturnHistory({ ...base, scope: 'portfolio' }, 7), /scope/)
  assert.throws(() => mapReturnHistory({ ...base, reporting_asset: 'BTC' }, 7), /reporting asset/)
  assert.throws(() => mapReturnHistory({ ...base, period_days: 30 }, 7), /period_days/)
  assert.throws(() => mapReturnHistory({ ...base, status: 'unknown' }, 7), /status/)
  assert.throws(() => mapReturnHistory({ ...base, points: [] }, 7), /points length/)
  assert.throws(() => mapReturnHistory({ ...base, period_start_at: Number(base.period_start_at) + 1 }, 7), /UTC period/)

  const brokenNumber = structuredClone(base)
  ;(brokenNumber.points as Array<Record<string, unknown>>)[0]!.amount = '1e3'
  assert.throws(() => mapReturnHistory(brokenNumber, 7), /amount/)

  const brokenContinuity = structuredClone(base)
  ;(brokenContinuity.points as Array<Record<string, unknown>>)[2]!.day_start_at = Number(
    (brokenContinuity.points as Array<Record<string, unknown>>)[2]!.day_start_at,
  ) + 1
  assert.throws(() => mapReturnHistory(brokenContinuity, 7), /UTC continuity/)

  const brokenCumulative = historyPayload(7, { amounts: [1, 0, 0, 0, 0, 0, 0] })
  ;(brokenCumulative.points as Array<Record<string, unknown>>)[3]!.cumulative_amount = '2'
  assert.throws(() => mapReturnHistory(brokenCumulative, 7), /cumulative consistency/)

  const brokenRate = historyPayload(1, { amounts: [1], bases: [4] })
  ;(brokenRate.points as Array<Record<string, unknown>>)[0]!.rate = '0.5'
  assert.throws(() => mapReturnHistory(brokenRate, 1), /rate consistency/)

  const completeWithMissing = structuredClone(base)
  ;(completeWithMissing.points as Array<Record<string, unknown>>)[0]!.missing_price_assets = ['BTC']
  assert.throws(() => mapReturnHistory(completeWithMissing, 7), /complete return history point/)
})

test('partial 点金额为空、首个缺价后累计为空且顶层 summary 与缺价并集严格一致', () => {
  const payload = historyPayload(7, {
    amounts: [1, 2, 3, 4, 5, 6, 7],
    bases: [10, 10, 10, 10, 10, 10, 10],
    partialIndex: 2,
  })
  const mapped = mapReturnHistory(payload, 7)
  assert.equal(mapped.status, 'partial')
  assert.deepEqual(mapped.summary, { amount: null, basisAmount: null, rate: null })
  assert.equal(mapped.points[0]?.cumulativeAmount, '1')
  assert.equal(mapped.points[1]?.cumulativeAmount, '3')
  assert.deepEqual(mapped.points[2], {
    dayStartAt: mapped.periodStartAt + RETURN_HISTORY_DAY_MS * 2,
    valuedAt: mapped.periodStartAt + RETURN_HISTORY_DAY_MS * 3,
    amount: null,
    basisAmount: null,
    rate: null,
    cumulativeAmount: null,
    status: 'partial',
    missingPriceAssets: ['BTC'],
  })
  assert.equal(mapped.points[3]?.amount, '4')
  assert.equal(mapped.points[3]?.cumulativeAmount, null)
  assert.deepEqual(mapped.missingPrices, [{
    dayStartAt: mapped.periodStartAt + RETURN_HISTORY_DAY_MS * 2,
    assetSymbol: 'BTC',
  }])

  const summaryLeak = structuredClone(payload)
  ;(summaryLeak.summary as Record<string, unknown>).amount = '3'
  assert.throws(() => mapReturnHistory(summaryLeak, 7), /partial.*summary/)

  const missingMismatch = structuredClone(payload)
  missingMismatch.missing_prices = []
  assert.throws(() => mapReturnHistory(missingMismatch, 7), /missing price consistency/)

  const cumulativeLeak = structuredClone(payload)
  ;(cumulativeLeak.points as Array<Record<string, unknown>>)[3]!.cumulative_amount = '7'
  assert.throws(() => mapReturnHistory(cumulativeLeak, 7), /cumulative after partial/)
})

test('收益几何为 1 日增加零基线、全零居中并让 y 域始终包含零', () => {
  const oneDay = mapReturnHistory(historyPayload(1, { amounts: [5], bases: [10] }), 1)
  const oneDayGeometry = buildReturnHistoryGeometry(oneDay)
  assert.ok(oneDayGeometry)
  assert.equal(oneDayGeometry.points.length, 2)
  assert.deepEqual(oneDayGeometry.points.map(({ x, value }) => ({ x, value })), [
    { x: 0, value: '0' },
    { x: 358, value: '5' },
  ])
  assert.equal(oneDayGeometry.latest.x, 358)
  assert.equal(oneDayGeometry.tone, 'positive')

  const zeroGeometry = buildReturnHistoryGeometry(mapReturnHistory(historyPayload(7), 7))
  assert.ok(zeroGeometry)
  assert.equal(zeroGeometry.points.length, 8)
  assert.ok(zeroGeometry.points.every((point) => point.y === 76.5))
  assert.equal(zeroGeometry.tone, 'neutral')

  const crossing = mapReturnHistory(
    historyPayload(7, { amounts: [5, -8, 0, 0, 0, 0, 0], bases: [10, 10, 0, 0, 0, 0, 0] }),
    7,
  )
  const crossingGeometry = buildReturnHistoryGeometry(crossing)
  assert.ok(crossingGeometry)
  assert.equal(crossingGeometry.tone, 'negative')
  assert.ok(crossingGeometry.points.every(({ x, y }) => x >= 0 && x <= 358 && y >= 12 && y <= 141))
  assert.doesNotMatch(crossingGeometry.path, /NaN|Infinity|C|Q/)
  assert.match(crossingGeometry.path, /^M /)
  assert.equal(buildReturnHistoryGeometry(mapReturnHistory(historyPayload(7, { partialIndex: 1 }), 7)), null)
})

test('收益历史生命周期隔离访客、周期 ABA、换 token、重试与卸载迟到响应', async () => {
  let sessionKey = ''
  let period: ReturnHistoryPeriodDays = 1
  const requests: Array<ReturnType<typeof deferred<ReturnHistory>>> = []
  const lifecycle = createReturnHistoryRequestLifecycle({
    sessionKey: () => sessionKey,
    fetchReturnHistory: () => {
      const request = deferred<ReturnHistory>()
      requests.push(request)
      return request.promise
    },
  })

  assert.deepEqual(await lifecycle.load(), { state: 'guest' })
  assert.equal(requests.length, 0)

  sessionKey = 'TOKEN_A'
  const firstA = lifecycle.load()
  period = 7
  const seven = lifecycle.load()
  period = 1
  const secondA = lifecycle.load()
  requests[0]!.resolve(mappedHistory(1, 1))
  requests[1]!.resolve(mappedHistory(7, 7))
  assert.deepEqual(await firstA, { state: 'stale' })
  assert.deepEqual(await seven, { state: 'stale' })
  requests[2]!.resolve(mappedHistory(1, 2))
  assert.deepEqual(await secondA, { state: 'loaded', value: mappedHistory(1, 2) })

  const beforeRetry = lifecycle.load()
  const retry = lifecycle.load()
  requests[3]!.reject(new Error('old request'))
  assert.deepEqual(await beforeRetry, { state: 'stale' })
  requests[4]!.resolve(mappedHistory(period, 3))
  assert.equal((await retry).state, 'loaded')

  const beforeTokenSwitch = lifecycle.load()
  sessionKey = 'TOKEN_B'
  requests[5]!.resolve(mappedHistory(1, 4))
  assert.deepEqual(await beforeTokenSwitch, { state: 'stale' })

  const beforeLogout = lifecycle.load()
  sessionKey = ''
  requests[6]!.resolve(mappedHistory(1, 5))
  assert.deepEqual(await beforeLogout, { state: 'stale' })

  sessionKey = 'TOKEN_C'
  const beforeUnmount = lifecycle.load()
  lifecycle.stop()
  requests[7]!.resolve(mappedHistory(1, 6))
  assert.deepEqual(await beforeUnmount, { state: 'stale' })
  assert.deepEqual(await lifecycle.load(), { state: 'stale' })
})

test('Home 只从真实接口绘图并为周期、隐私、状态和重试保留可访问边界', () => {
  assert.match(walletSource, /requestUrl\('\/wallet\/return-history'\)[\s\S]*params: \{ days: periodDays \}/)
  assert.match(homeSource, /data-portfolio-source="realized-return-history"/)
  assert.match(homeSource, /RETURN_HISTORY_PERIODS\.map/)
  assert.match(homeSource, /createReturnHistoryRequestLifecycle\(\{[\s\S]*sessionKey: \(\) => session\.token/)
  assert.match(homeSource, /fetchReturnHistory: \(\) => fetchReturnHistory\(selectedReturnHistoryPeriod\.value\)/)
  assert.match(homeSource, /buildReturnHistoryGeometry\(returnHistory\.value\)/)
  assert.match(homeSource, /<button[\s\S]*v-for="period in portfolioPeriods"[\s\S]*:aria-pressed="period\.days === selectedReturnHistoryPeriod"[\s\S]*aria-controls="portfolio-return-history-chart"/)
  assert.match(homeSource, /returnHistoryRequestLifecycle\.invalidate\(\)[\s\S]*returnHistory\.value = null[\s\S]*selectedReturnHistoryPeriod\.value = 1/)
  assert.match(homeSource, /onUnmounted\(\(\) => \{[\s\S]*returnHistoryRequestLifecycle\.stop\(\)/)
  assert.match(homeSource, /if \(!assetVisible\.value \|\| returnHistoryState\.value !== 'complete'/)
  assert.match(homeSource, /v-if="accessibleReturnHistoryPoints\.length" class="sr-only"/)
  assert.match(homeSource, /returnHistoryState === 'partial' \|\| returnHistoryState === 'error'/)
  assert.match(parityCss, /\.home-view \.portfolio-periods button[\s\S]*height:\s*44px;[\s\S]*min-height:\s*44px;/)
  assert.match(parityCss, /\.home-view \.portfolio-periods button:focus-visible/)
  assert.match(parityCss, /\.home-view \.portfolio-history-state button[\s\S]*min-height:\s*44px;/)
  assert.doesNotMatch(homeSource, /portfolioSamples|mockPortfolio|demoPortfolio|fallbackReturnHistory|random/i)
  assert.doesNotMatch(homeSource, /watch\(\s*\[\s*\(\) => session\.isAuthenticated,[\s\S]*totalAssetEstimate/)
})

test('收益历史状态与表格文案中英文对称', () => {
  for (const key of [
    'returnHistoryPeriodLabel',
    'returnHistoryLoading',
    'returnHistoryPartial',
    'returnHistoryUnavailable',
    'returnHistoryHidden',
    'returnHistoryChartSummary',
    'returnHistoryTableCaption',
    'returnHistoryDate',
    'returnHistoryDaily',
    'returnHistoryCumulative',
  ] as const) {
    assert.equal(typeof zhCN.home[key], 'string')
    assert.equal(typeof en.home[key], 'string')
  }
})

function historyPayload(
  periodDays: ReturnHistoryPeriodDays,
  options: {
    amounts?: number[]
    bases?: number[]
    partialIndex?: number
  } = {},
): BackendReturnHistory {
  const periodStartAt = Date.UTC(2026, 7, 9) - (periodDays - 1) * RETURN_HISTORY_DAY_MS
  const amounts = Array.from({ length: periodDays }, (_, index) => options.amounts?.[index] ?? 0)
  const bases = Array.from({ length: periodDays }, (_, index) => options.bases?.[index] ?? 0)
  const zero = normalizeDecimalText('0')
  let cumulative = zero
  let cumulativeKnown = true
  let basisTotal = zero
  const points = amounts.map((amount, index) => {
    const dayStartAt = periodStartAt + index * RETURN_HISTORY_DAY_MS
    if (index === options.partialIndex) {
      cumulativeKnown = false
      return {
        day_start_at: dayStartAt,
        valued_at: index === periodDays - 1 ? CALCULATED_AT : dayStartAt + RETURN_HISTORY_DAY_MS,
        amount: null,
        basis_amount: null,
        rate: null,
        cumulative_amount: null,
        status: 'partial',
        missing_price_assets: ['BTC'],
      }
    }
    const amountText = normalizeDecimalText(String(amount))
    const basisText = normalizeDecimalText(String(bases[index] ?? 0))
    basisTotal = decimalAdd(basisTotal, basisText)
    if (cumulativeKnown) cumulative = decimalAdd(cumulative, amountText)
    return {
      day_start_at: dayStartAt,
      valued_at: index === periodDays - 1 ? CALCULATED_AT : dayStartAt + RETURN_HISTORY_DAY_MS,
      amount: amountText,
      basis_amount: basisText,
      rate: decimalCompare(basisText, zero) > 0
        ? decimalDivide(amountText, basisText, 18)
        : zero,
      cumulative_amount: cumulativeKnown ? cumulative : null,
      status: 'complete',
      missing_price_assets: [],
    }
  })
  const partial = options.partialIndex !== undefined
  const missingDay = partial ? periodStartAt + options.partialIndex! * RETURN_HISTORY_DAY_MS : 0
  return {
    scope: 'realized',
    reporting_asset: 'USDT',
    period_days: periodDays,
    period_start_at: periodStartAt,
    calculated_at: CALCULATED_AT,
    status: partial ? 'partial' : 'complete',
    summary: partial
      ? { amount: null, basis_amount: null, rate: null }
      : {
          amount: cumulative,
          basis_amount: basisTotal,
          rate: decimalCompare(basisTotal, zero) > 0
            ? decimalDivide(cumulative, basisTotal, 18)
            : zero,
        },
    missing_prices: partial ? [{ day_start_at: missingDay, asset_symbol: 'BTC' }] : [],
    points,
  }
}

function mappedHistory(period: ReturnHistoryPeriodDays, marker: number): ReturnHistory {
  const amounts = Array.from({ length: period }, () => 0)
  amounts[period - 1] = marker
  const bases = Array.from({ length: period }, () => 0)
  bases[period - 1] = 10
  return mapReturnHistory(historyPayload(period, { amounts, bases }), period)
}

function deferred<T>(): {
  promise: Promise<T>
  resolve: (value: T) => void
  reject: (error: unknown) => void
} {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}
