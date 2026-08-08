import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import { goBackOr } from '../src/core/navigation.ts'
import {
  createSecondsHistoryRequestLifecycle,
  historicalSecondsOrders,
  type SecondsOrder,
} from '../src/core/secondsOrder.ts'

const routerSource = read('../src/router/index.ts')
const secondsSource = read('../src/views/SecondsView.vue')
const historySource = read('../src/views/SecondsHistoryView.vue')
const historyTemplate = templateOf(historySource)
const historyStyle = historySource.match(/<style\s+scoped>([\s\S]*?)<\/style>/)?.[1] || ''

function createSecondsRouter(): Router {
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/seconds', name: 'seconds', component: {}, meta: { depth: 1 } },
      {
        path: '/seconds/history',
        name: 'seconds-history',
        component: {},
        meta: { showBottomNav: false, depth: 2, backFallback: '/seconds' },
      },
    ],
  })
}

async function goBackOrAndWait(router: Router): Promise<void> {
  const navigation = new Promise<void>((resolve) => {
    const removeGuard = router.afterEach(() => {
      removeGuard()
      resolve()
    })
  })
  await goBackOr(router, '/seconds')
  await navigation
}

test('Seconds 历史命名路由支持 push、浏览器返回和直开兜底', async () => {
  assert.match(routerSource, /const SecondsHistoryView = \(\) => import\('@\/views\/SecondsHistoryView\.vue'\)/)
  assert.match(
    routerSource,
    /\{ path: '\/seconds\/history', name: 'seconds-history', component: SecondsHistoryView, meta: \{ showBottomNav: false, depth: 2, backFallback: '\/seconds' \} \}/,
  )
  assert.match(secondsSource, /function openHistory\(\): void \{\s*void router\.push\(\{ name: 'seconds-history' \}\)\s*\}/)
  assert.match(secondsSource, /:aria-label="t\('seconds\.historyTitle'\)" @click="openHistory"/)

  const stackedRouter = createSecondsRouter()
  await stackedRouter.push('/seconds')
  await stackedRouter.push({ name: 'seconds-history' })
  assert.equal(stackedRouter.currentRoute.value.fullPath, '/seconds/history')
  stackedRouter.options.history.replace('/seconds/history', {
    ...stackedRouter.options.history.state,
    back: '/seconds',
  })
  assert.equal(stackedRouter.options.history.state.back, '/seconds')
  await goBackOrAndWait(stackedRouter)
  assert.equal(stackedRouter.currentRoute.value.fullPath, '/seconds')

  const directRouter = createSecondsRouter()
  await directRouter.push({ name: 'seconds-history' })
  assert.equal(directRouter.options.history.state.back, undefined)
  await goBackOr(directRouter, '/seconds')
  assert.equal(directRouter.currentRoute.value.fullPath, '/seconds')
})

test('交易工作台只留活动订单，历史页只消费真实非活动订单', () => {
  assert.match(secondsSource, /const activeOrders = computed\(\(\) => activeSecondsOrders\(orders\.value\)\)/)
  assert.match(secondsSource, /v-for="order in activeOrders"/)
  assert.doesNotMatch(secondsSource, /seconds-session-records|seconds-orders|ordersSection|scrollToOrders/)
  assert.doesNotMatch(secondsSource, /formatDateTime/)

  const orders = [
    order(1, 'settled', 100),
    order(2, 'opened', 400),
    order(3, 'cancelled', 300),
    order(4, 'ACTIVE', 500),
  ]
  assert.deepEqual(historicalSecondsOrders(orders).map(({ id }) => id), [3, 1])

  assert.match(historySource, /createSecondsHistoryRequestLifecycle\(\{[\s\S]*?fetchOrders: fetchSecondsOrders/)
  assert.match(historySource, /historicalSecondsOrders\(orders\.value\)/)
  assert.match(historySource, /v-for="order in historyOrders"/)
  assert.match(historySource, /data-history-order="real"/)
  assert.match(historySource, /secondsOrderStatusPresentation\(order\)/)
})

test('历史请求生命周期隔离访客、重试、并发旧响应和退出登录', async () => {
  let authenticated = false
  const requests: Array<ReturnType<typeof deferred<SecondsOrder[]>>> = []
  const lifecycle = createSecondsHistoryRequestLifecycle({
    isAuthenticated: () => authenticated,
    fetchOrders: (limit) => {
      assert.equal(limit, 100)
      const request = deferred<SecondsOrder[]>()
      requests.push(request)
      return request.promise
    },
  })

  assert.deepEqual(await lifecycle.load(), { state: 'guest' })
  assert.equal(requests.length, 0)

  authenticated = true
  const first = lifecycle.load()
  const second = lifecycle.load()
  requests[1].resolve([order(2, 'settled', 200)])
  assert.deepEqual(await second, { state: 'loaded', orders: [order(2, 'settled', 200)] })
  requests[0].resolve([order(1, 'settled', 100)])
  assert.deepEqual(await first, { state: 'stale' })

  const beforeLogout = lifecycle.load()
  authenticated = false
  lifecycle.invalidate()
  requests[2].resolve([order(3, 'settled', 300)])
  assert.deepEqual(await beforeLogout, { state: 'stale' })

  authenticated = true
  const failed = lifecycle.load()
  const failure = new Error('network down')
  requests[3].reject(failure)
  assert.deepEqual(await failed, { state: 'error', error: failure })

  const retried = lifecycle.load()
  requests[4].resolve([order(4, 'cancelled', 400)])
  assert.deepEqual(await retried, { state: 'loaded', orders: [order(4, 'cancelled', 400)] })

  const beforeStop = lifecycle.load()
  lifecycle.stop()
  requests[5].resolve([order(5, 'settled', 500)])
  assert.deepEqual(await beforeStop, { state: 'stale' })
  assert.deepEqual(await lifecycle.load(), { state: 'stale' })
})

test('历史页完整展示 API 字段且缺失结算价不使用实时价替代', () => {
  assert.match(historyTemplate, /\{\{ order\.symbol \}\}/)
  assert.match(historyTemplate, /t\('seconds\.direction'\)[\s\S]*?order\.direction/)
  assert.match(historyTemplate, /t\('seconds\.stakeAmount'\)[\s\S]*?order\.stakeAmount[\s\S]*?order\.stakeAssetSymbol/)
  assert.match(historyTemplate, /t\('seconds\.term'\)[\s\S]*?order\.durationSeconds/)
  assert.match(historyTemplate, /t\('orders\.entryPrice'\)[\s\S]*?order\.entryPrice !== undefined \? formatPrice\(order\.entryPrice\) : '--'/)
  assert.match(historyTemplate, /t\('seconds\.settlementPrice'\)[\s\S]*?order\.settlementPrice !== undefined \? formatPrice\(order\.settlementPrice\) : '--'/)
  assert.match(historyTemplate, /orderStatusLabel\(order\)/)
  assert.match(historyTemplate, /t\('seconds\.createdTime'\)[\s\S]*?formatDateTime\(order\.createdAt\)/)
  assert.match(historyTemplate, /data-settlement-source="api-only"/)
  assert.doesNotMatch(historySource, /latestPrice|livePrice|ticker|marketStore|fetchSecondsProducts|fetchKlines/)
})

test('访客、加载、错误、列表和空态互斥且固定文案完全国际化', () => {
  assert.match(historyTemplate, /<LoginRequiredState\s+v-if="!session\.isAuthenticated"[\s\S]*?:description="t\('seconds\.historyLoginDescription'\)"/)
  assert.match(
    historyTemplate,
    /<template v-else>[\s\S]*?v-if="loading"[\s\S]*?v-else-if="error"[\s\S]*?v-else-if="historyOrders\.length"[\s\S]*?v-else class="seconds-history-state seconds-history-state--empty"/,
  )
  assert.match(historyTemplate, /v-else-if="error"[\s\S]*?role="alert"[\s\S]*?@click="load"[\s\S]*?t\('common\.retry'\)/)
  assert.doesNotMatch(historyTemplate, /[\u3400-\u9fff]/u)

  const keys = [
    'historyTitle',
    'historyContext',
    'historyLoginDescription',
    'historyLoading',
    'historyEmptyTitle',
    'historyEmptyDescription',
    'historyLoadFailed',
    'refreshHistory',
    'settlementPrice',
    'createdTime',
  ] as const
  for (const key of keys) {
    assert.equal(typeof en.seconds[key], 'string')
    assert.equal(typeof zhCN.seconds[key], 'string')
    assert.ok(en.seconds[key].length > 0)
    assert.ok(zhCN.seconds[key].length > 0)
  }
  assert.equal(en.seconds.historyTitle, 'Seconds order history')
  assert.equal(zhCN.seconds.historyTitle, '秒合约历史订单')
})

test('历史页使用语义主题、44px 操作、安全区与 320–448px 收缩结构', () => {
  assert.match(historyTemplate, /<PageHeader[\s\S]*?fallback="\/seconds"[\s\S]*?:pencil="true"/)
  assert.match(historyTemplate, /<RefreshCw/)
  assert.match(historyTemplate, /<ArrowUp[\s\S]*?<ArrowDown/)
  assert.match(historyStyle, /\.seconds-history-page\s*\{[\s\S]*?background: var\(--page\);[\s\S]*?overflow-x: clip;/)
  assert.match(historyStyle, /\.seconds-history-content\s*\{[\s\S]*?env\(safe-area-inset-right\)[\s\S]*?env\(safe-area-inset-bottom\)[\s\S]*?env\(safe-area-inset-left\)/)
  assert.match(historyStyle, /grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);/)
  assert.match(historyStyle, /\.seconds-history-order header\s*\{[\s\S]*?grid-template-columns: minmax\(0, 1fr\) minmax\(0, 45%\);/)
  assert.match(historyStyle, /\.seconds-history-order__status\s*\{[\s\S]*?overflow-wrap: anywhere;/)
  assert.match(historyStyle, /\.seconds-history-state--error button\s*\{[\s\S]*?min-height: 44px;/)
  assert.match(historyStyle, /\.seconds-history-login :deep\(\.button\)\s*\{[\s\S]*?min-height: 44px;/)
  assert.match(historyStyle, /@media \(max-width: 340px\)/)
  assert.match(historyStyle, /@media \(prefers-reduced-motion: reduce\)/)
  assert.doesNotMatch(historyStyle, /#[0-9a-f]{3,8}|rgba?\(/i)
  assert.doesNotMatch(historyStyle, /width:\s*100vw|overflow-x:\s*auto/)

  for (const width of [320, 390, 448]) {
    assert.ok(width - 32 >= 288, `${width}px history content must retain a usable inner width`)
  }
})

function read(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

function templateOf(source: string): string {
  const start = source.indexOf('<template>')
  const end = source.indexOf('<style scoped>')
  return start >= 0 && end > start ? source.slice(start + '<template>'.length, end) : ''
}

function order(id: number, status: string, createdAt: number): SecondsOrder {
  return {
    id,
    symbol: 'BTCUSDT',
    stakeAssetSymbol: 'USDT',
    direction: 'up',
    stakeAmount: 10,
    durationSeconds: 60,
    payoutRate: .8,
    status,
    createdAt,
    expiresAt: createdAt + 60_000,
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
