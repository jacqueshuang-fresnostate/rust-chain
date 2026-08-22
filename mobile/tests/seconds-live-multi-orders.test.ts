import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const pageHeaderSource = read('../src/components/PageHeader.vue')
const secondsApiSource = read('../src/api/seconds.ts')
const secondsSource = read('../src/views/SecondsView.vue')
const secondsStyle = secondsSource.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)?.[1] || ''

test('PageHeader 为 Seconds 提供向后兼容的中间插槽且交易对按选中稿绝对居中', () => {
  assert.match(
    pageHeaderSource,
    /<div class="page-header__copy">\s*<slot name="center">\s*<slot name="copy">[\s\S]*?<strong class="page-header__title">\{\{ title \}\}<\/strong>[\s\S]*?<\/slot>\s*<\/slot>\s*<\/div>/,
  )

  const pageHeader = secondsSource.match(/<PageHeader[\s\S]*?<\/PageHeader>/)?.[0] || ''
  assert.match(pageHeader, /<template #center>[\s\S]*?<label class="seconds-pair-field"[\s\S]*?<strong>\{\{ selectedPairLabel \}\}<\/strong>[\s\S]*?<ChevronDown[\s\S]*?<select[\s\S]*?product\.symbol[\s\S]*?<\/template>/)
  assert.match(pageHeader, /<template #actions>[\s\S]*?t\('seconds\.historyTitle'\)[\s\S]*?@click="openHistory"/)
  assert.match(secondsSource, /function openHistory\(\): void \{\s*clearSettlementResultQueue\(\)\s*void router\.push\(\{ name: 'seconds-history' \}\)\s*\}/)
  assert.equal((pageHeader.match(/class="seconds-pair-field"/g) || []).length, 1)
  assert.equal((secondsSource.match(/class="seconds-pair-field"/g) || []).length, 1)
  assert.doesNotMatch(secondsSource.slice(secondsSource.indexOf('</PageHeader>') + 13), /class="seconds-pair-field"/)

  const pairRule = cssRule(secondsStyle, '.seconds-pair-field {')
  assert.match(pairRule, /height: 22px;/)
  assert.match(pairRule, /min-width: 0;/)
  assert.match(pairRule, /position: relative;/)
  assert.match(pairRule, /width: 140px;/)

  const selectRule = cssRule(secondsStyle, '.seconds-pair-field select {')
  assert.match(selectRule, /height: 44px;/)
  assert.match(selectRule, /inset: -11px 0 auto;/)
  assert.match(selectRule, /opacity: 0;/)
  assert.match(selectRule, /width: 100%;/)
  const focusRule = cssRule(secondsStyle, '.seconds-pair-field:focus-within {')
  assert.match(focusRule, /outline: 2px solid var\(--focus\);/)
  assert.match(focusRule, /outline-offset: 3px;/)

  for (const width of [320, 390, 448]) {
    assert.ok(width - 20 * 2 - 40 * 2 >= 140, `${width}px header center track must fit the 140px title`)
  }
})

test('Seconds 使用内部 ticker 与 1m K 线会话并保留 REST/WS generation 竞态保护', () => {
  assert.match(secondsSource, /import \{ subscribeTickers, type TickerUpdate \} from '@\/api\/marketSocket'/)
  assert.match(secondsSource, /import \{ createMarketDetailStreamSession \} from '@\/api\/marketDetailStream'/)
  assert.match(secondsSource, /getUrl: publicMarketWebSocketUrl,\s*channels: \['kline'\]/)
  assert.match(secondsSource, /\.\.\.products\.value\.map\(\(product\) => normalizeProductSymbol\(product\.symbol\)\)/)
  assert.match(secondsSource, /\.\.\.activeOrders\.value\.map\(\(order\) => normalizeProductSymbol\(order\.symbol\)\)/)
  assert.match(secondsSource, /stopTickerSubscription = subscribeTickers\(normalizedSymbols, \(update\) => \{/)
  assert.match(secondsSource, /generation !== tickerSubscriptionGeneration/)
  assert.match(secondsSource, /const liveTickerSnapshots = ref<Record<string, TickerUpdate>>\(\{\}\)/)
  assert.match(secondsSource, /previous\?\.observedAt !== undefined[\s\S]*?update\.observedAt < previous\.observedAt[\s\S]*?return/)
  assert.match(secondsSource, /\[update\.symbol\]: \{[\s\S]*?\.\.\.previous,[\s\S]*?\.\.\.update,[\s\S]*?update\.changePercent === undefined[\s\S]*?update\.observedAt === undefined/)

  assert.match(secondsSource, /secondsKlineSession\.replace\(symbol, '1m', requestVersion\)/)
  assert.match(secondsSource, /secondsKlineSession\.beginKlineRequest\(context\)/)
  assert.match(secondsSource, /fetchKlines\(symbol, '1m'\)/)
  assert.match(secondsSource, /secondsKlineSession\.isCurrent\(context, symbol, '1m', requestVersion\)/)
  assert.match(secondsSource, /secondsKlineSession\.isCurrentKlineRequest\(request\)/)
  assert.match(secondsSource, /secondsKlineSession\.resolveKlineRequest\(request, nextPoints\)/)
  assert.match(secondsSource, /const livePrice = liveTickerSnapshots\.value\[normalized\]\?\.lastPrice[\s\S]*?sparklinePoints\.value\.at\(-1\)\?\.close[\s\S]*?marketStore\.tickerFor\(symbol\)\?\.lastPrice/)
  assert.match(secondsSource, /const selectedChangePercent = computed<number \| null>[\s\S]*?const liveChange = selectedLiveTicker\.value\?\.changePercent\s*if \(Number\.isFinite\(liveChange\)\) return Number\(liveChange\)\s*const snapshotChange = selectedTicker\.value\?\.changePercent/)

  assert.match(secondsSource, /onBeforeUnmount\(\(\) => \{[\s\S]*?chartRequestVersion \+= 1[\s\S]*?tickerSubscriptionGeneration \+= 1[\s\S]*?secondsKlineSession\.stop\(\)[\s\S]*?stopTickerSubscription\?\.\(\)/)
  assert.doesNotMatch(secondsSource, /https?:\/\/www\.tradingview|<iframe|<script[^>]+src=/i)
})

test('Seconds 确认层冻结提交快照并让同一次重试复用幂等键', () => {
  assert.match(secondsSource, /interface SecondsOrderReview \{[\s\S]*?readonly productId: number[\s\S]*?readonly referencePrice: number[\s\S]*?readonly idempotencyKey: string/)
  assert.match(
    secondsSource,
    /orderReview\.value = Object\.freeze\(\{[\s\S]*?productId: product\.id,[\s\S]*?durationSeconds: activeCycle\.durationSeconds,[\s\S]*?direction: direction\.value,[\s\S]*?stakeAmount: amountNumber\.value,[\s\S]*?referencePrice: selectedLatestPrice\.value,[\s\S]*?idempotencyKey: createSecondsOrderIdempotencyKey\(\)/,
  )
  assert.match(secondsSource, /function isOrderReviewValid\(review: SecondsOrderReview\): boolean \{[\s\S]*?review\.stakeAmount >= currentCycle\.minStake[\s\S]*?review\.stakeAmount <= \(currentAccount\?\.available \|\| 0\)/)

  const submitSource = secondsSource.match(
    /async function submit\(\): Promise<void> \{[\s\S]*?(?=\nasync function reconcileOpenedOrder)/,
  )?.[0] || ''
  assert.match(submitSource, /const review = orderReview\.value[\s\S]*?!review \|\| !isOrderReviewValid\(review\)/)
  assert.match(submitSource, /productId: review\.productId,[\s\S]*?durationSeconds: review\.durationSeconds,[\s\S]*?direction: review\.direction,[\s\S]*?stakeAmount: review\.stakeAmount,[\s\S]*?idempotencyKey: review\.idempotencyKey,/)
  assert.doesNotMatch(submitSource, /createSecondsOrderIdempotencyKey/)
  assert.match(secondsSource, /v-if="confirmOpen && orderReview"[\s\S]*?orderReview\.symbol[\s\S]*?orderReview\.direction[\s\S]*?orderReview\.stakeAmount[\s\S]*?orderReview\.payoutRate[\s\S]*?orderReview\.referencePrice/)

  assert.match(secondsApiSource, /idempotencyKey\?: string/)
  assert.match(secondsApiSource, /idempotency_key: input\.idempotencyKey \|\| createSecondsOrderIdempotencyKey\(\)/)
})

test('Seconds 渲染全部活动订单、本地方向筛选与并行下单表单并按订单批量到期对账', () => {
  assert.match(secondsSource, /const activeOrders = computed\(\(\) => activeSecondsOrders\(orders\.value\)\)/)
  assert.match(secondsSource, /const filteredActiveOrders = computed\(\(\) => \([\s\S]*?activeOrderFilter\.value === 'all'[\s\S]*?order\.direction === activeOrderFilter\.value/)
  assert.match(secondsSource, /@click="activeOrderFilter = 'all'"[\s\S]*?@click="activeOrderFilter = 'up'"[\s\S]*?@click="activeOrderFilter = 'down'"/)
  assert.match(secondsSource, /v-if="filteredActiveOrders\.length"[\s\S]*?:data-active-order-list="activeOrderFilter"/)
  assert.match(secondsSource, /v-for="order in filteredActiveOrders"/)
  assert.match(secondsSource, /:data-active-order-id="order\.id"/)
  assert.match(secondsSource, /<AssetMark[\s\S]*?:src="marketStore\.tickerFor\(order\.symbol\)\?\.baseIconUrl \|\| marketStore\.tickerFor\(order\.symbol\)\?\.iconUrl"/)
  assert.match(secondsSource, /displayProductSymbol\(order\.symbol\)[\s\S]*?orderCountdown\(order\)[\s\S]*?order\.stakeAmount[\s\S]*?order\.entryPrice[\s\S]*?orderEstimatedProfit\(order\)[\s\S]*?orderProgress\(order\)/)

  assert.equal((secondsSource.match(/:disabled="loading \|\| !selected"/g) || []).length, 4)
  assert.match(secondsSource, /:disabled="submitting \|\| loading \|\| !selected"/)
  assert.match(secondsSource, /async function submit\(\): Promise<void> \{\s*if \(submitting\.value\) return/)

  const mutationStart = secondsSource.indexOf('openedOrder = await openSecondsOrder({')
  const immediateUpsert = secondsSource.indexOf('orders.value = upsertSecondsOrder(orders.value, openedOrder)', mutationStart)
  const successCommit = secondsSource.indexOf("success.value = t('seconds.created')", immediateUpsert)
  const submittingReleased = secondsSource.indexOf('submitting.value = false', successCommit)
  const reconciliation = secondsSource.indexOf(
    'void reconcileOpenedOrder(mutationSessionGeneration)',
    submittingReleased,
  )
  assert.ok(
    mutationStart >= 0
    && immediateUpsert > mutationStart
    && successCommit > immediateUpsert
    && submittingReleased > successCommit
    && reconciliation > submittingReleased,
  )
  assert.match(secondsSource, /committedOrdersById\.set\(openedOrder\.id, openedOrder\)/)
  assert.match(secondsSource, /mergeSecondsOrderReconciliation\(nextOrders, committedOrders\)/)
  assert.match(secondsSource, /refreshWarning\.value = t\('seconds\.refreshAfterOrderFailed'\)/)
  assert.doesNotMatch(secondsSource.slice(mutationStart, secondsSource.indexOf('function statusLabel', mutationStart)), /await load\(\)/)

  assert.match(secondsSource, /const expiryRetryAtByOrderId = new Map<number, number>\(\)/)
  assert.match(secondsSource, /const queuedExpiryOrderIds = new Set<number>\(\)/)
  assert.match(secondsSource, /const reconcilingExpiryOrderIds = new Set<number>\(\)/)
  assert.match(secondsSource, /retryAt <= now && !reconcilingExpiryOrderIds\.has\(orderId\)/)
  assert.match(secondsSource, /if \(!queuedExpiryOrderIds\.size \|\| expiryReconciliationPromise\) return/)
  assert.match(secondsSource, /const batch = \[\.\.\.queuedExpiryOrderIds\][\s\S]*?const reconciliation = await reconcilePrivateState\(\)/)
  assert.match(secondsSource, /fetchSecondsOrders\(100\),\s*fetchWalletAccounts\(\)/)
  assert.match(secondsSource, /Date\.now\(\) \+ EXPIRY_RECONCILIATION_RETRY_MS/)
  assert.match(secondsSource, /fullyLoaded[\s\S]*?&& !activeIds\.has\(orderId\)[\s\S]*?&& !settlementResultTracker\.isTracking\(orderId\)[\s\S]*?expiryRetryAtByOrderId\.delete\(orderId\)/)
  assert.match(secondsSource, /queueExpiredOrderReconciliation\(currentTime\.value\)/)

  assert.equal(zhCN.seconds.activeOrders, '活动订单')
  assert.equal(en.seconds.activeOrders, 'Active orders')
  assert.match(zhCN.seconds.refreshAfterOrderFailed, /订单已创建/)
  assert.match(en.seconds.refreshAfterOrderFailed, /order was created/i)
})

test('Seconds 使用权威结果追踪器、FIFO 队列和非模态沉浸结算卡', () => {
  assert.match(secondsSource, /const settlementResultTracker = createSecondsSettlementResultTracker\(\)/)
  assert.match(secondsSource, /const settlementResultQueue = ref<SecondsOrder\[]>\(\[\]\)/)
  assert.match(
    secondsSource,
    /function applyReconciledOrders\(nextOrders: readonly SecondsOrder\[]\): void \{[\s\S]*?settlementResultTracker\.reconcile\(nextOrders\)[\s\S]*?enqueueSecondsSettlementResults\([\s\S]*?settlementResultQueue\.value,[\s\S]*?settledResults,[\s\S]*?mergeSecondsOrderReconciliation\(nextOrders, committedOrders\)/,
  )
  assert.match(
    secondsSource,
    /openedOrder = await openSecondsOrder\([\s\S]*?settlementResultTracker\.track\(openedOrder\)[\s\S]*?committedOrdersById\.set\(openedOrder\.id, openedOrder\)/,
  )
  assert.match(
    secondsSource,
    /const currentSettlementPresentation = computed\([\s\S]*?secondsOrderProfitLossPresentation\(currentSettlementResult\.value\)/,
  )
  assert.match(
    secondsSource,
    /const currentSettlementAmount = computed\([\s\S]*?presentation\.translationKey === 'seconds\.profitAmount' \? '\+' : ''[\s\S]*?formatAmount\(presentation\.amount\)[\s\S]*?order\.stakeAssetSymbol/,
  )

  assert.match(
    secondsSource,
    /function clearSecondsPrivateState\(\): void \{[\s\S]*?orders\.value = \[\][\s\S]*?settlementResultTracker\.reset\(\)[\s\S]*?clearSettlementResultQueue\(\)/,
  )
  assert.match(
    secondsSource,
    /watch\(\(\) => session\.isAuthenticated,[\s\S]*?if \(authenticated\) return[\s\S]*?clearSecondsPrivateState\(\)/,
  )
  assert.match(
    secondsSource,
    /onBeforeUnmount\(\(\) => \{[\s\S]*?settlementResultTracker\.reset\(\)[\s\S]*?clearSettlementResultQueue\(\)/,
  )
  assert.match(
    secondsSource,
    /function isCurrentSecondsMutationSession\(generation: number\): boolean \{[\s\S]*?componentActive[\s\S]*?session\.isAuthenticated[\s\S]*?generation === privateSessionGeneration/,
  )
  const submitSource = secondsSource.match(
    /async function submit\(\): Promise<void> \{[\s\S]*?(?=\nasync function reconcileOpenedOrder)/,
  )?.[0] || ''
  assert.match(submitSource, /const mutationSessionGeneration = privateSessionGeneration/)
  assert.match(
    submitSource,
    /catch \(reason\) \{\s*if \(isCurrentSecondsMutationSession\(mutationSessionGeneration\)\) \{\s*error\.value = apiErrorMessage/,
  )
  assert.match(
    submitSource,
    /if \(openedOrder && isCurrentSecondsMutationSession\(mutationSessionGeneration\)\) \{\s*void reconcileOpenedOrder\(mutationSessionGeneration\)/,
  )
  assert.match(
    secondsSource,
    /async function reconcileOpenedOrder\(mutationSessionGeneration: number\): Promise<void> \{[\s\S]*?await reconcilePrivateState\(\)[\s\S]*?isCurrentSecondsMutationSession\(mutationSessionGeneration\)/,
  )
  assert.match(secondsSource, /watch\(\(\) => session\.isAuthenticated,[\s\S]*?privateSessionGeneration \+= 1[\s\S]*?\{ flush: 'sync' \}/)

  const settlementTemplate = secondsSource.match(
    /<Teleport to="body">\s*<Transition name="seconds-result-reveal"[\s\S]*?<\/Teleport>/,
  )?.[0] || ''
  assert.match(settlementTemplate, /class="seconds-settlement-layer"/)
  assert.doesNotMatch(settlementTemplate, /data-app-theme|settlementTheme/)
  assert.match(settlementTemplate, /class="sr-only" role="status" aria-live="polite" aria-atomic="true"[\s\S]*?currentSettlementAnnouncement/)
  assert.match(settlementTemplate, /class="seconds-settlement-card"[\s\S]*?data-settlement-source="orders-api"[\s\S]*?:aria-labelledby=/)
  assert.doesNotMatch(settlementTemplate, /class="seconds-settlement-card"[\s\S]{0,180}?role="status"/)
  assert.match(settlementTemplate, /Trophy[\s\S]*?TrendingDown[\s\S]*?currentSettlementAmount/)
  assert.match(settlementTemplate, /currentSettlementResult\.symbol[\s\S]*?currentSettlementResult\.direction[\s\S]*?currentSettlementResult\.durationSeconds/)
  assert.match(settlementTemplate, /settlementResultsRemaining[\s\S]*?remainingSettlementResults/)
  assert.match(settlementTemplate, /@click="advanceSettlementResult"[\s\S]*?seconds\.continueTrading[\s\S]*?@click="openHistory"[\s\S]*?seconds\.viewHistory/)
  assert.doesNotMatch(settlementTemplate, /aria-modal|confirmation-layer|@click\.self|entryPrice|settlementPrice|latestPrice/i)

  const layerRule = cssRule(secondsStyle, '.seconds-settlement-layer {')
  assert.match(layerRule, /max-width: min\(448px, var\(--app-max-width, 448px\)\);/)
  assert.match(layerRule, /pointer-events: none;/)
  assert.match(layerRule, /top: calc\(env\(safe-area-inset-top, 0px\) \+ 60px\);/)
  const resultTokenAliases = {
    surface: 'surface',
    'surface-elevated': 'surface-elevated',
    ink: 'ink',
    muted: 'muted',
    line: 'line',
    shadow: 'dark-surface',
    positive: 'positive',
    negative: 'negative',
    'on-accent': 'on-accent',
    focus: 'focus',
  } as const
  for (const [localRole, rootToken] of Object.entries(resultTokenAliases)) {
    assert.match(
      layerRule,
      new RegExp(`--seconds-result-${localRole}: var\\(--${rootToken}\\);`),
    )
  }
  assert.match(layerRule, /--seconds-result-focus-ring: color-mix\(in srgb, var\(--focus\) 28%, transparent\);/)
  assert.doesNotMatch(secondsSource, /#[0-9a-f]{3,8}|rgba?\(/i)
  assert.doesNotMatch(secondsSource, /settlementTheme|syncSecondsTheme|data-app-theme/)
  assert.match(secondsSource, /chartThemeObserver = new MutationObserver\(drawSparkline\)/)
  const cardRule = cssRule(secondsStyle, '.seconds-settlement-card {')
  assert.match(cardRule, /pointer-events: auto;/)
  assert.match(cardRule, /border-radius: 28px;/)
  assert.match(secondsStyle, /\.seconds-settlement-card__panel\s*\{[\s\S]*?backdrop-filter: blur\(24px\) saturate\(148%\);/)
  assert.match(secondsStyle, /\.seconds-settlement-card__actions button\s*\{[\s\S]*?min-height: 44px;/)
  assert.match(secondsStyle, /@media \(max-width: 340px\)[\s\S]*?\.seconds-settlement-card__actions\s*\{[\s\S]*?grid-template-columns: 1fr;/)
  assert.match(secondsStyle, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.seconds-result-reveal-enter-active,[\s\S]*?transition: none !important;/)

  assert.equal(zhCN.seconds.settlementProfit, '结算盈利')
  assert.equal(zhCN.seconds.settlementLoss, '结算亏损')
  assert.equal(zhCN.seconds.continueTrading, '继续交易')
  assert.equal(zhCN.seconds.viewHistory, '查看历史订单')
  assert.match(zhCN.seconds.settlementAnnouncement, /\{title\}[\s\S]*\{amount\}[\s\S]*\{duration\}/)
  assert.equal(en.seconds.settlementProfit, 'Settlement profit')
  assert.equal(en.seconds.settlementLoss, 'Settlement loss')
  assert.equal(en.seconds.continueTrading, 'Continue trading')
  assert.equal(en.seconds.viewHistory, 'View order history')
  assert.match(en.seconds.settlementAnnouncement, /\{title\}[\s\S]*\{amount\}[\s\S]*\{duration\}/)
})

function read(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

function cssRule(source: string, marker: string): string {
  const markerIndex = source.indexOf(marker)
  assert.notEqual(markerIndex, -1, `missing CSS marker: ${marker}`)
  const openIndex = source.indexOf('{', markerIndex)
  const closeIndex = source.indexOf('}', openIndex)
  assert.notEqual(closeIndex, -1, `missing CSS closing brace: ${marker}`)
  return source.slice(openIndex + 1, closeIndex)
}
