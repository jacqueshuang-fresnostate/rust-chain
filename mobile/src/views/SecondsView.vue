<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  CircleAlert,
  History,
  LoaderCircle,
  RefreshCw,
  X,
} from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchKlines } from '@/api/market'
import { createMarketDetailStreamSession } from '@/api/marketDetailStream'
import { subscribeTickers } from '@/api/marketSocket'
import { normalizeMarketSocketSymbol } from '@/api/marketSocketProtocol'
import {
  fetchSecondsOrders,
  fetchSecondsProducts,
  openSecondsOrder,
  type SecondsCycle,
  type SecondsOrder,
  type SecondsProduct,
} from '@/api/seconds'
import { fetchWalletAccounts } from '@/api/wallet'
import { publicMarketWebSocketUrl } from '@/config/app'
import { formatAmount, formatPrice } from '@/core/format'
import {
  createBottomNavSecondsFallbackTarget,
  isBottomNavigationSecondsEntry,
} from '@/core/navigation'
import {
  activeSecondsOrders,
  mergeSecondsOrderReconciliation,
  secondsOrderEstimatedProfit,
  secondsOrderProgress,
  secondsOrderRemainingMs,
  secondsOrderStatusPresentation,
  upsertSecondsOrder,
} from '@/core/secondsOrder'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'
import type { KlinePoint, WalletAccount } from '@/core/types'

const session = useSessionStore()
const marketStore = useMarketStore()
const router = useRouter()
const { t } = useI18n()
const products = ref<SecondsProduct[]>([])
const orders = ref<SecondsOrder[]>([])
const accounts = ref<WalletAccount[]>([])
const sparklinePoints = ref<KlinePoint[]>([])
const liveTickerPrices = ref<Record<string, number>>({})
const selected = ref<SecondsProduct | null>(null)
const selectedCycleId = ref(0)
const direction = ref<'up' | 'down'>('up')
const amount = ref('')
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const success = ref('')
const refreshWarning = ref('')
const confirmOpen = ref(false)
const confirmDialog = ref<HTMLElement | null>(null)
const reviewButton = ref<HTMLButtonElement | null>(null)
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
let componentActive = true
const committedOrdersById = new Map<number, SecondsOrder>()
const expiryRetryAtByOrderId = new Map<number, number>()
const queuedExpiryOrderIds = new Set<number>()
const reconcilingExpiryOrderIds = new Set<number>()
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
const amountNumber = computed(() => Number(amount.value || 0))
const payoutRate = computed(() => cycle.value?.payoutRate || 0)
const estimatedProfit = computed(() => (
  Number.isFinite(amountNumber.value) && amountNumber.value > 0
    ? amountNumber.value * payoutRate.value
    : 0
))
const valid = computed(() => Boolean(
  cycle.value
  && Number.isFinite(amountNumber.value)
  && amountNumber.value >= cycle.value.minStake
  && (!cycle.value.maxStake || amountNumber.value <= cycle.value.maxStake)
  && amountNumber.value <= (account.value?.available || 0),
))
const quickAmounts = computed(() => {
  const activeCycle = cycle.value
  if (!activeCycle) return []
  const upperBound = Math.min(
    activeCycle.maxStake || Number.POSITIVE_INFINITY,
    account.value?.available || 0,
  )
  return [...new Set([
    activeCycle.minStake,
    activeCycle.minStake * 2,
    activeCycle.minStake * 5,
    upperBound,
  ])]
    .filter((value) => Number.isFinite(value) && value >= activeCycle.minStake && value <= upperBound)
    .slice(0, 4)
})
const quickAmountSlots = computed(() => (
  quickAmounts.value.length ? quickAmounts.value : [0, 0, 0, 0]
))
const homeFallback = createBottomNavSecondsFallbackTarget()
const preferHomeFallback = computed(() => {
  void router.currentRoute.value.fullPath
  return isBottomNavigationSecondsEntry(router.options.history.state)
})

function normalizeProductSymbol(value: string): string {
  return normalizeMarketSocketSymbol(value)
}

function countdownLabel(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.ceil(milliseconds / 1000))
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
}

function latestPriceForSymbol(symbol: string): number {
  const normalized = normalizeProductSymbol(symbol)
  const livePrice = liveTickerPrices.value[normalized]
  if (Number.isFinite(livePrice) && livePrice > 0) return livePrice
  if (normalized === normalizeProductSymbol(selected.value?.symbol || '')) {
    const candlePrice = sparklinePoints.value.at(-1)?.close
    if (Number.isFinite(candlePrice) && Number(candlePrice) > 0) return Number(candlePrice)
  }
  return marketStore.tickerFor(symbol)?.lastPrice || 0
}

const selectedLatestPrice = computed(() => latestPriceForSymbol(selected.value?.symbol || ''))

function orderCountdown(order: SecondsOrder): string {
  return countdownLabel(secondsOrderRemainingMs(order, currentTime.value))
}

function orderProgress(order: SecondsOrder): number {
  return secondsOrderProgress(order, currentTime.value)
}

function orderEstimatedProfit(order: SecondsOrder): number {
  return secondsOrderEstimatedProfit(order)
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
  liveTickerPrices.value = Object.fromEntries(
    Object.entries(liveTickerPrices.value).filter(([symbol]) => acceptedSymbols.has(symbol)),
  )
  if (!normalizedSymbols.length) return

  stopTickerSubscription = subscribeTickers(normalizedSymbols, (update) => {
    if (
      !componentActive
      || generation !== tickerSubscriptionGeneration
      || !acceptedSymbols.has(update.symbol)
      || update.lastPrice <= 0
    ) {
      return
    }
    liveTickerPrices.value = {
      ...liveTickerPrices.value,
      [update.symbol]: update.lastPrice,
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
  const lineColor = styles.getPropertyValue('--line').trim()
  const positiveColor = styles.getPropertyValue('--positive').trim()
  context.strokeStyle = lineColor
  context.lineWidth = 1
  for (const y of [34, 68, 102, 136]) {
    context.beginPath()
    context.moveTo(0, y + 0.5)
    context.lineTo(width, y + 0.5)
    context.stroke()
  }

  const closes = sparklinePoints.value.map((point) => point.close).filter((value) => Number.isFinite(value) && value > 0)
  if (closes.length < 2) return
  const minimum = Math.min(...closes)
  const maximum = Math.max(...closes)
  const range = maximum - minimum || 1
  const horizontalPadding = 2
  const verticalPadding = 14
  context.strokeStyle = positiveColor
  context.lineWidth = 1.5
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
  context.fillStyle = positiveColor
  context.beginPath()
  context.arc(width - horizontalPadding, lastY, 4, 0, Math.PI * 2)
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

function openHistory(): void {
  void router.push({ name: 'seconds-history' })
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
      if (!amount.value) amount.value = String(cycle.value?.minStake || '')
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
      orders.value = []
      accounts.value = []
      committedOrdersById.clear()
      expiryRetryAtByOrderId.clear()
      queuedExpiryOrderIds.clear()
      reconcilingExpiryOrderIds.clear()
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
    if (fullyLoaded && !activeIds.has(orderId)) expiryRetryAtByOrderId.delete(orderId)
    else expiryRetryAtByOrderId.set(orderId, retryAt)
  }
  refreshWarning.value = fullyLoaded ? '' : t('seconds.refreshAfterOrderFailed')
}

function selectProduct(product: SecondsProduct): void {
  selected.value = product
  selectedCycleId.value = product.cycles[0]?.id || 0
  direction.value = 'up'
  amount.value = String(product.cycles[0]?.minStake || '')
  error.value = ''
  success.value = ''
  void loadSparkline(product.symbol)
}

function selectProductFromEvent(event: Event): void {
  const productId = Number((event.target as HTMLSelectElement).value)
  const product = products.value.find((item) => item.id === productId)
  if (product) selectProduct(product)
}

function selectCycle(cycleId: number): void {
  selectedCycleId.value = cycleId
  amount.value = String(cycle.value?.minStake || '')
  error.value = ''
  success.value = ''
}

function setDirection(nextDirection: 'up' | 'down'): void {
  direction.value = nextDirection
  error.value = ''
  success.value = ''
}

function setAmount(value: string | number): void {
  amount.value = String(value)
  error.value = ''
  success.value = ''
}

function reviewOrder(): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/seconds' } })
    return
  }
  if (!selected.value || !cycle.value || !valid.value) {
    error.value = t('seconds.invalidAmount')
    return
  }
  error.value = ''
  confirmOpen.value = true
}

function closeConfirm(): void {
  if (submitting.value) return
  confirmOpen.value = false
}

async function submit(): Promise<void> {
  if (submitting.value) return
  if (!session.isAuthenticated) {
    error.value = t('seconds.loginDescription')
    return
  }
  if (!selected.value || !cycle.value || !valid.value) {
    error.value = t('seconds.invalidAmount')
    return
  }
  submitting.value = true
  error.value = ''
  refreshWarning.value = ''
  let openedOrder: SecondsOrder | null = null
  try {
    openedOrder = await openSecondsOrder({
      productId: selected.value.id,
      durationSeconds: cycle.value.durationSeconds,
      direction: direction.value,
      stakeAmount: amountNumber.value,
    })
    if (!componentActive) return
    committedOrdersById.set(openedOrder.id, openedOrder)
    orders.value = upsertSecondsOrder(orders.value, openedOrder)
    amount.value = ''
    success.value = t('seconds.created')
    confirmOpen.value = false
    replaceTickerSubscription()
  } catch (reason) {
    if (componentActive) error.value = apiErrorMessage(reason, t('seconds.orderFailed'))
  } finally {
    if (componentActive) submitting.value = false
  }
  if (openedOrder && componentActive) void reconcileOpenedOrder()
}

async function reconcileOpenedOrder(): Promise<void> {
  const reconciliation = await reconcilePrivateState()
  if (
    componentActive
    && (!reconciliation.ordersLoaded || !reconciliation.accountsLoaded)
  ) {
    refreshWarning.value = t('seconds.refreshAfterOrderFailed')
  }
}

function orderStatusLabel(order: SecondsOrder): string {
  const presentation = secondsOrderStatusPresentation(order)
  return presentation.translationKey ? t(presentation.translationKey) : presentation.source
}

function highestRate(product: SecondsProduct): string {
  const highest = Math.max(0, ...product.cycles.map((item) => item.payoutRate * 100))
  return highest.toFixed(0)
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
  secondsKlineSession.stop()
  stopTickerSubscription?.()
  stopTickerSubscription = null
  tickerSubscriptionKey = ''
  queuedExpiryOrderIds.clear()
  reconcilingExpiryOrderIds.clear()
  expiryRetryAtByOrderId.clear()
  committedOrdersById.clear()
  if (clockTimer) clearInterval(clockTimer)
  chartResizeObserver?.disconnect()
  chartThemeObserver?.disconnect()
  document.body.style.overflow = previousBodyOverflow
})
</script>

<template>
  <main
    class="page page--plain seconds-page"
    data-pencil-source="VL8er g9agt Lpt6q WxeB8"
    data-instrument-hero="pair-price"
  >
    <PageHeader
      :back="true"
      :eyebrow="t('seconds.scene')"
      :fallback="homeFallback"
      :pencil="true"
      :prefer-fallback="preferHomeFallback"
      :title="selected?.symbol || t('seconds.title')"
      :subtitle="t('seconds.context')"
    >
      <template #center>
        <label class="field seconds-pair-field">
          <span class="sr-only">{{ t('marketDetail.market') }}</span>
          <span class="seconds-select-shell">
            <select
              :value="selected?.id || ''"
              :disabled="loading || !products.length"
              :aria-label="t('marketDetail.market')"
              @change="selectProductFromEvent"
            >
              <option v-if="!products.length" value="">{{ loading ? t('seconds.loading') : t('seconds.noProducts') }}</option>
              <option v-for="product in products" :key="product.id" :value="product.id">
                {{ product.symbol }}
              </option>
            </select>
            <small v-if="selected" :data-highest-rate="highestRate(selected)">{{ t('seconds.title') }}</small>
          </span>
        </label>
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
          class="seconds-market-board"
          :data-seconds-market="selected ? 'live' : loading ? 'loading' : 'empty'"
          :aria-busy="loading || marketStore.loading"
        >
          <div class="seconds-price-row">
            <strong class="numeric">{{ selectedLatestPrice > 0 ? formatPrice(selectedLatestPrice) : '--' }}</strong>
            <span class="numeric">
              {{ cycle ? t('seconds.duration', { seconds: cycle.durationSeconds }) : '--' }}
              · {{ cycle ? `${(payoutRate * 100).toFixed(2)}%` : '--' }}
            </span>
          </div>

          <div class="seconds-micro-chart" :data-chart-state="sparklinePoints.length ? 'ready' : 'empty'">
            <canvas ref="sparklineCanvas" :aria-label="t('seconds.referencePrice')" />
            <span v-if="!sparklinePoints.length" role="status">
              {{ loading ? t('common.loading') : t('common.marketUnavailable') }}
            </span>
          </div>
        </section>

        <section
          v-if="activeOrders.length"
          class="seconds-active-orders"
          data-active-order-list="all"
          :aria-label="t('seconds.activeOrders')"
        >
          <article
            v-for="order in activeOrders"
            :key="order.id"
            class="seconds-active-order"
            data-active-order="real"
            :data-active-order-id="order.id"
          >
            <header>
              <span :class="order.direction">
                <ArrowUp v-if="order.direction === 'up'" :size="12" aria-hidden="true" />
                <ArrowDown v-else :size="12" aria-hidden="true" />
                {{ order.symbol }} · {{ t(order.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}
              </span>
              <b class="numeric">{{ t('seconds.duration', { seconds: order.durationSeconds }) }}</b>
              <strong class="numeric">{{ orderStatusLabel(order) }} {{ orderCountdown(order) }}</strong>
            </header>
            <div class="seconds-active-progress" aria-hidden="true">
              <i :style="{ width: `${orderProgress(order)}%` }" />
            </div>
            <dl>
              <div>
                <dt>{{ t('orders.entryPrice') }}</dt>
                <dd class="numeric">{{ order.entryPrice !== undefined ? formatPrice(order.entryPrice) : '--' }}</dd>
              </div>
              <div>
                <dt>{{ t('marketDetail.latestPrice') }}</dt>
                <dd class="positive numeric">
                  {{ latestPriceForSymbol(order.symbol) > 0 ? formatPrice(latestPriceForSymbol(order.symbol)) : '--' }}
                </dd>
              </div>
              <div>
                <dt>{{ t('seconds.stakeAmount') }}</dt>
                <dd class="numeric">{{ formatAmount(order.stakeAmount) }} {{ order.stakeAssetSymbol }}</dd>
              </div>
              <div>
                <dt>{{ t('seconds.estimatedProfit') }}</dt>
                <dd class="positive numeric">
                  +{{ formatAmount(orderEstimatedProfit(order)) }} {{ order.stakeAssetSymbol }}
                </dd>
              </div>
            </dl>
          </article>
        </section>

        <section
          class="instrument-plate seconds-order-console"
          data-instrument-plate="market-and-order"
        >
          <section class="seconds-control-group" aria-labelledby="seconds-direction-label">
            <div class="seconds-control-label">
              <span id="seconds-direction-label">{{ t('seconds.direction') }}</span>
            </div>
            <div class="seconds-direction-grid" role="group" :aria-label="t('seconds.direction')">
              <button
                type="button"
                class="up"
                :class="{ active: direction === 'up' }"
                :aria-pressed="direction === 'up'"
                :disabled="loading || !selected"
                @click="setDirection('up')"
              >
                <ArrowUp :size="16" aria-hidden="true" />
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
                <ArrowDown :size="16" aria-hidden="true" />
                <span>{{ t('seconds.bearish') }}</span>
              </button>
            </div>
          </section>

          <section class="seconds-control-group" aria-labelledby="seconds-duration-label">
            <div class="seconds-control-label">
              <span id="seconds-duration-label">{{ t('seconds.term') }}</span>
            </div>
            <div class="seconds-duration-grid" role="group" :aria-label="t('seconds.term')">
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
          </section>

          <label
            class="field seconds-amount-field"
            :data-field-state="amount && !valid ? 'invalid' : amount && valid ? 'complete' : 'idle'"
          >
            <span>{{ t('seconds.stakeAmount') }}</span>
            <div>
              <input
                v-model="amount"
                class="numeric"
                inputmode="decimal"
                :disabled="loading || !selected"
                :aria-invalid="Boolean(amount) && !valid"
                @input="setAmount(amount)"
              />
              <b>{{ selected?.stakeAssetSymbol || '--' }}</b>
            </div>
          </label>

          <div class="seconds-amount-presets" role="group" :aria-label="t('seconds.stakeAmount')">
            <button
              v-for="(value, index) in quickAmountSlots"
              :key="`${value}-${index}`"
              type="button"
              :aria-pressed="value > 0 && amountNumber === value"
              :disabled="loading || !selected || value <= 0"
              @click="setAmount(value)"
            >
              {{ value > 0 ? formatAmount(value) : '--' }}
            </button>
          </div>

          <dl class="seconds-order-summary">
            <div>
              <dt>{{ t('swap.spotWalletNote') }}</dt>
              <dd class="numeric">
                {{ selected && session.isAuthenticated && account
                  ? `${formatAmount(account.available)} ${selected.stakeAssetSymbol}`
                  : '--' }}
              </dd>
            </div>
          </dl>

          <div class="seconds-feedback" aria-live="polite">
            <div v-if="error" class="seconds-message seconds-message--error" role="alert">
              <CircleAlert :size="16" aria-hidden="true" />
              <span>{{ error }}</span>
              <button type="button" :aria-label="t('common.retry')" @click="load"><RefreshCw :size="16" /></button>
            </div>
            <div v-if="success" class="seconds-message seconds-message--success" data-session-feedback="created" role="status">
              <CheckCircle2 :size="16" aria-hidden="true" />
              <span>{{ success }}</span>
            </div>
            <div v-if="refreshWarning" class="seconds-message seconds-message--warning" role="status">
              <RefreshCw :size="16" aria-hidden="true" />
              <span>{{ refreshWarning }}</span>
            </div>
            <span v-if="loading && !error && !success && !refreshWarning"><LoaderCircle :size="15" class="spin" />{{ t('seconds.loading') }}</span>
            <span v-else-if="!session.isAuthenticated && !error && !success && !refreshWarning">{{ t('seconds.loginDescription') }}</span>
          </div>

          <button
            ref="reviewButton"
            class="button button--primary button--full seconds-submit"
            type="button"
            :disabled="submitting || loading || !selected"
            @click="reviewOrder"
          >
            {{ submitting ? t('common.submitting') : t('seconds.confirmOrder') }}
          </button>

          <p class="seconds-risk-note">
            <CircleAlert :size="14" aria-hidden="true" />
            <span>{{ t('seconds.introDescription') }}</span>
          </p>
        </section>

      </section>
    </div>

    <Teleport to="body">
      <div v-if="confirmOpen && selected && cycle" class="confirmation-layer seconds-mask" @click.self="closeConfirm">
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
              <small>{{ selected.symbol }} · {{ t('seconds.settledIn', { asset: selected.stakeAssetSymbol }) }}</small>
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
              {{ selected.symbol }} · {{ t(direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }} ·
              {{ formatAmount(amountNumber) }} {{ selected.stakeAssetSymbol }}
            </p>

            <dl class="confirmation-detail">
              <div><dt>{{ t('seconds.direction') }}</dt><dd>{{ t(direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}</dd></div>
              <div><dt>{{ t('seconds.term') }}</dt><dd>{{ t('seconds.duration', { seconds: cycle.durationSeconds }) }}</dd></div>
              <div><dt>{{ t('seconds.stakeAmount') }}</dt><dd>{{ formatAmount(amountNumber) }} {{ selected.stakeAssetSymbol }}</dd></div>
              <div>
                <dt>{{ t('seconds.payoutRate') }}</dt>
                <dd>{{ (cycle.payoutRate * 100).toFixed(2) }}% · +{{ formatAmount(estimatedProfit) }} {{ selected.stakeAssetSymbol }}</dd>
              </div>
              <div><dt>{{ t('marketDetail.latestPrice') }}</dt><dd>{{ selectedLatestPrice > 0 ? formatPrice(selectedLatestPrice) : '--' }}</dd></div>
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
  </main>
</template>

<style scoped>
.seconds-page {
  background: var(--page);
  color: var(--text);
  min-width: 0;
  overflow-x: clip;
  position: relative;
}

.seconds-content {
  min-width: 0;
  padding: 0 0 calc(24px + env(safe-area-inset-bottom));
}

.seconds-workspace,
.seconds-market-board,
.seconds-order-console {
  min-width: 0;
}

.seconds-workspace {
  display: grid;
  gap: 0;
  width: 100%;
}

.seconds-pair-field {
  display: grid;
  justify-self: center;
  max-width: 260px;
  min-width: 0;
  width: 100%;
}

.seconds-select-shell {
  align-items: center;
  background: var(--surface);
  border: 1px solid transparent;
  border-radius: 12px;
  box-sizing: border-box;
  display: grid;
  gap: 6px;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 44px;
  justify-content: center;
  min-height: 44px;
  min-width: 0;
  padding: 0 6px;
  width: 100%;
}

.seconds-select-shell:focus-within {
  border-color: var(--focus);
  box-shadow: inset 0 0 0 1px var(--focus);
}

.seconds-select-shell select {
  background: transparent;
  border: 0;
  box-shadow: none;
  color: var(--text);
  font-size: 15px;
  font-weight: 750;
  height: 42px;
  min-height: 42px;
  min-width: 0;
  outline: 0;
  padding: 0 2px 0 0;
  text-align: right;
  width: 100%;
}

.seconds-select-shell small {
  background: var(--positive-soft);
  border-radius: 50%;
  color: var(--positive);
  font-size: 9px;
  font-weight: 650;
  line-height: 20px;
  padding: 0 7px;
  white-space: nowrap;
}

.seconds-select-shell select:focus,
.seconds-select-shell select:focus-visible {
  border: 0;
  box-shadow: none;
  outline: 0;
}

.seconds-select-shell:has(select:disabled) {
  opacity: .64;
}

.seconds-market-board {
  background: var(--page);
  border: 0;
  display: grid;
  gap: 6px;
  overflow: hidden;
  padding: 4px 20px 0;
}

.seconds-price-row {
  align-items: baseline;
  display: flex;
  gap: 10px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-price-row strong {
  color: var(--text);
  font-size: 34px;
  font-weight: 750;
  letter-spacing: -.8px;
  line-height: 42px;
  min-width: 0;
  overflow-wrap: anywhere;
}

.seconds-price-row span {
  color: var(--positive);
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 650;
  line-height: 16px;
  text-align: right;
}

.seconds-micro-chart {
  height: 170px;
  min-width: 0;
  position: relative;
}

.seconds-micro-chart canvas {
  display: block;
  height: 170px;
  width: 100%;
}

.seconds-micro-chart > span {
  color: var(--muted);
  font-size: 11px;
  left: 50%;
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  white-space: nowrap;
}

.seconds-active-orders {
  display: grid;
  gap: 10px;
  margin: 12px 20px 0;
  min-width: 0;
}

.seconds-active-order {
  background: var(--surface);
  border: 1px solid var(--positive);
  border-radius: 14px;
  display: grid;
  gap: 10px;
  margin: 0;
  min-width: 0;
  padding: 12px 14px;
}

.seconds-active-order header {
  align-items: center;
  display: flex;
  gap: 8px;
  min-width: 0;
}

.seconds-active-order header > span {
  align-items: center;
  background: var(--positive);
  border-radius: 11px;
  color: var(--on-accent);
  display: inline-flex;
  font-size: 10px;
  font-weight: 750;
  gap: 4px;
  height: 22px;
  padding: 0 10px;
}

.seconds-active-order header > span.down {
  background: var(--negative);
}

.seconds-active-order header b,
.seconds-active-order header strong {
  font-size: 10px;
}

.seconds-active-order header strong {
  color: var(--positive);
  margin-left: auto;
  text-align: right;
}

.seconds-active-progress {
  background: var(--positive-soft);
  border-radius: 3px;
  height: 6px;
  overflow: hidden;
}

.seconds-active-progress i {
  background: var(--positive);
  border-radius: inherit;
  display: block;
  height: 100%;
  transition: width .2s linear;
}

.seconds-active-order dl {
  display: grid;
  gap: 6px;
  margin: 0;
}

.seconds-active-order dl > div {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-active-order dt,
.seconds-active-order dd {
  font-size: 10px;
  line-height: 15px;
  margin: 0;
}

.seconds-active-order dt {
  color: var(--muted);
}

.seconds-active-order dd {
  color: var(--text);
  font-weight: 650;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.seconds-active-order dd.positive {
  color: var(--positive);
}

.seconds-order-console {
  background: var(--page);
  border: 0;
  border-radius: 0;
  box-shadow: none;
  display: grid;
  gap: 12px;
  padding: 12px 20px 20px;
}

.seconds-control-group,
.seconds-amount-field {
  display: grid;
  gap: 8px;
  min-width: 0;
}

.seconds-control-label,
.seconds-amount-field > span {
  color: var(--text);
  font-size: 11px;
  font-weight: 650;
  line-height: 16px;
}

.seconds-direction-grid,
.seconds-duration-grid {
  display: grid;
  min-width: 0;
}

.seconds-direction-grid {
  gap: 10px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.seconds-direction-grid button {
  min-height: 52px;
  align-items: center;
  background: var(--positive);
  border: 1px solid transparent;
  border-radius: 12px;
  color: var(--on-accent);
  display: inline-flex;
  font-size: 14px;
  font-weight: 750;
  gap: 6px;
  justify-content: center;
  min-width: 0;
}

.seconds-direction-grid button.down {
  background: var(--negative-soft);
  color: var(--negative);
}

.seconds-direction-grid button:not(.active) {
  opacity: .72;
}

.seconds-direction-grid button:disabled {
  cursor: default;
}

.seconds-duration-grid {
  gap: 8px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.seconds-duration-grid button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line-strong);
  border-radius: 18px;
  color: var(--text);
  display: inline-flex;
  font-size: 11px;
  font-weight: 650;
  height: 36px;
  justify-content: center;
  min-height: 36px;
  min-width: 0;
  padding: 0 6px;
}

.seconds-duration-grid button.active {
  background: var(--positive-soft);
  border-color: transparent;
  color: var(--positive);
}

.seconds-amount-field {
  gap: 0;
  height: 52px;
  min-height: 52px;
  padding: 0;
  position: relative;
}

.seconds-amount-field > span {
  left: 0;
  position: absolute;
  top: 2px;
  z-index: 1;
}

.seconds-page .seconds-order-console .seconds-amount-field > div {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 1px solid var(--line);
  box-shadow: none;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 52px;
  min-height: 52px;
  min-width: 0;
  padding: 13px 0 0;
}

.seconds-page .seconds-order-console .seconds-amount-field:focus-within > div {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.seconds-page .seconds-order-console .seconds-amount-field[data-field-state="invalid"] > div {
  border-color: var(--negative);
}

.seconds-page .seconds-order-console .seconds-amount-field input {
  background: transparent;
  border: 0;
  box-shadow: none;
  color: var(--text);
  font-size: 22px;
  font-weight: 750;
  height: 38px;
  min-height: 38px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.seconds-page .seconds-order-console .seconds-amount-field input:focus,
.seconds-page .seconds-order-console .seconds-amount-field input:focus-visible {
  border: 0;
  box-shadow: none;
  outline: 0;
}

.seconds-amount-field b {
  font-size: 12px;
  overflow-wrap: anywhere;
}

.seconds-amount-presets {
  display: none;
}

.seconds-order-summary {
  margin: -4px 0 0;
  min-width: 0;
}

.seconds-order-summary > div {
  align-items: baseline;
  display: flex;
  gap: 6px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-order-summary dt,
.seconds-order-summary dd {
  color: var(--muted);
  font-size: 10px;
  line-height: 16px;
  margin: 0;
}

.seconds-order-summary dd {
  color: var(--text);
  font-weight: 650;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.seconds-feedback {
  display: grid;
  gap: 6px;
  min-height: 0;
  min-width: 0;
}

.seconds-feedback > span {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 10px;
  gap: 7px;
  line-height: 16px;
}

.seconds-message {
  align-items: center;
  border: 1px solid currentColor;
  border-radius: 10px;
  display: grid;
  font-size: 11px;
  gap: 9px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  line-height: 16px;
  min-height: 44px;
  min-width: 0;
  padding: 3px 4px 3px 11px;
}

.seconds-message--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.seconds-message--success {
  background: var(--positive-soft);
  color: var(--positive);
  grid-template-columns: auto minmax(0, 1fr);
}

.seconds-message--warning {
  background: var(--accent-soft);
  color: var(--accent);
  grid-template-columns: auto minmax(0, 1fr);
}

.seconds-message button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 36px;
  min-width: 36px;
  place-items: center;
}

.seconds-submit {
  border-radius: 26px;
  min-height: 52px;
  width: 100%;
}

.seconds-submit:disabled {
  background: var(--positive-soft);
  color: var(--positive);
  opacity: 1;
}

.seconds-risk-note {
  align-items: flex-start;
  background: var(--negative-soft);
  border-radius: 10px;
  color: var(--negative);
  display: flex;
  font-size: 11px;
  font-weight: 500;
  gap: 8px;
  line-height: 16px;
  margin: 0;
  padding: 10px 12px;
}

.seconds-risk-note svg {
  flex: 0 0 auto;
  margin-top: 1px;
}

.seconds-duration-grid button:focus-visible,
.seconds-direction-grid button:focus-visible,
.seconds-submit:focus-visible,
.seconds-message button:focus-visible,
.seconds-dialog button:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: 0;
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
  .seconds-market-board,
  .seconds-order-console {
    padding-inline: 16px;
  }

  .seconds-active-orders {
    margin-inline: 16px;
  }

  .seconds-price-row strong {
    font-size: 30px;
  }

  .seconds-duration-grid {
    gap: 6px;
  }

  .seconds-duration-grid button {
    font-size: 10px;
    padding-inline: 3px;
  }

  .dialog-actions {
    grid-template-columns: 1fr;
  }
}

@media (prefers-reduced-motion: reduce) {
  .seconds-page *,
  .seconds-mask *,
  .seconds-page *::before,
  .seconds-mask *::before,
  .seconds-page *::after,
  .seconds-mask *::after {
    animation-duration: .01ms !important;
    animation-iteration-count: 1 !important;
    scroll-behavior: auto !important;
    transition-duration: .01ms !important;
  }

  .seconds-page button:active,
  .seconds-mask button:active {
    transform: none;
  }

  .spin {
    animation: none;
  }
}
</style>
