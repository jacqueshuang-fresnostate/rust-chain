<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowLeft,
  ArrowLeftRight,
  ChartNoAxesCombined,
  ChevronDown,
  CircleAlert,
  ClipboardList,
  Maximize2,
  Minimize2,
  RefreshCw,
  Share2,
  Star,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import MobileMarketChart from '@/components/MobileMarketChart.vue'
import OrderBookPanel from '@/components/OrderBookPanel.vue'
import { fetchKlines, fetchOrderBook, fetchRecentTrades } from '@/api/market'
import {
  createMarketDetailStreamSession,
  type MarketDetailStreamContext,
} from '@/api/marketDetailStream'
import {
  MARKET_KLINE_INTERVALS,
  mergeMarketTradeHistory,
  mergeMarketTrades,
  normalizeMarketKlineInterval,
  type MarketKlineInterval,
} from '@/api/marketSocketProtocol'
import { publicMarketWebSocketUrl } from '@/config/app'
import { formatAmount, formatPercent, formatPrice } from '@/core/format'
import { normalizeMarketChartPoints } from '@/core/marketChart'
import {
  calculateMarketMovingAverages,
  latestMarketMovingAverages,
} from '@/core/marketIndicators'
import { currentIntlLocale } from '@/i18n'
import { goBackOr } from '@/core/navigation'
import { useMarketStore } from '@/stores/market'
import { useMarketFavoritesStore } from '@/stores/marketFavorites'
import { useSessionStore } from '@/stores/session'
import type { KlinePoint, OrderBookLevel, TradePrint } from '@/core/types'

const props = defineProps<{ symbol: string }>()
const route = useRoute()
const router = useRouter()
const marketStore = useMarketStore()
const marketFavorites = useMarketFavoritesStore()
const session = useSessionStore()
const { t } = useI18n()
const interval = ref<MarketKlineInterval>('15m')
const loading = ref(true)
const chartLoading = ref(true)
const klineError = ref(false)
const depthError = ref(false)
const tradesError = ref(false)
const points = ref<KlinePoint[]>([])
const bids = ref<OrderBookLevel[]>([])
const asks = ref<OrderBookLevel[]>([])
const trades = ref<TradePrint[]>([])
const liveDetailActive = ref(false)
const liveDetailUpdatedAt = ref(0)
type MarketDataPanel = 'orderBook' | 'trades'
type MarketSection = 'chart' | 'overview'

interface ChartScrollLock {
  bodyOverflow: string
  bodyPosition: string
  bodyTop: string
  bodyWidth: string
  rootOverflow: string
  rootScrollBehavior: string
  scrollY: number
}

const marketDataPanel = ref<MarketDataPanel>('orderBook')
const activeSection = ref<MarketSection>('chart')
const chartExpanded = ref(false)
const summarySection = ref<HTMLElement | null>(null)
const chartSection = ref<HTMLElement | null>(null)
const orderBookTabButton = ref<HTMLButtonElement | null>(null)
const tradesTabButton = ref<HTMLButtonElement | null>(null)
const chartToggleButton = ref<HTMLButtonElement | null>(null)
let chartScrollLock: ChartScrollLock | null = null
let requestVersion = 0

const pairSymbol = computed(() => props.symbol.replace(/[_-]/g, '/').toUpperCase())
const ticker = computed(() => marketStore.tickerFor(pairSymbol.value))
const baseAsset = computed(() => pairSymbol.value.split('/')[0] || '')
const quoteAsset = computed(() => pairSymbol.value.split('/')[1] || 'USDT')
const isFavorite = computed(() => marketFavorites.isFavorite(pairSymbol.value))
const favoriteSaving = computed(() => marketFavorites.isPending(pairSymbol.value))
const dataError = computed(() => klineError.value || depthError.value || tradesError.value)
const normalizedChartPoints = computed(() => normalizeMarketChartPoints(points.value))
const movingAverages = computed(() => calculateMarketMovingAverages(normalizedChartPoints.value))
const latestAverages = computed(() => latestMarketMovingAverages(movingAverages.value))
const latestCandle = computed(() => normalizedChartPoints.value.at(-1))
const latestPrice = computed(() => (
  trades.value[0]?.price
  ?? latestCandle.value?.close
  ?? ticker.value?.lastPrice
  ?? 0
))
const hasLatestPrice = computed(() => Number.isFinite(latestPrice.value) && latestPrice.value > 0)
const latestChangePercent = computed(() => {
  const market = ticker.value
  if (!market) return 0
  if (market.openPrice <= 0 || latestPrice.value <= 0) return market.changePercent
  return ((latestPrice.value - market.openPrice) / market.openPrice) * 100
})
const observedAt = computed(() => liveDetailUpdatedAt.value || ticker.value?.observedAt || marketStore.updatedAt)

const detailStreamSession = createMarketDetailStreamSession({
  getUrl: publicMarketWebSocketUrl,
  onDepth: (_context, snapshot) => {
    liveDetailActive.value = true
    liveDetailUpdatedAt.value = Date.now()
    bids.value = snapshot.bids
    asks.value = snapshot.asks
    depthError.value = false
  },
  onTrade: (_context, trade) => {
    liveDetailActive.value = true
    liveDetailUpdatedAt.value = Date.now()
    trades.value = mergeMarketTrades(trades.value, trade, 16)
    tradesError.value = false
  },
  onKlines: (_context, nextPoints) => {
    liveDetailActive.value = true
    liveDetailUpdatedAt.value = Date.now()
    points.value = nextPoints
    klineError.value = false
    chartLoading.value = false
  },
})

function stopLiveDetail(): void {
  detailStreamSession.stop()
}

function isCurrentLiveDetail(state: MarketDetailStreamContext, version: number): boolean {
  return detailStreamSession.isCurrent(
    state,
    pairSymbol.value,
    interval.value,
    version,
  ) && version === requestVersion
}

function startLiveDetail(
  symbol: string,
  selectedInterval: string,
  version: number,
): MarketDetailStreamContext {
  liveDetailActive.value = false
  liveDetailUpdatedAt.value = 0
  const state = detailStreamSession.replace(symbol, selectedInterval, version)
  points.value = detailStreamSession.currentPoints()
  return state
}

async function load(forceMarket = false): Promise<void> {
  const version = ++requestVersion
  const symbol = pairSymbol.value
  const selectedInterval = interval.value
  loading.value = true
  chartLoading.value = true
  klineError.value = false
  depthError.value = false
  tradesError.value = false
  bids.value = []
  asks.value = []
  trades.value = []
  const liveState = startLiveDetail(symbol, selectedInterval, version)
  const klineRequest = detailStreamSession.beginKlineRequest(liveState)
  void marketStore.refresh(forceMarket)
  const [klineResult, depthResult, tradesResult] = await Promise.allSettled([
    fetchKlines(pairSymbol.value, interval.value),
    fetchOrderBook(pairSymbol.value),
    fetchRecentTrades(pairSymbol.value),
  ])
  if (version !== requestVersion || symbol !== pairSymbol.value) return

  const hasKlines = klineResult.status === 'fulfilled' && klineResult.value.length > 0
  if (
    klineRequest
    && isCurrentLiveDetail(liveState, version)
    && detailStreamSession.isCurrentKlineRequest(klineRequest)
  ) {
    const restPoints = hasKlines ? klineResult.value : []
    const nextPoints = detailStreamSession.resolveKlineRequest(klineRequest, restPoints)
    if (nextPoints) points.value = nextPoints
    klineError.value = klineResult.status === 'rejected'
      && !liveState.klineReceived
      && points.value.length === 0
    chartLoading.value = false
  }
  const currentLiveState = detailStreamSession.current()
  const currentDepthReceived = Boolean(
    currentLiveState
    && detailStreamSession.isCurrent(currentLiveState, symbol)
    && currentLiveState.depthReceived,
  )
  if (!liveState.depthReceived && !currentDepthReceived) {
    bids.value = depthResult.status === 'fulfilled' ? depthResult.value.bids : []
    asks.value = depthResult.status === 'fulfilled' ? depthResult.value.asks : []
    depthError.value = depthResult.status === 'rejected'
  }
  const restTrades = tradesResult.status === 'fulfilled' ? tradesResult.value : []
  trades.value = mergeMarketTradeHistory(trades.value, restTrades, 16)
  tradesError.value = tradesResult.status === 'rejected' && trades.value.length === 0
  loading.value = false
}

async function refreshKlines(liveState: MarketDetailStreamContext): Promise<void> {
  const version = liveState.requestVersion
  const symbol = pairSymbol.value
  const selectedInterval = interval.value
  const klineRequest = detailStreamSession.beginKlineRequest(liveState)
  if (!klineRequest || !isCurrentLiveDetail(liveState, version)) return
  chartLoading.value = true
  klineError.value = false
  try {
    const nextPoints = await fetchKlines(pairSymbol.value, interval.value)
    if (
      !isCurrentLiveDetail(liveState, version)
      || !detailStreamSession.isCurrentKlineRequest(klineRequest)
    ) {
      return
    }
    const mergedPoints = detailStreamSession.resolveKlineRequest(klineRequest, nextPoints)
    if (mergedPoints) points.value = mergedPoints
  } catch {
    if (
      !isCurrentLiveDetail(liveState, version)
      || !detailStreamSession.isCurrentKlineRequest(klineRequest)
    ) {
      return
    }
    klineError.value = detailStreamSession.currentPoints().length === 0
  } finally {
    if (
      symbol === pairSymbol.value
      && selectedInterval === interval.value
      && isCurrentLiveDetail(liveState, version)
      && detailStreamSession.isCurrentKlineRequest(klineRequest)
    ) {
      chartLoading.value = false
    }
  }
}

function retry(): void {
  void load(true)
}

function chooseInterval(value: string): void {
  const selectedInterval = normalizeMarketKlineInterval(value)
  if (!selectedInterval || interval.value === selectedInterval) return
  interval.value = selectedInterval
  chartLoading.value = true
  klineError.value = false
  const liveState = startLiveDetail(pairSymbol.value, selectedInterval, requestVersion)
  void refreshKlines(liveState)
}

function openTrade(mode: 'spot' | 'contract' = 'spot'): void {
  void router.replace({
    name: 'trade',
    params: { symbol: pairSymbol.value.replace('/', '_') },
    query: mode === 'contract' ? { mode: 'contract' } : undefined,
  })
}

function openOrders(): void {
  void router.push({
    name: 'orders',
    query: { symbol: pairSymbol.value.replace('/', '_') },
  })
}

function openPairPicker(): void {
  void router.push({ name: 'markets' })
}

function toggleFavorite(): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: route.fullPath } })
    return
  }
  void marketFavorites.toggle(pairSymbol.value)
}

function prefersReducedMotion(): boolean {
  return window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
}

async function scrollToSection(section: MarketSection): Promise<void> {
  activeSection.value = section
  await nextTick()
  const target = section === 'chart'
    ? chartSection.value
    : summarySection.value
  target?.scrollIntoView({
    behavior: prefersReducedMotion() ? 'auto' : 'smooth',
    block: 'start',
  })
}

function selectMarketDataPanel(panel: MarketDataPanel): void {
  marketDataPanel.value = panel
  activeSection.value = 'chart'
}

function handleMarketDataTabKeydown(event: KeyboardEvent, panel: MarketDataPanel): void {
  let nextPanel: MarketDataPanel | null = null
  if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
    nextPanel = panel === 'orderBook' ? 'trades' : 'orderBook'
  }
  if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
    nextPanel = panel === 'orderBook' ? 'trades' : 'orderBook'
  }
  if (event.key === 'Home') nextPanel = 'orderBook'
  if (event.key === 'End') nextPanel = 'trades'
  if (!nextPanel) return
  event.preventDefault()
  selectMarketDataPanel(nextPanel)
  void nextTick(() => {
    const target = nextPanel === 'orderBook' ? orderBookTabButton.value : tradesTabButton.value
    target?.focus()
  })
}

function formatObservedTime(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return '--'
  const timestamp = value < 1_000_000_000_000 ? value * 1000 : value
  return new Intl.DateTimeFormat(currentIntlLocale(), {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).format(new Date(timestamp))
}

function formatMovingAverage(value: number | null): string {
  return value === null ? '--' : formatPrice(value)
}

function handleChartKeydown(event: KeyboardEvent): void {
  if (!chartExpanded.value) return
  if (event.key === 'Escape') {
    event.preventDefault()
    closeExpandedChart()
    return
  }
  if (event.key !== 'Tab' || !chartSection.value) return
  const focusable = [...chartSection.value.querySelectorAll<HTMLElement>(
    'button:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])',
  )]
  if (!focusable.length) {
    event.preventDefault()
    return
  }
  const first = focusable[0]
  const last = focusable.at(-1)
  const focusInside = chartSection.value.contains(document.activeElement)
  if (event.shiftKey && (!focusInside || document.activeElement === first)) {
    event.preventDefault()
    last?.focus()
  } else if (!event.shiftKey && (!focusInside || document.activeElement === last)) {
    event.preventDefault()
    first?.focus()
  }
}

function lockChartScroll(): void {
  if (chartScrollLock) return
  chartScrollLock = {
    bodyOverflow: document.body.style.overflow,
    bodyPosition: document.body.style.position,
    bodyTop: document.body.style.top,
    bodyWidth: document.body.style.width,
    rootOverflow: document.documentElement.style.overflow,
    rootScrollBehavior: document.documentElement.style.scrollBehavior,
    scrollY: window.scrollY,
  }
  document.body.style.overflow = 'hidden'
  document.body.style.position = 'fixed'
  document.body.style.top = `-${chartScrollLock.scrollY}px`
  document.body.style.width = '100%'
  document.documentElement.style.overflow = 'hidden'
  window.addEventListener('keydown', handleChartKeydown)
}

function releaseChartScroll(): ChartScrollLock | null {
  const lock = chartScrollLock
  window.removeEventListener('keydown', handleChartKeydown)
  if (!lock) return null
  document.body.style.overflow = lock.bodyOverflow
  document.body.style.position = lock.bodyPosition
  document.body.style.top = lock.bodyTop
  document.body.style.width = lock.bodyWidth
  document.documentElement.style.overflow = lock.rootOverflow
  chartScrollLock = null
  return lock
}

function restoreChartScroll(lock: ChartScrollLock): void {
  document.documentElement.style.scrollBehavior = 'auto'
  window.scrollTo(0, lock.scrollY)
  document.documentElement.style.scrollBehavior = lock.rootScrollBehavior
}

function openExpandedChart(): void {
  if (chartExpanded.value) return
  lockChartScroll()
  chartExpanded.value = true
  void nextTick(() => chartToggleButton.value?.focus({ preventScroll: true }))
}

function closeExpandedChart(): void {
  if (!chartExpanded.value) return
  chartExpanded.value = false
  const lock = releaseChartScroll()
  void nextTick(() => {
    if (!chartSection.value) return
    if (lock) restoreChartScroll(lock)
    chartToggleButton.value?.focus({ preventScroll: true })
  })
}

function toggleChartExpanded(): void {
  if (chartExpanded.value) closeExpandedChart()
  else openExpandedChart()
}

async function shareMarket(): Promise<void> {
  const url = window.location.href
  try {
    if (navigator.share) {
      await navigator.share({ title: pairSymbol.value, url })
      return
    }
    await navigator.clipboard?.writeText(url)
  } catch {
    return
  }
}

function goBack(): void {
  void goBackOr(router, { name: 'markets' })
}

watch(() => props.symbol, () => { void load() }, { immediate: true })

onBeforeUnmount(() => {
  const lock = releaseChartScroll()
  if (lock) restoreChartScroll(lock)
})

onUnmounted(() => {
  requestVersion += 1
  stopLiveDetail()
})
</script>

<template>
  <main
    class="market-detail"
    :class="{ 'is-chart-expanded': chartExpanded }"
    data-market-workspace="live"
  >
    <header class="market-detail__header">
      <button class="market-detail__icon-button" type="button" :aria-label="t('common.back')" @click="goBack">
        <ArrowLeft :size="24" />
      </button>
      <button
        class="market-detail__instrument"
        type="button"
        :aria-label="t('marketDetail.selectPair')"
        @click="openPairPicker"
      >
        <AssetMark :symbol="baseAsset" :src="ticker?.iconUrl" :fallback-src="ticker?.baseIconUrl" :size="24" />
        <strong>{{ baseAsset }}/{{ quoteAsset }}</strong>
        <ChevronDown :size="20" aria-hidden="true" />
      </button>
      <button
        class="market-detail__icon-button market-detail__favorite"
        :class="{ 'is-active': isFavorite }"
        type="button"
        :aria-label="t(isFavorite ? 'rootPrototype.removeFavorite' : 'rootPrototype.addFavorite', { symbol: pairSymbol })"
        :aria-pressed="isFavorite"
        :aria-busy="favoriteSaving"
        :disabled="favoriteSaving"
        @click="toggleFavorite"
      >
        <Star :size="24" :fill="isFavorite ? 'currentColor' : 'none'" />
      </button>
      <button class="market-detail__icon-button" type="button" :aria-label="t('marketDetail.share')" @click="shareMarket">
        <Share2 :size="20" />
      </button>
    </header>

    <nav class="market-detail__rail" :aria-label="t('marketDetail.details')">
      <button
        type="button"
        aria-controls="market-chart"
        :class="{ 'is-active': activeSection === 'chart' }"
        :aria-current="activeSection === 'chart' ? 'location' : undefined"
        @click="scrollToSection('chart')"
      >
        {{ t('marketDetail.market') }}
      </button>
      <button
        type="button"
        aria-controls="market-overview"
        :class="{ 'is-active': activeSection === 'overview' }"
        :aria-current="activeSection === 'overview' ? 'location' : undefined"
        @click="scrollToSection('overview')"
      >
        {{ t('marketDetail.overview') }}
      </button>
    </nav>

    <section
      id="market-overview"
      ref="summarySection"
      class="market-detail__summary"
      :aria-busy="marketStore.loading"
    >
      <div class="market-detail__quote">
        <span>{{ t('marketDetail.latestPrice') }}</span>
        <strong
          v-if="hasLatestPrice"
          class="numeric"
          :class="ticker ? latestChangePercent >= 0 ? 'up' : 'down' : undefined"
        >
          {{ formatPrice(latestPrice) }}
        </strong>
        <strong v-else class="numeric">--</strong>
        <p v-if="hasLatestPrice" class="market-detail__quote-line">
          <span class="numeric">≈ {{ formatPrice(latestPrice) }}</span>
          <b v-if="ticker" :class="latestChangePercent >= 0 ? 'up' : 'down'">
            {{ formatPercent(latestChangePercent) }}
          </b>
        </p>
        <p v-else>{{ marketStore.loading ? t('common.loading') : t('common.marketUnavailable') }}</p>
      </div>
      <dl>
        <div>
          <dt>{{ t('marketDetail.high24h') }}</dt>
          <dd class="numeric">{{ ticker ? formatPrice(ticker.highPrice) : '--' }}</dd>
        </div>
        <div>
          <dt>{{ t('marketDetail.low24h') }}</dt>
          <dd class="numeric">{{ ticker ? formatPrice(ticker.lowPrice) : '--' }}</dd>
        </div>
        <div>
          <dt>{{ t('marketDetail.volume24h') }}</dt>
          <dd class="numeric">{{ ticker ? `${formatAmount(ticker.volume)} ${baseAsset}` : '--' }}</dd>
        </div>
        <div>
          <dt>{{ t('marketDetail.turnover24h') }}</dt>
          <dd class="numeric">-- {{ quoteAsset }}</dd>
        </div>
      </dl>
      <p class="sr-only" role="status">
        {{ liveDetailActive
          ? t('common.liveData')
          : ticker
            ? t('marketDetail.snapshotData')
            : marketStore.loading
              ? t('common.loading')
              : t('common.marketUnavailable') }}
        {{ observedAt ? formatObservedTime(observedAt) : '' }}
      </p>
    </section>

    <div v-if="dataError || marketStore.error" class="market-detail__error" role="alert">
      <CircleAlert :size="18" />
      <span>{{ t('common.marketLoadFailed') }}</span>
      <button type="button" :disabled="loading" :aria-label="t('common.retry')" @click="retry">
        <RefreshCw :size="17" :class="{ spin: loading }" />
      </button>
    </div>

    <section
      id="market-chart"
      ref="chartSection"
      class="market-detail__chart-panel"
      :class="{ 'is-expanded': chartExpanded }"
      :data-chart-mode="chartExpanded ? 'expanded' : 'inline'"
      :role="chartExpanded ? 'dialog' : 'region'"
      :aria-modal="chartExpanded ? 'true' : undefined"
      aria-labelledby="market-chart-heading"
    >
      <h1 id="market-chart-heading" class="sr-only">
        {{ t('marketDetail.chartWorkstation') }} · {{ pairSymbol }} · {{ interval }}
      </h1>
      <nav class="market-detail__intervals" :aria-label="t('marketDetail.indicators')">
        <button
          v-for="item in MARKET_KLINE_INTERVALS"
          :key="item"
          type="button"
          :class="{ 'is-active': interval === item }"
          :aria-pressed="interval === item"
          @click="chooseInterval(item)"
        >
          {{ item }}
        </button>
      </nav>
      <div class="market-detail__chart">
        <button
          ref="chartToggleButton"
          class="market-detail__chart-toggle"
          type="button"
          :aria-label="chartExpanded ? t('marketDetail.collapseChart') : t('marketDetail.expandChart')"
          :aria-pressed="chartExpanded"
          :title="chartExpanded ? t('marketDetail.collapseChart') : t('marketDetail.expandChart')"
          @click="toggleChartExpanded"
        >
          <Minimize2 v-if="chartExpanded" :size="18" />
          <Maximize2 v-else :size="18" />
        </button>
        <MobileMarketChart
          :points="points"
          :loading="chartLoading"
          :interval="interval"
          :symbol="pairSymbol"
          show-engine-switch
          compact-engine-switch
        />
      </div>
      <div class="market-detail__indicator-legend" :aria-label="t('marketDetail.realIndicators')">
        <span class="is-ma5 numeric"><b>MA5</b> {{ formatMovingAverage(latestAverages.ma5) }}</span>
        <span class="is-ma10 numeric"><b>MA10</b> {{ formatMovingAverage(latestAverages.ma10) }}</span>
        <span class="is-ma20 numeric"><b>MA20</b> {{ formatMovingAverage(latestAverages.ma20) }}</span>
        <span class="is-volume numeric">
          <b>{{ t('marketDetail.candleVolume') }}</b>
          {{ latestCandle ? formatAmount(latestCandle.volume) : '--' }}
        </span>
      </div>
    </section>

    <section class="market-detail__microstructure" :aria-label="t('marketDetail.marketData')">
      <nav class="market-detail__data-tabs" role="tablist" :aria-label="t('marketDetail.marketData')">
        <button
          id="market-order-book-tab"
          ref="orderBookTabButton"
          type="button"
          role="tab"
          aria-controls="market-order-book"
          :aria-selected="marketDataPanel === 'orderBook'"
          :aria-pressed="marketDataPanel === 'orderBook'"
          :tabindex="marketDataPanel === 'orderBook' ? 0 : -1"
          :class="{ 'is-active': marketDataPanel === 'orderBook' }"
          @click="selectMarketDataPanel('orderBook')"
          @keydown="handleMarketDataTabKeydown($event, 'orderBook')"
        >
          {{ t('orderBook.title') }}
        </button>
        <button
          id="market-latest-trades-tab"
          ref="tradesTabButton"
          type="button"
          role="tab"
          aria-controls="market-latest-trades"
          :aria-selected="marketDataPanel === 'trades'"
          :aria-pressed="marketDataPanel === 'trades'"
          :tabindex="marketDataPanel === 'trades' ? 0 : -1"
          :class="{ 'is-active': marketDataPanel === 'trades' }"
          @click="selectMarketDataPanel('trades')"
          @keydown="handleMarketDataTabKeydown($event, 'trades')"
        >
          {{ t('marketDetail.latestTrades') }}
        </button>
      </nav>

      <div
        v-show="marketDataPanel === 'orderBook'"
        id="market-order-book"
        class="market-detail__data-panel"
        role="tabpanel"
        aria-labelledby="market-order-book-tab"
        tabindex="0"
      >
        <OrderBookPanel
          layout="paired"
          :bids="bids"
          :asks="asks"
          :current-price="latestPrice"
          :base-asset="baseAsset"
          :quote-asset="quoteAsset"
          :loading="loading"
        />
      </div>

      <div
        v-show="marketDataPanel === 'trades'"
        id="market-latest-trades"
        class="market-detail__data-panel market-detail__trades"
        role="tabpanel"
        aria-labelledby="market-latest-trades-tab"
        tabindex="0"
        :aria-busy="loading"
      >
        <div class="market-detail__trade-head">
          <span>{{ t('marketDetail.price') }} · {{ quoteAsset }}</span>
          <span>{{ t('marketDetail.quantity') }} · {{ baseAsset }}</span>
          <span>{{ t('common.time') }}</span>
        </div>
        <div v-if="loading && !trades.length" class="market-detail__trade-state">
          {{ t('common.loading') }}
        </div>
        <div v-else-if="!trades.length" class="market-detail__trade-state">
          {{ t('common.marketUnavailable') }}
        </div>
        <div v-else>
          <div v-for="trade in trades.slice(0, 7)" :key="trade.id" class="market-detail__trade">
            <span class="numeric" :class="trade.side === 'buy' ? 'up' : 'down'">
              {{ formatPrice(trade.price) }}
            </span>
            <span class="numeric">{{ formatAmount(trade.quantity) }}</span>
            <span class="numeric">
              {{ new Date(trade.time).toLocaleTimeString(currentIntlLocale(), { hour: '2-digit', minute: '2-digit' }) }}
            </span>
          </div>
        </div>
      </div>
    </section>

    <nav class="market-detail__actions" :aria-label="t('marketDetail.actions')">
      <button type="button" @click="openTrade('contract')">
        <ChartNoAxesCombined :size="24" />
        <span>{{ t('marketDetail.contract') }}</span>
      </button>
      <button type="button" @click="openOrders">
        <ClipboardList :size="24" />
        <span>{{ t('marketDetail.orders') }}</span>
      </button>
      <button class="is-primary" type="button" @click="openTrade('spot')">
        <ArrowLeftRight :size="22" />
        <span>{{ t('marketDetail.spotTrade') }}</span>
      </button>
    </nav>
  </main>
</template>

<style scoped>
.market-detail {
  --detail-background: var(--market-detail-background);
  --detail-surface: var(--market-detail-surface);
  --detail-ink: var(--market-detail-ink);
  --detail-muted: var(--market-detail-muted);
  --detail-line: var(--market-detail-line);
  --detail-positive: var(--market-detail-positive);
  --detail-negative: var(--market-detail-negative);
  --detail-action: var(--market-detail-action);
  --surface: var(--detail-surface);
  --surface-elevated: var(--detail-surface);
  --ink: var(--detail-ink);
  --muted: var(--detail-muted);
  --muted-strong: var(--detail-muted);
  --line: var(--detail-line);
  --positive: var(--detail-positive);
  --negative: var(--detail-negative);
  background: var(--detail-background);
  color: var(--detail-ink);
  min-height: 100dvh;
  min-width: 0;
  overflow-x: clip;
  padding: 0 0 calc(67px + env(safe-area-inset-bottom));
}

.market-detail.view-stack {
  will-change: opacity;
}

.sr-only {
  clip: rect(0, 0, 0, 0);
  clip-path: inset(50%);
  height: 1px;
  overflow: hidden;
  position: absolute;
  white-space: nowrap;
  width: 1px;
}

.market-detail__header {
  align-items: center;
  background: var(--detail-background);
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr) 48px 48px;
  height: calc(64px + env(safe-area-inset-top));
  min-width: 0;
  padding: env(safe-area-inset-top) 8px 0;
  position: sticky;
  top: 0;
  z-index: var(--layer-sticky-header);
}

.market-detail__icon-button,
.market-detail__instrument {
  background: transparent;
  border: 0;
  box-shadow: none;
  color: var(--detail-ink);
  min-height: 44px;
}

.market-detail__icon-button {
  align-items: center;
  display: inline-flex;
  height: 44px;
  justify-content: center;
  padding: 0;
  width: 44px;
}

.market-detail__favorite.is-active {
  color: var(--detail-positive);
}

.market-detail__instrument {
  align-items: center;
  display: flex;
  gap: 9px;
  min-width: 0;
  overflow: hidden;
  padding: 0 2px;
  text-align: left;
}

.market-detail__instrument strong {
  color: var(--detail-ink);
  font-family: var(--data-font);
  font-size: 18px;
  font-variant-numeric: tabular-nums;
  font-weight: 570;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__instrument > svg {
  flex: none;
}

.market-detail__icon-button:focus-visible,
.market-detail__instrument:focus-visible,
.market-detail__rail button:focus-visible,
.market-detail__intervals button:focus-visible,
.market-detail__data-tabs button:focus-visible,
.market-detail__actions button:focus-visible,
.market-detail__chart-toggle:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: none;
}

.market-detail__rail {
  align-items: stretch;
  background: var(--detail-background);
  display: flex;
  gap: 24px;
  height: 42px;
  min-width: 0;
  padding: 0 16px;
  position: sticky;
  top: calc(64px + env(safe-area-inset-top));
  z-index: calc(var(--layer-sticky-header) - 1);
}

.market-detail__rail button {
  background: transparent;
  border: 0;
  color: var(--detail-muted);
  font-size: 15px;
  font-weight: 560;
  min-height: 42px;
  padding: 0;
  position: relative;
  white-space: nowrap;
}

.market-detail__rail button.is-active {
  color: var(--detail-positive);
}

.market-detail__rail button.is-active::after {
  background: var(--detail-positive);
  border-radius: 2px;
  bottom: 0;
  content: '';
  height: 2px;
  left: 50%;
  position: absolute;
  transform: translateX(-50%);
  width: 22px;
}

.market-detail__summary {
  align-items: center;
  background: var(--detail-surface);
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) minmax(138px, .78fr);
  height: 112px;
  min-width: 0;
  padding: 9px 16px 11px;
  scroll-margin-top: calc(106px + env(safe-area-inset-top));
}

.market-detail__quote {
  min-width: 0;
}

.market-detail__quote > span {
  color: var(--detail-muted);
  display: block;
  font-size: 10px;
  font-weight: 650;
}

.market-detail__quote > strong {
  color: var(--detail-positive);
  display: block;
  font-family: var(--data-font);
  font-size: clamp(29px, 8.8vw, 35px);
  font-variant-numeric: tabular-nums;
  font-weight: 720;
  letter-spacing: -.75px;
  line-height: 1.16;
  margin-top: 6px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__quote-line {
  align-items: center;
  display: flex;
  gap: 22px;
  margin: 5px 0 0;
  min-width: 0;
}

.market-detail__quote-line > span,
.market-detail__quote-line > b {
  color: var(--detail-positive);
  font-family: var(--data-font);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  font-weight: 520;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__quote > p:not(.market-detail__quote-line) {
  color: var(--detail-muted);
  font-size: 10px;
  margin: 7px 0 0;
}

.market-detail__summary dl {
  display: grid;
  gap: 0;
  margin: 0;
  min-width: 0;
}

.market-detail__summary dl > div {
  align-items: center;
  display: grid;
  gap: 7px;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 20px;
  min-width: 0;
}

.market-detail__summary dt {
  color: var(--detail-muted);
  font-size: 10px;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__summary dd {
  color: var(--detail-ink);
  font-family: var(--data-font);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  font-weight: 620;
  margin: 0;
  max-width: 102px;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__error {
  align-items: center;
  background: color-mix(in srgb, var(--detail-negative) 12%, var(--detail-surface));
  border: 1px solid color-mix(in srgb, var(--detail-negative) 42%, var(--detail-line));
  border-radius: 8px;
  color: var(--detail-negative);
  display: grid;
  font-size: 10px;
  gap: 8px;
  grid-template-columns: auto minmax(0, 1fr) 44px;
  left: 12px;
  min-height: 44px;
  padding-left: 10px;
  position: fixed;
  right: 12px;
  top: calc(112px + env(safe-area-inset-top));
  z-index: calc(var(--layer-sticky-header) + 2);
}

.market-detail__error button {
  align-items: center;
  background: transparent;
  color: inherit;
  display: flex;
  height: 44px;
  justify-content: center;
  padding: 0;
}

.market-detail__chart-panel {
  background: var(--detail-surface);
  height: 280px;
  min-width: 0;
  overflow: visible;
  scroll-margin-top: calc(106px + env(safe-area-inset-top));
}

.market-detail__intervals {
  align-items: center;
  background: var(--detail-surface);
  display: grid;
  grid-template-columns: repeat(5, minmax(44px, 54px)) minmax(0, 1fr);
  height: 48px;
  min-width: 0;
  padding-left: 10px;
}

.market-detail__intervals button {
  background: transparent;
  border: 0;
  color: var(--detail-muted);
  font-family: var(--data-font);
  font-size: 10px;
  height: 44px;
  min-height: 44px;
  min-width: 44px;
  padding: 0;
}

.market-detail__intervals button.is-active {
  color: var(--detail-positive);
  font-weight: 750;
}

.market-detail__chart {
  height: 204px;
  min-height: 0;
  min-width: 0;
  position: relative;
}

.market-detail__chart-toggle {
  align-items: center;
  background: color-mix(in srgb, var(--detail-ink) 4%, var(--detail-surface));
  border: 1px solid color-mix(in srgb, var(--detail-muted) 36%, var(--detail-line));
  border-radius: 8px;
  box-shadow: 0 5px 12px color-mix(in srgb, var(--detail-ink) 10%, transparent);
  color: var(--detail-ink);
  display: flex;
  height: 32px;
  justify-content: center;
  padding: 0;
  position: absolute;
  right: 16px;
  top: 12px;
  width: 32px;
  z-index: 4;
}

.market-detail__indicator-legend {
  align-items: center;
  background: var(--detail-surface);
  display: grid;
  gap: 6px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  height: 28px;
  min-width: 0;
  padding: 0 16px;
}

.market-detail__indicator-legend span {
  font-family: var(--data-font);
  font-size: 9px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__indicator-legend b {
  font-weight: 750;
}

.market-detail__indicator-legend .is-ma5 {
  color: var(--detail-positive);
}

.market-detail__indicator-legend .is-ma10 {
  color: var(--detail-negative);
}

.market-detail__indicator-legend .is-ma20 {
  color: var(--market-detail-ma20);
}

.market-detail__indicator-legend .is-volume {
  color: var(--detail-muted);
}

.market-detail__chart-panel.is-expanded {
  background: var(--detail-surface);
  display: grid;
  grid-template-rows: calc(48px + env(safe-area-inset-top)) minmax(0, 1fr) calc(34px + env(safe-area-inset-bottom));
  height: 100dvh;
  inset: 0;
  margin: 0;
  padding: 0 env(safe-area-inset-right) 0 env(safe-area-inset-left);
  position: fixed;
  width: 100%;
  z-index: var(--layer-overlay);
}

.market-detail__chart-panel.is-expanded .market-detail__intervals {
  height: calc(48px + env(safe-area-inset-top));
  padding-top: env(safe-area-inset-top);
}

.market-detail__chart-panel.is-expanded .market-detail__chart {
  height: auto;
  min-height: 0;
}

.market-detail__chart-panel.is-expanded .market-detail__indicator-legend {
  height: calc(34px + env(safe-area-inset-bottom));
  padding-bottom: env(safe-area-inset-bottom);
}

.market-detail__chart-panel.is-expanded .market-detail__chart-toggle {
  right: 10px;
  top: 8px;
}

.market-detail__microstructure {
  background: var(--detail-surface);
  height: 320px;
  min-width: 0;
}

.market-detail__data-tabs {
  background: var(--detail-surface);
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  height: 48px;
}

.market-detail__data-tabs button {
  background: transparent;
  border: 0;
  color: var(--detail-muted);
  font-size: 14px;
  font-weight: 570;
  height: 48px;
  min-width: 0;
  padding: 0;
  position: relative;
}

.market-detail__data-tabs button.is-active {
  color: var(--detail-positive);
}

.market-detail__data-tabs button.is-active::after {
  background: var(--detail-positive);
  border-radius: 2px;
  bottom: 0;
  content: '';
  height: 2px;
  left: 50%;
  position: absolute;
  transform: translateX(-50%);
  width: 18px;
}

.market-detail__data-panel {
  height: 272px;
  min-width: 0;
  overflow: hidden;
  scroll-margin-top: calc(106px + env(safe-area-inset-top));
}

.market-detail__data-panel:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: none;
}

.market-detail__trades {
  background: var(--detail-surface);
  min-height: 272px;
}

.market-detail__trade-head,
.market-detail__trade {
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) minmax(70px, .8fr) minmax(48px, .55fr);
  min-width: 0;
  padding: 0 12px;
}

.market-detail__trade-head {
  color: var(--detail-muted);
  font-size: 8px;
  height: 34px;
}

.market-detail__trade-head span,
.market-detail__trade span {
  align-self: center;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__trade-head span:nth-child(n + 2),
.market-detail__trade span:nth-child(n + 2) {
  text-align: right;
}

.market-detail__trade {
  border-top: 1px solid color-mix(in srgb, var(--detail-line) 58%, transparent);
  color: var(--detail-ink);
  font-family: var(--data-font);
  font-size: 10px;
  height: 34px;
}

.market-detail__trade-state {
  align-items: center;
  color: var(--detail-muted);
  display: flex;
  font-size: 10px;
  height: 238px;
  justify-content: center;
  padding: 20px;
  text-align: center;
}

.market-detail__actions {
  align-items: center;
  background: var(--detail-background);
  bottom: 0;
  display: grid;
  gap: 18px;
  grid-template-columns: 40px 40px minmax(0, 1fr);
  height: calc(67px + env(safe-area-inset-bottom));
  left: 50%;
  max-width: var(--app-max-width);
  padding: 6px 16px calc(9px + env(safe-area-inset-bottom));
  position: fixed;
  transform: translateX(-50%);
  width: 100%;
  z-index: var(--layer-navigation);
}

.market-detail__actions button {
  align-items: center;
  align-self: stretch;
  background: transparent;
  border: 0;
  color: var(--detail-muted);
  display: flex;
  flex-direction: column;
  font-size: 9px;
  gap: 3px;
  justify-content: center;
  min-height: 52px;
  min-width: 0;
  padding: 0;
}

.market-detail__actions button span {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__actions .is-primary {
  align-self: center;
  background: var(--detail-action);
  border-radius: 26px;
  color: var(--market-detail-on-action);
  flex-direction: row;
  font-size: 14px;
  font-weight: 720;
  gap: 10px;
  height: 52px;
  justify-self: stretch;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 360px) {
  .market-detail__header {
    grid-template-columns: 44px minmax(0, 1fr) 44px 44px;
    padding-inline: 6px;
  }

  .market-detail__instrument {
    gap: 6px;
  }

  .market-detail__instrument strong {
    font-size: 16px;
  }

  .market-detail__summary {
    gap: 6px;
    grid-template-columns: minmax(0, 1fr) 132px;
    padding-inline: 12px;
  }

  .market-detail__summary dd {
    font-size: 10px;
    max-width: 91px;
  }

  .market-detail__quote-line {
    gap: 12px;
  }

  .market-detail__intervals {
    grid-template-columns: repeat(5, minmax(42px, 50px)) minmax(0, 1fr);
    padding-left: 4px;
  }

  .market-detail__indicator-legend {
    gap: 4px;
    padding-inline: 10px;
  }

  .market-detail__indicator-legend span {
    font-size: 8px;
  }

  .market-detail__actions {
    gap: 14px;
    padding-inline: 12px;
  }
}

@media (max-width: 340px) {
  .market-detail__header {
    grid-template-columns: 44px minmax(0, 1fr) 44px 44px;
    padding-inline: 4px;
  }

  .market-detail__icon-button {
    width: 44px;
  }

  .market-detail__instrument strong {
    font-size: 14px;
  }

  .market-detail__summary {
    grid-template-columns: minmax(0, 1fr) 122px;
    padding-inline: 9px;
  }

  .market-detail__summary dt,
  .market-detail__summary dd {
    font-size: 9px;
  }

  .market-detail__summary dd {
    max-width: 84px;
  }

  .market-detail__quote-line > span,
  .market-detail__quote-line > b {
    font-size: 10px;
  }

  .market-detail__intervals {
    grid-template-columns: repeat(5, 44px) minmax(0, 1fr);
    padding-left: 0;
  }

  .market-detail__trade-head,
  .market-detail__trade {
    gap: 5px;
    grid-template-columns: minmax(0, 1fr) 66px 46px;
    padding-inline: 8px;
  }

  .market-detail__actions {
    gap: 10px;
    grid-template-columns: 38px 38px minmax(0, 1fr);
    padding-inline: 9px;
  }
}

@media (orientation: landscape) and (max-height: 600px) {
  .market-detail__chart-panel.is-expanded {
    grid-template-rows: calc(44px + env(safe-area-inset-top)) minmax(0, 1fr) calc(30px + env(safe-area-inset-bottom));
  }

  .market-detail__chart-panel.is-expanded .market-detail__intervals {
    height: calc(44px + env(safe-area-inset-top));
  }

  .market-detail__chart-panel.is-expanded .market-detail__indicator-legend {
    height: calc(30px + env(safe-area-inset-bottom));
  }
}

@media (prefers-reduced-motion: reduce) {
  .market-detail *,
  .market-detail *::before,
  .market-detail *::after {
    scroll-behavior: auto;
    transition: none;
  }

  .spin {
    animation: none;
  }
}
</style>
