import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  createTodayReturnRequestLifecycle,
  isCompleteTodayReturn,
  mapTodayReturn,
  type TodayReturn,
} from '../src/core/todayReturn.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import { formatAmount, formatPercent } from '../src/core/format.ts'

const walletSource = readFileSync(new URL('../src/api/wallet.ts', import.meta.url), 'utf8')
const homeSource = readFileSync(new URL('../src/views/HomeView.vue', import.meta.url), 'utf8')

test('今日收益适配器保留 realized 合同、UTC 周期和真实零收益', () => {
  const result = mapTodayReturn({
    scope: 'realized',
    reporting_asset: 'usdt',
    amount: '0.000000000000000000',
    basis_amount: '0.000000000000000000',
    rate: '0.000000000000000000',
    period_start_at: 1_754_697_600_000,
    calculated_at: 1_754_733_645_000,
    status: 'complete',
    missing_price_assets: [],
  })

  assert.deepEqual(result, {
    scope: 'realized',
    reportingAsset: 'USDT',
    amount: 0,
    basisAmount: 0,
    rate: 0,
    periodStartAt: 1_754_697_600_000,
    calculatedAt: 1_754_733_645_000,
    status: 'complete',
    missingPriceAssets: [],
  })
  assert.equal(isCompleteTodayReturn(result), true)
  assert.equal(formatAmount(result.amount), '0')
  assert.equal(formatPercent(result.rate * 100), '0.00%')
})

test('partial 响应保留缺价资产但不满足首页数值展示条件', () => {
  const result = mapTodayReturn({
    scope: 'realized',
    reporting_asset: 'USDT',
    amount: '99',
    basis_amount: '100',
    rate: '0.99',
    period_start_at: 1_754_697_600,
    calculated_at: 1_754_733_645,
    status: 'partial',
    missing_price_assets: ['btc', 'BTC'],
  })

  assert.equal(result.amount, 99)
  assert.equal(result.periodStartAt, 1_754_697_600_000)
  assert.deepEqual(result.missingPriceAssets, ['BTC'])
  assert.equal(isCompleteTodayReturn(result), false)
})

test('今日收益适配器拒绝未知口径、状态、非十进制金额和矛盾时间/完整性', () => {
  const base = {
    scope: 'realized',
    reporting_asset: 'USDT',
    amount: '1',
    basis_amount: '10',
    rate: '0.1',
    period_start_at: 1_754_697_600_000,
    calculated_at: 1_754_733_645_000,
    status: 'complete',
    missing_price_assets: [],
  }

  assert.throws(() => mapTodayReturn({ ...base, scope: 'portfolio' }), /scope/)
  assert.throws(() => mapTodayReturn({ ...base, status: 'unknown' }), /status/)
  assert.throws(() => mapTodayReturn({ ...base, amount: 'not-a-number' }), /amount/)
  assert.throws(() => mapTodayReturn({ ...base, amount: null }), /amount/)
  assert.throws(() => mapTodayReturn({ ...base, amount: '  ' }), /amount/)
  assert.throws(() => mapTodayReturn({ ...base, amount: '0x10' }), /amount/)
  assert.throws(() => mapTodayReturn({ ...base, amount: '1e3' }), /amount/)
  assert.throws(() => mapTodayReturn({ ...base, rate: false }), /rate/)
  assert.throws(() => mapTodayReturn({ ...base, basis_amount: '-1' }), /basis_amount/)
  assert.throws(() => mapTodayReturn({ ...base, missing_price_assets: undefined }), /missing price assets/)
  assert.throws(() => mapTodayReturn({ ...base, missing_price_assets: ['BTC', null] }), /missing price asset/)
  assert.throws(() => mapTodayReturn({ ...base, missing_price_assets: ['BTC'] }), /complete.*missing price assets/)
  assert.throws(() => mapTodayReturn({ ...base, period_start_at: 1_754_697_601_000 }), /UTC period/)
  assert.throws(() => mapTodayReturn({ ...base, calculated_at: 1_754_697_599_000 }), /UTC period/)
  assert.throws(() => mapTodayReturn({ ...base, calculated_at: 1_754_784_000_000 }), /UTC period/)
  assert.throws(() => mapTodayReturn({ ...base, calculated_at: 1_754_733_645.5 }), /calculated_at/)
})

test('今日收益请求生命周期隔离访客、最新请求、换号登录和卸载迟到响应', async () => {
  let sessionKey = ''
  const requests: Array<ReturnType<typeof deferred<TodayReturn>>> = []
  const lifecycle = createTodayReturnRequestLifecycle({
    sessionKey: () => sessionKey,
    fetchTodayReturn: () => {
      const request = deferred<TodayReturn>()
      requests.push(request)
      return request.promise
    },
  })

  assert.deepEqual(await lifecycle.load(), { state: 'guest' })
  assert.equal(requests.length, 0)

  sessionKey = 'TOKEN_A'
  const first = lifecycle.load()
  const second = lifecycle.load()
  requests[1].resolve(todayReturn(2))
  assert.deepEqual(await second, { state: 'loaded', value: todayReturn(2) })
  requests[0].resolve(todayReturn(1))
  assert.deepEqual(await first, { state: 'stale' })

  const beforeAccountSwitch = lifecycle.load()
  sessionKey = 'TOKEN_B'
  requests[2].resolve(todayReturn(3))
  assert.deepEqual(await beforeAccountSwitch, { state: 'stale' })

  const afterAccountSwitch = lifecycle.load()
  requests[3].reject(new Error('network down'))
  const failed = await afterAccountSwitch
  assert.equal(failed.state, 'error')

  const beforeUnmount = lifecycle.load()
  lifecycle.stop()
  requests[4].resolve(todayReturn(4))
  assert.deepEqual(await beforeUnmount, { state: 'stale' })
  assert.deepEqual(await lifecycle.load(), { state: 'stale' })
})

test('首页独立请求受保护接口并仅在 complete 时格式化数值', () => {
  assert.match(walletSource, /client\.get<BackendTodayReturn>\(requestUrl\('\/wallet\/today-return'\)\)/)
  assert.match(homeSource, /const todayReturnState = ref<'idle' \| 'loading' \| 'complete' \| 'partial' \| 'error'>/)
  assert.match(homeSource, /createTodayReturnRequestLifecycle\(\{[\s\S]*sessionKey: \(\) => session\.token,[\s\S]*fetchTodayReturn/)
  assert.match(homeSource, /const result = await todayReturnRequestLifecycle\.load\(\)/)
  assert.match(homeSource, /todayReturnState\.value = result\.value\.status/)
  assert.match(homeSource, /todayReturnState\.value !== 'complete' \|\| !isCompleteTodayReturn\(value\)\) return '--'/)
  assert.match(homeSource, /todayReturnState\.value === 'partial'[\s\S]*todayReturnPartial/)
  assert.match(homeSource, /value\.rate \* 100/)
  assert.match(homeSource, /value\.amount > 0[\s\S]*return 'positive'[\s\S]*value\.amount < 0[\s\S]*return 'negative'/)
  assert.match(homeSource, /const displayedTodayReturnAmount = computed\(\(\) => \{\s*if \(!assetVisible\.value\) return '••••'/)
  assert.match(homeSource, /const displayedTodayReturnDetail = computed\(\(\) => \{\s*if \(!assetVisible\.value\) return '••••'/)
  assert.match(homeSource, /watch\(\(\) => session\.token,[\s\S]*todayReturnRequestLifecycle\.invalidate\(\)/)
  assert.match(homeSource, /onUnmounted\(\(\) => \{[\s\S]*todayReturnRequestLifecycle\.stop\(\)/)
  assert.doesNotMatch(homeSource, /fallbackTodayReturn|mockTodayReturn|demoTodayReturn/)
})

test('今日收益加载、失败和不完整文案中英文对称', () => {
  for (const key of [
    'todayReturnLoading',
    'todayReturnUnavailable',
    'todayReturnPartial',
    'todayReturnPartialUnknown',
  ] as const) {
    assert.equal(typeof zhCN.home[key], 'string')
    assert.equal(typeof en.home[key], 'string')
  }
})

function todayReturn(amount: number): TodayReturn {
  return {
    scope: 'realized',
    reportingAsset: 'USDT',
    amount,
    basisAmount: 10,
    rate: amount / 10,
    periodStartAt: 1_754_697_600_000,
    calculatedAt: 1_754_733_645_000,
    status: 'complete',
    missingPriceAssets: [],
  }
}

function deferred<T>(): {
  promise: Promise<T>
  resolve: (value: T) => void
  reject: (reason: unknown) => void
} {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
