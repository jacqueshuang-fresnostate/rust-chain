import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const pageHeaderSource = read('../src/components/PageHeader.vue')
const secondsApiSource = read('../src/api/seconds.ts')
const secondsFinancialSource = read('../src/core/secondsFinancial.ts')
const secondsSource = read('../src/views/SecondsView.vue')
const secondsStyle = secondsSource.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)?.[1] || ''
const selectedPageCss = read('../src/styles/pencil-selected-pages.css')

test('PageHeader 为 Seconds 提供向后兼容的中间插槽且交易对按选中稿绝对居中', () => {
  assert.match(
    pageHeaderSource,
    /<div class="page-header__copy">\s*<slot name="center">\s*<slot name="copy">[\s\S]*?<strong class="page-header__title">\{\{ title \}\}<\/strong>[\s\S]*?<\/slot>\s*<\/slot>\s*<\/div>/,
  )

  const pageHeader = secondsSource.match(/<PageHeader[\s\S]*?<\/PageHeader>/)?.[0] || ''
  assert.match(pageHeader, /<template #center>[\s\S]*?<button[\s\S]*?class="seconds-pair-field"[\s\S]*?aria-haspopup="dialog"[\s\S]*?<strong>\{\{ selectedPairLabel \}\}<\/strong>[\s\S]*?<ChevronDown[\s\S]*?<\/template>/)
  assert.match(pageHeader, /<template #actions>[\s\S]*?t\('seconds\.historyTitle'\)[\s\S]*?@click="openHistory"/)
  assert.match(secondsSource, /function openHistory\(\): void \{\s*pairPickerOpen\.value = false\s*clearSettlementResultQueue\(\)\s*void router\.push\(\{ name: 'seconds-history' \}\)\s*\}/)
  assert.equal((pageHeader.match(/class="seconds-pair-field"/g) || []).length, 1)
  assert.equal((secondsSource.match(/class="seconds-pair-field"/g) || []).length, 1)
  assert.doesNotMatch(secondsSource.slice(secondsSource.indexOf('</PageHeader>') + 13), /class="seconds-pair-field"/)

  const pairRule = cssRule(secondsStyle, '.seconds-pair-field {')
  assert.match(pairRule, /height: 44px;/)
  assert.match(pairRule, /margin: -11px 0;/)
  assert.match(pairRule, /min-width: 0;/)
  assert.match(pairRule, /padding: 11px 0;/)
  assert.match(pairRule, /position: relative;/)
  assert.match(pairRule, /width: 140px;/)

  assert.doesNotMatch(pageHeader, /<select\b|<option\b/)
  const focusRule = cssRule(secondsStyle, '.seconds-pair-field:focus-visible {')
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
  assert.match(secondsSource, /const latestPrice = computed\(\(\) => exactPriceForSymbol\(selected\.value\?\.symbol \|\| ''\)\)/)
  assert.match(secondsFinancialSource, /const exactPriceForSymbol = \(symbol: string\): DecimalText \| null => \{[\s\S]*?liveTickerFor\(symbol\)\?\.lastPriceText[\s\S]*?marketTickerFor\(symbol\)\?\.lastPriceText/)
  assert.match(secondsFinancialSource, /const priceFor = \(symbol: string\): DecimalText \| number \| null => \{[\s\S]*?selectedCandleClose\(\)[\s\S]*?positiveLegacyDisplayNumber/)
  assert.match(secondsSource, /const selectedChangePercent = computed\(\(\) => displayChangePercent\([\s\S]*?selectedLiveTicker\.value,[\s\S]*?selectedTicker\.value/)
  assert.match(secondsFinancialSource, /const displayChangePercent = \([\s\S]*?finiteDisplayNumber\(liveTicker\?\.changePercent\)[\s\S]*?finiteDisplayNumber\(snapshotTicker\?\.changePercent\)/)

  assert.match(secondsSource, /onBeforeUnmount\(\(\) => \{[\s\S]*?chartRequestVersion \+= 1[\s\S]*?tickerSubscriptionGeneration \+= 1[\s\S]*?secondsKlineSession\.stop\(\)[\s\S]*?stopTickerSubscription\?\.\(\)/)
  assert.doesNotMatch(secondsSource, /https?:\/\/www\.tradingview|<iframe|<script[^>]+src=/i)
})

test('Seconds 确认层冻结提交快照并让同一次重试复用幂等键', () => {
  assert.match(secondsFinancialSource, /interface SecondsOrderReviewSnapshot \{[\s\S]*?readonly productId: number[\s\S]*?readonly referencePrice: DecimalText \| null[\s\S]*?readonly idempotencyKey: string/)
  assert.match(
    secondsSource,
    /const review = createReview\(\{[\s\S]*?productId: product\.id,[\s\S]*?durationSeconds: activeCycle\.durationSeconds,[\s\S]*?direction: direction\.value,[\s\S]*?stakeAmount: amount\.value,[\s\S]*?referencePrice: latestPrice\.value,[\s\S]*?idempotencyKey: createSecondsOrderIdempotencyKey\(\)[\s\S]*?orderReview\.value = review/,
  )
  assert.match(secondsSource, /function isOrderReviewValid\(review: Readonly<OrderReview>\): boolean \{[\s\S]*?validateStake\(review\.stakeAmount,[\s\S]*?currentStakeValidation\.isValid/)

  const submitSource = secondsSource.match(
    /async function submit\(\): Promise<void> \{[\s\S]*?(?=\nasync function reconcileOpenedOrder)/,
  )?.[0] || ''
  assert.match(submitSource, /const review = orderReview\.value[\s\S]*?!review \|\| !isOrderReviewValid\(review\)/)
  assert.match(submitSource, /productId: review\.productId,[\s\S]*?durationSeconds: review\.durationSeconds,[\s\S]*?direction: review\.direction,[\s\S]*?stakeAmount: review\.stakeAmountText,[\s\S]*?idempotencyKey: review\.idempotencyKey,/)
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
  assert.match(secondsSource, /displayProductSymbol\(order\.symbol\)[\s\S]*?orderCountdown\(order\)[\s\S]*?orderMoney\(order\)\.stakeAmount[\s\S]*?orderMoney\(order\)\.entryPrice[\s\S]*?orderProfit\(order\)[\s\S]*?orderProgress\(order\)/)

  assert.equal((secondsSource.match(/:disabled="loading \|\| !selected"/g) || []).length, 4)
  assert.match(secondsSource, /:disabled="submitting \|\| loading \|\| !selected"/)
  assert.match(secondsSource, /async function submit\(\): Promise<void> \{\s*if \(submitting\.value\) return/)

  const mutationStart = secondsSource.indexOf('openedOrder = await openSecondsOrder({')
  const immediateUpsert = secondsSource.indexOf('orders.value = upsertSecondsOrder(orders.value, openedOrder)', mutationStart)
  const amountReset = secondsSource.indexOf("amount.value = ''", immediateUpsert)
  const confirmationClosed = secondsSource.indexOf('confirmOpen.value = false', amountReset)
  const submittingReleased = secondsSource.indexOf('submitting.value = false', confirmationClosed)
  const reconciliation = secondsSource.indexOf(
    'void reconcileOpenedOrder(mutationSessionGeneration)',
    submittingReleased,
  )
  assert.ok(
    mutationStart >= 0
    && immediateUpsert > mutationStart
    && amountReset > immediateUpsert
    && confirmationClosed > amountReset
    && submittingReleased > confirmationClosed
    && reconciliation > submittingReleased,
  )
  assert.doesNotMatch(secondsSource, /seconds-message--success|data-session-feedback="created"|success\.value/)
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
  assert.match(secondsSource, /fullyLoaded[\s\S]*?&& !activeIds\.has\(orderId\)[\s\S]*?&& !resultTracker\.isTracking\(orderId\)[\s\S]*?expiryRetryAtByOrderId\.delete\(orderId\)/)
  assert.match(secondsSource, /queueExpiredOrderReconciliation\(currentTime\.value\)/)

  assert.equal(zhCN.seconds.activeOrders, '活动订单')
  assert.equal(en.seconds.activeOrders, 'Active orders')
  assert.match(zhCN.seconds.refreshAfterOrderFailed, /订单已创建/)
  assert.match(en.seconds.refreshAfterOrderFailed, /order was created/i)
})

test('Seconds 使用权威结果追踪器、FIFO 队列和 Pencil 模态结算弹窗', () => {
  assert.match(secondsSource, /const resultTracker = createSecondsSettlementResultTracker\(\)/)
  assert.match(secondsSource, /const resultQueue = ref<SecondsOrder\[]>\(\[\]\)/)
  assert.match(
    secondsSource,
    /function applyReconciledOrders\(nextOrders: readonly SecondsOrder\[]\): void \{[\s\S]*?resultTracker\.reconcile\(nextOrders\)[\s\S]*?enqueueSecondsSettlementResults\([\s\S]*?resultQueue\.value,[\s\S]*?settledResults,[\s\S]*?mergeSecondsOrderReconciliation\(nextOrders, committedOrders\)/,
  )
  assert.match(
    secondsSource,
    /openedOrder = await openSecondsOrder\([\s\S]*?resultTracker\.track\(openedOrder\)[\s\S]*?committedOrdersById\.set\(openedOrder\.id, openedOrder\)/,
  )
  assert.match(
    secondsSource,
    /const settlementPnl = computed\([\s\S]*?profitLoss\(settled\.value\)/,
  )
  assert.match(
    secondsSource,
    /const settlementAmount = computed\([\s\S]*?presentation\.kind === 'profit' \? '\+' : ''[\s\S]*?moneyText\(presentation\.amount\)[\s\S]*?order\.stakeAssetSymbol/,
  )
  assert.match(
    secondsSource,
    /const settlementRate = computed\([\s\S]*?returnPercent\(presentation\.amount, financials\.stakeAmount\)[\s\S]*?percentText\(rate, currentIntlLocale\(\), 2\)/,
  )

  assert.match(
    secondsSource,
    /function clearSecondsPrivateState\(\): void \{[\s\S]*?orders\.value = \[\][\s\S]*?resultTracker\.reset\(\)[\s\S]*?clearSettlementResultQueue\(\)/,
  )
  assert.match(
    secondsSource,
    /watch\(\(\) => session\.isAuthenticated,[\s\S]*?if \(authenticated\) return[\s\S]*?clearSecondsPrivateState\(\)/,
  )
  assert.match(
    secondsSource,
    /onBeforeUnmount\(\(\) => \{[\s\S]*?resultTracker\.reset\(\)[\s\S]*?clearSettlementResultQueue\(\)/,
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
  assert.match(settlementTemplate, /v-if="settlementDialogOpen && settled"[\s\S]*?class="seconds-settlement-layer"/)
  assert.doesNotMatch(settlementTemplate, /:key="settled\.id"/)
  assert.match(settlementTemplate, /data-pencil-source="tFcTH FBdqS"/)
  assert.match(settlementTemplate, /@click\.self="advanceSettlementResult"/)
  assert.doesNotMatch(settlementTemplate, /data-app-theme|settlementTheme/)
  assert.match(settlementTemplate, /class="sr-only" role="status" aria-live="polite" aria-atomic="true"[\s\S]*?currentSettlementAnnouncement/)
  assert.match(settlementTemplate, /ref="settlementDialog"[\s\S]*?class="seconds-settlement-card"/)
  assert.match(settlementTemplate, /data-settlement-source="orders-api"/)
  assert.match(settlementTemplate, /role="dialog"[\s\S]*?aria-modal="true"/)
  assert.match(settlementTemplate, /@keydown="handleSettlementDialogKeydown"/)
  assert.match(settlementTemplate, /CircleCheckBig[\s\S]*?seconds\.statusSettled[\s\S]*?data-settlement-initial[\s\S]*?<X/)
  assert.match(settlementTemplate, /BadgeDollarSign[\s\S]*?settlementTitle[\s\S]*?settlementAmount[\s\S]*?settlementRate/)
  assert.match(settlementTemplate, /moneyText\(orderMoney\(settled\)\.entryPrice\)/)
  assert.match(settlementTemplate, /moneyText\(orderMoney\(settled\)\.settlementPrice\)/)
  assert.match(settlementTemplate, /displayProductSymbol\(settled\.symbol\)[\s\S]*?settled\.direction[\s\S]*?settled\.durationSeconds/)
  assert.match(settlementTemplate, /settlementResultsRemaining[\s\S]*?remainingResults/)
  assert.match(settlementTemplate, /@click="openHistory"[\s\S]*?seconds\.viewHistory/)
  assert.doesNotMatch(settlementTemplate, /seconds\.continueTrading|latestPrice|Trophy|TrendingDown/)
  assert.match(
    secondsSource,
    /useModalDialog\([\s\S]*?settlementDialogOpen,[\s\S]*?settlementDialog,[\s\S]*?'\[data-settlement-initial\]'[\s\S]*?\)/,
  )

  const layerRule = cssRule(secondsStyle, '.seconds-settlement-layer {')
  assert.match(layerRule, /background: var\(--seconds-result-backdrop\);/)
  assert.match(layerRule, /inset: 0;/)
  assert.match(layerRule, /overflow-y: auto;/)
  assert.match(layerRule, /pointer-events: auto;/)
  assert.doesNotMatch(secondsSource, /#[0-9a-f]{3,8}|rgba?\(/i)
  assert.doesNotMatch(secondsSource, /settlementTheme|syncSecondsTheme|data-app-theme/)
  assert.match(secondsSource, /chartThemeObserver = new MutationObserver\(drawSparkline\)/)
  const cardRule = cssRule(secondsStyle, '.seconds-settlement-card {')
  assert.match(cardRule, /border-radius: 24px;/)
  assert.match(cardRule, /gap: 14px;/)
  assert.match(cardRule, /max-width: 358px;/)
  assert.match(cardRule, /min-height: 541px;/)
  assert.match(cardRule, /padding: 20px 20px 18px;/)
  assert.match(cardRule, /transform: translateY\(-13\.5px\);/)
  assert.match(secondsStyle, /\.seconds-settlement-card__result\s*\{[\s\S]*?height: 176px;/)
  assert.match(secondsStyle, /\.seconds-settlement-card__prices\s*\{[\s\S]*?height: 68px;/)
  assert.match(secondsStyle, /\.seconds-settlement-card__summary\s*\{[\s\S]*?height: 64px;/)
  assert.match(secondsStyle, /\.seconds-settlement-card__note\s*\{[\s\S]*?min-height: 39px;/)
  assert.match(secondsStyle, /\.seconds-settlement-card__history\s*\{[\s\S]*?height: 52px;/)
  assert.match(secondsStyle, /\.seconds-settlement-card__close\s*\{[\s\S]*?height: 44px;[\s\S]*?margin: -5px;/)
  assert.match(secondsStyle, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.seconds-result-reveal-enter-active,[\s\S]*?transition: none !important;/)

  const lightThemeRule = cssRule(selectedPageCss, ".seconds-settlement-layer[data-pencil-source='tFcTH FBdqS'] {")
  assert.match(lightThemeRule, /--seconds-result-backdrop: #00000099;/)
  assert.match(lightThemeRule, /--seconds-result-card: #ffffff;/)
  assert.match(lightThemeRule, /--seconds-result-card-border: #dde7e1;/)
  assert.match(lightThemeRule, /--seconds-result-positive: #079863;/)
  const darkThemeRule = cssRule(selectedPageCss, "html[data-theme='dark'] .seconds-settlement-layer[data-pencil-source='tFcTH FBdqS'] {")
  assert.match(darkThemeRule, /--seconds-result-backdrop: #000000b8;/)
  assert.match(darkThemeRule, /--seconds-result-card: #101713;/)
  assert.match(darkThemeRule, /--seconds-result-card-border: #2c3a32;/)
  assert.match(darkThemeRule, /--seconds-result-positive: #56f0b2;/)

  assert.equal(zhCN.seconds.settlementProfit, '本单结算盈利')
  assert.equal(zhCN.seconds.settlementLoss, '本单结算亏损')
  assert.equal(zhCN.seconds.settlementEntryPrice, '买入价格')
  assert.equal(zhCN.seconds.settlementDirection, '方向')
  assert.equal(zhCN.seconds.settlementCycle, '周期')
  assert.match(zhCN.seconds.settlementReturnRate, /\{rate\}/)
  assert.match(zhCN.seconds.settlementAutoSummary, /\{amount\}[\s\S]*\{asset\}/)
  assert.equal(zhCN.seconds.viewHistory, '查看历史订单')
  assert.match(zhCN.seconds.settlementAnnouncement, /\{title\}[\s\S]*\{amount\}[\s\S]*\{duration\}/)
  assert.equal(en.seconds.settlementProfit, 'This order settled in profit')
  assert.equal(en.seconds.settlementLoss, 'This order settled at a loss')
  assert.equal(en.seconds.settlementEntryPrice, 'Entry price')
  assert.equal(en.seconds.settlementDirection, 'Direction')
  assert.equal(en.seconds.settlementCycle, 'Cycle')
  assert.match(en.seconds.settlementReturnRate, /\{rate\}/)
  assert.match(en.seconds.settlementAutoSummary, /\{amount\}[\s\S]*\{asset\}/)
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
