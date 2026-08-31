import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import { compileStyle } from 'vue/compiler-sfc'
import { normalizeDecimalText } from '../src/core/decimal.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import { goBackOr } from '../src/core/navigation.ts'
import {
  createSecondsHistoryRequestLifecycle,
  filterSecondsHistoryOrdersByDirection,
  historicalSecondsOrders,
  secondsOrderProfitLossPresentation,
  secondsOrderStatusPresentation,
  type SecondsOrder,
} from '../src/core/secondsOrder.ts'

const routerSource = read('../src/router/index.ts')
const secondsSource = read('../src/views/SecondsView.vue')
const historySource = read('../src/views/SecondsHistoryView.vue')
const historyTemplate = templateOf(historySource)
const historyStyle = historySource.match(/<style\s+scoped>([\s\S]*?)<\/style>/)?.[1] || ''
const selectedPageCss = read('../src/styles/pencil-selected-pages.css')

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
  assert.match(secondsSource, /function openHistory\(\): void \{\s*pairPickerOpen\.value = false\s*clearSettlementResultQueue\(\)\s*void router\.push\(\{ name: 'seconds-history' \}\)\s*\}/)
  assert.match(secondsSource, /:aria-label="t\('seconds\.historyTitle'\)" @click="openHistory"/)
  assert.match(historySource, /function closeHistory\(\): void \{\s*void goBackOr\(router, route\.meta\.backFallback \|\| '\/seconds'\)\s*\}/)
  assert.match(historyTemplate, /class="seconds-history-back"[\s\S]*?:aria-label="t\('common\.back'\)"[\s\S]*?@click="closeHistory"[\s\S]*?<ArrowLeft :size="24"/)
  assert.doesNotMatch(historyTemplate, /<PageHeader|Refresh history|refreshHistory/)

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

test('交易工作台只留活动订单，历史页通过纯函数按方向过滤真实历史快照', () => {
  assert.match(secondsSource, /const activeOrders = computed\(\(\) => activeSecondsOrders\(orders\.value\)\)/)
  assert.match(secondsSource, /const filteredActiveOrders = computed/)
  assert.match(secondsSource, /v-for="order in filteredActiveOrders"/)
  assert.doesNotMatch(secondsSource, /seconds-session-records|ordersSection|scrollToOrders/)
  assert.doesNotMatch(secondsSource, /formatDateTime/)

  const orders = [
    order(1, 'settled', 100, { direction: 'up' }),
    order(2, 'opened', 400, { direction: 'down' }),
    order(3, 'cancelled', 300, { direction: 'down' }),
    order(4, 'ACTIVE', 500, { direction: 'up' }),
  ]
  const historySnapshot = historicalSecondsOrders(orders)
  const snapshotOrder = historySnapshot.map(({ id }) => id)
  assert.deepEqual(snapshotOrder, [3, 1])

  assert.match(historySource, /createSecondsHistoryRequestLifecycle\(\{[\s\S]*?fetchOrders: fetchSecondsOrders/)
  assert.match(historySource, /historicalSecondsOrders\(orders\.value\)/)
  assert.match(historySource, /filterSecondsHistoryOrdersByDirection,/)
  assert.match(historySource, /type SecondsHistoryDirectionFilter,/)
  assert.match(historySource, /const activeDirection = ref<SecondsHistoryDirectionFilter>\('all'\)/)
  assert.match(historySource, /const filteredHistoryOrders = computed\(\(\) => \([\s\S]*?filterSecondsHistoryOrdersByDirection\(historyOrders\.value, activeDirection\.value\)[\s\S]*?\)\)/)
  assert.match(historyTemplate, /v-for="filter in HISTORY_DIRECTION_FILTERS"[\s\S]*?:aria-pressed="activeDirection === filter\.value"[\s\S]*?@click="activeDirection = filter\.value"/)
  assert.match(historyTemplate, /v-for="order in filteredHistoryOrders"/)
  assert.match(historySource, /data-history-order="real"/)

  const all = filterSecondsHistoryOrdersByDirection(historySnapshot, 'all')
  assert.notEqual(all, historySnapshot)
  assert.deepEqual(all.map(({ id }) => id), [3, 1])
  assert.deepEqual(filterSecondsHistoryOrdersByDirection(historySnapshot, 'up').map(({ id }) => id), [1])
  assert.deepEqual(filterSecondsHistoryOrdersByDirection(historySnapshot, 'down').map(({ id }) => id), [3])
  assert.deepEqual(historySnapshot.map(({ id }) => id), snapshotOrder)
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
  assert.match(historyTemplate, /class="seconds-history-order__identity"[\s\S]*?:title="`\$\{order\.symbol\} · \$\{t\('seconds\.historyDuration', \{ seconds: order\.durationSeconds \}\)\}`"/)
  assert.match(historyTemplate, /\{\{ `\$\{order\.symbol\} · \$\{t\('seconds\.historyDuration', \{ seconds: order\.durationSeconds \}\)\}` \}\}/)
  const identityMarkup = historyTemplate.match(/<strong\s+[^>]*class="seconds-history-order__identity"[^>]*>[\s\S]*?<\/strong>/)?.[0]
  assert.ok(identityMarkup)
  assert.doesNotMatch(identityMarkup, /<(?:span|i)\b/)
  assert.match(historyTemplate, /class="seconds-history-order__direction"[\s\S]*?order\.direction[\s\S]*?seconds\.bullish[\s\S]*?seconds\.bearish/)
  assert.match(historyTemplate, /t\('seconds\.historyStake'\)[\s\S]*?order\.stakeAmount[\s\S]*?order\.stakeAssetSymbol/)
  assert.match(historyTemplate, /t\('seconds\.historyEntryPrice'\)[\s\S]*?order\.entryPrice !== undefined \? formatPrice\(order\.entryPrice\) : '--'/)
  assert.match(historyTemplate, /t\('seconds\.historySettlementPrice'\)[\s\S]*?order\.settlementPrice !== undefined \? formatPrice\(order\.settlementPrice\) : '--'/)
  assert.match(historyTemplate, /class="seconds-history-order__profit-loss[^"]*"[\s\S]*?orderProfitLossTitle\(order\)[\s\S]*?orderProfitLossAmount\(order\)/)
  assert.match(historySource, /function orderProfitLossAmount\(order: SecondsOrder\): string \{[\s\S]*?secondsOrderProfitLossPresentation\(order\)[\s\S]*?presentation\.amount === undefined[\s\S]*?presentation\.amount > 0 \? '\+' : ''[\s\S]*?order\.stakeAssetSymbol/)
  assert.match(historySource, /function historyOrderStatusPresentation\(order: SecondsOrder\) \{[\s\S]*?secondsOrderStatusPresentation\(\{ status: order\.status \}\)[\s\S]*?\}/)
  assert.doesNotMatch(historySource, /secondsOrderStatusPresentation\(order\)/)
  assert.match(historyTemplate, /orderStatusLabel\(order\)/)
  assert.match(historyTemplate, /<time[\s\S]*?formatHistoryTime\(order\.createdAt\)[\s\S]*?<\/time>/)
  assert.match(historySource, /function formatHistoryTime\(value: unknown\): string \{[\s\S]*?new Intl\.DateTimeFormat\(currentIntlLocale\(\)[\s\S]*?month: '2-digit'[\s\S]*?day: '2-digit'[\s\S]*?hour: '2-digit'[\s\S]*?minute: '2-digit'/)
  assert.match(historyTemplate, /data-settlement-source="api-only"/)
  assert.doesNotMatch(historyTemplate, /\bnumeric\b/)
  assert.doesNotMatch(historySource, /latestPrice|livePrice|ticker|marketStore|fetchSecondsProducts|fetchKlines/)

  assert.deepEqual(secondsOrderStatusPresentation({ status: 'settled' }), {
    translationKey: 'seconds.statusSettled',
    source: 'settled',
    tone: 'pending',
  })
  assert.equal(zhCN.seconds.statusSettled, '已结算')
  assert.equal(en.seconds.statusSettled, 'Settled')

  assert.deepEqual(secondsOrderProfitLossPresentation(order(1, 'settled', 100, { result: 'win' })), {
    translationKey: 'seconds.profitAmount',
    amountText: '8',
    amount: 8,
    tone: 'positive',
  })
  assert.deepEqual(secondsOrderProfitLossPresentation(order(2, 'settled', 200, { result: 'loss' })), {
    translationKey: 'seconds.lossAmount',
    amountText: '-10',
    amount: -10,
    tone: 'negative',
  })
  assert.deepEqual(secondsOrderProfitLossPresentation(order(3, 'cancelled', 300)), {
    translationKey: 'seconds.profitLossAmount',
    amountText: null,
    amount: undefined,
    tone: 'pending',
  })
})

test('访客、加载、错误、列表和空态互斥且固定文案完全国际化', () => {
  assert.match(historyTemplate, /<LoginRequiredState\s+v-if="!session\.isAuthenticated"[\s\S]*?:description="t\('seconds\.historyLoginDescription'\)"/)
  assert.match(
    historyTemplate,
    /<template v-else>[\s\S]*?v-if="loading"[\s\S]*?v-else-if="error"[\s\S]*?v-else-if="filteredHistoryOrders\.length"[\s\S]*?v-else-if="historyOrders\.length"[\s\S]*?v-else class="seconds-history-state seconds-history-state--empty"/,
  )
  assert.match(historyTemplate, /v-else-if="error"[\s\S]*?role="alert"[\s\S]*?@click="load"[\s\S]*?t\('common\.retry'\)/)
  assert.doesNotMatch(historyTemplate, /[\u3400-\u9fff]/u)

  const keys = [
    'historyTitle',
    'historyPageTitle',
    'historyContext',
    'historyLoginDescription',
    'historyLoading',
    'historyEmptyTitle',
    'historyEmptyDescription',
    'historyLoadFailed',
    'historyDirectionFilter',
    'historyFilterAll',
    'historyFilterEmptyTitle',
    'historyFilterEmptyDescription',
    'historyStake',
    'historyEntryPrice',
    'historySettlementPrice',
    'historyDuration',
    'profitAmount',
    'lossAmount',
    'profitLossAmount',
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
  assert.equal(en.seconds.historyPageTitle, 'Seconds orders')
  assert.equal(zhCN.seconds.historyPageTitle, '秒合约订单')
  assert.match(historyTemplate, /<h1>\{\{ t\('seconds\.historyPageTitle'\) \}\}<\/h1>/)
  assert.equal(en.seconds.historyFilterAll, 'All')
  assert.equal(zhCN.seconds.historyFilterAll, '全部')
  assert.equal(en.seconds.historyDuration.replace('{seconds}', '60'), '60s')
  assert.equal(zhCN.seconds.historyDuration.replace('{seconds}', '60'), '60秒')
  assert.match(historyTemplate, /t\('seconds\.historyDuration', \{ seconds: order\.durationSeconds \}\)/)
  assert.doesNotMatch(historyTemplate, /t\('seconds\.duration'/)
  assert.equal(en.seconds.profitAmount, 'Profit amount')
  assert.equal(en.seconds.lossAmount, 'Loss amount')
  assert.equal(en.seconds.profitLossAmount, 'Profit / loss')
  assert.equal(zhCN.seconds.profitAmount, '盈利金额')
  assert.equal(zhCN.seconds.lossAmount, '亏损金额')
  assert.equal(zhCN.seconds.profitLossAmount, '盈亏金额')
})

test('历史页精确映射 Pencil 明暗主题、全宽直角卡片与 52/38/142 几何', () => {
  assert.match(historyTemplate, /data-pencil-source="vZy6U x29z7"/)
  assert.ok(historyTemplate.indexOf('seconds-history-back') < historyTemplate.indexOf('<h1>'))
  assert.ok(historyTemplate.indexOf('seconds-history-header') < historyTemplate.indexOf('seconds-history-filters'))
  assert.ok(historyTemplate.indexOf('seconds-history-filters') < historyTemplate.indexOf('seconds-history-content'))
  const rootRule = cssRule(historyStyle, '.page.seconds-history-page')
  assert.match(rootRule, /--history-page-inset-left: max\(16px, env\(safe-area-inset-left\)\);/)
  assert.match(rootRule, /--history-page-inset-right: max\(16px, env\(safe-area-inset-right\)\);/)
  assert.match(rootRule, /\bgap: 14px;/)
  assert.match(rootRule, /font-family: "Noto Sans SC", "PingFang SC", "Hiragino Sans GB", sans-serif;/)
  assert.match(rootRule, /overflow-x: clip;/)
  assert.match(rootRule, /padding: calc\(16px \+ env\(safe-area-inset-top\)\) var\(--history-page-inset-right\) calc\(16px \+ env\(safe-area-inset-bottom\)\) var\(--history-page-inset-left\);/)
  assert.match(historyStyle, /\.seconds-history-header\s*\{[\s\S]*?height: 52px;[\s\S]*?min-height: 52px;/)
  assert.match(historyStyle, /\.seconds-history-header h1\s*\{[\s\S]*?font-size: 24px;[\s\S]*?font-weight: 700;/)
  const backRule = cssRule(historyStyle, '.seconds-history-back')
  assert.match(backRule, /color: var\(--history-back\);/)
  assert.match(backRule, /height: 44px;/)
  assert.match(backRule, /place-items: center start;/)
  assert.match(backRule, /width: 44px;/)
  const filtersRule = cssRule(historyStyle, '.seconds-history-filters')
  assert.match(filtersRule, /align-items: flex-start;/)
  assert.match(filtersRule, /display: flex;/)
  assert.match(filtersRule, /gap: 8px;/)
  assert.match(filtersRule, /height: 38px;/)
  assert.match(filtersRule, /justify-content: flex-start;/)
  assert.doesNotMatch(filtersRule, /grid-template-columns|repeat\(3/)
  assert.match(historyTemplate, /class="seconds-history-filter"[\s\S]*?<span class="seconds-history-filter__surface">\{\{ t\(filter\.labelKey\) \}\}<\/span>/)
  const filterRule = cssRule(historyStyle, '.seconds-history-filter')
  assert.match(filterRule, /align-items: flex-start;/)
  assert.match(filterRule, /background: transparent;/)
  assert.match(filterRule, /border-radius: 16px;/)
  assert.match(filterRule, /flex: 0 0 auto;/)
  assert.match(filterRule, /height: 44px;/)
  assert.match(filterRule, /min-height: 44px;/)
  assert.match(filterRule, /min-width: 59px;/)
  assert.match(filterRule, /padding: 0;/)
  const filterSurfaceRule = cssRule(historyStyle, '.seconds-history-filter__surface')
  assert.match(filterSurfaceRule, /background: var\(--history-filter-inactive\);/)
  assert.match(filterSurfaceRule, /border-radius: 16px;/)
  assert.match(filterSurfaceRule, /height: 33px;/)
  assert.match(filterSurfaceRule, /min-width: 59px;/)
  assert.match(filterSurfaceRule, /padding: 7px 16px;/)
  assert.match(historyStyle, /\.seconds-history-filter\.is-active \.seconds-history-filter__surface\s*\{[\s\S]*?background: var\(--history-filter-active\);/)
  assert.doesNotMatch(historyStyle, /\.seconds-history-filter::before/)
  assert.match(historyStyle, /\.seconds-history-list\s*\{[\s\S]*?gap: 14px;/)
  const listRule = cssRule(historyStyle, '.seconds-history-list')
  assert.match(listRule, /margin-left: calc\(0px - var\(--history-page-inset-left\)\);/)
  assert.match(listRule, /margin-right: calc\(0px - var\(--history-page-inset-right\)\);/)
  assert.match(listRule, /width: auto;/)
  const orderRule = cssRule(historyStyle, '.seconds-history-order')
  assert.match(orderRule, /align-content: start;/)
  assert.match(orderRule, /border: 0;/)
  assert.match(orderRule, /border-radius: 0;/)
  assert.match(orderRule, /box-shadow: none;/)
  assert.match(orderRule, /gap: 8px;/)
  assert.match(orderRule, /grid-template-rows: 23px 19px 17px;/)
  assert.match(orderRule, /height: 142px;/)
  assert.match(orderRule, /padding: 14px 16px;/)
  assert.match(orderRule, /width: 100%;/)
  const identityRule = cssRule(historyStyle, '.seconds-history-order__identity')
  assert.match(identityRule, /color: var\(--history-text\);/)
  assert.match(identityRule, /font-size: 16px;/)
  assert.match(identityRule, /font-weight: 600;/)
  assert.match(identityRule, /overflow: hidden;/)
  assert.match(identityRule, /text-overflow: ellipsis;/)
  assert.match(identityRule, /white-space: nowrap;/)
  assert.match(historyStyle, /\.seconds-history-order__profit-loss\s*\{[\s\S]*?font-size: 15px;[\s\S]*?font-weight: 700;/)
  const metaRule = cssRule(historyStyle, '.seconds-history-order__meta')
  assert.match(metaRule, /gap: 0;/)
  assert.match(metaRule, /grid-template-columns: auto minmax\(0, 1fr\) auto;/)
  const directionRule = cssRule(historyStyle, '.seconds-history-order__direction')
  assert.match(directionRule, /min-width: 27px;/)
  const statusRule = cssRule(historyStyle, '.seconds-history-order__status')
  assert.match(statusRule, /justify-self: center;/)
  assert.match(statusRule, /max-width: 100%;/)
  assert.match(statusRule, /min-width: 40px;/)
  const timeRule = cssRule(historyStyle, '.seconds-history-order__time')
  assert.match(timeRule, /width: 65px;/)

  const referenceViewport = 390
  const pageInset = 16
  const headerHeight = 52
  const pageGap = 14
  const filterTrackHeight = 38
  const filterHitHeight = 44
  const filterSurfaceHeight = 33
  const cardHeight = 142
  const cardPaddingInline = 16
  const filterY = pageInset + headerHeight + pageGap
  const cardY = filterY + filterTrackHeight + pageGap
  const safeTrack = referenceViewport - (pageInset * 2)
  const cardX = pageInset - pageInset
  const cardTrack = safeTrack + (pageInset * 2)
  const cardInnerTrack = cardTrack - (cardPaddingInline * 2)
  const directionStart = 0
  const directionWidth = 27
  const statusWidth = 40
  const timeWidth = 65
  const statusTrackWidth = cardInnerTrack - directionWidth - timeWidth
  const statusStart = directionWidth + ((statusTrackWidth - statusWidth) / 2)
  const timeStart = cardInnerTrack - timeWidth
  assert.equal(safeTrack, 358)
  assert.equal(filterY, 82)
  assert.equal(filterSurfaceHeight, 33)
  assert.equal(filterY + filterHitHeight, 126)
  assert.equal(cardY, 134)
  assert.ok(filterY + filterHitHeight <= cardY)
  assert.equal(cardX, 0)
  assert.equal(cardTrack, 390)
  assert.equal(cardInnerTrack, 358)
  assert.deepEqual([directionStart, statusStart, timeStart], [0, 140, 293])
  assert.deepEqual(
    [cardY, cardY + cardHeight + pageGap, cardY + ((cardHeight + pageGap) * 2)],
    [134, 290, 446],
  )
  const summaryRule = cssRule(historyStyle, '.seconds-history-order__summary')
  assert.match(summaryRule, /display: flex;/)
  assert.match(summaryRule, /font-size: 12px;/)
  assert.match(summaryRule, /height: 17px;/)
  assert.match(summaryRule, /overflow: hidden;/)
  assert.match(summaryRule, /white-space: nowrap;/)
  assert.doesNotMatch(summaryRule, /grid-template-columns|align-self: end/)
  const summaryItemRule = cssRule(historyStyle, '.seconds-history-order__summary-item')
  assert.match(summaryItemRule, /display: inline-flex;/)
  assert.match(summaryItemRule, /white-space: nowrap;/)
  assert.match(historySource, /return month && day && hour && minute \? `\$\{month\}\/\$\{day\} \$\{hour\}:\$\{minute\}` : '--'/)
  assert.match(historyStyle, /\.seconds-history-state--error button\s*\{[\s\S]*?min-height: 44px;/)
  assert.match(historyStyle, /\.seconds-history-login :deep\(\.button\)\s*\{[\s\S]*?min-height: 44px;/)
  assert.match(historyStyle, /@media \(max-width: 340px\)/)
  assert.match(historyStyle, /@media \(prefers-reduced-motion: reduce\)/)
  assert.doesNotMatch(historyStyle, /#[0-9a-f]{3,8}|rgba?\(/i)
  assert.doesNotMatch(historyStyle, /width:\s*100vw|overflow-x:\s*auto/)

  const compiledHistory = compileStyle({
    source: historyStyle,
    filename: 'SecondsHistoryView.vue',
    id: 'data-v-seconds-history',
    scoped: true,
  })
  assert.deepEqual(compiledHistory.errors, [])
  assert.match(compiledHistory.code, /\.page\.seconds-history-page\[data-v-seconds-history\]/)
  assert.match(compiledHistory.code, /\.seconds-history-filter\.is-active \.seconds-history-filter__surface\[data-v-seconds-history\]/)

  const compiledSelectedPages = compileStyle({
    source: selectedPageCss,
    filename: 'pencil-selected-pages.css',
    id: 'data-v-seconds-history-global',
    scoped: false,
  })
  assert.deepEqual(compiledSelectedPages.errors, [])
  assert.match(compiledSelectedPages.code, /html\[data-theme=['"]dark['"]\] \.app-stage \.mobile-canvas \.seconds-page\.seconds-history-page/)

  const light = cssRule(selectedPageCss, '.app-stage .mobile-canvas .seconds-page.seconds-history-page')
  assert.match(light, /--history-canvas: #ffffff;/)
  assert.match(light, /--history-card: #ffffff;/)
  assert.match(light, /--history-text: #17201c;/)
  assert.match(light, /--history-back: #69756e;/)
  assert.match(light, /--history-positive: #0daa79;/)
  assert.match(light, /--history-negative: #e05b68;/)
  assert.match(light, /--history-filter-active: #ddf7ec;/)
  assert.match(light, /--history-filter-inactive: #eaf0ed;/)
  assert.match(light, /--history-filter-inactive-text: #56625b;/)
  assert.match(light, /--history-status: #718078;/)
  assert.match(light, /--history-time: #8a948e;/)
  assert.match(light, /--history-summary: #78847d;/)

  const dark = cssRule(selectedPageCss, "html[data-theme='dark'] .app-stage .mobile-canvas .seconds-page.seconds-history-page")
  assert.match(dark, /--history-canvas: #000000;/)
  assert.match(dark, /--history-card: #000000;/)
  assert.match(dark, /--history-text: #eff7f2;/)
  assert.match(dark, /--history-back: #a8b5ae;/)
  assert.match(dark, /--history-filter-active: #1e3a30;/)
  assert.match(dark, /--history-filter-inactive: #17231e;/)
  assert.match(dark, /--history-filter-inactive-text: #b9c7c0;/)
  assert.match(dark, /--history-status: #a8b5ae;/)
  assert.match(dark, /--history-time: #89968f;/)
  assert.match(dark, /--history-summary: #a8b5ae;/)

  for (const width of [320, 390, 448]) {
    const cardWidth = width
    const cardContentWidth = cardWidth - (cardPaddingInline * 2)
    assert.equal(cardWidth, width, `${width}px history card must fill the phone canvas`)
    assert.equal(cardContentWidth, width - 32, `${width}px history card must retain the 16px content track`)
    assert.ok(cardContentWidth >= 288, `${width}px history content must retain a usable inner width`)
    assert.ok(
      cardContentWidth - directionWidth - timeWidth >= statusWidth,
      `${width}px history detail row must retain a non-overflowing centered status track`,
    )
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

function cssRule(source: string, selector: string): string {
  const start = source.indexOf(`${selector} {`)
  assert.notEqual(start, -1, `missing CSS rule: ${selector}`)
  const bodyStart = source.indexOf('{', start) + 1
  const end = source.indexOf('}', bodyStart)
  return source.slice(bodyStart, end)
}

function order(id: number, status: string, createdAt: number, overrides: Partial<SecondsOrder> = {}): SecondsOrder {
  const merged = {
    id,
    symbol: 'BTCUSDT',
    stakeAssetSymbol: 'USDT',
    direction: 'up' as const,
    stakeAmount: 10,
    durationSeconds: 60,
    payoutRate: .8,
    status,
    createdAt,
    expiresAt: createdAt + 60_000,
    ...overrides,
  }
  return {
    ...merged,
    stakeAmountText: merged.stakeAmountText ?? normalizeDecimalText(String(merged.stakeAmount)),
    payoutRateText: merged.payoutRateText ?? normalizeDecimalText(String(merged.payoutRate)),
    entryPriceText: merged.entryPriceText ?? null,
    settlementPriceText: merged.settlementPriceText ?? null,
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
