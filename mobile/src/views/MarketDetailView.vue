<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, onUnmounted, ref, watch } from 'vue'
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
type MarketContentPanel = 'chart' | 'depth' | 'trades' | 'overview'

interface ChartScrollLock {
  bodyOverflow: string
  bodyPosition: string
  bodyTop: string
  bodyWidth: string
  rootOverflow: string
  rootScrollBehavior: string
  scrollY: number
}

const activeContentPanel = ref<MarketContentPanel>('chart')
const chartExpanded = ref(false)
const chartSection = ref<HTMLElement | null>(null)
const contentTabList = ref<HTMLElement | null>(null)
const chartToggleButton = ref<HTMLButtonElement | null>(null)
const contentPanels: readonly MarketContentPanel[] = ['chart', 'depth', 'trades', 'overview']
const contentPanelKeys: Record<MarketContentPanel, string> = {
  chart: 'marketDetail.chart',
  depth: 'marketDetail.depth',
  trades: 'marketDetail.latestTrades',
  overview: 'marketDetail.overview',
}
let chartScrollLock: ChartScrollLock | null = null
let requestVersion = 0
let viewActive = true

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
const latestPrice = computed(() => ticker.value?.lastPrice ?? 0)
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

function selectContentPanel(panel: MarketContentPanel): void {
  activeContentPanel.value = panel
}

function handleContentTabKeydown(event: KeyboardEvent, panel: MarketContentPanel): void {
  const currentIndex = contentPanels.indexOf(panel)
  let nextIndex: number | null = null
  if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
    nextIndex = (currentIndex - 1 + contentPanels.length) % contentPanels.length
  }
  if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
    nextIndex = (currentIndex + 1) % contentPanels.length
  }
  if (event.key === 'Home') nextIndex = 0
  if (event.key === 'End') nextIndex = contentPanels.length - 1
  if (nextIndex === null) return
  event.preventDefault()
  const nextPanel = contentPanels[nextIndex]
  selectContentPanel(nextPanel)
  void nextTick(() => {
    contentTabList.value
      ?.querySelector<HTMLButtonElement>(`[data-content-tab="${nextPanel}"]`)
      ?.focus()
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

onMounted(async () => {
  await marketStore.refresh()
  if (viewActive) marketStore.startLiveUpdates('market-detail')
})

onBeforeUnmount(() => {
  const lock = releaseChartScroll()
  if (lock) restoreChartScroll(lock)
})

onUnmounted(() => {
  viewActive = false
  marketStore.stopLiveUpdates('market-detail')
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

    <nav class="market-detail__mode-tabs" :aria-label="t('marketDetail.modeSwitch')">
      <button
        type="button"
        :aria-pressed="false"
        @click="openTrade('spot')"
      >
        {{ t('marketDetail.trade') }}
      </button>
      <button
        class="is-active"
        type="button"
        aria-current="page"
        :aria-pressed="true"
      >
        {{ t('marketDetail.chart') }}
      </button>
    </nav>

    <section
      class="market-detail__summary"
      :aria-label="t('marketDetail.quoteSummary')"
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

    <nav
      ref="contentTabList"
      class="market-detail__content-tabs"
      role="tablist"
      :aria-label="t('marketDetail.contentTabs')"
    >
      <button
        v-for="panel in contentPanels"
        :id="`market-content-${panel}-tab`"
        :key="panel"
        type="button"
        role="tab"
        :data-content-tab="panel"
        :aria-controls="`market-content-${panel}`"
        :aria-selected="activeContentPanel === panel"
        :tabindex="activeContentPanel === panel ? 0 : -1"
        :class="{ 'is-active': activeContentPanel === panel }"
        @click="selectContentPanel(panel)"
        @keydown="handleContentTabKeydown($event, panel)"
      >
        {{ t(contentPanelKeys[panel]) }}
      </button>
    </nav>

    <section
      v-show="activeContentPanel === 'chart'"
      id="market-content-chart"
      ref="chartSection"
      class="market-detail__chart-panel"
      :class="{ 'is-expanded': chartExpanded }"
      :data-chart-mode="chartExpanded ? 'expanded' : 'inline'"
      :role="chartExpanded ? 'dialog' : 'tabpanel'"
      :aria-modal="chartExpanded ? 'true' : undefined"
      :aria-labelledby="chartExpanded ? 'market-chart-heading' : 'market-content-chart-tab'"
      :tabindex="chartExpanded ? undefined : 0"
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

    <section
      v-show="activeContentPanel === 'depth'"
      id="market-content-depth"
      class="market-detail__content-panel market-detail__depth"
      role="tabpanel"
      aria-labelledby="market-content-depth-tab"
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
    </section>

    <section
      v-show="activeContentPanel === 'trades'"
      id="market-content-trades"
      class="market-detail__content-panel market-detail__trades"
      role="tabpanel"
      aria-labelledby="market-content-trades-tab"
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
    </section>

    <section
      v-show="activeContentPanel === 'overview'"
      id="market-content-overview"
      class="market-detail__content-panel market-detail__overview"
      role="tabpanel"
      aria-labelledby="market-content-overview-tab"
      tabindex="0"
    >
      <header>
        <AssetMark :symbol="baseAsset" :src="ticker?.iconUrl" :fallback-src="ticker?.baseIconUrl" :size="36" />
        <div>
          <strong>{{ baseAsset }}/{{ quoteAsset }}</strong>
          <span>{{ t('marketDetail.spotPair') }}</span>
        </div>
      </header>
      <dl>
        <div>
          <dt>{{ t('marketDetail.latestPrice') }}</dt>
          <dd class="numeric">{{ hasLatestPrice ? formatPrice(latestPrice) : '--' }}</dd>
        </div>
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
      </dl>
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
  padding: 0 env(safe-area-inset-right) calc(67px + env(safe-area-inset-bottom)) env(safe-area-inset-left);
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
.market-detail__mode-tabs button:focus-visible,
.market-detail__content-tabs button:focus-visible,
.market-detail__intervals button:focus-visible,
.market-detail__actions button:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: none;
}

.market-detail__mode-tabs {
  align-items: center;
  background: var(--detail-background);
  display: flex;
  gap: 4px;
  height: 48px;
  min-width: 0;
  padding: 2px 16px;
  position: sticky;
  top: calc(64px + env(safe-area-inset-top));
  z-index: calc(var(--layer-sticky-header) - 1);
}

.market-detail__mode-tabs button {
  background: transparent;
  border: 1px solid transparent;
  border-radius: 9px;
  color: var(--detail-muted);
  font-size: 13px;
  font-weight: 620;
  height: 44px;
  min-height: 44px;
  min-width: 76px;
  padding: 0 14px;
  white-space: nowrap;
}

.market-detail__mode-tabs button.is-active {
  background: color-mix(in srgb, var(--detail-positive) 10%, var(--detail-surface));
  border-color: color-mix(in srgb, var(--detail-positive) 36%, var(--detail-line));
  color: var(--detail-positive);
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
  top: calc(120px + env(safe-area-inset-top));
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

.market-detail__content-tabs {
  background: var(--detail-surface);
  border-bottom: 1px solid var(--detail-line);
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  height: 48px;
  min-width: 0;
}

.market-detail__content-tabs button {
  background: transparent;
  border: 0;
  color: var(--detail-muted);
  font-size: 12px;
  font-weight: 590;
  height: 48px;
  min-height: 44px;
  min-width: 0;
  overflow: hidden;
  padding: 0 5px;
  position: relative;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__content-tabs button.is-active {
  color: var(--detail-positive);
}

.market-detail__content-tabs button.is-active::after {
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

.market-detail__chart-panel {
  background: var(--detail-surface);
  height: 456px;
  min-width: 0;
  overflow: visible;
}

.market-detail__chart-panel:focus-visible,
.market-detail__content-panel:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: none;
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
  height: 376px;
  min-height: 0;
  min-width: 0;
  position: relative;
}

/* Outrank the legacy dark-theme box-shadow bridge in prototype-parity.css. */
.market-detail .market-detail__chart > button.market-detail__chart-toggle {
  align-items: center;
  -webkit-backdrop-filter: blur(14px) saturate(145%);
  backdrop-filter: blur(14px) saturate(145%);
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--detail-surface) 82%, transparent) 0%,
    color-mix(in srgb, var(--detail-background) 58%, transparent) 100%
  );
  border: 1px solid color-mix(in srgb, var(--detail-line) 72%, var(--detail-ink));
  border-radius: 12px;
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, var(--detail-surface) 92%, var(--detail-ink)),
    0 8px 18px color-mix(in srgb, var(--detail-background) 46%, transparent),
    0 2px 6px color-mix(in srgb, var(--detail-ink) 10%, transparent);
  color: var(--detail-ink);
  display: flex;
  height: 44px;
  justify-content: center;
  left: 16px;
  padding: 0;
  position: absolute;
  top: 12px;
  transition: background 140ms ease, box-shadow 140ms ease, transform 100ms ease;
  width: 44px;
  z-index: 4;
}

.market-detail .market-detail__chart > button.market-detail__chart-toggle:active {
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, var(--detail-surface) 88%, var(--detail-ink)),
    0 3px 9px color-mix(in srgb, var(--detail-background) 42%, transparent),
    0 1px 3px color-mix(in srgb, var(--detail-ink) 10%, transparent);
  transform: translateY(1px);
}

.market-detail .market-detail__chart > button.market-detail__chart-toggle:focus-visible {
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, var(--detail-surface) 92%, var(--detail-ink)),
    0 8px 18px color-mix(in srgb, var(--detail-background) 46%, transparent),
    0 2px 6px color-mix(in srgb, var(--detail-ink) 10%, transparent);
  outline: 2px solid var(--focus);
  outline-offset: 3px;
}

.market-detail__indicator-legend {
  align-items: center;
  background: var(--detail-surface);
  display: grid;
  gap: 6px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  height: 32px;
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

.market-detail__chart-panel.is-expanded .market-detail__chart > button.market-detail__chart-toggle {
  left: 10px;
  top: 8px;
}

.market-detail__content-panel {
  background: var(--detail-surface);
  height: 272px;
  min-width: 0;
  overflow: hidden;
}

.market-detail__trades {
  background: var(--detail-surface);
  min-height: 272px;
}

.market-detail__overview {
  padding: 20px 16px;
}

.market-detail__overview > header {
  align-items: center;
  display: flex;
  gap: 12px;
  min-width: 0;
}

.market-detail__overview > header > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.market-detail__overview > header strong {
  font-family: var(--data-font);
  font-size: 17px;
  font-variant-numeric: tabular-nums;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-detail__overview > header span,
.market-detail__overview dt {
  color: var(--detail-muted);
  font-size: 11px;
}

.market-detail__overview dl {
  display: grid;
  gap: 0;
  margin: 16px 0 0;
}

.market-detail__overview dl > div {
  align-items: center;
  border-top: 1px solid color-mix(in srgb, var(--detail-line) 64%, transparent);
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 40px;
}

.market-detail__overview dd {
  font-family: var(--data-font);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  margin: 0;
  max-width: 190px;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  grid-template-columns: 44px 44px minmax(0, 1fr);
  height: calc(67px + env(safe-area-inset-bottom));
  left: 50%;
  max-width: var(--app-max-width);
  padding:
    6px max(16px, env(safe-area-inset-right))
    calc(9px + env(safe-area-inset-bottom))
    max(16px, env(safe-area-inset-left));
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
    padding-left: max(12px, env(safe-area-inset-left));
    padding-right: max(12px, env(safe-area-inset-right));
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

  .market-detail__content-tabs button {
    font-size: 10px;
  }

  .market-detail__trade-head,
  .market-detail__trade {
    gap: 5px;
    grid-template-columns: minmax(0, 1fr) 66px 46px;
    padding-inline: 8px;
  }

  .market-detail__actions {
    gap: 10px;
    grid-template-columns: 44px 44px minmax(0, 1fr);
    padding-left: max(9px, env(safe-area-inset-left));
    padding-right: max(9px, env(safe-area-inset-right));
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

  .market-detail .market-detail__chart > button.market-detail__chart-toggle {
    transition: none;
  }

  .market-detail .market-detail__chart > button.market-detail__chart-toggle:active {
    transform: none;
  }

  .spin {
    animation: none;
  }
}
</style>
