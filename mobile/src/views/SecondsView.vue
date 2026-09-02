<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ArrowRight,
  BadgeDollarSign,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleCheckBig,
  CircleAlert,
  History,
  Info,
  LoaderCircle,
  RefreshCw,
  Search,
  X,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchKlines } from '@/api/market'
import { createMarketDetailStreamSession } from '@/api/marketDetailStream'
import { subscribeTickers, type TickerUpdate } from '@/api/marketSocket'
import { normalizeMarketSocketSymbol } from '@/api/marketSocketProtocol'
import {
  createSecondsOrderIdempotencyKey,
  fetchSecondsOrders,
  fetchSecondsProducts,
  openSecondsOrder,
  type SecondsCycle,
  type SecondsOrder,
  type SecondsProduct,
} from '@/api/seconds'
import { fetchWalletAccounts } from '@/api/wallet'
import { publicMarketWebSocketUrl } from '@/config/app'
import { formatPercent } from '@/core/format'
import { useModalDialog } from '@/core/modalDialog'
import {
  createBottomNavSecondsFallbackTarget,
  isBottomNavigationSecondsEntry,
} from '@/core/navigation'
import {
  isRovingListboxSelectionKey,
  moveRovingOptionId,
  stableRovingOptionId,
  type RovingListboxNavigationKey,
} from '@/core/rovingListbox'
import {
  activeSecondsOrders,
  createSecondsSettlementResultTracker,
  enqueueSecondsSettlementResults,
  mergeSecondsOrderReconciliation,
  secondsOrderProgress,
  secondsOrderRemainingMs,
  secondsOrderStatusPresentation,
  upsertSecondsOrder,
} from '@/core/secondsOrder'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'
import type { KlinePoint, WalletAccount } from '@/core/types'
import {
  createSecondsFinancialPresentation as bindFinancial,
  createSecondsOrderReviewSnapshot as createReview,
  deriveSecondsReturnRatePercent as returnPercent,
  formatSecondsPercent as percentText,
  positiveSecondsBoundary as positive,
  validateSecondsStake as validateStake,
  type SecondsFinancialOrderValues as OrderMoney,
  type SecondsOrderReviewSnapshot as OrderReview,
} from '@/core/secondsFinancial'
import { currentIntlLocale } from '@/i18n'
const session = useSessionStore()
const marketStore = useMarketStore()
const router = useRouter()
const { t } = useI18n()
const products = ref<SecondsProduct[]>([])
const orders = ref<SecondsOrder[]>([])
const accounts = ref<WalletAccount[]>([])
const sparklinePoints = ref<KlinePoint[]>([])
const liveTickerSnapshots = ref<Record<string, TickerUpdate>>({})
const selected = ref<SecondsProduct | null>(null)
const selectedCycleId = ref(0)
const direction = ref<'up' | 'down'>('up')
const activeOrderFilter = ref<'all' | 'up' | 'down'>('all')
const amount = ref('')
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const refreshWarning = ref('')
const confirmOpen = ref(false)
const pairPickerOpen = ref(false)
const pairSearch = ref('')
const pairPickerDialog = ref<HTMLElement | null>(null)
const pairPickerTrigger = ref<HTMLButtonElement | null>(null)
const activePairProductId = ref<number | null>(null)
const orderReview = ref<OrderReview | null>(null)
const confirmDialog = ref<HTMLElement | null>(null)
const reviewButton = ref<HTMLButtonElement | null>(null)
const resultQueue = ref<SecondsOrder[]>([])
const settlementDialogOpen = ref(false)
const settlementDialog = ref<HTMLElement | null>(null)
const sparklineCanvas = ref<HTMLCanvasElement | null>(null)
const currentTime = ref(Date.now())
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''
let clockTimer: ReturnType<typeof setInterval> | null = null
let chartResizeObserver: ResizeObserver | null = null
let chartThemeObserver: MutationObserver | null = null
let chartRequestVersion = 0
let loadRequestVersion = 0
let tickerSubscriptionGeneration = 0
let tickerSubscriptionKey = ''
let stopTickerSubscription: (() => void) | null = null
let expiryReconciliationPromise: Promise<void> | null = null
let privateReconciliationGeneration = 0
let privateSessionGeneration = 0
let componentActive = true
const committedOrdersById = new Map<number, SecondsOrder>()
const exactMoney = new Map<number, Pick<OrderMoney, 'stakeAmount' | 'payoutRate'>>()
const {
  baseSymbol,
  countdownLabel,
  cycleHasMaximum,
  cycleMaximum: cycleMax,
  cycleMinimum: cycleMin,
  displayChangePercent,
  displayProductSymbol,
  estimatedProfit: orderProfit,
  exactCyclePayoutRate,
  exactPriceForSymbol,
  formatCycleLimit,
  formatOrderAction,
  formatPayoutRate: payoutText,
  formatValue: moneyText,
  hasExactStakeRange,
  matchesProductSearch,
  normalizeProductSymbol,
  orderFinancials: orderMoney,
  priceFor,
  profitLoss,
  walletAvailable,
} = bindFinancial({
  locale: currentIntlLocale,
  exactByOrderId: exactMoney,
  normalizeSymbol: normalizeMarketSocketSymbol,
  liveTickerFor: (symbol) => liveTickerSnapshots.value[normalizeMarketSocketSymbol(symbol)],
  marketTickerFor: (symbol) => marketStore.tickerFor(symbol),
  selectedSymbol: () => selected.value?.symbol || '',
  selectedCandleClose: () => sparklinePoints.value.at(-1)?.close,
  translate: (key, params) => params ? t(key, params) : t(key),
})
const expiryRetryAtByOrderId = new Map<number, number>()
const queuedExpiryOrderIds = new Set<number>()
const reconcilingExpiryOrderIds = new Set<number>()
const resultTracker = createSecondsSettlementResultTracker()
const EXPIRY_RECONCILIATION_RETRY_MS = 5_000
const cycle = computed<SecondsCycle | undefined>(() => (
  selected.value?.cycles.find((item) => item.id === selectedCycleId.value)
  || selected.value?.cycles[0]
))
const account = computed(() => accounts.value.find((item) => item.assetId === selected.value?.stakeAssetId))
const selectedTicker = computed(() => marketStore.tickerFor(selected.value?.symbol || ''))
const activeOrders = computed(() => activeSecondsOrders(orders.value))
const selectedActiveOrders = computed(() => {
  const symbol = normalizeProductSymbol(selected.value?.symbol || '')
  return activeOrders.value.filter((order) => normalizeProductSymbol(order.symbol) === symbol)
})
const filteredActiveOrders = computed(() => (
  activeOrderFilter.value === 'all'
    ? activeOrders.value
    : activeOrders.value.filter((order) => order.direction === activeOrderFilter.value)
))
const activeOrderCounts = computed(() => ({
  all: activeOrders.value.length,
  up: activeOrders.value.filter((order) => order.direction === 'up').length,
  down: activeOrders.value.filter((order) => order.direction === 'down').length,
}))
const nearestSelectedActiveOrder = computed(() => (
  [...selectedActiveOrders.value].sort((left, right) => {
    if (left.expiresAt !== right.expiresAt) return left.expiresAt - right.expiresAt
    return left.id - right.id
  })[0] || null
))
const selectedPairLabel = computed(() => (
  selected.value ? displayProductSymbol(selected.value.symbol) : t('seconds.title')
))
const filteredPairProducts = computed(() => products.value.filter((product) => (
  matchesProductSearch(product.symbol, pairSearch.value)
)))
const filteredPairProductIds = computed(() => filteredPairProducts.value.map((product) => product.id))
const selectedLiveTicker = computed(() => (
  liveTickerSnapshots.value[normalizeProductSymbol(selected.value?.symbol || '')]
))
const selectedChangePercent = computed(() => displayChangePercent(
  selectedLiveTicker.value,
  selectedTicker.value,
))
const roundStatusLabel = computed(() => {
  const order = nearestSelectedActiveOrder.value
  if (order) {
    return t('seconds.activeRoundStatus', {
      id: order.id,
      countdown: orderCountdown(order),
    })
  }
  return t('seconds.readyState')
})
const settled = computed(() => resultQueue.value[0] || null)
const settlementPnl = computed(() => (
  settled.value
    ? profitLoss(settled.value)
    : null
))
const settlementTone = computed<'positive' | 'negative'>(() => (
  settlementPnl.value?.kind === 'profit'
    ? 'positive'
    : 'negative'
))
const settlementTitle = computed(() => t(
  settlementTone.value === 'positive'
    ? 'seconds.settlementProfit'
    : 'seconds.settlementLoss',
))
const settlementAmount = computed(() => {
  const order = settled.value
  const presentation = settlementPnl.value
  if (!order || !presentation || presentation.amount === null) return '--'
  const sign = presentation.kind === 'profit' ? '+' : ''
  return `${sign}${moneyText(presentation.amount)} ${order.stakeAssetSymbol}`
})
const settlementRate = computed(() => {
  const order = settled.value
  const presentation = settlementPnl.value
  if (!order || !presentation || presentation.amount === null) return '--'
  const financials = orderMoney(order)
  if (!financials.stakeAmount) return '--'
  const rate = returnPercent(presentation.amount, financials.stakeAmount)
  if (!rate) return '--'
  const sign = presentation.kind === 'profit' ? '+' : ''
  return `${sign}${percentText(rate, currentIntlLocale(), 2)}%`
})
const currentSettlementAnnouncement = computed(() => {
  const order = settled.value
  if (!order) return ''
  return t('seconds.settlementAnnouncement', {
    title: settlementTitle.value,
    amount: settlementAmount.value,
    symbol: order.symbol,
    direction: t(order.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish'),
    duration: t('seconds.duration', { seconds: order.durationSeconds }),
  })
})
const remainingResults = computed(() => Math.max(0, resultQueue.value.length - 1))
const { trapFocus: trapSettlementDialogFocus } = useModalDialog(
  settlementDialogOpen,
  settlementDialog,
  '[data-settlement-initial]',
)
const {
  trapFocus: trapPairPickerFocus,
  setReturnFocus: setPairPickerReturnFocus,
} = useModalDialog(
  pairPickerOpen,
  pairPickerDialog,
  '[data-seconds-pair-search]',
)
const exactPayoutRate = computed(() => exactCyclePayoutRate(cycle.value))
// The legacy numeric rate is retained for compatibility presentation only.
const payoutRateDisplay = computed(() => exactPayoutRate.value ?? cycle.value?.payoutRate ?? null)
const cycleMinimum = computed(() => cycleMin(cycle.value))
const cycleMaximum = computed(() => cycleMax(cycle.value))
const hasExactCycleRange = computed(() => hasExactStakeRange(cycle.value))
const availableStakeBalance = computed(() => walletAvailable(account.value))
const reviewProfit = computed(() => (
  orderReview.value?.estimatedProfit ?? null
))
const stakeValidation = computed(() => validateStake(amount.value, {
  minimum: cycleMinimum.value,
  maximum: cycleHasMaximum(cycle.value) ? cycleMaximum.value : undefined,
  available: availableStakeBalance.value,
}))
const valid = computed(() => Boolean(
  cycle.value
  && exactPayoutRate.value
  && hasExactCycleRange.value
  && stakeValidation.value.isValid,
))
const amountFieldInvalid = computed(() => Boolean(
  session.isAuthenticated
  && !loading.value
  && amount.value
  && !valid.value,
))
const cycleLimitLabel = computed(() => formatCycleLimit(
  cycle.value,
  selected.value?.stakeAssetSymbol || '--',
))
const orderActionLabel = computed(() => formatOrderAction(
  direction.value,
  stakeValidation.value.stakeAmount,
  selected.value?.stakeAssetSymbol || '--',
))
const homeFallback = createBottomNavSecondsFallbackTarget()
const preferHomeFallback = computed(() => {
  void router.currentRoute.value.fullPath
  return isBottomNavigationSecondsEntry(router.options.history.state)
})

const latestPrice = computed(() => exactPriceForSymbol(selected.value?.symbol || ''))
const latestDisplayPrice = computed(() => priceFor(selected.value?.symbol || ''))

function orderCountdown(order: SecondsOrder): string {
  return countdownLabel(secondsOrderRemainingMs(order, currentTime.value))
}

function orderProgress(order: SecondsOrder): number {
  return secondsOrderProgress(order, currentTime.value)
}

const secondsKlineSession = createMarketDetailStreamSession({
  getUrl: publicMarketWebSocketUrl,
  channels: ['kline'],
  onDepth: () => undefined,
  onTrade: () => undefined,
  onKlines: (_context, nextPoints) => {
    sparklinePoints.value = nextPoints.slice(-48)
  },
})

function replaceTickerSubscription(): void {
  const normalizedSymbols = [...new Set([
    ...products.value.map((product) => normalizeProductSymbol(product.symbol)),
    ...activeOrders.value.map((order) => normalizeProductSymbol(order.symbol)),
  ].filter(Boolean))].sort()
  const subscriptionKey = normalizedSymbols.join(',')
  if (subscriptionKey === tickerSubscriptionKey && stopTickerSubscription) return

  const generation = ++tickerSubscriptionGeneration
  stopTickerSubscription?.()
  stopTickerSubscription = null
  tickerSubscriptionKey = subscriptionKey
  const acceptedSymbols = new Set(normalizedSymbols)
  liveTickerSnapshots.value = Object.fromEntries(
    Object.entries(liveTickerSnapshots.value).filter(([symbol]) => acceptedSymbols.has(symbol)),
  )
  if (!normalizedSymbols.length) return

  stopTickerSubscription = subscribeTickers(normalizedSymbols, (update) => {
    if (
      !componentActive
      || generation !== tickerSubscriptionGeneration
      || !acceptedSymbols.has(update.symbol)
      || !positive(update.lastPriceText)
    ) {
      return
    }
    const previous = liveTickerSnapshots.value[update.symbol]
    if (
      previous?.observedAt !== undefined
      && update.observedAt !== undefined
      && update.observedAt < previous.observedAt
    ) {
      return
    }
    liveTickerSnapshots.value = {
      ...liveTickerSnapshots.value,
      [update.symbol]: {
        ...previous,
        ...update,
        ...(update.changePercent === undefined && previous?.changePercent !== undefined
          ? { changePercent: previous.changePercent }
          : {}),
        ...(update.observedAt === undefined && previous?.observedAt !== undefined
          ? { observedAt: previous.observedAt }
          : {}),
      },
    }
  })
}

async function loadSparkline(symbol: string): Promise<void> {
  const requestVersion = ++chartRequestVersion
  if (!symbol) {
    secondsKlineSession.stop()
    sparklinePoints.value = []
    return
  }
  const context = secondsKlineSession.replace(symbol, '1m', requestVersion)
  const request = secondsKlineSession.beginKlineRequest(context)
  sparklinePoints.value = secondsKlineSession.currentPoints().slice(-48)
  if (!request) return
  try {
    const nextPoints = await fetchKlines(symbol, '1m')
    if (
      requestVersion !== chartRequestVersion
      || !secondsKlineSession.isCurrent(context, symbol, '1m', requestVersion)
      || !secondsKlineSession.isCurrentKlineRequest(request)
    ) {
      return
    }
    const mergedPoints = secondsKlineSession.resolveKlineRequest(request, nextPoints)
    if (mergedPoints) sparklinePoints.value = mergedPoints.slice(-48)
  } catch {
    if (
      requestVersion === chartRequestVersion
      && secondsKlineSession.isCurrent(context, symbol, '1m', requestVersion)
      && secondsKlineSession.isCurrentKlineRequest(request)
      && !context.klineReceived
    ) {
      sparklinePoints.value = []
    }
  }
}

function drawSparkline(): void {
  const canvas = sparklineCanvas.value
  if (!canvas) return
  const width = Math.max(1, canvas.clientWidth)
  const height = Math.max(1, canvas.clientHeight)
  const dpr = Math.min(window.devicePixelRatio || 1, 2)
  canvas.width = Math.round(width * dpr)
  canvas.height = Math.round(height * dpr)
  const context = canvas.getContext('2d')
  if (!context) return
  context.setTransform(dpr, 0, 0, dpr, 0, 0)
  context.clearRect(0, 0, width, height)

  const styles = getComputedStyle(canvas)
  const lineColor = styles.getPropertyValue('--seconds-line').trim()
  const positiveColor = styles.getPropertyValue('--seconds-signal').trim()
  context.strokeStyle = lineColor
  context.lineWidth = 1
  for (let index = 0; index < 4; index += 1) {
    const y = 0.5 + ((height - 1) * index) / 3
    context.beginPath()
    context.moveTo(0, y)
    context.lineTo(width, y)
    context.stroke()
  }

  const closes = sparklinePoints.value.map((point) => point.close).filter((value) => Number.isFinite(value) && value > 0)
  if (closes.length < 2) return
  const minimum = Math.min(...closes)
  const maximum = Math.max(...closes)
  const range = maximum - minimum || 1
  const horizontalPadding = 2
  const verticalPadding = 10
  context.strokeStyle = positiveColor
  context.lineWidth = 2
  context.lineJoin = 'round'
  context.lineCap = 'round'
  context.beginPath()
  closes.forEach((value, index) => {
    const x = horizontalPadding + ((width - horizontalPadding * 2) * index) / (closes.length - 1)
    const y = verticalPadding + ((maximum - value) / range) * (height - verticalPadding * 2)
    if (index === 0) context.moveTo(x, y)
    else context.lineTo(x, y)
  })
  context.stroke()

  const last = closes.at(-1) || 0
  const lastY = verticalPadding + ((maximum - last) / range) * (height - verticalPadding * 2)
  const glow = context.createRadialGradient(
    width - horizontalPadding,
    lastY,
    0,
    width - horizontalPadding,
    lastY,
    12,
  )
  glow.addColorStop(0, positiveColor)
  glow.addColorStop(1, 'transparent')
  context.globalAlpha = 0.38
  context.fillStyle = glow
  context.beginPath()
  context.arc(width - horizontalPadding, lastY, 12, 0, Math.PI * 2)
  context.fill()
  context.globalAlpha = 1
  context.fillStyle = positiveColor
  context.beginPath()
  context.arc(width - horizontalPadding, lastY, 4.5, 0, Math.PI * 2)
  context.fill()
}

function initializeSparkline(): void {
  const canvas = sparklineCanvas.value
  if (!canvas) return
  chartResizeObserver = new ResizeObserver(drawSparkline)
  chartResizeObserver.observe(canvas)
  chartThemeObserver = new MutationObserver(drawSparkline)
  chartThemeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
  const stage = canvas.closest('.app-stage')
  if (stage) chartThemeObserver.observe(stage, { attributes: true, attributeFilter: ['class'] })
  drawSparkline()
}

function advanceSettlementResult(): void {
  resultQueue.value = resultQueue.value.slice(1)
  if (!resultQueue.value.length) {
    settlementDialogOpen.value = false
    return
  }
  void nextTick(() => {
    settlementDialog.value?.querySelector<HTMLElement>('[data-settlement-initial]')?.focus()
  })
}

function clearSettlementResultQueue(): void {
  settlementDialogOpen.value = false
  resultQueue.value = []
}

function handleSettlementDialogKeydown(event: KeyboardEvent): void {
  trapSettlementDialogFocus(event, advanceSettlementResult)
}

function openHistory(): void {
  pairPickerOpen.value = false
  clearSettlementResultQueue()
  void router.push({ name: 'seconds-history' })
}

function openPairPicker(): void {
  if (confirmOpen.value || settlementDialogOpen.value) return
  pairSearch.value = ''
  activePairProductId.value = stableRovingOptionId(
    products.value.map((product) => product.id),
    activePairProductId.value,
    selected.value?.id ?? null,
  )
  setPairPickerReturnFocus(pairPickerTrigger.value)
  pairPickerOpen.value = true
}

function closePairPicker(): void {
  pairPickerOpen.value = false
}

function handlePairPickerKeydown(event: KeyboardEvent): void {
  const target = event.target instanceof HTMLElement ? event.target : null
  const option = target?.closest<HTMLElement>('[data-seconds-pair-option-id]') ?? null
  const isSearch = target?.matches('[data-seconds-pair-search]') ?? false
  const navigationKeys = new Set<string>(['ArrowDown', 'ArrowUp', 'Home', 'End'])

  if (navigationKeys.has(event.key) && !(isSearch && (event.key === 'Home' || event.key === 'End'))) {
    event.preventDefault()
    // Resolve the DOM identifier against an existing product instead of parsing arbitrary text.
    const focusedId = option
      ? filteredPairProducts.value.find((item) => (
        String(item.id) === option.dataset.secondsPairOptionId
      ))?.id ?? null
      : null
    const currentId = focusedId ?? activePairProductId.value
    const nextId = moveRovingOptionId(
      filteredPairProductIds.value,
      currentId,
      event.key as RovingListboxNavigationKey,
    )
    activePairProductId.value = nextId
    if (nextId !== null) {
      void nextTick(() => {
        pairPickerDialog.value
          ?.querySelector<HTMLElement>(`[data-seconds-pair-option-id="${nextId}"]`)
          ?.focus()
      })
    }
    return
  }

  if (option && isRovingListboxSelectionKey(event.key, event.code)) {
    event.preventDefault()
    const product = filteredPairProducts.value.find((item) => (
      String(item.id) === option.dataset.secondsPairOptionId
    ))
    if (product) choosePairProduct(product)
    return
  }
  trapPairPickerFocus(event, closePairPicker)
}

/** Clears order baselines, expiry retries, and notices at a private-session boundary. */
function clearSecondsPrivateState(): void {
  orders.value = []
  accounts.value = []
  committedOrdersById.clear()
  exactMoney.clear()
  expiryRetryAtByOrderId.clear()
  queuedExpiryOrderIds.clear()
  reconcilingExpiryOrderIds.clear()
  resultTracker.reset()
  clearSettlementResultQueue()
}

async function load(): Promise<void> {
  const requestVersion = ++loadRequestVersion
  const privateGeneration = ++privateReconciliationGeneration
  loading.value = true
  error.value = ''
  const privateStatePromise: Promise<[
    PromiseSettledResult<SecondsOrder[]>,
    PromiseSettledResult<WalletAccount[]>,
  ] | null> = session.isAuthenticated
    ? Promise.allSettled([fetchSecondsOrders(100), fetchWalletAccounts()])
    : Promise.resolve(null)
  try {
    const currentProductId = selected.value?.id
    const nextProducts = await fetchSecondsProducts()
    if (!componentActive || requestVersion !== loadRequestVersion) return
    products.value = nextProducts
    selected.value = nextProducts.find((product) => product.id === currentProductId) || nextProducts[0] || null
    if (selected.value) {
      const stillAvailable = selected.value.cycles.some((item) => item.id === selectedCycleId.value)
      if (!stillAvailable) selectedCycleId.value = selected.value.cycles[0]?.id || 0
      if (!amount.value) amount.value = cycleMin(cycle.value) ?? ''
      void loadSparkline(selected.value.symbol)
    } else {
      void loadSparkline('')
    }
    replaceTickerSubscription()

    const privateResults = await privateStatePromise
    if (
      !componentActive
      || requestVersion !== loadRequestVersion
      || privateGeneration !== privateReconciliationGeneration
    ) {
      return
    }
    if (!privateResults) {
      clearSecondsPrivateState()
      replaceTickerSubscription()
      return
    }
    const [ordersResult, accountsResult] = privateResults
    if (ordersResult.status === 'fulfilled') applyReconciledOrders(ordersResult.value)
    if (accountsResult.status === 'fulfilled') accounts.value = accountsResult.value
    replaceTickerSubscription()
    const failedResult = [ordersResult, accountsResult].find((result) => result.status === 'rejected')
    if (failedResult?.status === 'rejected') {
      error.value = apiErrorMessage(failedResult.reason, t('seconds.loadFailed'))
    }
  } catch (reason) {
    if (componentActive && requestVersion === loadRequestVersion) {
      error.value = apiErrorMessage(reason, t('seconds.loadFailed'))
    }
  } finally {
    if (componentActive && requestVersion === loadRequestVersion) loading.value = false
  }
}

interface PrivateReconciliationResult {
  ordersLoaded: boolean
  accountsLoaded: boolean
}

async function reconcilePrivateState(): Promise<PrivateReconciliationResult> {
  if (!session.isAuthenticated || !componentActive) {
    return { ordersLoaded: true, accountsLoaded: true }
  }
  const generation = ++privateReconciliationGeneration
  const [ordersResult, accountsResult] = await Promise.allSettled([
    fetchSecondsOrders(100),
    fetchWalletAccounts(),
  ])
  if (!componentActive) {
    return { ordersLoaded: false, accountsLoaded: false }
  }
  if (generation !== privateReconciliationGeneration) {
    // A newer reconciliation owns both resources; do not let the superseded
    // request surface a false refresh failure after the newer one succeeds.
    return { ordersLoaded: true, accountsLoaded: true }
  }

  if (ordersResult.status === 'fulfilled') applyReconciledOrders(ordersResult.value)
  if (accountsResult.status === 'fulfilled') accounts.value = accountsResult.value
  replaceTickerSubscription()
  return {
    ordersLoaded: ordersResult.status === 'fulfilled',
    accountsLoaded: accountsResult.status === 'fulfilled',
  }
}

function applyReconciledOrders(nextOrders: readonly SecondsOrder[]): void {
  // Consume the raw server snapshot before locally committed creates are merged.
  const settledResults = resultTracker.reconcile(nextOrders)
  if (settledResults.length) {
    resultQueue.value = enqueueSecondsSettlementResults(
      resultQueue.value,
      settledResults,
    )
  }
  const committedOrders = [...committedOrdersById.values()]
  orders.value = mergeSecondsOrderReconciliation(nextOrders, committedOrders)
  nextOrders.forEach((order) => committedOrdersById.delete(order.id))
}

function queueExpiredOrderReconciliation(now = Date.now()): void {
  if (!session.isAuthenticated || !componentActive) return
  for (const order of activeOrders.value) {
    if (secondsOrderRemainingMs(order, now) > 0) continue
    if (!expiryRetryAtByOrderId.has(order.id)) expiryRetryAtByOrderId.set(order.id, 0)
  }
  for (const [orderId, retryAt] of expiryRetryAtByOrderId) {
    if (retryAt <= now && !reconcilingExpiryOrderIds.has(orderId)) queuedExpiryOrderIds.add(orderId)
  }
  if (!queuedExpiryOrderIds.size || expiryReconciliationPromise) return

  expiryReconciliationPromise = reconcileExpiredOrders()
    .finally(() => {
      expiryReconciliationPromise = null
      if (queuedExpiryOrderIds.size) queueExpiredOrderReconciliation(Date.now())
    })
}

async function reconcileExpiredOrders(): Promise<void> {
  const batch = [...queuedExpiryOrderIds]
  queuedExpiryOrderIds.clear()
  batch.forEach((orderId) => reconcilingExpiryOrderIds.add(orderId))
  const reconciliation = await reconcilePrivateState()
  batch.forEach((orderId) => reconcilingExpiryOrderIds.delete(orderId))
  if (!componentActive) return

  const activeIds = new Set(activeOrders.value.map((order) => order.id))
  const fullyLoaded = reconciliation.ordersLoaded && reconciliation.accountsLoaded
  const retryAt = Date.now() + EXPIRY_RECONCILIATION_RETRY_MS
  for (const orderId of batch) {
    // Keep polling a non-active order while its authoritative result is pending.
    if (
      fullyLoaded
      && !activeIds.has(orderId)
      && !resultTracker.isTracking(orderId)
    ) {
      expiryRetryAtByOrderId.delete(orderId)
    }
    else expiryRetryAtByOrderId.set(orderId, retryAt)
  }
  refreshWarning.value = fullyLoaded ? '' : t('seconds.refreshAfterOrderFailed')
}

function selectProduct(product: SecondsProduct): void {
  selected.value = product
  selectedCycleId.value = product.cycles[0]?.id || 0
  direction.value = 'up'
  amount.value = cycleMin(product.cycles[0]) ?? ''
  error.value = ''
  void loadSparkline(product.symbol)
}

function choosePairProduct(product: SecondsProduct): void {
  selectProduct(product)
  closePairPicker()
}

function selectCycle(cycleId: number): void {
  selectedCycleId.value = cycleId
  amount.value = cycleMin(cycle.value) ?? ''
  error.value = ''
}

function setDirection(nextDirection: 'up' | 'down'): void {
  direction.value = nextDirection
  error.value = ''
}

function setAmount(value: string): void {
  amount.value = value
  error.value = ''
}

function reviewOrder(): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/seconds' } })
    return
  }
  const product = selected.value
  const activeCycle = cycle.value
  const activePayoutRate = exactPayoutRate.value
  if (!product || !activeCycle || !activePayoutRate || !valid.value) {
    error.value = t('seconds.invalidAmount')
    return
  }
  error.value = ''
  const review = createReview({
    productId: product.id,
    cycleId: activeCycle.id,
    symbol: product.symbol,
    stakeAssetId: product.stakeAssetId,
    stakeAssetSymbol: product.stakeAssetSymbol,
    durationSeconds: activeCycle.durationSeconds,
    direction: direction.value,
    stakeAmount: amount.value,
    minimumStake: cycleMinimum.value,
    maximumStake: cycleHasMaximum(activeCycle) ? cycleMaximum.value : undefined,
    available: availableStakeBalance.value,
    payoutRate: activePayoutRate,
    referencePrice: latestPrice.value,
    idempotencyKey: createSecondsOrderIdempotencyKey(),
  })
  if (!review) {
    error.value = t('seconds.invalidAmount')
    return
  }
  orderReview.value = review
  confirmOpen.value = true
}

function closeConfirm(): void {
  if (submitting.value) return
  confirmOpen.value = false
  orderReview.value = null
}

function isOrderReviewValid(review: Readonly<OrderReview>): boolean {
  const product = products.value.find((item) => item.id === review.productId)
  const currentCycle = product?.cycles.find((item) => item.id === review.cycleId)
  const currentAccount = accounts.value.find((item) => item.assetId === review.stakeAssetId)
  const currentPayoutRate = exactCyclePayoutRate(currentCycle)
  const currentStakeValidation = validateStake(review.stakeAmount, {
    minimum: cycleMin(currentCycle),
    maximum: cycleHasMaximum(currentCycle) ? cycleMax(currentCycle) : undefined,
    available: walletAvailable(currentAccount),
  })
  return Boolean(
    product
    && currentCycle
    && hasExactStakeRange(currentCycle)
    && currentPayoutRate === review.payoutRate
    && currentCycle.durationSeconds === review.durationSeconds
    && currentStakeValidation.isValid,
  )
}

function isCurrentSecondsMutationSession(generation: number): boolean {
  return (
    componentActive
    && session.isAuthenticated
    && generation === privateSessionGeneration
  )
}

async function submit(): Promise<void> {
  if (submitting.value) return
  if (!session.isAuthenticated) {
    error.value = t('seconds.loginDescription')
    return
  }
  const review = orderReview.value
  if (!review || !isOrderReviewValid(review)) {
    error.value = t('seconds.invalidAmount')
    return
  }
  submitting.value = true
  error.value = ''
  refreshWarning.value = ''
  const mutationSessionGeneration = privateSessionGeneration
  let openedOrder: SecondsOrder | null = null
  try {
    openedOrder = await openSecondsOrder({
      productId: review.productId,
      durationSeconds: review.durationSeconds,
      direction: review.direction,
      stakeAmount: review.stakeAmountText,
      idempotencyKey: review.idempotencyKey,
    })
    if (!isCurrentSecondsMutationSession(mutationSessionGeneration)) return
    // The create response is the earliest authoritative active-order observation.
    exactMoney.set(openedOrder.id, {
      stakeAmount: review.stakeAmount,
      payoutRate: review.payoutRate,
    })
    resultTracker.track(openedOrder)
    committedOrdersById.set(openedOrder.id, openedOrder)
    orders.value = upsertSecondsOrder(orders.value, openedOrder)
    amount.value = ''
    confirmOpen.value = false
    orderReview.value = null
    replaceTickerSubscription()
  } catch (reason) {
    if (isCurrentSecondsMutationSession(mutationSessionGeneration)) {
      error.value = apiErrorMessage(reason, t('seconds.orderFailed'))
    }
  } finally {
    if (componentActive) submitting.value = false
  }
  if (openedOrder && isCurrentSecondsMutationSession(mutationSessionGeneration)) {
    void reconcileOpenedOrder(mutationSessionGeneration)
  }
}

async function reconcileOpenedOrder(mutationSessionGeneration: number): Promise<void> {
  const reconciliation = await reconcilePrivateState()
  if (
    isCurrentSecondsMutationSession(mutationSessionGeneration)
    && (!reconciliation.ordersLoaded || !reconciliation.accountsLoaded)
  ) {
    refreshWarning.value = t('seconds.refreshAfterOrderFailed')
  }
}

function orderStatusLabel(order: SecondsOrder): string {
  const presentation = secondsOrderStatusPresentation(order)
  return presentation.translationKey ? t(presentation.translationKey) : presentation.source
}

function trapDialogFocus(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    closeConfirm()
    return
  }
  if (event.key !== 'Tab' || !confirmDialog.value) return
  const focusable = Array.from(confirmDialog.value.querySelectorAll<HTMLElement>(
    'button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])',
  ))
  if (!focusable.length) return
  const first = focusable[0]
  const last = focusable.at(-1) || first
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}

watch([filteredPairProductIds, () => selected.value?.id ?? null], ([optionIds, selectedId]) => {
  activePairProductId.value = stableRovingOptionId(
    optionIds,
    activePairProductId.value,
    selectedId,
  )
}, { flush: 'sync' })

watch(() => session.isAuthenticated, (authenticated) => {
  privateSessionGeneration += 1
  if (authenticated) return
  privateReconciliationGeneration += 1
  clearSecondsPrivateState()
  refreshWarning.value = ''
  pairPickerOpen.value = false
  confirmOpen.value = false
  orderReview.value = null
  replaceTickerSubscription()
}, { flush: 'sync' })

watch(confirmOpen, async (open) => {
  if (open) {
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : reviewButton.value
    previousBodyOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    await nextTick()
    confirmDialog.value?.querySelector<HTMLElement>('[data-dialog-cancel]')?.focus()
    return
  }
  document.body.style.overflow = previousBodyOverflow
  await nextTick()
  returnFocus?.focus()
  returnFocus = null
})

watch([settled, confirmOpen, pairPickerOpen], async ([result, confirmationOpen, pickerOpen]) => {
  if (!result || confirmationOpen || pickerOpen) {
    settlementDialogOpen.value = false
    return
  }
  if (settlementDialogOpen.value) return
  // Let the confirmation dialog restore scroll and focus before the settlement dialog acquires them.
  await nextTick()
  if (settled.value && !confirmOpen.value && !pairPickerOpen.value) {
    settlementDialogOpen.value = true
  }
}, { immediate: true })

watch(sparklinePoints, async () => {
  await nextTick()
  drawSparkline()
})

onMounted(async () => {
  await nextTick()
  initializeSparkline()
  clockTimer = setInterval(() => {
    currentTime.value = Date.now()
    queueExpiredOrderReconciliation(currentTime.value)
  }, 1000)
  void Promise.all([load(), marketStore.refresh()])
})

onBeforeUnmount(() => {
  componentActive = false
  loadRequestVersion += 1
  chartRequestVersion += 1
  tickerSubscriptionGeneration += 1
  privateReconciliationGeneration += 1
  privateSessionGeneration += 1
  secondsKlineSession.stop()
  stopTickerSubscription?.()
  stopTickerSubscription = null
  tickerSubscriptionKey = ''
  queuedExpiryOrderIds.clear()
  reconcilingExpiryOrderIds.clear()
  expiryRetryAtByOrderId.clear()
  committedOrdersById.clear()
  exactMoney.clear()
  resultTracker.reset()
  clearSettlementResultQueue()
  if (clockTimer) clearInterval(clockTimer)
  chartResizeObserver?.disconnect()
  chartThemeObserver?.disconnect()
  document.body.style.overflow = previousBodyOverflow
})
</script>

<template>
  <main
    class="page page--plain seconds-page"
    data-pencil-source="VL8er/g9agt"
    data-instrument-hero="pair-price"
  >
    <PageHeader
      class="seconds-header"
      :back="true"
      :eyebrow="t('seconds.scene')"
      :fallback="homeFallback"
      :pencil="true"
      :prefer-fallback="preferHomeFallback"
      :title="selected?.symbol || t('seconds.title')"
      :subtitle="t('seconds.context')"
    >
      <template #center>
        <button
          ref="pairPickerTrigger"
          class="seconds-pair-field"
          type="button"
          aria-haspopup="dialog"
          :aria-expanded="pairPickerOpen"
          aria-controls="seconds-pair-picker"
          :aria-label="t('seconds.pairPickerTitle')"
          :data-state="loading ? 'loading' : products.length ? 'ready' : 'empty'"
          @click="openPairPicker"
        >
          <span class="seconds-pair-copy" aria-hidden="true">
            <strong>{{ selectedPairLabel }}</strong>
            <small>{{ t('seconds.title') }}</small>
            <ChevronDown :size="15" :stroke-width="2" />
          </span>
        </button>
      </template>
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('seconds.historyTitle')" @click="openHistory">
          <History :size="18" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>

    <div class="page-content seconds-content">
      <section
        class="seconds-workspace"
        data-seconds-workspace="live"
        :data-seconds-state="activeOrders.length ? 'active' : 'default'"
        :class="{ 'seconds-guest': !session.isAuthenticated }"
      >
        <section
          class="seconds-trading-operation"
          :data-seconds-market="selected ? 'live' : loading ? 'loading' : 'empty'"
          :aria-busy="loading || marketStore.loading"
        >
          <div class="seconds-market-status">
            <span class="seconds-round-state">
              <i aria-hidden="true" />
              <b class="numeric">{{ selected ? roundStatusLabel : loading ? t('seconds.loading') : t('seconds.readyState') }}</b>
            </span>
            <span class="seconds-return-rate numeric">
              {{ cycle ? t('seconds.returnRate', { rate: payoutText(payoutRateDisplay, 0) }) : '--' }}
            </span>
          </div>

          <div class="seconds-price-panel">
            <strong class="numeric">{{ latestDisplayPrice ? moneyText(latestDisplayPrice) : '--' }}</strong>
            <div class="seconds-price-meta">
              <span class="numeric">
                {{ selectedChangePercent === null ? '--' : formatPercent(selectedChangePercent) }} ·
                {{ t('seconds.referencePrice') }}
                {{ latestDisplayPrice ? moneyText(latestDisplayPrice) : '--' }}
              </span>
            </div>
            <span
              class="seconds-live-state"
              :data-state="selectedLiveTicker ? 'live' : latestDisplayPrice ? 'snapshot' : 'unavailable'"
            >
              {{ selectedLiveTicker
                ? t('common.liveData')
                : latestDisplayPrice
                  ? t('seconds.marketSnapshot')
                  : t('common.marketUnavailable') }}
            </span>
          </div>

          <div class="seconds-micro-chart" :data-chart-state="sparklinePoints.length ? 'ready' : 'empty'">
            <canvas ref="sparklineCanvas" role="img" :aria-label="t('seconds.referencePrice')" />
            <span v-if="!sparklinePoints.length" role="status">
              {{ loading ? t('common.loading') : t('common.marketUnavailable') }}
            </span>
          </div>

          <section
            class="instrument-plate seconds-order-console"
            data-instrument-plate="market-and-order"
          >
            <div class="seconds-duration-scroll" role="group" :aria-label="t('seconds.term')">
              <div class="seconds-duration-grid">
              <template v-if="selected?.cycles.length">
                <button
                  v-for="item in selected.cycles"
                  :key="item.id"
                  type="button"
                  :class="{ active: cycle?.id === item.id }"
                  :aria-pressed="cycle?.id === item.id"
                  :disabled="loading || !selected"
                  @click="selectCycle(item.id)"
                >
                  <span>{{ t('seconds.duration', { seconds: item.durationSeconds }) }}</span>
                </button>
              </template>
              <template v-else>
                <button v-for="slot in 4" :key="slot" type="button" disabled><span>--</span></button>
              </template>
              </div>
            </div>

            <div class="seconds-cycle-limit">
              <span>{{ cycle ? t('seconds.cycleOrderLimit', { seconds: cycle.durationSeconds }) : '--' }}</span>
              <b class="numeric">{{ cycleLimitLabel }}</b>
            </div>

            <label
              class="seconds-amount-field"
              :data-field-state="amountFieldInvalid ? 'invalid' : amount && valid ? 'complete' : 'idle'"
            >
              <span>{{ t('seconds.stakeAmount') }}</span>
              <div>
                <input
                  v-model="amount"
                  class="numeric"
                  inputmode="decimal"
                  autocomplete="off"
                  :disabled="loading || !selected"
                  :aria-invalid="amountFieldInvalid"
                  aria-describedby="seconds-balance-hint"
                  @input="setAmount(amount)"
                />
                <b>{{ selected?.stakeAssetSymbol || '--' }}</b>
              </div>
              <small id="seconds-balance-hint" class="sr-only">
                {{ selected && session.isAuthenticated && account
                  ? t('seconds.balanceMinimum', {
                    available: moneyText(walletAvailable(account)),
                    asset: selected.stakeAssetSymbol,
                    minimum: moneyText(cycleMin(cycle), 8, '0'),
                  })
                  : t('seconds.loginDescription') }}
              </small>
            </label>

            <div class="seconds-direction-grid" role="group" :aria-label="t('seconds.direction')">
              <button
                type="button"
                class="up"
                :class="{ active: direction === 'up' }"
                :aria-pressed="direction === 'up'"
                :disabled="loading || !selected"
                @click="setDirection('up')"
              >
                <i aria-hidden="true" />
                <span>{{ t('seconds.bullish') }}</span>
              </button>
              <button
                type="button"
                class="down"
                :class="{ active: direction === 'down' }"
                :aria-pressed="direction === 'down'"
                :disabled="loading || !selected"
                @click="setDirection('down')"
              >
                <i aria-hidden="true" />
                <span>{{ t('seconds.bearish') }}</span>
              </button>
            </div>

            <button
              ref="reviewButton"
              class="button button--primary button--full seconds-submit"
              :class="{ 'seconds-submit--down': direction === 'down' }"
              type="button"
              :disabled="submitting || loading || !selected"
              :aria-busy="submitting"
              @click="reviewOrder"
            >
              {{ submitting ? t('common.submitting') : orderActionLabel }}
            </button>
          </section>
        </section>

        <section class="seconds-orders-workspace" :aria-label="t('seconds.inProgressOrders')">
          <header class="seconds-orders-heading">
            <div>
              <h2>{{ t('seconds.inProgressOrders') }}</h2>
              <span class="numeric" :aria-label="t('seconds.activeOrderCount', { count: activeOrders.length })">
                {{ activeOrders.length }}
              </span>
            </div>
            <button type="button" @click="openHistory">
              <span>{{ t('seconds.allOrders') }}</span>
              <ChevronRight :size="14" aria-hidden="true" />
            </button>
          </header>

          <div class="seconds-order-filters" role="group" :aria-label="t('seconds.orderFilter')">
            <button
              type="button"
              :class="{ active: activeOrderFilter === 'all' }"
              :aria-pressed="activeOrderFilter === 'all'"
              @click="activeOrderFilter = 'all'"
            >
              {{ t('common.all') }} {{ activeOrderCounts.all }}
            </button>
            <button
              type="button"
              class="up"
              :class="{ active: activeOrderFilter === 'up' }"
              :aria-pressed="activeOrderFilter === 'up'"
              @click="activeOrderFilter = 'up'"
            >
              {{ t('seconds.bullish') }} {{ activeOrderCounts.up }}
            </button>
            <button
              type="button"
              class="down"
              :class="{ active: activeOrderFilter === 'down' }"
              :aria-pressed="activeOrderFilter === 'down'"
              @click="activeOrderFilter = 'down'"
            >
              {{ t('seconds.bearish') }} {{ activeOrderCounts.down }}
            </button>
          </div>

          <div v-if="error || refreshWarning || loading" class="seconds-feedback" aria-live="polite">
            <div v-if="error" class="seconds-message seconds-message--error" role="alert">
              <CircleAlert :size="16" aria-hidden="true" />
              <span>{{ error }}</span>
              <button type="button" :aria-label="t('common.retry')" @click="load"><RefreshCw :size="16" /></button>
            </div>
            <div v-else-if="refreshWarning" class="seconds-message seconds-message--warning" role="status">
              <RefreshCw :size="16" aria-hidden="true" />
              <span>{{ refreshWarning }}</span>
            </div>
            <span v-else><LoaderCircle :size="15" class="spin" />{{ t('seconds.loading') }}</span>
          </div>

          <div
            v-if="filteredActiveOrders.length"
            class="seconds-active-order-list"
            :data-active-order-list="activeOrderFilter"
          >
            <article
              v-for="order in filteredActiveOrders"
              :key="order.id"
              class="seconds-active-order"
              :class="order.direction"
              data-active-order="real"
              :data-active-order-id="order.id"
            >
              <header>
                <div class="seconds-active-order__identity">
                  <AssetMark
                    :symbol="baseSymbol(order.symbol)"
                    :src="marketStore.tickerFor(order.symbol)?.baseIconUrl || marketStore.tickerFor(order.symbol)?.iconUrl"
                    :fallback-src="marketStore.tickerFor(order.symbol)?.iconUrl"
                    :size="22"
                  />
                  <strong>{{ displayProductSymbol(order.symbol) }}</strong>
                  <span :class="order.direction">
                    {{ t(order.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}
                  </span>
                </div>
                <span
                  class="seconds-active-order__countdown numeric"
                  :aria-label="`${orderStatusLabel(order)} ${orderCountdown(order)}`"
                >
                  <i aria-hidden="true" />
                  <b>{{ orderCountdown(order) }}</b>
                </span>
              </header>
              <dl>
                <div>
                  <dt>{{ t('seconds.stakeAmount') }}</dt>
                  <dd class="numeric">{{ moneyText(orderMoney(order).stakeAmount) }} {{ order.stakeAssetSymbol }}</dd>
                </div>
                <div>
                  <dt>{{ t('orders.entryPrice') }}</dt>
                  <dd class="numeric">{{ moneyText(orderMoney(order).entryPrice) }}</dd>
                </div>
                <div>
                  <dt>{{ t('seconds.estimatedProfit') }}</dt>
                  <dd class="numeric">
                    {{ orderProfit(order)
                      ? `+${moneyText(orderProfit(order))} ${order.stakeAssetSymbol}`
                      : '--' }}
                  </dd>
                </div>
              </dl>
              <div class="seconds-active-progress" aria-hidden="true">
                <i :style="{ width: `${orderProgress(order)}%` }" />
              </div>
            </article>
          </div>

          <p v-else class="seconds-active-orders-empty" role="status">
            {{ !session.isAuthenticated
              ? t('seconds.loginDescription')
              : loading
                ? t('seconds.loading')
                : t('seconds.activeOrdersEmpty') }}
          </p>
        </section>
      </section>
    </div>

    <Teleport to="body">
      <Transition name="seconds-pair-picker-reveal">
        <div
          v-if="pairPickerOpen"
          class="seconds-pair-picker-layer"
          data-pencil-source="vONcc kLXCs"
          @click.self="closePairPicker"
        >
          <section
            id="seconds-pair-picker"
            ref="pairPickerDialog"
            class="seconds-pair-picker"
            role="dialog"
            aria-modal="true"
            aria-labelledby="seconds-pair-picker-title"
            tabindex="-1"
            @keydown="handlePairPickerKeydown"
          >
            <header class="seconds-pair-picker__header">
              <h2 id="seconds-pair-picker-title" class="seconds-pair-picker__title">
                {{ t('seconds.pairPickerTitle') }}
              </h2>
              <button
                class="seconds-pair-picker__close"
                type="button"
                :aria-label="t('common.close')"
                @click="closePairPicker"
              >
                <span class="seconds-pair-picker__close-face" aria-hidden="true">
                  <X :size="18" :stroke-width="1.9" />
                </span>
              </button>
            </header>

            <label class="seconds-pair-picker__search">
              <Search :size="18" :stroke-width="1.9" aria-hidden="true" />
              <input
                v-model="pairSearch"
                data-seconds-pair-search
                type="search"
                autocomplete="off"
                spellcheck="false"
                :aria-label="t('seconds.pairPickerSearchLabel')"
                :placeholder="t('seconds.pairPickerSearchPlaceholder')"
              />
            </label>

            <div
              class="seconds-pair-picker__list"
              role="listbox"
              :aria-label="t('seconds.pairPickerProductsLabel')"
            >
              <div
                v-for="product in filteredPairProducts"
                :key="product.id"
                :id="`seconds-pair-option-${product.id}`"
                class="seconds-pair-picker__row"
                :class="{ 'is-selected': selected?.id === product.id }"
                role="option"
                :aria-selected="selected?.id === product.id"
                :tabindex="activePairProductId === product.id ? 0 : -1"
                :data-seconds-pair-option-id="product.id"
                @focus="activePairProductId = product.id"
                @click="choosePairProduct(product)"
              >
                <AssetMark
                  :symbol="baseSymbol(product.symbol)"
                  :src="marketStore.tickerFor(product.symbol)?.baseIconUrl || marketStore.tickerFor(product.symbol)?.iconUrl"
                  :fallback-src="marketStore.tickerFor(product.symbol)?.iconUrl"
                  :size="30"
                />
                <strong class="numeric">{{ displayProductSymbol(product.symbol) }}</strong>
                <span class="seconds-pair-picker__price numeric">
                  {{ priceFor(product.symbol)
                    ? moneyText(priceFor(product.symbol))
                    : '--' }}
                </span>
                <Check
                  v-if="selected?.id === product.id"
                  :size="17"
                  :stroke-width="2.2"
                  aria-hidden="true"
                />
              </div>

              <p v-if="loading" class="seconds-pair-picker__state" role="status">
                <LoaderCircle :size="18" class="spin" aria-hidden="true" />
                <span>{{ t('seconds.loading') }}</span>
              </p>
              <p v-else-if="!products.length" class="seconds-pair-picker__state" role="status">
                {{ t('seconds.noProducts') }}
              </p>
              <p v-else-if="!filteredPairProducts.length" class="seconds-pair-picker__state" role="status">
                {{ t('seconds.pairPickerNoResults') }}
              </p>
            </div>

            <p class="seconds-pair-picker__note">{{ t('seconds.pairPickerNote') }}</p>
          </section>
        </div>
      </Transition>
    </Teleport>

    <Teleport to="body">
      <div v-if="confirmOpen && orderReview" class="confirmation-layer seconds-mask" @click.self="closeConfirm">
        <section
          ref="confirmDialog"
          class="confirmation-sheet seconds-dialog"
          role="dialog"
          aria-modal="true"
          :aria-busy="submitting"
          aria-labelledby="seconds-confirm-title"
          aria-describedby="seconds-confirm-summary"
          tabindex="-1"
          @keydown="trapDialogFocus"
        >
          <header>
            <span class="confirmation-icon"><CheckCircle2 :size="20" /></span>
            <div>
              <strong id="seconds-confirm-title">{{ t('seconds.confirmOrder') }}</strong>
              <small>{{ orderReview.symbol }} · {{ t('seconds.settledIn', { asset: orderReview.stakeAssetSymbol }) }}</small>
            </div>
            <button
              class="icon-button"
              type="button"
              :aria-label="t('common.close')"
              :disabled="submitting"
              data-dialog-cancel
              @click="closeConfirm"
            >
              <X :size="21" />
            </button>
          </header>

          <div class="seconds-dialog__body">
            <p id="seconds-confirm-summary">
              {{ orderReview.symbol }} · {{ t(orderReview.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }} ·
              {{ moneyText(orderReview.stakeAmount) }} {{ orderReview.stakeAssetSymbol }}
            </p>

            <dl class="confirmation-detail">
              <div><dt>{{ t('seconds.direction') }}</dt><dd>{{ t(orderReview.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}</dd></div>
              <div><dt>{{ t('seconds.term') }}</dt><dd>{{ t('seconds.duration', { seconds: orderReview.durationSeconds }) }}</dd></div>
              <div><dt>{{ t('seconds.stakeAmount') }}</dt><dd>{{ moneyText(orderReview.stakeAmount) }} {{ orderReview.stakeAssetSymbol }}</dd></div>
              <div>
                <dt>{{ t('seconds.payoutRate') }}</dt>
                <dd>{{ payoutText(orderReview.payoutRate, 2) }}% · +{{ moneyText(reviewProfit) }} {{ orderReview.stakeAssetSymbol }}</dd>
              </div>
              <div><dt>{{ t('marketDetail.latestPrice') }}</dt><dd>{{ moneyText(orderReview.referencePrice) }}</dd></div>
            </dl>

            <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
          </div>

          <div class="confirmation-actions dialog-actions">
            <button type="button" class="button button--secondary" :disabled="submitting" @click="closeConfirm">
              {{ t('common.cancel') }}
            </button>
            <button
              type="button"
              class="button button--primary confirmation-primary"
              :disabled="submitting"
              :aria-busy="submitting"
              @click="submit"
            >
              {{ submitting ? t('common.submitting') : t('seconds.confirmOrder') }}
            </button>
          </div>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <Transition name="seconds-result-reveal" mode="out-in">
        <aside
          v-if="settlementDialogOpen && settled"
          class="seconds-settlement-layer"
          data-pencil-source="tFcTH FBdqS"
          @click.self="advanceSettlementResult"
        >
          <p class="sr-only" role="status" aria-live="polite" aria-atomic="true">
            {{ currentSettlementAnnouncement }}
          </p>
          <article
            ref="settlementDialog"
            class="seconds-settlement-card"
            :data-tone="settlementTone"
            :data-direction="settled.direction"
            data-settlement-source="orders-api"
            role="dialog"
            aria-modal="true"
            :aria-labelledby="`seconds-settlement-title-${settled.id}`"
            :aria-describedby="`seconds-settlement-note-${settled.id}`"
            tabindex="-1"
            @keydown="handleSettlementDialogKeydown"
          >
            <header class="seconds-settlement-card__status-row">
              <span class="seconds-settlement-card__status">
                <CircleCheckBig :size="17" :stroke-width="2" aria-hidden="true" />
                <span>{{ t('seconds.statusSettled') }}</span>
              </span>
              <button
                class="seconds-settlement-card__close"
                type="button"
                :aria-label="t('common.close')"
                :title="t('common.close')"
                data-settlement-initial
                @click="advanceSettlementResult"
              >
                <span class="seconds-settlement-card__close-surface" aria-hidden="true">
                  <X :size="18" :stroke-width="1.9" />
                </span>
              </button>
            </header>

            <section class="seconds-settlement-card__result">
              <span class="seconds-settlement-card__result-icon" aria-hidden="true">
                <BadgeDollarSign :size="34" :stroke-width="1.75" />
              </span>
              <strong :id="`seconds-settlement-title-${settled.id}`">
                {{ settlementTitle }}
              </strong>
              <b class="seconds-settlement-card__amount numeric">{{ settlementAmount }}</b>
              <span class="seconds-settlement-card__rate">
                {{ t('seconds.settlementReturnRate', { rate: settlementRate }) }}
              </span>
            </section>

            <section class="seconds-settlement-card__prices" :aria-label="t('seconds.settlementPriceComparison')">
              <span class="seconds-settlement-card__price">
                <small>{{ t('seconds.settlementEntryPrice') }}</small>
                <strong class="numeric">
                  {{ moneyText(orderMoney(settled).entryPrice) }}
                </strong>
              </span>
              <span class="seconds-settlement-card__price-arrow" aria-hidden="true">
                <ArrowRight :size="16" :stroke-width="1.9" />
              </span>
              <span class="seconds-settlement-card__price seconds-settlement-card__price--settled">
                <small>{{ t('seconds.settlementPrice') }}</small>
                <strong class="numeric">
                  {{ moneyText(orderMoney(settled).settlementPrice) }}
                </strong>
              </span>
            </section>

            <dl class="seconds-settlement-card__summary" :aria-label="t('seconds.settlementResultDetails')">
              <div>
                <dt>{{ t('seconds.settlementPair') }}</dt>
                <dd>{{ displayProductSymbol(settled.symbol) }}</dd>
              </div>
              <div>
                <dt>{{ t('seconds.settlementDirection') }}</dt>
                <dd>{{ t(settled.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}</dd>
              </div>
              <div>
                <dt>{{ t('seconds.settlementCycle') }}</dt>
                <dd>{{ t('seconds.historyDuration', { seconds: settled.durationSeconds }) }}</dd>
              </div>
            </dl>

            <p
              :id="`seconds-settlement-note-${settled.id}`"
              class="seconds-settlement-card__note"
            >
              <Info :size="17" :stroke-width="1.8" aria-hidden="true" />
              <span>
                {{ t('seconds.settlementAutoSummary', {
                  amount: moneyText(orderMoney(settled).stakeAmount),
                  asset: settled.stakeAssetSymbol,
                }) }}
              </span>
            </p>

            <p v-if="remainingResults" class="sr-only">
              {{ t('seconds.settlementResultsRemaining', { count: remainingResults }) }}
            </p>

            <div class="seconds-settlement-card__actions">
              <button type="button" class="seconds-settlement-card__history" @click="openHistory">
                <History :size="16" :stroke-width="2" aria-hidden="true" />
                <span>{{ t('seconds.viewHistory') }}</span>
              </button>
            </div>
          </article>
        </aside>
      </Transition>
    </Teleport>
  </main>
</template>

<style scoped>
.seconds-page {
  background: var(--seconds-page);
  color: var(--seconds-text);
  font-family: var(--font-geist-sans), Inter, "PingFang SC", sans-serif;
  min-width: 0;
  overflow-x: clip;
  padding-bottom: 0;
  position: relative;
}

.seconds-page .numeric {
  font-family: var(--font-geist-mono), var(--data-font);
  font-variant-numeric: tabular-nums;
}

.seconds-content {
  min-width: 0;
  padding: 0;
}

.seconds-workspace,
.seconds-trading-operation,
.seconds-order-console,
.seconds-orders-workspace {
  min-width: 0;
}

.seconds-workspace {
  display: block;
  width: 100%;
}

.seconds-header {
  background: var(--seconds-page) !important;
  box-sizing: border-box;
  grid-template-columns: 40px minmax(0, 1fr) 40px !important;
  height: 60px !important;
  min-height: 60px !important;
  padding: 10px 20px !important;
}

.seconds-header :deep(.page-header__copy) {
  height: 22px;
  left: 50%;
  min-width: 0;
  padding: 0;
  position: absolute;
  top: 19px;
  transform: translateX(-50%);
  width: 140px;
}

.seconds-header :deep(.page-header__back.icon-button),
.seconds-header :deep(.page-header__actions),
.seconds-header :deep(.page-header__actions > .icon-button) {
  height: 40px !important;
  min-height: 40px !important;
  min-width: 40px !important;
  width: 40px !important;
}

.seconds-header :deep(.page-header__actions) {
  grid-column: 3;
}

.seconds-header :deep(.icon-button) {
  color: var(--seconds-text);
  position: relative;
}

.seconds-header :deep(.icon-button)::before {
  content: '';
  inset: -2px;
  position: absolute;
}

.seconds-pair-field {
  align-items: center;
  appearance: none;
  background: transparent;
  border: 0;
  border-radius: 6px;
  color: inherit;
  cursor: pointer;
  display: block;
  height: 44px;
  margin: -11px 0;
  min-width: 0;
  padding: 11px 0;
  position: relative;
  touch-action: manipulation;
  width: 140px;
}

.seconds-pair-copy {
  align-items: center;
  color: var(--seconds-text);
  display: grid;
  gap: 4px;
  grid-template-columns: minmax(0, 1fr) auto 15px;
  height: 22px;
  min-width: 0;
  width: 100%;
}

.seconds-pair-copy strong {
  font-size: 17px;
  font-weight: 750;
  letter-spacing: -.2px;
  line-height: 22px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-pair-copy small {
  color: var(--seconds-muted);
  font-size: 10px;
  font-weight: 500;
  line-height: 14px;
  max-width: 48px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-pair-copy svg {
  display: block;
  transition: transform 180ms ease;
}

.seconds-pair-field[aria-expanded='true'] .seconds-pair-copy svg {
  transform: rotate(180deg);
}

.seconds-pair-field:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 3px;
}

.seconds-pair-picker-layer {
  align-items: flex-end;
  background: var(--seconds-pair-backdrop);
  display: flex;
  inset: 0;
  justify-content: center;
  position: fixed;
  z-index: 120;
}

.seconds-pair-picker {
  background: var(--seconds-pair-sheet);
  border: 0;
  border-radius: 24px 24px 0 0;
  box-shadow:
    inset 0 1px 0 var(--seconds-pair-sheet-border),
    inset 1px 0 0 var(--seconds-pair-sheet-border),
    inset -1px 0 0 var(--seconds-pair-sheet-border),
    0 -12px 42px var(--seconds-pair-sheet-shadow);
  box-sizing: border-box;
  color: var(--seconds-pair-text);
  display: flex;
  flex-direction: column;
  font-family: var(--font-geist-sans), Inter, "PingFang SC", sans-serif;
  gap: 14px;
  height: calc(100dvh - 80px);
  max-height: 840px;
  max-width: 390px;
  min-height: 0;
  overflow: hidden;
  padding: 18px 20px calc(16px + env(safe-area-inset-bottom));
  width: 100%;
}

.seconds-pair-picker__header {
  align-items: center;
  display: flex;
  flex: 0 0 auto;
  height: 34px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-pair-picker__title {
  color: var(--seconds-pair-text);
  font-size: 20px;
  font-weight: 700;
  letter-spacing: -.2px;
  line-height: 26px;
  margin: 0;
}

.seconds-pair-picker__close {
  align-items: center;
  appearance: none;
  background: transparent;
  border: 0;
  color: inherit;
  cursor: pointer;
  display: inline-flex;
  height: 44px;
  justify-content: center;
  margin: -5px;
  padding: 5px;
  touch-action: manipulation;
  width: 44px;
}

.seconds-pair-picker__close-face {
  align-items: center;
  background: var(--seconds-pair-close);
  border: 1px solid var(--seconds-pair-close-border);
  border-radius: 50%;
  box-sizing: border-box;
  color: var(--seconds-pair-close-icon);
  display: inline-flex;
  height: 34px;
  justify-content: center;
  transition: background-color 160ms ease, transform 160ms ease;
  width: 34px;
}

.seconds-pair-picker__close:active .seconds-pair-picker__close-face {
  transform: scale(.94);
}

.seconds-pair-picker__close:focus-visible,
.seconds-pair-picker__row:focus-visible,
.seconds-pair-picker__search:focus-within {
  outline: 2px solid var(--seconds-pair-focus);
  outline-offset: 2px;
}

.seconds-pair-picker__search {
  align-items: center;
  background: var(--seconds-pair-search);
  border: 1px solid var(--seconds-pair-search-border);
  border-radius: 12px;
  box-sizing: border-box;
  color: var(--seconds-pair-muted);
  display: flex;
  flex: 0 0 auto;
  gap: 10px;
  height: 46px;
  padding: 0 14px;
  transition: border-color 160ms ease, box-shadow 160ms ease;
}

.seconds-pair-picker__search:focus-within {
  border-color: var(--seconds-pair-focus);
  box-shadow: 0 0 0 3px var(--seconds-pair-focus-ring);
  outline: 0;
}

.seconds-pair-picker__search svg {
  flex: 0 0 auto;
}

.seconds-pair-picker__search input {
  appearance: none;
  background: transparent;
  border: 0;
  color: var(--seconds-pair-text);
  flex: 1 1 auto;
  font: 450 13px/18px var(--font-geist-sans), Inter, "PingFang SC", sans-serif;
  height: 100%;
  min-width: 0;
  outline: 0;
  padding: 0;
}

.seconds-pair-picker__search input::placeholder {
  color: var(--seconds-pair-muted);
  opacity: 1;
}

.seconds-pair-picker__search input::-webkit-search-cancel-button {
  appearance: none;
}

.seconds-pair-picker__list {
  display: grid;
  flex: 0 1 auto;
  gap: 8px;
  max-height: calc(100% - 136px);
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: none;
}

.seconds-pair-picker__list::-webkit-scrollbar {
  display: none;
}

.seconds-pair-picker__row {
  align-items: center;
  appearance: none;
  background: var(--seconds-pair-row);
  border: 1px solid var(--seconds-pair-row-border);
  border-radius: 12px;
  box-sizing: border-box;
  color: var(--seconds-pair-text);
  cursor: pointer;
  display: grid;
  flex: 0 0 auto;
  gap: 12px;
  grid-template-columns: 30px minmax(0, 1fr) max-content 17px;
  height: 64px;
  min-height: 64px;
  padding: 0 14px;
  text-align: left;
  touch-action: manipulation;
  transition: background-color 160ms ease, border-color 160ms ease, transform 160ms ease;
  width: 100%;
}

.seconds-pair-picker__row.is-selected {
  background: var(--seconds-pair-selected);
  border-color: var(--seconds-pair-signal);
}

.seconds-pair-picker__row:active {
  transform: scale(.985);
}

.seconds-pair-picker__row > strong {
  font: 700 15px/20px var(--font-geist-mono), var(--data-font);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-pair-picker__price {
  font-size: 14px;
  font-weight: 650;
  line-height: 20px;
  white-space: nowrap;
}

.seconds-pair-picker__row > svg {
  color: var(--seconds-pair-check);
  display: block;
}

.seconds-pair-picker__state {
  align-items: center;
  border: 1px dashed var(--seconds-pair-row-border);
  border-radius: 12px;
  color: var(--seconds-pair-muted);
  display: flex;
  font-size: 13px;
  gap: 8px;
  justify-content: center;
  line-height: 18px;
  margin: 0;
  min-height: 64px;
  padding: 0 14px;
  text-align: center;
}

.seconds-pair-picker__note {
  color: var(--seconds-pair-muted);
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 450;
  line-height: 14px;
  margin: 0;
  min-height: 14px;
}

.seconds-pair-picker-reveal-enter-active,
.seconds-pair-picker-reveal-leave-active {
  transition: opacity 220ms ease;
}

.seconds-pair-picker-reveal-enter-active .seconds-pair-picker,
.seconds-pair-picker-reveal-leave-active .seconds-pair-picker {
  transition: transform 300ms cubic-bezier(.22, 1, .36, 1);
}

.seconds-pair-picker-reveal-enter-from,
.seconds-pair-picker-reveal-leave-to {
  opacity: 0;
}

.seconds-pair-picker-reveal-enter-from .seconds-pair-picker,
.seconds-pair-picker-reveal-leave-to .seconds-pair-picker {
  transform: translateY(18px);
}

.seconds-trading-operation {
  align-content: start;
  background: var(--seconds-page);
  box-sizing: border-box;
  display: grid;
  grid-template-rows: 22px 53px 112px 202px;
  height: 420px;
  padding: 2px 20px 10px;
  row-gap: 6px;
}

.seconds-market-status {
  align-items: center;
  display: flex;
  height: 22px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-round-state {
  align-items: center;
  color: var(--seconds-muted);
  display: inline-flex;
  gap: 6px;
  min-width: 0;
}

.seconds-round-state > i,
.seconds-active-order__countdown > i {
  background: var(--seconds-signal);
  border-radius: 50%;
  display: block;
  flex: 0 0 auto;
  height: 6px;
  width: 6px;
}

.seconds-round-state b {
  font-size: 10px;
  font-weight: 600;
  line-height: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-return-rate {
  align-items: center;
  background: var(--seconds-positive-soft);
  border-radius: 9px;
  color: var(--seconds-positive-text);
  display: inline-flex;
  flex: 0 0 82px;
  font-size: 10px;
  font-weight: 650;
  height: 22px;
  justify-content: center;
  line-height: 14px;
  overflow: hidden;
  padding: 0 8px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-price-panel {
  display: grid;
  grid-template-rows: 35px 18px;
  height: 53px;
  min-width: 0;
  position: relative;
}

.seconds-price-panel > strong {
  color: var(--seconds-positive-text);
  font-size: 31px;
  font-weight: 800;
  letter-spacing: -.8px;
  line-height: 35px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-price-meta {
  align-items: center;
  color: var(--seconds-muted);
  display: flex;
  font-size: 10px;
  font-weight: 500;
  line-height: 14px;
  min-width: 0;
  padding-right: 84px;
}

.seconds-price-meta > span {
  min-width: 0;
  white-space: nowrap;
}

.seconds-live-state {
  align-items: center;
  bottom: 2px;
  color: var(--seconds-muted);
  display: inline-flex;
  font-size: 10px;
  font-weight: 600;
  line-height: 14px;
  max-width: 82px;
  overflow: hidden;
  position: absolute;
  right: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-live-state[data-state='unavailable'] {
  color: var(--seconds-negative);
}

.seconds-micro-chart {
  height: 112px;
  min-width: 0;
  position: relative;
}

.seconds-micro-chart canvas {
  display: block;
  height: 112px;
  width: 100%;
}

.seconds-micro-chart > span {
  color: var(--seconds-muted);
  font-size: 10px;
  left: 50%;
  line-height: 14px;
  max-width: calc(100% - 24px);
  overflow: hidden;
  position: absolute;
  text-overflow: ellipsis;
  top: 50%;
  transform: translate(-50%, -50%);
  white-space: nowrap;
}

.seconds-order-console {
  background: transparent;
  border: 0;
  border-radius: 0;
  box-shadow: none;
  box-sizing: border-box;
  display: grid;
  gap: 6px;
  grid-template-rows: 30px 26px 38px 40px 44px;
  height: 202px;
  min-width: 0;
  padding: 0;
}

.seconds-duration-scroll {
  box-sizing: border-box;
  height: 44px;
  margin-block: -7px;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  padding-block: 7px;
  scrollbar-width: none;
}

.seconds-duration-scroll::-webkit-scrollbar {
  display: none;
}

.seconds-duration-grid {
  display: grid;
  gap: 6px;
  grid-auto-columns: calc((100% - 18px) / 4);
  grid-auto-flow: column;
  grid-template-columns: none;
  height: 30px;
  min-width: 100%;
  width: 100%;
}

.seconds-duration-grid button {
  align-items: center;
  background: var(--seconds-control-surface);
  border: 1px solid var(--seconds-line);
  border-radius: 9px;
  color: var(--seconds-text);
  display: inline-flex;
  font-size: 11px;
  font-weight: 600;
  height: 30px;
  justify-content: center;
  max-height: 30px;
  min-height: 30px !important;
  min-width: 0;
  padding: 0 4px;
  position: relative;
  box-shadow: none;
  white-space: nowrap;
}

.seconds-duration-grid button::before {
  content: '';
  inset: -8px 0;
  position: absolute;
}

.seconds-duration-grid button.active {
  background: var(--seconds-positive-soft);
  border-color: var(--seconds-positive-text);
  color: var(--seconds-positive-text);
  font-weight: 700;
}

.seconds-duration-grid button:disabled {
  cursor: default;
  opacity: .55;
}

.seconds-cycle-limit {
  align-items: center;
  background: var(--seconds-positive-soft);
  border-radius: 8px;
  color: var(--seconds-muted);
  display: flex;
  font-size: 9px;
  gap: 8px;
  height: 26px;
  justify-content: space-between;
  line-height: 13px;
  min-width: 0;
  padding: 0 10px;
}

.seconds-cycle-limit > span {
  color: var(--seconds-positive-text);
  flex: 0 0 auto;
  font-size: 10px;
  font-weight: 600;
}

.seconds-cycle-limit > b {
  color: var(--seconds-text);
  font-size: 10px;
  font-weight: 600;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-amount-field {
  align-items: center;
  background: var(--seconds-control-surface);
  border: 1px solid var(--seconds-line);
  border-radius: 10px;
  box-sizing: border-box;
  color: var(--seconds-text);
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  height: 38px;
  min-width: 0;
  padding: 0 12px;
  position: relative;
}

.seconds-amount-field::before {
  content: '';
  inset: -4px 0;
  position: absolute;
}

.seconds-amount-field:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.seconds-amount-field[data-field-state='invalid'] {
  border-color: var(--seconds-negative);
}

.seconds-amount-field > span {
  color: var(--seconds-muted);
  font-size: 10px;
  font-weight: 550;
  line-height: 14px;
}

.seconds-amount-field > div {
  align-items: center;
  display: flex;
  gap: 5px;
  justify-content: flex-end;
  min-width: 0;
}

.seconds-amount-field input {
  appearance: textfield;
  background: transparent;
  border: 0;
  box-shadow: none;
  color: var(--seconds-text);
  flex: 1 1 auto;
  font-size: 15px;
  font-weight: 750;
  height: 36px;
  min-width: 0;
  outline: 0;
  padding: 0;
  text-align: right;
  width: 100%;
}

.seconds-amount-field input:focus,
.seconds-amount-field input:focus-visible {
  border: 0;
  box-shadow: none;
  outline: 0;
}

.seconds-amount-field b {
  flex: 0 0 auto;
  font-size: 10px;
  font-weight: 650;
}

.seconds-direction-grid {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  height: 40px;
  min-width: 0;
}

.seconds-direction-grid button {
  align-items: center;
  background: var(--seconds-control-surface);
  border: 1px solid var(--seconds-line);
  border-radius: 14px;
  color: var(--seconds-muted);
  display: inline-flex;
  font-size: 14px;
  font-weight: 800;
  gap: 7px;
  height: 40px;
  justify-content: center;
  max-height: 40px;
  min-height: 40px !important;
  min-width: 0;
  padding: 0 8px;
  position: relative;
  box-shadow: none;
}

.seconds-direction-grid button::before {
  content: '';
  inset: -3px 0;
  position: absolute;
}

.seconds-direction-grid button > i {
  border: 1px solid currentColor;
  border-radius: 50%;
  box-sizing: border-box;
  display: block;
  height: 8px;
  width: 8px;
}

.seconds-direction-grid button.up.active {
  background: var(--seconds-positive-soft);
  border-color: var(--seconds-positive-text);
  box-shadow: none;
  color: var(--seconds-positive-text);
}

.seconds-direction-grid button.down.active {
  background: var(--seconds-negative-soft);
  border-color: var(--seconds-negative);
  box-shadow: none;
  color: var(--seconds-negative);
}

.seconds-direction-grid button.active > i {
  background: currentColor;
  box-shadow: inset 0 0 0 2px var(--seconds-control-surface);
}

.seconds-direction-grid button:disabled {
  cursor: default;
  opacity: .55;
}

.seconds-submit {
  background: var(--seconds-signal) !important;
  background-image: none !important;
  border: 0;
  border-radius: 10px;
  color: var(--seconds-on-signal) !important;
  font-size: 14px;
  font-weight: 700;
  height: 44px;
  max-height: 44px;
  min-height: 44px !important;
  overflow: hidden;
  padding: 0 12px;
  box-shadow: none !important;
  text-overflow: ellipsis;
  white-space: nowrap;
  width: 100%;
}

.seconds-submit.seconds-submit--down {
  background: var(--seconds-negative) !important;
}

.seconds-submit:disabled {
  cursor: default;
  opacity: .55;
}

.seconds-orders-workspace {
  align-content: start;
  background: var(--seconds-orders-surface);
  border-top: 1px solid var(--seconds-line);
  box-sizing: border-box;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  min-height: 362px;
  padding: 12px 20px calc(16px + env(safe-area-inset-bottom));
  width: 100%;
}

.seconds-orders-heading {
  align-items: center;
  display: flex;
  height: 24px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-orders-heading > div {
  align-items: center;
  display: flex;
  gap: 7px;
  min-width: 0;
}

.seconds-orders-heading h2 {
  color: var(--seconds-text);
  font-size: 15px;
  font-weight: 800;
  line-height: 20px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-orders-heading > div > span {
  align-items: center;
  background: var(--seconds-positive-soft);
  border-radius: 8px;
  color: var(--seconds-positive-text);
  display: inline-flex;
  flex: 0 0 22px;
  font-size: 10px;
  font-weight: 700;
  height: 22px;
  justify-content: center;
  min-width: 22px;
}

.seconds-orders-heading > button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--seconds-positive-text);
  display: inline-flex;
  font-size: 11px;
  font-weight: 650;
  gap: 4px;
  height: 24px;
  max-height: 24px;
  min-height: 24px !important;
  padding: 0;
  position: relative;
}

.seconds-orders-heading > button::before {
  content: '';
  inset: -10px 0;
  position: absolute;
}

.seconds-order-filters {
  display: grid;
  gap: 6px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  height: 30px;
  margin-top: 6px;
  min-width: 0;
}

.seconds-order-filters button {
  background: var(--seconds-card-surface);
  border: 1px solid var(--seconds-line);
  border-radius: 9px;
  color: var(--seconds-muted);
  font-size: 10px;
  font-weight: 600;
  height: 30px;
  max-height: 30px;
  min-height: 30px !important;
  min-width: 0;
  padding: 0 6px;
  position: relative;
}

.seconds-order-filters button::before {
  content: '';
  inset: -8px 0;
  position: absolute;
}

.seconds-order-filters button.active {
  background: var(--seconds-positive-soft);
  border-color: var(--seconds-positive-text);
  color: var(--seconds-positive-text);
  font-weight: 750;
}

.seconds-order-filters button.down.active {
  background: var(--seconds-negative-soft);
  border-color: var(--seconds-negative);
  color: var(--seconds-negative);
}

.seconds-feedback {
  display: grid;
  gap: 6px;
  margin-top: 8px;
  min-width: 0;
}

.seconds-feedback > span {
  align-items: center;
  color: var(--seconds-muted);
  display: flex;
  font-size: 10px;
  gap: 7px;
  line-height: 16px;
  min-height: 44px;
}

.seconds-message {
  align-items: center;
  border: 1px solid currentColor;
  border-radius: 10px;
  display: grid;
  font-size: 10px;
  gap: 8px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  line-height: 15px;
  min-height: 44px;
  min-width: 0;
  padding: 3px 4px 3px 10px;
}

.seconds-message--error {
  background: var(--seconds-negative-soft);
  color: var(--seconds-negative);
}

.seconds-message--warning {
  background: var(--seconds-control-surface);
  color: var(--seconds-positive-text);
  grid-template-columns: auto minmax(0, 1fr);
}

.seconds-message button {
  background: transparent;
  border: 0;
  color: inherit;
  display: grid;
  min-height: 36px;
  min-width: 36px;
  padding: 0;
  place-items: center;
}

.seconds-active-order-list {
  display: grid;
  gap: 8px;
  margin-top: 8px;
  min-width: 0;
}

.seconds-active-order {
  background: var(--seconds-card-surface);
  border: 1px solid var(--seconds-line);
  border-radius: 12px;
  box-sizing: border-box;
  display: grid;
  gap: 7px;
  grid-template-rows: 22px 25px 3px;
  height: 82px;
  min-width: 0;
  padding: 8px 10px;
}

.seconds-active-order > header {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 22px;
  min-width: 0;
}

.seconds-active-order__identity {
  align-items: center;
  display: flex;
  gap: 6px;
  min-width: 0;
}

.seconds-active-order__identity > strong {
  color: var(--seconds-text);
  font-size: 11px;
  font-weight: 750;
  line-height: 15px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-active-order__identity > span {
  background: var(--seconds-positive-soft);
  border-radius: 6px;
  color: var(--seconds-positive-text);
  flex: 0 0 auto;
  font-size: 8px;
  font-weight: 700;
  line-height: 16px;
  padding: 0 6px;
}

.seconds-active-order__identity > span.down {
  background: var(--seconds-negative-soft);
  color: var(--seconds-negative);
}

.seconds-active-order__countdown {
  align-items: center;
  color: var(--seconds-positive-text);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 10px;
  gap: 5px;
  line-height: 14px;
}

.seconds-active-order.down .seconds-active-order__countdown {
  color: var(--seconds-negative);
}

.seconds-active-order.down .seconds-active-order__countdown > i {
  background: var(--seconds-negative);
}

.seconds-active-order__countdown b {
  font-weight: 700;
}

.seconds-active-order dl {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  height: 25px;
  margin: 0;
  min-width: 0;
}

.seconds-active-order dl > div {
  display: grid;
  gap: 1px;
  grid-template-rows: 10px 14px;
  min-width: 0;
}

.seconds-active-order dt,
.seconds-active-order dd {
  line-height: 1;
  margin: 0;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-active-order dt {
  color: var(--seconds-muted);
  font-size: 8px;
  font-weight: 500;
}

.seconds-active-order dd {
  color: var(--seconds-text);
  font-size: 9px;
  font-weight: 650;
  line-height: 14px;
}

.seconds-active-order dl > div:last-child,
.seconds-active-order dl > div:last-child dd {
  text-align: right;
}

.seconds-active-order dl > div:nth-child(2),
.seconds-active-order dl > div:nth-child(2) dd {
  text-align: center;
}

.seconds-active-order.up dl > div:last-child dd {
  color: var(--seconds-positive-text);
}

.seconds-active-order.down dl > div:last-child dd {
  color: var(--seconds-negative);
}

.seconds-active-progress {
  background: var(--seconds-line);
  border-radius: 2px;
  height: 3px;
  overflow: hidden;
}

.seconds-active-progress > i {
  background: var(--seconds-signal);
  border-radius: inherit;
  display: block;
  height: 100%;
  transition: width .2s linear;
}

.seconds-active-order.down .seconds-active-progress > i {
  background: var(--seconds-negative);
}

.seconds-active-orders-empty {
  align-items: center;
  border: 1px dashed var(--seconds-line);
  border-radius: 12px;
  color: var(--seconds-muted);
  display: flex;
  font-size: 10px;
  height: 82px;
  justify-content: center;
  line-height: 15px;
  margin: 8px 0 0;
  min-width: 0;
  padding: 0 16px;
  text-align: center;
}

.seconds-duration-grid button:focus-visible,
.seconds-direction-grid button:focus-visible,
.seconds-order-filters button:focus-visible,
.seconds-orders-heading > button:focus-visible,
.seconds-submit:focus-visible,
.seconds-message button:focus-visible,
.seconds-dialog button:focus-visible {
  box-shadow: 0 0 0 3px var(--focus-ring), inset 0 0 0 2px var(--focus);
  outline: 0;
}

.seconds-settlement-layer {
  align-items: center;
  background: var(--seconds-result-backdrop);
  box-sizing: border-box;
  display: grid;
  inset: 0;
  justify-items: center;
  min-height: 100dvh;
  overflow-x: clip;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding:
    max(16px, env(safe-area-inset-top))
    max(16px, env(safe-area-inset-right))
    max(16px, env(safe-area-inset-bottom))
    max(16px, env(safe-area-inset-left));
  pointer-events: auto;
  position: fixed;
  width: 100%;
  z-index: calc(var(--layer-overlay, 80) + 2);
}

.seconds-settlement-card {
  --seconds-result-tone: var(--seconds-result-positive);
  --seconds-result-tone-soft: var(--seconds-result-icon-soft);
  --seconds-result-tone-border: var(--seconds-result-icon-border);
  --seconds-result-tone-shadow: var(--seconds-result-icon-shadow);

  background: var(--seconds-result-card);
  border: 0;
  border-radius: 24px;
  box-shadow:
    0 16px 40px var(--seconds-result-card-shadow),
    inset 0 0 0 1px var(--seconds-result-card-border);
  box-sizing: border-box;
  color: var(--seconds-result-text);
  display: grid;
  font-family: "Noto Sans SC", "PingFang SC", "Hiragino Sans GB", sans-serif;
  gap: 14px;
  margin: auto;
  max-width: 358px;
  min-height: 541px;
  min-width: 0;
  overflow: hidden;
  padding: 20px 20px 18px;
  transform: translateY(-13.5px);
  width: 100%;
}

.seconds-settlement-card[data-tone='negative'] {
  --seconds-result-tone: var(--seconds-result-negative);
  --seconds-result-tone-soft: var(--seconds-result-negative-soft);
  --seconds-result-tone-border: color-mix(in srgb, var(--seconds-result-negative) 42%, transparent);
  --seconds-result-tone-shadow: color-mix(in srgb, var(--seconds-result-negative) 22%, transparent);
}

.seconds-settlement-card__status-row {
  align-items: center;
  display: flex;
  height: 34px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-settlement-card__status {
  align-items: center;
  background: var(--seconds-result-status-soft);
  border-radius: 99px;
  box-sizing: border-box;
  color: var(--seconds-result-positive);
  display: inline-flex;
  font-size: 13px;
  font-weight: 600;
  gap: 7px;
  height: 33px;
  line-height: 19px;
  padding: 7px 11px;
}

.seconds-settlement-card__status svg {
  flex: 0 0 auto;
}

.seconds-settlement-card__close {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 50%;
  display: grid;
  height: 44px;
  justify-items: center;
  margin: -5px;
  min-height: 44px;
  min-width: 44px;
  padding: 5px;
  width: 44px;
}

.seconds-settlement-card__close-surface {
  align-items: center;
  background: var(--seconds-result-close);
  border: 1px solid var(--seconds-result-close-border);
  border-radius: 50%;
  box-sizing: border-box;
  color: var(--seconds-result-close-icon);
  display: inline-flex;
  height: 34px;
  justify-content: center;
  width: 34px;
}

.seconds-settlement-card__result {
  align-items: center;
  display: flex;
  flex-direction: column;
  gap: 5px;
  height: 176px;
  min-width: 0;
}

.seconds-settlement-card__result-icon {
  align-items: center;
  background: var(--seconds-result-tone-soft);
  border: 1px solid var(--seconds-result-tone-border);
  border-radius: 50%;
  box-shadow: 0 6px 18px var(--seconds-result-tone-shadow);
  box-sizing: border-box;
  color: var(--seconds-result-tone);
  display: inline-flex;
  flex: 0 0 68px;
  height: 68px;
  justify-content: center;
  width: 68px;
}

.seconds-settlement-card__result > strong {
  color: var(--seconds-result-text);
  font-size: 18px;
  font-weight: 650;
  line-height: 26px;
  margin: 0;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-settlement-card__amount {
  color: var(--seconds-result-tone);
  font-family: var(--font-geist-mono), var(--data-font);
  font-size: 36px;
  font-variant-numeric: tabular-nums;
  font-weight: 750;
  letter-spacing: -.8px;
  line-height: 47px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-settlement-card__rate {
  color: var(--seconds-result-tone);
  font-size: 14px;
  font-weight: 650;
  line-height: 20px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-settlement-card__prices {
  align-items: center;
  background: var(--seconds-result-price-surface);
  border-radius: 14px;
  box-sizing: border-box;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 30px minmax(0, 1fr);
  height: 68px;
  min-width: 0;
  padding: 12px 14px;
}

.seconds-settlement-card__price {
  display: grid;
  gap: 3px;
  justify-self: start;
  min-width: 0;
}

.seconds-settlement-card__price--settled {
  justify-items: end;
  justify-self: end;
}

.seconds-settlement-card__price small {
  color: var(--seconds-result-label);
  font-size: 13px;
  font-weight: 400;
  line-height: 19px;
}

.seconds-settlement-card__price strong {
  color: var(--seconds-result-text);
  font-family: var(--font-geist-mono), var(--data-font);
  font-size: 17px;
  font-variant-numeric: tabular-nums;
  font-weight: 650;
  line-height: 22px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-settlement-card__price--settled strong {
  color: var(--seconds-result-tone);
}

.seconds-settlement-card__price-arrow {
  align-items: center;
  background: var(--seconds-result-arrow-surface);
  border-radius: 50%;
  color: var(--seconds-result-arrow-icon);
  display: inline-flex;
  height: 30px;
  justify-content: center;
  width: 30px;
}

.seconds-settlement-card__summary {
  align-items: center;
  background: var(--seconds-result-summary-surface);
  border-radius: 14px;
  box-sizing: border-box;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  height: 64px;
  margin: 0;
  min-width: 0;
  padding: 12px 14px;
}

.seconds-settlement-card__summary > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.seconds-settlement-card__summary > div:nth-child(2) {
  justify-items: end;
  justify-self: center;
}

.seconds-settlement-card__summary > div:last-child {
  justify-items: end;
  justify-self: end;
}

.seconds-settlement-card__summary dt {
  color: var(--seconds-result-label);
  font-size: 12px;
  font-weight: 400;
  line-height: 17px;
}

.seconds-settlement-card__summary dd {
  color: var(--seconds-result-text);
  font-size: 14px;
  font-weight: 650;
  line-height: 20px;
  margin: 0;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-settlement-card[data-direction='up'] .seconds-settlement-card__summary > div:nth-child(2) dd {
  color: var(--seconds-result-positive);
}

.seconds-settlement-card[data-direction='down'] .seconds-settlement-card__summary > div:nth-child(2) dd {
  color: var(--seconds-result-negative);
}

.seconds-settlement-card__note {
  align-items: center;
  background: var(--seconds-result-note-surface);
  border-radius: 12px;
  box-sizing: border-box;
  color: var(--seconds-result-label);
  display: flex;
  font-size: 13px;
  font-weight: 400;
  gap: 8px;
  line-height: 19px;
  margin: 0;
  min-height: 39px;
  min-width: 0;
  padding: 10px 12px;
}

.seconds-settlement-card__note svg {
  flex: 0 0 auto;
}

.seconds-settlement-card__note span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-settlement-card__actions {
  display: block;
  height: 52px;
  min-width: 0;
}

.seconds-settlement-card__history {
  align-items: center;
  background: var(--seconds-result-action);
  border: 0;
  border-radius: 14px;
  color: var(--seconds-result-action-text);
  display: inline-flex;
  font-size: 16px;
  font-weight: 700;
  gap: 7px;
  height: 52px;
  justify-content: center;
  min-height: 52px;
  padding: 0 14px;
  width: 100%;
}

.seconds-settlement-card__close:focus-visible,
.seconds-settlement-card__history:focus-visible {
  box-shadow: 0 0 0 3px var(--seconds-result-focus-ring);
  outline: 2px solid var(--seconds-result-focus);
  outline-offset: 2px;
}

.seconds-result-reveal-enter-active,
.seconds-result-reveal-leave-active {
  transition: opacity .2s cubic-bezier(.32, .72, 0, 1);
}

.seconds-result-reveal-enter-active .seconds-settlement-card,
.seconds-result-reveal-leave-active .seconds-settlement-card {
  transition:
    opacity .2s cubic-bezier(.32, .72, 0, 1),
    transform .24s cubic-bezier(.32, .72, 0, 1);
}

.seconds-result-reveal-enter-from,
.seconds-result-reveal-leave-to {
  opacity: 0;
}

.seconds-result-reveal-enter-from .seconds-settlement-card,
.seconds-result-reveal-leave-to .seconds-settlement-card {
  opacity: 0;
  transform: translateY(-1.5px) scale(.98);
}

.seconds-mask {
  --page: var(--background);
  --surface-2: var(--soft);
  --text: var(--ink);
  --green: var(--accent);
  --cyan: var(--focus);
  --coral: var(--negative);
  align-items: flex-end;
  background: var(--overlay);
  box-sizing: border-box;
  color: var(--text);
  display: flex;
  height: 100dvh;
  inset: 0;
  justify-content: center;
  max-width: 100%;
  padding:
    max(16px, env(safe-area-inset-top))
    max(16px, env(safe-area-inset-right))
    max(16px, env(safe-area-inset-bottom))
    max(16px, env(safe-area-inset-left));
  position: fixed;
  width: 100%;
  z-index: 90;
}

.seconds-dialog {
  background: var(--surface);
  border: 1px solid var(--line-strong);
  border-radius: 16px;
  box-shadow: var(--shadow-soft);
  box-sizing: border-box;
  display: grid;
  gap: 15px;
  grid-template-rows: auto minmax(0, 1fr) auto;
  max-height: calc(100dvh - max(16px, env(safe-area-inset-top)) - max(16px, env(safe-area-inset-bottom)));
  max-width: 520px;
  overflow: hidden;
  overscroll-behavior: auto;
  padding: 17px;
  width: 100%;
}

.seconds-dialog__body {
  align-content: start;
  display: grid;
  gap: 15px;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
}

.seconds-dialog header {
  align-items: center;
  display: grid;
  gap: 12px;
  grid-template-columns: auto minmax(0, 1fr) 44px;
  min-width: 0;
}

.seconds-dialog header > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.seconds-dialog header strong {
  font-size: 18px;
}

.seconds-dialog header small {
  color: var(--muted);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.seconds-dialog header .icon-button {
  height: 44px;
  min-height: 44px;
  min-width: 44px;
  width: 44px;
}

.seconds-dialog__body > #seconds-confirm-summary {
  border-block: 1px solid var(--line);
  color: var(--muted);
  font-size: 11px;
  line-height: 1.5;
  margin: 0;
  overflow-wrap: anywhere;
  padding: 12px 0;
}

.seconds-dialog dl {
  border-block: 1px solid var(--line);
  display: grid;
  margin: 0;
}

.seconds-dialog dl > div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 44px;
}

.seconds-dialog dl > div:last-child {
  border-bottom: 0;
}

.seconds-dialog dt,
.seconds-dialog dd {
  font-size: 12px;
  margin: 0;
}

.seconds-dialog dt {
  color: var(--muted);
}

.seconds-dialog dd {
  font-variant-numeric: tabular-nums;
  font-weight: 750;
  max-width: 64%;
  overflow-wrap: anywhere;
  text-align: right;
}

.dialog-feedback {
  background: var(--negative-soft);
  border-left: 3px solid var(--negative);
  color: var(--negative);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
  padding: 8px 10px;
}

.dialog-actions {
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, .8fr) minmax(0, 1.2fr);
}

.dialog-actions .button {
  min-height: 48px;
  padding-inline: 10px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 340px) {
  .seconds-pair-picker {
    padding-inline: 16px;
  }

  .seconds-pair-picker__row {
    gap: 9px;
    padding-inline: 12px;
  }

  .seconds-pair-picker__price {
    font-size: 12px;
  }

  .seconds-settlement-card__amount {
    font-size: 32px;
  }

  .seconds-settlement-card__note {
    font-size: 12px;
  }

  .seconds-price-panel > strong {
    font-size: 29px;
  }

  .seconds-price-meta {
    font-size: 9px;
    gap: 5px;
    padding-right: 70px;
  }

  .seconds-live-state {
    font-size: 9px;
    max-width: 68px;
  }

  .seconds-cycle-limit > b {
    font-size: 8px;
  }

  .seconds-direction-grid button {
    font-size: 11px;
  }

  .seconds-active-order__identity {
    gap: 4px;
  }

  .seconds-active-order__identity > span {
    padding-inline: 4px;
  }

  .dialog-actions {
    grid-template-columns: 1fr;
  }
}

@media (max-height: 640px) {
  .seconds-pair-picker {
    height: calc(100dvh - 32px);
    max-height: none;
  }
}

@media (max-height: 600px) {
  .seconds-settlement-card {
    margin-block: 0;
    transform: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .seconds-page *,
  .seconds-mask *,
  .seconds-pair-picker-layer *,
  .seconds-settlement-layer *,
  .seconds-page *::before,
  .seconds-mask *::before,
  .seconds-pair-picker-layer *::before,
  .seconds-settlement-layer *::before,
  .seconds-page *::after,
  .seconds-mask *::after,
  .seconds-pair-picker-layer *::after,
  .seconds-settlement-layer *::after {
    animation-duration: .01ms !important;
    animation-iteration-count: 1 !important;
    scroll-behavior: auto !important;
    transition-duration: .01ms !important;
  }

  .seconds-page button:active,
  .seconds-mask button:active,
  .seconds-pair-picker-layer button:active,
  .seconds-settlement-layer button:active {
    transform: none;
  }

  .seconds-result-reveal-enter-active,
  .seconds-result-reveal-leave-active,
  .seconds-pair-picker-reveal-enter-active,
  .seconds-pair-picker-reveal-leave-active {
    transition: none !important;
  }

  .seconds-pair-picker-reveal-enter-from,
  .seconds-pair-picker-reveal-leave-to {
    opacity: 1;
  }

  .seconds-pair-picker-reveal-enter-from .seconds-pair-picker,
  .seconds-pair-picker-reveal-leave-to .seconds-pair-picker {
    transform: none;
  }

  .seconds-result-reveal-enter-from,
  .seconds-result-reveal-leave-to {
    opacity: 1;
  }

  .seconds-result-reveal-enter-from .seconds-settlement-card,
  .seconds-result-reveal-leave-to .seconds-settlement-card {
    opacity: 1;
    transform: translateY(-13.5px);
  }

  .spin {
    animation: none;
  }
}
</style>
