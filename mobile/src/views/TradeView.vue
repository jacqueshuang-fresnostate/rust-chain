<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowLeft,
  ArrowLeftRight,
  Bitcoin,
  BriefcaseBusiness,
  CheckCircle2,
  ChevronDown,
  CirclePlus,
  Download,
  History,
  Info,
  List,
  RefreshCcw,
  Share2,
  Star,
  X,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import MobileMarketChart from '@/components/MobileMarketChart.vue'
import OrderBookPanel from '@/components/OrderBookPanel.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchKlines, fetchOrderBook, fetchRecentTrades } from '@/api/market'
import {
  createMarketDetailStreamSession,
  type MarketDetailStreamContext,
} from '@/api/marketDetailStream'
import {
  mergeMarketTradeHistory,
  mergeMarketTrades,
  normalizeMarketKlineInterval,
  type MarketKlineInterval,
} from '@/api/marketSocketProtocol'
import {
  fetchMarginProducts,
  fetchMarginWallets,
  placeMarginOrder,
  placeSpotOrder,
  type MarginPosition,
  updateMarginLeverage,
} from '@/api/trading'
import { fetchWalletAccounts } from '@/api/wallet'
import { publicMarketWebSocketUrl } from '@/config/app'
import { formatAmount, formatPrice, normalizeSymbol } from '@/core/format'
import { goBackOr } from '@/core/navigation'
import { quantityForBalancePercentage } from '@/core/tradeForm'
import { currentIntlLocale } from '@/i18n'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'
import { useNavigationStore } from '@/stores/navigation'
import type { KlinePoint, MarginProduct, OrderBookLevel, TradePrint, WalletAccount } from '@/core/types'

const route = useRoute()
const router = useRouter()
const marketStore = useMarketStore()
const session = useSessionStore()
const navigation = useNavigationStore()
const { t } = useI18n()
const mode = ref<'spot' | 'contract'>(route.query.mode === 'contract' ? 'contract' : 'spot')
const side = ref<'buy' | 'sell'>('buy')
const orderType = ref<'limit' | 'market'>('limit')
const price = ref('')
const quantity = ref('')
const percentage = ref(0)
const leverage = ref(5)
const marginMode = ref<'isolated'>('isolated')
const products = ref<MarginProduct[]>([])
const spotWallets = ref<WalletAccount[]>([])
const marginWallets = ref<WalletAccount[]>([])
const marginPositions = ref<MarginPosition[]>([])
const bids = ref<OrderBookLevel[]>([])
const asks = ref<OrderBookLevel[]>([])
const points = ref<KlinePoint[]>([])
const trades = ref<TradePrint[]>([])
const interval = ref<MarketKlineInterval>('15m')
const marketDataPanel = ref<'orderBook' | 'trades'>('orderBook')
const spotChartOpen = ref(false)
const liveDetailActive = ref(false)
const liveDetailUpdatedAt = ref(0)
const feedback = ref('')
const feedbackTone = ref<'success' | 'error'>('error')
const submitting = ref(false)
const settingsSaving = ref(false)
const depthLoading = ref(false)
const depthError = ref(false)
const chartLoading = ref(false)
const productsLoading = ref(false)
const balancesLoading = ref(false)
const balancesError = ref(false)
const confirmOpen = ref(false)
const confirmDialog = ref<HTMLElement | null>(null)
const reviewButton = ref<HTMLButtonElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''
let marketRequestVersion = 0
const FAVORITES_STORAGE_KEY = 'hippo-mobile-market-favorites'

function loadFavoriteSymbols(): Set<string> {
  if (typeof window === 'undefined') return new Set()
  try {
    const stored = JSON.parse(window.localStorage.getItem(FAVORITES_STORAGE_KEY) || '[]')
    return new Set(Array.isArray(stored) ? stored.filter((item): item is string => typeof item === 'string') : [])
  } catch {
    return new Set()
  }
}

const favoriteSymbols = ref(loadFavoriteSymbols())

const pairSymbol = computed(() => String(route.params.symbol || 'BTC_USDT').replace(/[_-]/g, '/').toUpperCase())
const isSpotMode = computed(() => mode.value === 'spot')
const ticker = computed(() => marketStore.tickerFor(pairSymbol.value))
const baseAsset = computed(() => pairSymbol.value.split('/')[0] || '')
const quoteAsset = computed(() => pairSymbol.value.split('/')[1] || 'USDT')
const isFavorite = computed(() => favoriteSymbols.value.has(pairSymbol.value))
const spotVisibleBalances = computed(() => spotWallets.value.filter((wallet) => (
  [baseAsset.value, quoteAsset.value].includes(wallet.symbol)
  && wallet.available + wallet.frozen + wallet.locked > 0
)))
const selectedProduct = computed(() => products.value.find((product) => normalizeSymbol(product.symbol) === normalizeSymbol(pairSymbol.value)))
const visibleMarginPositions = computed(() => {
  const product = selectedProduct.value
  if (!product) return []
  return marginPositions.value
    .filter((position) => (
      position.productId === product.id
      && !['closed', 'liquidated', 'cancelled', 'canceled'].includes(position.status.toLowerCase())
    ))
    .slice(0, 3)
})
const currentPrice = computed(() => trades.value[0]?.price ?? points.value.at(-1)?.close ?? ticker.value?.lastPrice ?? 0)
const isLive = computed(() => currentPrice.value > 0 && (!marketStore.error || liveDetailActive.value))
const selectedOrderType = computed(() => mode.value === 'contract' ? 'market' : orderType.value)
const effectivePrice = computed(() => selectedOrderType.value === 'limit' ? Number(price.value) : currentPrice.value)
const availableAsset = computed(() => {
  if (mode.value === 'contract') return selectedProduct.value?.marginAssetSymbol || quoteAsset.value
  return side.value === 'buy' ? quoteAsset.value : baseAsset.value
})
const availableBalance = computed(() => {
  const wallets = mode.value === 'contract' ? marginWallets.value : spotWallets.value
  return wallets.find((wallet) => wallet.symbol === availableAsset.value)?.available || 0
})
const amountValue = computed({
  get: () => {
    const value = Number(quantity.value) * effectivePrice.value
    return Number.isFinite(value) && value > 0 ? String(Number(value.toFixed(8))) : ''
  },
  set: (value: string) => {
    const orderAmount = Number(value)
    quantity.value = Number.isFinite(orderAmount) && orderAmount > 0 && effectivePrice.value > 0
      ? String(Number((orderAmount / effectivePrice.value).toFixed(8)))
      : ''
  },
})
const contractNotionalValue = computed(() => {
  const marginAmount = Number(quantity.value)
  const value = marginAmount * leverage.value
  return Number.isFinite(value) && value > 0 ? String(Number(value.toFixed(8))) : ''
})
const contractOpenQuantity = computed(() => {
  if (currentPrice.value <= 0) return 0
  return (availableBalance.value * leverage.value) / currentPrice.value
})
const orderButtonLabel = computed(() => {
  if (mode.value === 'contract') {
    return side.value === 'buy'
      ? t('trade.longAction', { leverage: leverage.value })
      : t('trade.shortAction', { leverage: leverage.value })
  }
  return side.value === 'buy'
    ? t('trade.buyAsset', { asset: baseAsset.value })
    : t('trade.sellAsset', { asset: baseAsset.value })
})
const feedbackIsPositive = computed(() => feedbackTone.value === 'success')

const detailStreamSession = createMarketDetailStreamSession({
  getUrl: publicMarketWebSocketUrl,
  onDepth: (_context, snapshot) => {
    liveDetailActive.value = true
    liveDetailUpdatedAt.value = Date.now()
    bids.value = snapshot.bids
    asks.value = snapshot.asks
    depthError.value = false
    depthLoading.value = false
  },
  onTrade: (_context, trade) => {
    liveDetailActive.value = true
    liveDetailUpdatedAt.value = Date.now()
    trades.value = mergeMarketTrades(trades.value, trade, 16)
  },
  onKlines: (_context, nextPoints) => {
    liveDetailActive.value = true
    liveDetailUpdatedAt.value = Date.now()
    points.value = nextPoints
    chartLoading.value = false
  },
})

function setFeedback(message: string, tone: 'success' | 'error' = 'error'): void {
  feedback.value = message
  feedbackTone.value = tone
}

function isCurrentMarketRequest(context: MarketDetailStreamContext, version: number): boolean {
  return version === marketRequestVersion
    && detailStreamSession.isCurrent(context, pairSymbol.value, interval.value, version)
}

async function loadMarketData(forceMarket = false): Promise<void> {
  const version = ++marketRequestVersion
  const symbol = pairSymbol.value
  const selectedInterval = interval.value
  depthLoading.value = true
  chartLoading.value = true
  depthError.value = false
  liveDetailActive.value = false
  liveDetailUpdatedAt.value = 0
  bids.value = []
  asks.value = []
  trades.value = []
  points.value = []

  const liveContext = detailStreamSession.replace(symbol, selectedInterval, version)
  const klineRequest = detailStreamSession.beginKlineRequest(liveContext)
  if (forceMarket) void marketStore.refresh(true)
  const [klineResult, depthResult, tradesResult] = await Promise.allSettled([
    fetchKlines(symbol, selectedInterval),
    fetchOrderBook(symbol),
    fetchRecentTrades(symbol),
  ])
  if (!isCurrentMarketRequest(liveContext, version)) return

  if (klineRequest && detailStreamSession.isCurrentKlineRequest(klineRequest)) {
    const restPoints = klineResult.status === 'fulfilled' ? klineResult.value : []
    const mergedPoints = detailStreamSession.resolveKlineRequest(klineRequest, restPoints)
    if (mergedPoints) points.value = mergedPoints
  }
  if (!liveContext.depthReceived) {
    bids.value = depthResult.status === 'fulfilled' ? depthResult.value.bids : []
    asks.value = depthResult.status === 'fulfilled' ? depthResult.value.asks : []
    depthError.value = depthResult.status === 'rejected'
  }
  const restTrades = tradesResult.status === 'fulfilled' ? tradesResult.value : []
  trades.value = mergeMarketTradeHistory(trades.value, restTrades, 16)
  chartLoading.value = false
  depthLoading.value = false
}

async function retryMarket(): Promise<void> {
  await loadMarketData(true)
}

async function refreshIntervalKlines(selectedInterval: MarketKlineInterval): Promise<void> {
  const version = ++marketRequestVersion
  const symbol = pairSymbol.value
  chartLoading.value = true
  liveDetailActive.value = false
  const liveContext = detailStreamSession.replace(symbol, selectedInterval, version)
  const klineRequest = detailStreamSession.beginKlineRequest(liveContext)
  if (!klineRequest) {
    chartLoading.value = false
    return
  }
  try {
    const restPoints = await fetchKlines(symbol, selectedInterval)
    if (!isCurrentMarketRequest(liveContext, version) || !detailStreamSession.isCurrentKlineRequest(klineRequest)) return
    const mergedPoints = detailStreamSession.resolveKlineRequest(klineRequest, restPoints)
    if (mergedPoints) points.value = mergedPoints
  } catch {
    if (!isCurrentMarketRequest(liveContext, version) || liveContext.klineReceived) return
    points.value = []
  } finally {
    if (isCurrentMarketRequest(liveContext, version)) chartLoading.value = false
  }
}

async function loadMarginProducts(): Promise<void> {
  if (mode.value !== 'contract' || !session.isAuthenticated) {
    products.value = []
    productsLoading.value = false
    return
  }
  productsLoading.value = true
  try {
    products.value = await fetchMarginProducts()
    const product = selectedProduct.value
    if (product) {
      leverage.value = product.leverageLevels.includes(5) ? 5 : product.leverageLevels[0] || 1
      marginMode.value = 'isolated'
    }
  } catch {
    products.value = []
  } finally {
    productsLoading.value = false
  }
}

async function loadTradingBalances(): Promise<void> {
  if (!session.isAuthenticated) {
    spotWallets.value = []
    marginWallets.value = []
    marginPositions.value = []
    balancesLoading.value = false
    balancesError.value = false
    return
  }
  balancesLoading.value = true
  balancesError.value = false
  try {
    if (mode.value === 'contract') {
      const margin = await fetchMarginWallets()
      marginWallets.value = margin.wallets
      marginPositions.value = margin.positions
    } else {
      spotWallets.value = await fetchWalletAccounts()
      marginPositions.value = []
    }
  } catch {
    if (mode.value === 'contract') {
      marginWallets.value = []
      marginPositions.value = []
    }
    else spotWallets.value = []
    balancesError.value = true
  } finally {
    balancesLoading.value = false
  }
}

function setQuantity(percent: number): void {
  percentage.value = percent
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  const nextQuantity = quantityForBalancePercentage({
    available: availableBalance.value,
    mode: mode.value,
    percentage: percent / 100,
    price: effectivePrice.value,
    side: side.value,
  })
  quantity.value = nextQuantity > 0 ? String(Number(nextQuantity.toFixed(8))) : ''
}

function chooseInterval(value: string): void {
  const selectedInterval = normalizeMarketKlineInterval(value)
  if (!selectedInterval || interval.value === selectedInterval) return
  interval.value = selectedInterval
  void refreshIntervalKlines(selectedInterval)
}

function selectMarketDataPanel(panel: 'orderBook' | 'trades'): void {
  marketDataPanel.value = panel
}

function formatTradeTime(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return '--'
  const timestamp = value < 1_000_000_000_000 ? value * 1000 : value
  return new Intl.DateTimeFormat(currentIntlLocale(), {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).format(new Date(timestamp))
}

function openPairPicker(): void {
  void router.push({ name: 'markets', query: { purpose: 'trade', mode: mode.value } })
}

function toggleFavorite(): void {
  const next = new Set(favoriteSymbols.value)
  if (next.has(pairSymbol.value)) next.delete(pairSymbol.value)
  else next.add(pairSymbol.value)
  favoriteSymbols.value = next
  try {
    window.localStorage.setItem(FAVORITES_STORAGE_KEY, JSON.stringify([...next]))
  } catch {
    // Keep the in-memory state when persistence is unavailable.
  }
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

function toggleSpotChart(): void {
  spotChartOpen.value = !spotChartOpen.value
}

function toggleSpotOrderType(): void {
  orderType.value = orderType.value === 'limit' ? 'market' : 'limit'
}

function openDeposit(): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/assets/deposit' } })
    return
  }
  void router.push({ name: 'deposit-asset' })
}

function openAssets(): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/assets' } })
    return
  }
  void router.push({ name: 'assets' })
}

function goBack(): void {
  void goBackOr(router, { name: 'markets' })
}

function openLogin(): void {
  void router.push({ name: 'login', query: { redirect: route.fullPath } })
}

function openOrders(tab: 'spot' | 'positions' | 'history' = 'spot'): void {
  void router.push({ name: 'orders', query: { tab } })
}

async function changeLeverage(): Promise<void> {
  const product = selectedProduct.value
  if (!product) {
    setFeedback(t('trade.unavailableContract'))
    return
  }
  const levels = product.leverageLevels.length ? product.leverageLevels : [product.maxLeverage]
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  const nextIndex = (levels.indexOf(leverage.value) + 1) % levels.length
  const nextLeverage = levels[nextIndex]
  settingsSaving.value = true
  feedback.value = ''
  try {
    await updateMarginLeverage(product.id, nextLeverage)
    leverage.value = nextLeverage
    setFeedback(t('trade.leverageChanged'), 'success')
  } catch (reason) {
    setFeedback(apiErrorMessage(reason, t('trade.leverageChangeFailed')))
  } finally {
    settingsSaving.value = false
  }
}

function reviewOrder(): void {
  feedback.value = ''
  const orderAmount = Number(quantity.value)
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  if (!isLive.value) {
    setFeedback(t('trade.marketUnavailable'))
    return
  }
  if (mode.value === 'contract' && !selectedProduct.value) {
    setFeedback(t('trade.unavailableContract'))
    return
  }
  if (!Number.isFinite(orderAmount) || orderAmount <= 0 || !Number.isFinite(effectivePrice.value) || effectivePrice.value <= 0) {
    setFeedback(t('trade.invalidOrder'))
    return
  }
  confirmOpen.value = true
}

function reviewContractOrder(nextSide: 'buy' | 'sell'): void {
  side.value = nextSide
  reviewOrder()
}

function closeConfirm(): void {
  if (submitting.value) return
  confirmOpen.value = false
}

async function submitOrder(): Promise<void> {
  feedback.value = ''
  const orderAmount = Number(quantity.value)
  const submittedOrderType = mode.value === 'contract' ? 'market' : orderType.value
  const limitPrice = effectivePrice.value
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  if (!isLive.value) {
    setFeedback(t('trade.marketUnavailable'))
    return
  }
  if (!Number.isFinite(orderAmount) || orderAmount <= 0 || !Number.isFinite(limitPrice) || limitPrice <= 0) {
    setFeedback(t('trade.invalidOrder'))
    return
  }

  submitting.value = true
  try {
    if (mode.value === 'spot') {
      await placeSpotOrder({
        symbol: pairSymbol.value,
        side: side.value,
        type: submittedOrderType,
        price: limitPrice,
        quantity: orderAmount,
      })
    } else {
      if (!selectedProduct.value) throw new Error(t('trade.unavailableContract'))
      await placeMarginOrder({
        productId: selectedProduct.value.id,
        side: side.value === 'buy' ? 'long' : 'short',
        marginMode: marginMode.value,
        leverage: leverage.value,
        marginAmount: orderAmount,
      })
    }
    setFeedback(t('trade.orderSubmitted'), 'success')
    quantity.value = ''
    percentage.value = 0
    confirmOpen.value = false
    await loadTradingBalances()
  } catch (reason) {
    setFeedback(apiErrorMessage(reason, t('trade.orderFailed')))
  } finally {
    submitting.value = false
  }
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

onMounted(async () => {
  await marketStore.refresh()
})

watch(pairSymbol, (symbol) => {
  navigation.rememberTradeSymbol(symbol)
  marketDataPanel.value = 'orderBook'
  spotChartOpen.value = false
  void loadMarketData()
}, { immediate: true })

watch(() => route.query.mode, (nextMode) => {
  mode.value = nextMode === 'contract' ? 'contract' : 'spot'
  navigation.rememberTradeMode(mode.value)
  percentage.value = 0
  quantity.value = ''
  void loadMarginProducts()
}, { immediate: true })

watch([mode, () => session.isAuthenticated], () => {
  void loadTradingBalances()
}, { immediate: true })

watch(currentPrice, (value) => {
  if (!price.value && value > 0) price.value = String(value)
}, { immediate: true })

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

onBeforeUnmount(() => {
  detailStreamSession.stop()
  document.body.style.overflow = previousBodyOverflow
})
</script>

<template>
  <main
    class="view trade-view prototype-root-view"
    :class="mode === 'contract' ? 'contract-trade' : 'spot-trade'"
    :data-trade-mode="mode"
    :data-trade-surface="mode"
  >
    <template v-if="isSpotMode">
      <header class="spot-pencil-header" data-pencil-source="yzOPc-bo8k5">
        <button class="spot-header-control" type="button" :aria-label="t('common.back')" @click="goBack">
          <ArrowLeft :size="24" aria-hidden="true" />
        </button>
        <button class="spot-pair-control" type="button" :aria-label="t('markets.pickerTitle')" @click="openPairPicker">
          <span v-if="baseAsset === 'BTC'" class="spot-bitcoin-mark" role="img" :aria-label="t('common.assetIcon', { symbol: baseAsset })">
            <Bitcoin :size="14" aria-hidden="true" />
          </span>
          <AssetMark v-else :symbol="baseAsset" :src="ticker?.iconUrl" :size="24" />
          <span>
            <strong>{{ pairSymbol }}</strong>
            <small
              class="numeric"
              :class="(ticker?.changePercent || 0) >= 0 ? 'positive' : 'negative'"
            >
              {{ ticker ? `${ticker.changePercent >= 0 ? '+' : ''}${ticker.changePercent.toFixed(2)}%` : '--' }}
            </small>
          </span>
          <ChevronDown :size="18" aria-hidden="true" />
        </button>
        <div class="spot-header-actions">
          <button
            class="spot-header-control"
            :class="{ active: isFavorite }"
            type="button"
            :aria-label="t(isFavorite ? 'rootPrototype.removeFavorite' : 'rootPrototype.addFavorite', { symbol: pairSymbol })"
            :aria-pressed="isFavorite"
            @click="toggleFavorite"
          >
            <Star :size="23" :fill="isFavorite ? 'currentColor' : 'none'" aria-hidden="true" />
          </button>
          <button class="spot-header-control" type="button" :aria-label="t('marketDetail.share')" @click="shareMarket">
            <Share2 :size="22" aria-hidden="true" />
          </button>
        </div>
      </header>

      <section
        class="spot-pencil-workspace"
        data-spot-layout="pencil-split"
        :data-live-detail="liveDetailActive ? 'stream' : 'snapshot'"
      >
        <div class="spot-order-console" data-order-surface="live">
          <div class="spot-side-switch" role="group" :aria-label="t('trade.category')">
            <button
              type="button"
              :class="{ active: side === 'buy' }"
              :aria-pressed="side === 'buy'"
              @click="side = 'buy'"
            >
              {{ t('trade.buy') }}
            </button>
            <button
              type="button"
              :class="{ active: side === 'sell' }"
              :aria-pressed="side === 'sell'"
              @click="side = 'sell'"
            >
              {{ t('trade.sell') }}
            </button>
          </div>

          <button
            class="spot-type-field"
            type="button"
            :aria-label="t('trade.category')"
            @click="toggleSpotOrderType"
          >
            <Info :size="14" aria-hidden="true" />
            <strong>{{ orderType === 'limit' ? t('trade.limitOrderShort') : t('trade.marketOrderShort') }}</strong>
            <ChevronDown :size="15" aria-hidden="true" />
          </button>

          <label class="spot-field-shell">
            <span>{{ t('trade.priceField', { asset: quoteAsset }) }}</span>
            <input
              v-model="price"
              inputmode="decimal"
              :readonly="orderType === 'market'"
              :placeholder="orderType === 'market' ? t('trade.marketPrice') : t('trade.pricePlaceholder')"
            />
          </label>
          <label class="spot-field-shell spot-field-shell--unit">
            <span>{{ t('common.quantity') }}</span>
            <input v-model="quantity" inputmode="decimal" :placeholder="t('trade.quantityPlaceholder')" />
            <b>{{ baseAsset }}</b>
          </label>

          <div class="spot-percentage" role="group" :aria-label="t('rootPrototype.balancePercentage')">
            <button
              v-for="value in [0, 25, 50, 75, 100]"
              :key="value"
              type="button"
              :class="{ active: percentage === value }"
              :aria-pressed="percentage === value"
              @click="setQuantity(value)"
            >
              <i aria-hidden="true" />
              <span class="numeric">{{ value }}%</span>
            </button>
          </div>

          <label class="spot-field-shell spot-field-shell--unit">
            <span>{{ t('trade.turnover') }}</span>
            <input v-model="amountValue" inputmode="decimal" :placeholder="t('trade.turnover')" />
            <b>{{ quoteAsset }}</b>
          </label>

          <div class="spot-tpsl-row" aria-disabled="true">
            <span aria-hidden="true" />
            <b>{{ t('rootPrototype.takeProfitStopLoss') }}</b>
          </div>

          <div class="spot-available-row">
            <span>{{ t('trade.available') }}</span>
            <button v-if="!session.isAuthenticated" type="button" @click="openLogin">
              {{ t('trade.viewAfterLogin') }}
            </button>
            <button v-else-if="balancesError" type="button" :disabled="balancesLoading" @click="loadTradingBalances">
              {{ t('common.retry') }}
            </button>
            <strong v-else class="numeric">
              {{ balancesLoading ? t('trade.loadBalance') : `${formatAmount(availableBalance)} ${availableAsset}` }}
              <CirclePlus :size="14" aria-hidden="true" />
            </strong>
          </div>

          <p
            v-if="feedback || balancesLoading"
            class="spot-trade-feedback"
            :class="feedback ? (feedbackIsPositive ? 'positive' : 'negative') : ''"
            aria-live="polite"
          >
            {{ feedback || t('trade.loadBalance') }}
          </p>

          <button
            ref="reviewButton"
            class="spot-submit-order"
            :class="side"
            type="button"
            :disabled="submitting || !isLive"
            @click="reviewOrder"
          >
            {{ submitting ? t('trade.submittingOrder') : orderButtonLabel }}
          </button>
        </div>

        <OrderBookPanel
          class="spot-mini-book"
          :asks="asks"
          :bids="bids"
          :current-price="currentPrice"
          :base-asset="baseAsset"
          :loading="depthLoading"
          :quote-asset="quoteAsset"
          layout="mini"
        />
      </section>

      <section class="spot-account-workspace" :aria-label="t('trade.positionsAndAssets')">
        <nav class="spot-account-tabs" :aria-label="t('orders.category')">
          <button class="active" type="button" @click="openOrders('spot')">
            {{ t('trade.orders') }} <ChevronDown :size="12" aria-hidden="true" />
          </button>
          <button type="button" @click="openOrders('positions')">
            {{ t('trade.positionsAndAssets') }} <ChevronDown :size="12" aria-hidden="true" />
          </button>
          <button type="button" :aria-label="t('trade.orderHistory')" @click="openOrders('history')">
            <History :size="19" aria-hidden="true" />
          </button>
        </nav>

        <div class="spot-order-filter">
          <span><i aria-hidden="true" />{{ t('trade.onlyCurrent') }}</span>
          <button type="button" @click="openOrders('spot')">{{ t('orders.cancelAll') }}</button>
        </div>

        <div v-if="balancesLoading" class="spot-account-state" role="status">
          <RefreshCcw :size="22" class="spin" aria-hidden="true" />
          <strong>{{ t('trade.loadBalance') }}</strong>
        </div>
        <div v-else-if="balancesError" class="spot-account-state" role="alert">
          <strong>{{ t('assets.loadFailed') }}</strong>
          <button type="button" @click="loadTradingBalances">{{ t('common.retry') }}</button>
        </div>
        <div v-else-if="session.isAuthenticated && spotVisibleBalances.length" class="spot-balance-preview">
          <article v-for="wallet in spotVisibleBalances" :key="wallet.symbol">
            <span>{{ wallet.symbol }}</span>
            <strong class="numeric">{{ formatAmount(wallet.available) }}</strong>
            <small>{{ t('assets.frozen', { amount: formatAmount(wallet.frozen + wallet.locked) }) }}</small>
          </article>
        </div>
        <div v-else class="spot-account-state">
          <strong>{{ t('trade.spotAssetEmpty') }}</strong>
          <span>{{ t('trade.spotAssetEmptyHint') }}</span>
          <div class="spot-account-actions">
            <button type="button" @click="openDeposit">
              <i><Download :size="21" aria-hidden="true" /></i>
              {{ t('assets.deposit') }}
            </button>
            <button type="button" @click="openAssets">
              <i><ArrowLeftRight :size="21" aria-hidden="true" /></i>
              {{ t('assets.transfer') }}
            </button>
          </div>
        </div>
      </section>

      <button
        class="spot-chart-entry"
        type="button"
        :aria-expanded="spotChartOpen"
        aria-controls="spot-local-chart"
        @click="toggleSpotChart"
      >
        <span>{{ pairSymbol }} {{ t('marketDetail.chart') }}</span>
        <ChevronDown :size="17" :class="{ open: spotChartOpen }" aria-hidden="true" />
      </button>

      <section v-if="spotChartOpen" id="spot-local-chart" class="spot-chart-drawer">
        <div class="chart-tools">
          <div class="interval-rail" role="group" :aria-label="t('marketDetail.chart')">
            <button
              v-for="time in ['1m', '5m', '15m', '1h', '1d']"
              :key="time"
              type="button"
              :class="{ active: interval === time }"
              :aria-pressed="interval === time"
              @click="chooseInterval(time)"
            >
              {{ time }}
            </button>
          </div>
          <div class="chart-action-rail" role="group" :aria-label="t('marketDetail.actions')">
            <button type="button" :aria-label="t('markets.refresh')" @click="retryMarket">
              <RefreshCcw :size="17" aria-hidden="true" />
            </button>
            <button type="button" :aria-label="t('trade.viewOpenOrders')" @click="openOrders('spot')">
              <List :size="17" aria-hidden="true" />
            </button>
          </div>
        </div>
        <div class="spot-chart-canvas" :aria-busy="chartLoading">
          <MobileMarketChart :points="points" :loading="chartLoading" :interval="interval" :symbol="pairSymbol" />
        </div>
        <div class="spot-market-data__tabs" role="tablist" :aria-label="t('marketDetail.marketData')">
          <button
            id="spot-order-book-tab"
            type="button"
            role="tab"
            :class="{ active: marketDataPanel === 'orderBook' }"
            :aria-selected="marketDataPanel === 'orderBook'"
            aria-controls="spot-order-book-panel"
            @click="selectMarketDataPanel('orderBook')"
          >
            {{ t('orderBook.title') }}
          </button>
          <button
            id="spot-trades-tab"
            type="button"
            role="tab"
            :class="{ active: marketDataPanel === 'trades' }"
            :aria-selected="marketDataPanel === 'trades'"
            aria-controls="spot-trades-panel"
            @click="selectMarketDataPanel('trades')"
          >
            {{ t('marketDetail.latestTrades') }}
          </button>
        </div>
        <OrderBookPanel
          v-if="marketDataPanel === 'orderBook'"
          id="spot-order-book-panel"
          class="trade-order-book"
          role="tabpanel"
          aria-labelledby="spot-order-book-tab"
          :asks="asks"
          :bids="bids"
          :current-price="currentPrice"
          :base-asset="baseAsset"
          :loading="depthLoading"
          :quote-asset="quoteAsset"
          layout="split"
        />
        <section
          v-else
          id="spot-trades-panel"
          class="spot-recent-trades"
          role="tabpanel"
          aria-labelledby="spot-trades-tab"
        >
          <header>
            <span>{{ t('marketDetail.price') }} <small>{{ quoteAsset }}</small></span>
            <span>{{ t('marketDetail.quantity') }} <small>{{ baseAsset }}</small></span>
            <span>{{ t('trade.tradeTime') }}</span>
          </header>
          <div v-if="trades.length" class="spot-recent-trades__rows">
            <div v-for="trade in trades.slice(0, 8)" :key="trade.id" class="spot-recent-trades__row">
              <strong class="numeric" :class="trade.side === 'buy' ? 'positive' : 'negative'">
                {{ formatPrice(trade.price) }}
              </strong>
              <span class="numeric">{{ formatAmount(trade.quantity) }}</span>
              <time class="numeric" :datetime="new Date(trade.time).toISOString()">{{ formatTradeTime(trade.time) }}</time>
            </div>
          </div>
          <p v-else class="spot-recent-trades__empty" role="status">
            {{ depthLoading ? t('common.loading') : t('trade.noRecentTrades') }}
          </p>
        </section>
      </section>

      <p class="sr-only" aria-live="polite">
        {{ liveDetailActive ? t('trade.restAndSocket') : t('marketDetail.snapshotData') }}
      </p>
    </template>

    <template v-else>
      <section
        class="contract-pencil-surface"
        data-pencil-source="by3G9 pKHeU"
        data-instrument-hero="pair-price"
        data-market-quote="live"
        data-order-surface="live"
        :data-contract-state="visibleMarginPositions.length ? 'positions' : 'empty'"
      >
        <header class="contract-pencil-header">
          <button class="contract-header-control" type="button" :aria-label="t('common.back')" @click="goBack">
            <ArrowLeft :size="22" aria-hidden="true" />
          </button>
          <div class="trade-quote">
            <button class="contract-pair-selector" type="button" :aria-label="t('markets.pickerTitle')" @click="openPairPicker">
              <span>
                <strong>{{ pairSymbol.replace('/', '') }}</strong>
                <ChevronDown :size="14" aria-hidden="true" />
              </span>
              <small class="numeric" :class="(ticker?.changePercent || 0) >= 0 ? 'positive' : 'negative'">
                {{ t('trade.perpetual') }}
                {{ ticker ? `${ticker.changePercent >= 0 ? '+' : ''}${ticker.changePercent.toFixed(2)}%` : '' }}
              </small>
            </button>
          </div>
          <button
            class="contract-header-control"
            :class="{ active: isFavorite }"
            type="button"
            :aria-label="t(isFavorite ? 'rootPrototype.removeFavorite' : 'rootPrototype.addFavorite', { symbol: pairSymbol })"
            :aria-pressed="isFavorite"
            @click="toggleFavorite"
          >
            <Star :size="19" :fill="isFavorite ? 'currentColor' : 'none'" aria-hidden="true" />
          </button>
          <p class="sr-only">
            {{ t('marketDetail.high24h') }} {{ ticker ? formatPrice(ticker.highPrice) : t('common.marketUnavailable') }} ·
            {{ t('marketDetail.low24h') }} {{ ticker ? formatPrice(ticker.lowPrice) : t('common.marketUnavailable') }}
          </p>
        </header>

        <section
          class="contract-pencil-module"
          data-instrument-plate="market-and-order"
          :data-live-detail="liveDetailActive ? 'stream' : 'snapshot'"
          :data-market-data-panel="marketDataPanel"
        >
          <div class="chart-panel trade-chart-panel">
            <div class="contract-book-status">
              <span>{{ t('trade.restAndSocket') }}</span>
              <strong>{{ liveDetailActive ? t('trade.depthLive') : t('trade.depthSnapshot') }}</strong>
            </div>
            <div class="trade-order-book">
              <OrderBookPanel
                class="contract-mini-book"
                :asks="asks"
                :bids="bids"
                :current-price="currentPrice"
                :base-asset="baseAsset"
                :loading="depthLoading"
                :quote-asset="quoteAsset"
                layout="mini"
              />
            </div>
            <p class="chart-semantic-summary">
              {{ pairSymbol }} · {{ ticker ? formatPrice(currentPrice) : t('common.marketUnavailable') }}
            </p>
          </div>

          <div class="trade-console">
            <div class="contract-open-close" role="group" :aria-label="t('orders.stateCategory')">
              <button type="button" class="active" aria-pressed="true">
                {{ t('ledger.typeMarginOpen') }}
              </button>
              <button type="button" aria-pressed="false" @click="openOrders('positions')">
                {{ t('ledger.typeMarginClose') }}
              </button>
            </div>

            <div class="contract-mode-row" :aria-label="t('trade.settings')">
              <button type="button" disabled>
                <span>{{ t('trade.isolated') }}</span>
                <ChevronDown :size="12" aria-hidden="true" />
              </button>
              <button type="button" :disabled="settingsSaving || productsLoading" @click="changeLeverage">
                <span class="numeric">{{ leverage }}x</span>
                <ChevronDown :size="12" aria-hidden="true" />
              </button>
            </div>

            <button
              class="contract-order-type"
              type="button"
              :class="{ active: selectedOrderType === 'market' }"
              disabled
            >
              <span>{{ t('trade.marketOrderShort') }}</span>
              <Info :size="11" aria-hidden="true" />
              <ChevronDown :size="12" aria-hidden="true" />
            </button>

            <div class="contract-price-row">
              <label class="contract-field contract-price-field">
                <span>{{ t('common.price') }}</span>
                <input
                  class="numeric"
                  :value="currentPrice > 0 ? formatPrice(currentPrice) : ''"
                  :placeholder="t('trade.marketPrice')"
                  inputmode="decimal"
                  readonly
                />
              </label>
              <button type="button" :aria-label="t('marketDetail.latestPrice')" @click="price = String(currentPrice || '')">
                {{ t('trade.marketPrice') }}
              </button>
            </div>

            <label class="contract-field contract-amount-field">
              <span>{{ t('trade.marginField', { asset: availableAsset }) }}</span>
              <input
                v-model="quantity"
                class="numeric"
                inputmode="decimal"
                :placeholder="t('trade.quantityPlaceholder')"
              />
              <b>{{ availableAsset }}</b>
            </label>

            <div class="contract-percentage">
              <div class="percent-row" role="group" :aria-label="t('rootPrototype.balancePercentage')">
                <button
                  v-for="value in [0, 25, 50, 75, 100]"
                  :key="value"
                  type="button"
                  :class="{ active: percentage === value }"
                  :aria-pressed="percentage === value"
                  @click="setQuantity(value)"
                >
                  {{ value }}%
                </button>
              </div>
            </div>

            <div class="contract-tpsl" aria-disabled="true">
              <span aria-hidden="true" />
              {{ t('rootPrototype.takeProfitStopLoss') }}
            </div>

            <dl class="contract-balance-rows">
              <div>
                <dt>{{ t('trade.available') }}</dt>
                <dd v-if="!session.isAuthenticated">
                  <button type="button" @click="openLogin">{{ t('trade.viewAfterLogin') }}</button>
                </dd>
                <dd v-else-if="balancesError">
                  <button type="button" :disabled="balancesLoading" @click="loadTradingBalances">{{ t('common.retry') }}</button>
                </dd>
                <dd v-else class="numeric">
                  {{ balancesLoading ? t('trade.loadBalance') : `${formatAmount(availableBalance)} ${availableAsset}` }}
                </dd>
              </div>
              <div>
                <dt>{{ t('rootPrototype.contractQuantity') }}</dt>
                <dd class="numeric">{{ formatAmount(contractOpenQuantity) }} {{ baseAsset }}</dd>
              </div>
            </dl>

            <p
              v-if="feedback"
              class="contract-feedback"
              :class="feedbackIsPositive ? 'positive' : 'negative'"
              aria-live="polite"
            >
              {{ feedback }}
            </p>

            <button
              ref="reviewButton"
              class="contract-submit contract-submit--long submit-order"
              type="button"
              :disabled="submitting || productsLoading || !isLive"
              @click="reviewContractOrder('buy')"
            >
              {{ t('trade.longAction', { leverage }) }}
            </button>
            <button
              class="contract-submit contract-submit--short"
              type="button"
              :disabled="submitting || productsLoading || !isLive"
              @click="reviewContractOrder('sell')"
            >
              {{ t('trade.shortAction', { leverage }) }}
            </button>
          </div>
        </section>

        <nav class="contract-position-tabs" :aria-label="t('orders.category')">
          <button class="active" type="button" @click="openOrders(mode === 'contract' ? 'positions' : 'spot')">
            {{ t('orders.positions') }} ({{ visibleMarginPositions.length }})
          </button>
          <button type="button" @click="openOrders('positions')">
            {{ t('orders.current') }} <ChevronDown :size="12" aria-hidden="true" />
          </button>
          <button type="button" @click="openAssets">{{ t('nav.assets') }}</button>
          <span aria-hidden="true" />
          <button type="button" :aria-label="t('orders.history')" @click="openOrders('history')">
            <History :size="17" aria-hidden="true" />
          </button>
        </nav>

        <section v-if="visibleMarginPositions.length" class="contract-position-list" :aria-label="t('orders.positions')">
          <article v-for="position in visibleMarginPositions" :key="position.id">
            <div>
              <strong>{{ pairSymbol }}</strong>
              <span :class="position.direction === 'long' ? 'positive' : 'negative'">
                {{ t(position.direction === 'long' ? 'orders.long' : 'orders.short') }}
              </span>
            </div>
            <div class="numeric">
              <span>{{ t('orders.margin') }} {{ formatAmount(position.marginAmount) }} {{ availableAsset }}</span>
              <span>{{ t('orders.entryPrice') }} {{ formatPrice(position.entryPrice) }}</span>
            </div>
          </article>
        </section>
        <section v-else class="contract-position-empty" role="status">
          <span><BriefcaseBusiness :size="24" aria-hidden="true" /></span>
          <strong>{{ session.isAuthenticated ? t('orders.noPositions') : t('trade.ordersLoginHint') }}</strong>
          <button v-if="!session.isAuthenticated" type="button" @click="openLogin">{{ t('common.loginNow') }}</button>
        </section>
      </section>
    </template>

    <div v-if="confirmOpen" class="confirmation-layer">
      <button
        class="confirmation-overlay-dismiss"
        type="button"
        :aria-label="t('common.close')"
        :disabled="submitting"
        tabindex="-1"
        @click="closeConfirm"
      />
      <section
        ref="confirmDialog"
        class="confirmation-sheet"
        role="dialog"
        aria-modal="true"
        :aria-busy="submitting"
        :aria-label="t('common.confirm')"
        tabindex="-1"
        @keydown="trapDialogFocus"
      >
        <header>
          <span class="confirmation-icon"><CheckCircle2 :size="20" /></span>
          <div><span>{{ t('common.confirm') }}</span><h2>{{ orderButtonLabel }}</h2></div>
        </header>
        <p>{{ pairSymbol }} · {{ formatAmount(Number(quantity || 0)) }} {{ mode === 'contract' ? availableAsset : baseAsset }}</p>
        <div class="confirmation-detail">
          <span>{{ t('common.price') }} {{ formatPrice(effectivePrice) }} {{ quoteAsset }}</span>
          <span>
            {{ mode === 'contract' ? t('rootPrototype.estimatedNotional') : t('common.amount') }}
            {{ formatAmount(Number(mode === 'contract' ? contractNotionalValue : amountValue) || 0) }}
            {{ mode === 'contract' ? availableAsset : quoteAsset }}
          </span>
        </div>
        <div class="confirmation-actions">
          <button data-dialog-cancel type="button" :disabled="submitting" @click="closeConfirm">
            <X :size="16" /> {{ t('common.cancel') }}
          </button>
          <button class="confirmation-primary" type="button" :disabled="submitting" @click="submitOrder">
            {{ submitting ? t('trade.submittingOrder') : t('common.confirm') }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style
  scoped
>
.trade-view {
  min-width: 0;
  overflow-x: clip;
  padding-bottom: calc(112px + env(safe-area-inset-bottom));
}

.spot-trade {
  background: var(--page);
  color: var(--text);
  min-height: 100dvh;
  padding-bottom: calc(84px + env(safe-area-inset-bottom));
}

.spot-pencil-header {
  align-items: center;
  background: color-mix(in srgb, var(--page) 94%, transparent);
  backdrop-filter: blur(18px) saturate(1.1);
  display: grid;
  gap: 4px;
  grid-template-columns: 44px minmax(0, 1fr) auto;
  height: 64px;
  padding: 0 16px;
  position: sticky;
  top: 0;
  z-index: 42;
}

.spot-header-control,
.spot-pair-control {
  appearance: none;
  background: transparent;
  border: 0;
  color: var(--text);
  min-width: 0;
}

.spot-header-control {
  align-items: center;
  border-radius: 999px;
  display: inline-flex;
  height: 44px;
  justify-content: center;
  padding: 0;
  width: 44px;
}

.spot-header-control.active {
  color: var(--positive);
}

.spot-header-control:active {
  background: var(--surface-2);
  transform: scale(.96);
}

.spot-pair-control {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: 24px minmax(0, auto) 18px;
  justify-content: start;
  min-height: 52px;
  padding: 0 2px 0 4px;
  text-align: left;
}

.spot-pair-control > span {
  display: grid;
  gap: 1px;
  min-width: 0;
}

.spot-pair-control > .spot-bitcoin-mark {
  align-items: center;
  background: var(--bitcoin-orange);
  border-radius: 999px;
  color: var(--on-negative);
  display: inline-flex;
  height: 24px;
  justify-content: center;
  width: 24px;
}

.spot-pair-control strong {
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 20px;
  font-weight: 640;
  line-height: 1.08;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.spot-pair-control small {
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 10px;
  font-weight: 650;
}

.spot-header-actions {
  display: grid;
  grid-template-columns: repeat(2, 44px);
}

.spot-pencil-header :is(button):focus-visible,
.spot-pencil-workspace button:focus-visible,
.spot-account-workspace button:focus-visible,
.spot-chart-entry:focus-visible,
.spot-chart-drawer button:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: 0;
}

.spot-pencil-workspace {
  align-items: stretch;
  background: var(--page);
  display: grid;
  gap: 14px;
  grid-template-columns: minmax(0, 1fr) 148px;
  min-width: 0;
  padding: 8px 16px 10px;
}

.spot-order-console {
  display: grid;
  gap: 10px;
  min-width: 0;
}

.spot-side-switch {
  background: var(--surface-2);
  border-radius: 8px;
  display: grid;
  gap: 4px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  height: 40px;
  padding: 4px;
}

.spot-side-switch button {
  background: transparent;
  border: 0;
  border-radius: 6px;
  color: var(--muted);
  font-size: 13px;
  font-weight: 680;
  min-width: 0;
}

.spot-trade .spot-side-switch button {
  min-height: 32px;
}

.spot-side-switch button.active {
  background: var(--signal-green);
  color: var(--on-positive);
}

.spot-type-field {
  align-items: center;
  background: var(--surface-2);
  border: 1px solid transparent;
  border-radius: 8px;
  color: var(--text);
  display: grid;
  font-size: 13px;
  grid-template-columns: 16px minmax(0, 1fr) 16px;
  height: 40px;
  padding: 0 12px;
}

.spot-trade .spot-type-field {
  min-height: 40px;
}

.spot-type-field strong {
  font-weight: 560;
  overflow: hidden;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.spot-type-field > svg {
  color: var(--muted);
}

.spot-field-shell {
  align-content: center;
  background: var(--surface-2);
  border: 1px solid transparent;
  border-radius: 8px;
  display: grid;
  gap: 2px;
  grid-template-columns: minmax(0, 1fr);
  height: 44px;
  min-width: 0;
  padding: 4px 12px;
  position: relative;
}

.spot-field-shell--unit {
  grid-template-columns: minmax(0, 1fr) auto;
}

.spot-field-shell > span {
  color: var(--muted);
  font-size: 9px;
  grid-column: 1;
  line-height: 1;
}

.spot-field-shell input {
  appearance: none;
  background: transparent;
  border: 0;
  color: var(--text);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 14px;
  grid-column: 1;
  height: 20px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.spot-field-shell input:focus-visible {
  box-shadow: none;
  outline: 0;
}

.spot-field-shell b {
  align-self: end;
  color: var(--muted);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 10px;
  font-weight: 650;
  grid-column: 2;
  grid-row: 1 / 3;
  padding-bottom: 3px;
}

.spot-field-shell:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.spot-percentage {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  min-height: 40px;
}

.spot-percentage button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted);
  display: grid;
  font-size: 8px;
  gap: 5px;
  justify-items: center;
  min-width: 0;
  padding: 1px 0;
}

.spot-trade .spot-percentage button {
  min-height: 40px;
}

.spot-percentage button i {
  background: var(--border);
  border: 3px solid transparent;
  border-radius: 999px;
  height: 8px;
  width: 8px;
}

.spot-percentage button.active {
  color: var(--text);
}

.spot-percentage button.active i {
  background: var(--signal-green);
  border-color: color-mix(in srgb, var(--signal-green) 18%, transparent);
  box-sizing: content-box;
}

.spot-tpsl-row,
.spot-available-row {
  align-items: center;
  display: flex;
  min-width: 0;
}

.spot-tpsl-row {
  gap: 7px;
  min-height: 22px;
}

.spot-tpsl-row > span {
  border: 1px solid var(--border);
  border-radius: 3px;
  height: 14px;
  width: 14px;
}

.spot-tpsl-row b {
  font-size: 11px;
  font-weight: 560;
}

.spot-available-row {
  font-size: 10px;
  justify-content: space-between;
  min-height: 22px;
}

.spot-available-row > span {
  color: var(--muted);
}

.spot-available-row button,
.spot-available-row strong {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--text);
  display: inline-flex;
  font-size: 9px;
  gap: 4px;
  min-width: 0;
  padding: 0;
}

.spot-trade .spot-available-row button {
  min-height: 22px;
}

.spot-available-row svg {
  color: var(--positive);
  flex: 0 0 auto;
}

.spot-trade-feedback {
  font-size: 9px;
  line-height: 1.35;
  margin: -4px 0;
  min-height: 12px;
}

.spot-submit-order {
  align-items: center;
  background: var(--signal-green);
  border: 1px solid var(--signal-green);
  border-radius: 999px;
  color: var(--on-positive);
  display: inline-flex;
  font-size: 13px;
  font-weight: 760;
  height: 46px;
  justify-content: center;
  min-width: 0;
  padding: 0 14px;
}

.spot-submit-order.sell {
  background: var(--negative);
  border-color: var(--negative);
  color: var(--on-negative);
}

.spot-submit-order:disabled {
  cursor: not-allowed;
  opacity: .46;
}

.spot-mini-book {
  align-self: stretch;
  min-width: 0;
  width: 148px;
}

.spot-account-workspace {
  background: var(--page);
  border-top: 1px solid var(--line);
  min-width: 0;
}

.spot-account-tabs {
  align-items: center;
  display: grid;
  gap: 14px;
  grid-template-columns: auto minmax(0, 1fr) 44px;
  min-height: 48px;
  padding: 0 16px;
}

.spot-account-tabs button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted);
  display: inline-flex;
  font-size: 12px;
  gap: 4px;
  min-height: 44px;
  min-width: 0;
  padding: 0;
  position: relative;
  white-space: nowrap;
}

.spot-account-tabs button:nth-child(2) {
  overflow: hidden;
  text-overflow: ellipsis;
}

.spot-account-tabs button.active {
  color: var(--text);
  font-weight: 660;
}

.spot-account-tabs button.active::after {
  background: var(--accent);
  border-radius: 999px;
  bottom: 0;
  content: "";
  height: 2px;
  left: 10px;
  position: absolute;
  width: 18px;
}

.spot-account-tabs button:last-child {
  justify-content: center;
}

.spot-order-filter {
  align-items: center;
  border-top: 1px solid color-mix(in srgb, var(--line) 66%, transparent);
  display: flex;
  font-size: 10px;
  justify-content: space-between;
  min-height: 34px;
  padding: 0 16px;
}

.spot-order-filter > span {
  align-items: center;
  color: var(--muted);
  display: inline-flex;
  gap: 7px;
}

.spot-order-filter i {
  border: 1px solid var(--border);
  border-radius: 999px;
  height: 12px;
  width: 12px;
}

.spot-order-filter button {
  background: transparent;
  border: 0;
  color: var(--text);
  font-size: 10px;
  min-height: 32px;
  padding: 0;
}

.spot-account-state {
  align-items: center;
  display: flex;
  flex-direction: column;
  gap: 7px;
  justify-content: center;
  min-height: 198px;
  padding: 24px 16px 26px;
  text-align: center;
}

.spot-account-state > strong {
  font-size: 13px;
  font-weight: 560;
}

.spot-account-state > span {
  color: var(--muted);
  font-size: 10px;
}

.spot-account-state > button {
  background: transparent;
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--text);
  min-height: 40px;
  padding: 0 16px;
}

.spot-account-actions {
  display: grid;
  gap: 36px;
  grid-template-columns: repeat(2, 72px);
  margin-top: 10px;
}

.spot-account-actions button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--text);
  display: grid;
  font-size: 11px;
  gap: 8px;
  justify-items: center;
  min-height: 84px;
  padding: 0;
}

.spot-account-actions i {
  align-items: center;
  border: 1px solid var(--border);
  border-radius: 999px;
  display: inline-flex;
  height: 52px;
  justify-content: center;
  width: 52px;
}

.spot-balance-preview {
  display: grid;
  gap: 8px;
  min-height: 198px;
  padding: 18px 16px 24px;
}

.spot-balance-preview article {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 4px 12px;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 64px;
}

.spot-balance-preview article strong {
  font-size: 15px;
}

.spot-balance-preview article small {
  color: var(--muted);
  grid-column: 1 / -1;
}

.spot-chart-entry {
  align-items: center;
  background: var(--page);
  border: 0;
  border-top: 1px solid var(--line);
  color: var(--text);
  display: flex;
  font-size: 12px;
  justify-content: space-between;
  min-height: 48px;
  padding: 0 16px;
  width: 100%;
}

.spot-chart-entry svg {
  color: var(--muted);
  transition: transform 180ms ease;
}

.spot-chart-entry svg.open {
  transform: rotate(180deg);
}

.spot-chart-drawer {
  background: var(--surface);
  border-top: 1px solid var(--line-strong);
  min-width: 0;
}

.spot-chart-drawer .chart-tools {
  background: var(--surface);
  border-bottom: 1px solid var(--line-strong);
  display: grid;
  grid-template-columns: minmax(220px, 1fr) 88px;
  min-width: 0;
}

.spot-chart-drawer .interval-rail,
.spot-chart-drawer .chart-action-rail {
  display: grid;
  min-width: 0;
}

.spot-chart-drawer .interval-rail {
  grid-template-columns: repeat(5, minmax(44px, 1fr));
}

.spot-chart-drawer .chart-action-rail {
  box-shadow: inset 1px 0 0 var(--line-strong);
  grid-template-columns: repeat(2, 44px);
}

.spot-chart-drawer .chart-tools button {
  align-items: center;
  background: transparent;
  border: 0;
  border-right: 1px solid var(--line);
  color: var(--muted);
  display: inline-flex;
  font-size: 10px;
  font-weight: 700;
  justify-content: center;
  min-height: 48px;
  min-width: 44px;
  padding: 0;
}

.spot-chart-drawer .chart-tools button.active {
  box-shadow: inset 0 -2px 0 var(--positive);
  color: var(--positive);
}

.spot-chart-canvas {
  height: 252px;
  min-height: 252px;
  min-width: 0;
  position: relative;
}

.spot-chart-canvas > * {
  height: 100%;
  min-height: 0;
}

.instrument-hero,
.instrument-plate,
.trade-heading,
.trade-quote,
.trade-workspace,
.trade-console {
  min-width: 0;
}

.trade-instrument-hero {
  background:
    linear-gradient(118deg, color-mix(in srgb, var(--positive) 8%, transparent), transparent 52%),
    var(--page);
  border-bottom: 1px solid var(--line-strong);
}

.trade-heading {
  align-items: center;
  background: transparent;
  border: 0;
  display: grid;
  gap: 8px;
  grid-template-columns: 44px minmax(0, 1fr) 44px;
  min-height: 64px;
  padding: 4px 16px 0;
}

.trade-heading-control {
  align-items: center;
  background: linear-gradient(180deg, var(--surface), var(--surface-elevated));
  border: 1px solid var(--line-strong);
  border-radius: 999px;
  color: var(--text);
  display: inline-flex;
  height: 44px;
  justify-content: center;
  min-height: 44px;
  min-width: 44px;
  padding: 0;
  width: 44px;
}

.trade-heading-control:active {
  box-shadow: inset 0 2px 4px color-mix(in srgb, var(--ink) 15%, transparent);
  transform: translateY(1px);
}

.symbol-selector {
  max-width: 100%;
  min-height: 48px;
  min-width: 0;
  padding-inline: 2px;
}

.trade-symbol-copy {
  min-width: 0;
}

.trade-symbol-meta {
  align-items: center;
  display: flex;
  gap: 7px;
  min-height: 20px;
}

.trade-mode-badge {
  align-items: center;
  background: color-mix(in srgb, var(--positive) 10%, var(--surface));
  border: 1px solid var(--positive);
  border-radius: 999px;
  color: var(--positive) !important;
  display: inline-flex;
  min-height: 20px;
  padding-inline: 7px;
}

.trade-symbol-copy strong {
  font-size: 20px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.trade-symbol-copy small {
  color: var(--muted);
  font-size: 10px;
}

.trade-quote {
  align-items: end;
  background: transparent;
  display: grid;
  gap: 16px;
  grid-template-columns: minmax(0, 1fr) minmax(118px, .72fr);
  padding: 8px 16px 20px;
}

.trade-quote > div:first-child {
  gap: 7px;
  min-width: 0;
}

.trade-latest-label {
  color: var(--muted);
  display: block;
  font-size: 9px;
  font-weight: 620;
  margin-bottom: 7px;
}

.trade-quote > div:first-child strong {
  display: block;
  font-size: 36px;
  line-height: 1;
  overflow-wrap: normal;
  white-space: nowrap;
}

.trade-quote > div:first-child span {
  font-size: 12px;
  font-weight: 720;
}

.quote-stats {
  gap: 8px;
  min-width: 0;
}

.quote-stats span {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  font-size: 9px;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 27px;
  min-width: 0;
}

.spot-live-telemetry {
  align-items: center;
  border-top: 1px solid var(--line);
  display: flex;
  flex-wrap: wrap;
  gap: 8px 16px;
  margin: 0 16px;
  min-height: 34px;
  padding-block: 5px;
}

.spot-live-telemetry span {
  align-items: center;
  color: var(--muted);
  display: inline-flex;
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 9px;
  font-weight: 650;
  gap: 6px;
  white-space: nowrap;
}

.spot-live-telemetry i {
  background: var(--positive);
  border-radius: 999px;
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--positive) 10%, transparent);
  height: 6px;
  width: 6px;
}

.spot-live-telemetry[data-live-detail="snapshot"] i {
  background: var(--muted);
  box-shadow: none;
}

.quote-stats b {
  color: var(--text);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.trade-workspace {
  background: var(--surface);
  border-bottom: 1px solid var(--line-strong);
}

.trade-chart-panel {
  background: var(--surface);
  border: 0;
  display: grid;
  height: auto;
  min-width: 0;
}

.trade-view .chart-tools {
  background: var(--surface);
  border-bottom: 1px solid var(--line-strong);
  display: grid;
  grid-template-columns: minmax(220px, 1fr) 88px;
  inset: auto;
  min-width: 0;
  position: static;
}

.interval-rail,
.chart-action-rail {
  display: grid;
  min-width: 0;
}

.interval-rail {
  grid-template-columns: repeat(5, minmax(44px, 1fr));
}

.chart-action-rail {
  box-shadow: inset 1px 0 0 var(--line-strong);
  grid-template-columns: repeat(2, 44px);
}

.trade-view .chart-tools button {
  align-items: center;
  background: transparent;
  border: 0;
  border-right: 1px solid var(--line);
  border-radius: 0;
  color: var(--muted);
  display: inline-flex;
  font-size: 10px;
  font-weight: 720;
  justify-content: center;
  min-height: 48px;
  min-width: 44px;
  padding: 0 6px;
}

.trade-view .chart-tools button:last-child {
  border-right: 0;
}

.trade-view .chart-tools button.active,
.trade-view .chart-tools button[aria-pressed="true"] {
  background: color-mix(in srgb, var(--positive) 9%, var(--surface));
  box-shadow: inset 0 -2px 0 var(--positive);
  color: var(--positive);
}

.trade-view .chart-tools button:focus-visible,
.trade-heading-control:focus-visible,
.symbol-selector:focus-visible,
.side-switch button:focus-visible,
.order-type-row button:focus-visible,
.percent-row button:focus-visible,
.risk-chip:focus-visible,
.submit-order:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: 0;
}

.trade-view .chart-panel__canvas {
  bottom: auto;
  height: 292px;
  inset: auto;
  left: auto;
  min-height: 292px;
  position: relative;
  right: auto;
}

.live-price-line {
  right: 0;
  top: 48%;
}

.trade-order-book {
  background: var(--surface);
  min-width: 0;
}

.spot-market-data {
  background: var(--surface);
  border-top: 1px solid var(--line-strong);
  min-width: 0;
}

.spot-market-data__tabs {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  min-height: 50px;
}

.spot-market-data__tabs button {
  background: var(--surface);
  border: 0;
  border-bottom: 1px solid var(--line-strong);
  color: var(--muted);
  font-size: 12px;
  font-weight: 650;
  min-height: 50px;
  min-width: 44px;
  position: relative;
}

.spot-market-data__tabs button.active {
  color: var(--text);
}

.spot-market-data__tabs button.active::after {
  background: var(--positive);
  bottom: -1px;
  content: "";
  height: 2px;
  left: 25%;
  position: absolute;
  width: 50%;
}

.spot-market-data__tabs button:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: 0;
}

.spot-recent-trades {
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--ink) 3%, transparent), transparent 62px),
    var(--surface);
  min-height: 252px;
  padding: 0 12px 12px;
}

.spot-recent-trades header,
.spot-recent-trades__row {
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) minmax(0, .8fr) minmax(72px, .72fr);
  min-width: 0;
}

.spot-recent-trades header {
  align-items: center;
  border-bottom: 1px solid var(--line);
  color: var(--muted);
  font-size: 9px;
  min-height: 42px;
}

.spot-recent-trades header span:nth-child(n + 2),
.spot-recent-trades__row > :nth-child(n + 2) {
  text-align: right;
}

.spot-recent-trades header small {
  color: var(--muted);
  display: block;
  font-size: 8px;
  margin-top: 2px;
}

.spot-recent-trades__row {
  align-items: center;
  border-bottom: 1px solid var(--line);
  font-size: 10px;
  min-height: 32px;
}

.spot-recent-trades__row strong,
.spot-recent-trades__row span,
.spot-recent-trades__row time {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.spot-recent-trades__row time {
  color: var(--muted);
}

.spot-recent-trades__empty {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 11px;
  justify-content: center;
  min-height: 198px;
  text-align: center;
}

.trade-console {
  background: var(--surface);
  border-top: 1px solid var(--line-strong);
  padding: 0 16px 24px;
}

.spot-trade .trade-console {
  background:
    linear-gradient(128deg, color-mix(in srgb, var(--positive) 5%, transparent), transparent 58%),
    var(--surface-elevated);
}

.spot-trade .trade-quote {
  padding-bottom: 10px;
}

.spot-trade .order-surface-heading {
  border-bottom: 1px solid var(--line);
  min-height: 72px;
}

.spot-trade .order-surface-heading > div > span {
  color: var(--positive);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 9px;
  letter-spacing: .06em;
}

.spot-trade .risk-chip {
  border-radius: 999px;
  max-width: 44%;
}

.order-surface-heading {
  align-items: center;
  min-height: 68px;
}

.order-surface-heading > div {
  min-width: 0;
}

.order-surface-heading strong,
.order-surface-heading span {
  overflow-wrap: anywhere;
}

.risk-chip {
  align-items: center;
  background: transparent;
  border: 1px solid var(--line-strong);
  color: var(--text);
  display: inline-flex;
  justify-content: center;
  max-width: 48%;
  min-height: 44px;
  overflow-wrap: anywhere;
  padding: 5px 10px;
  text-align: center;
  white-space: normal;
}

.contract-settings {
  gap: 0;
}

.contract-settings button {
  background: var(--surface);
  border-color: var(--line-strong);
  min-height: 52px;
}

.contract-settings button + button {
  border-left: 0;
}

.side-switch {
  background: transparent;
  border: 1px solid var(--line-strong);
  gap: 0;
  margin-top: 12px;
  min-height: 52px;
}

.side-switch button {
  background: var(--surface);
  border: 0;
  border-radius: 0;
  min-height: 50px;
}

.side-switch button + button {
  border-left: 1px solid var(--line-strong);
}

.side-switch button.active.buy {
  background: color-mix(in srgb, var(--positive) 10%, var(--surface));
}

.side-switch button.active.sell {
  background: color-mix(in srgb, var(--negative) 10%, var(--surface));
}

.order-type-row {
  display: grid;
  gap: 0;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  padding: 12px 0;
}

.order-type-row button {
  background: transparent;
  border: 1px solid var(--line);
  border-radius: 0;
  font-size: 10px;
  line-height: 1.25;
  min-height: 48px;
  min-width: 0;
  overflow-wrap: anywhere;
  padding: 4px 7px;
}

.order-type-row button + button {
  border-left: 0;
}

.order-type-row button.active {
  background: color-mix(in srgb, var(--text) 7%, var(--surface));
  border-color: var(--line-strong);
  color: var(--text);
}

.input-stack {
  gap: 8px;
}

.input-stack .field-shell {
  background: var(--surface);
  border: 1px solid var(--line-strong);
  border-radius: 0;
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(72px, .8fr) minmax(0, 1fr) auto;
  height: 52px;
  min-height: 52px;
  min-width: 0;
  padding: 0 12px;
}

.input-stack .field-shell:focus-within {
  background: color-mix(in srgb, var(--focus) 5%, var(--surface));
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.input-stack .field-shell input {
  border: 0;
  box-shadow: none;
  color: var(--text);
  min-height: 44px;
  min-width: 0;
  outline: 0;
}

.input-stack .field-shell input:focus,
.input-stack .field-shell input:focus-visible {
  border: 0;
  box-shadow: none;
  outline: 0;
}

.spot-trade .input-stack .field-shell {
  grid-template-areas:
    "label unit"
    "input input";
  grid-template-columns: minmax(0, 1fr) auto;
  grid-template-rows: 18px minmax(0, 1fr);
  height: 68px;
  min-height: 68px;
  padding: 7px 12px 5px;
}

.spot-trade .input-stack .field-shell > span {
  grid-area: label;
}

.spot-trade .input-stack .field-shell > b {
  grid-area: unit;
  text-align: right;
}

.spot-trade .input-stack .field-shell > input {
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 17px;
  font-weight: 640;
  grid-area: input;
  min-height: 34px;
  padding: 0;
}

.input-stack .field-shell span,
.input-stack .field-shell b {
  font-size: 10px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.amount-control {
  margin-top: 14px;
}

.amount-control input[type="range"] {
  height: 44px;
  min-height: 44px;
}

.percent-row {
  display: grid;
  gap: 0;
  grid-template-columns: repeat(5, minmax(44px, 1fr));
  margin-top: 4px;
}

.percent-row button {
  background: transparent;
  border: 1px solid var(--line);
  border-radius: 0;
  min-height: 48px;
  min-width: 44px;
}

.percent-row button + button {
  border-left: 0;
}

.percent-row button.active {
  background: color-mix(in srgb, var(--positive) 9%, var(--surface));
  border-color: var(--positive);
  color: var(--positive);
}

.available-row {
  gap: 12px;
  margin: 4px 0 10px;
  min-height: 44px;
  min-width: 0;
}

.available-row > * {
  min-width: 0;
  overflow-wrap: anywhere;
}

.available-row > button {
  min-height: 44px;
}

.trade-helper {
  background: transparent;
  border-block: 1px solid var(--line);
  border-radius: 0;
  margin: 0 0 8px;
  min-height: 48px;
}

.trade-feedback {
  align-items: center;
  display: flex;
  min-height: 40px;
  overflow-wrap: anywhere;
}

.trade-feedback:empty {
  min-height: 12px;
}

.submit-order {
  min-height: 52px;
}

.spot-trade .submit-order {
  min-height: 54px;
}

.trade-disclaimer {
  min-height: 44px;
}

.confirmation-layer {
  max-width: 100%;
}

.confirmation-sheet {
  padding-bottom: calc(18px + env(safe-area-inset-bottom));
}

.contract-trade {
  background: var(--page);
  color: var(--text);
  min-height: 100dvh;
  padding-bottom: calc(24px + env(safe-area-inset-bottom));
}

.contract-pencil-surface {
  background: var(--page);
  min-height: 100dvh;
  min-width: 0;
}

.contract-pencil-header {
  align-items: center;
  background: var(--page);
  display: grid;
  grid-template-columns: 40px minmax(0, 1fr) 40px;
  height: 60px;
  padding: 10px 20px;
  position: sticky;
  top: 0;
  z-index: var(--layer-sticky-header);
}

.contract-header-control {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 999px;
  color: var(--text);
  display: inline-flex;
  height: 40px;
  justify-content: center;
  padding: 0;
  width: 40px;
}

.contract-header-control.active {
  color: var(--positive);
}

.contract-trade .trade-quote {
  align-items: center;
  background: transparent;
  display: flex;
  gap: 0;
  grid-template-columns: none;
  justify-content: center;
  padding: 0;
}

.contract-pair-selector {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--text);
  display: grid;
  gap: 2px;
  justify-items: center;
  min-height: 40px;
  min-width: 0;
  padding: 0 8px;
}

.contract-pair-selector > span {
  align-items: center;
  display: inline-flex;
  gap: 4px;
  min-width: 0;
}

.contract-pair-selector strong {
  font-size: 17px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-pair-selector small {
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 10px;
  font-weight: 650;
  white-space: nowrap;
}

.contract-pencil-module {
  align-items: start;
  background: var(--page);
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) 150px;
  min-width: 0;
  padding: 2px 16px 4px;
}

.contract-trade .trade-console {
  background: transparent;
  border: 0;
  display: grid;
  gap: 8px;
  grid-column: 1;
  grid-row: 1;
  min-width: 0;
  padding: 0;
}

.contract-open-close {
  background: var(--page);
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  display: grid;
  gap: 4px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  height: 38px;
  padding: 4px;
}

.contract-open-close button {
  background: transparent;
  border: 0;
  border-radius: 6px;
  color: var(--muted);
  font-size: 11px;
  font-weight: 650;
  min-width: 0;
  padding: 0 4px;
}

.contract-open-close button.active {
  background: var(--accent);
  color: var(--on-positive);
}

.contract-mode-row {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.contract-mode-row button,
.contract-order-type,
.contract-price-row > button {
  align-items: center;
  background: var(--page);
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  color: var(--text);
  display: flex;
  height: 36px;
  justify-content: space-between;
  min-width: 0;
  padding: 0 10px;
}

.contract-mode-row button:disabled,
.contract-order-type:disabled {
  cursor: default;
  opacity: 1;
}

.contract-mode-row span,
.contract-order-type span {
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-order-type {
  gap: 5px;
  width: 100%;
}

.contract-order-type > span {
  flex: 1;
  text-align: left;
}

.contract-order-type > svg {
  color: var(--muted);
}

.contract-price-row {
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) 62px;
  min-width: 0;
}

.contract-price-row > button {
  font-size: 10px;
  height: 44px;
  justify-content: center;
  padding-inline: 5px;
}

.contract-field {
  align-items: center;
  background: var(--page);
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  display: grid;
  min-width: 0;
}

.contract-field:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.contract-field > span {
  color: var(--muted);
  font-size: 9px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-field input {
  background: transparent;
  border: 0;
  box-shadow: none;
  color: var(--text);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.contract-price-field {
  grid-template-rows: 13px 22px;
  height: 44px;
  padding: 4px 10px 3px;
}

.contract-price-field input {
  font-size: 12px;
  height: 22px;
}

.contract-amount-field {
  grid-template-columns: minmax(0, 1fr) auto;
  height: 40px;
  padding: 0 10px;
}

.contract-amount-field > span {
  grid-column: 1;
  grid-row: 1;
}

.contract-amount-field input {
  font-size: 12px;
  grid-column: 1;
  grid-row: 1;
  height: 38px;
  padding-top: 12px;
}

.contract-amount-field b {
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 11px;
  grid-column: 2;
  grid-row: 1;
}

.contract-percentage {
  display: grid;
  min-height: 22px;
}

.contract-trade .contract-percentage .percent-row {
  display: grid;
  gap: 0;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  margin: 0;
}

.contract-percentage button {
  background: transparent;
  border: 0;
  color: var(--muted);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 8px;
  min-height: 22px;
  min-width: 0;
  padding: 0;
}

.contract-percentage button.active {
  color: var(--positive);
  font-weight: 750;
}

.contract-trade .contract-percentage .percent-row button {
  background: transparent;
  border: 0;
  color: var(--muted);
  min-height: 22px;
  min-width: 0;
}

.contract-trade .contract-percentage .percent-row button.active {
  background: transparent;
  border: 0;
  color: var(--positive);
}

.contract-tpsl {
  align-items: center;
  display: flex;
  font-size: 10px;
  font-weight: 550;
  gap: 6px;
  min-height: 16px;
}

.contract-tpsl > span {
  border: 1px solid var(--line-strong);
  border-radius: 3px;
  height: 13px;
  width: 13px;
}

.contract-balance-rows {
  display: grid;
  gap: 5px;
  margin: 0;
}

.contract-balance-rows > div {
  align-items: center;
  display: flex;
  font-size: 9px;
  gap: 8px;
  justify-content: space-between;
  min-height: 14px;
  min-width: 0;
}

.contract-balance-rows dt {
  color: var(--muted);
  flex: 0 0 auto;
}

.contract-balance-rows dd {
  margin: 0;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-balance-rows button {
  background: transparent;
  border: 0;
  color: var(--text);
  font-size: 9px;
  min-height: 14px;
  padding: 0;
}

.contract-feedback {
  font-size: 9px;
  line-height: 1.35;
  margin: -2px 0;
  overflow-wrap: anywhere;
}

.contract-submit {
  border: 0;
  border-radius: 23px;
  font-size: 13px;
  font-weight: 750;
  height: 46px;
  min-width: 0;
  padding: 0 8px;
  width: 100%;
}

.contract-submit--long,
.contract-trade .contract-submit--long.submit-order {
  background: var(--accent);
  color: var(--on-positive);
  min-height: 46px;
}

.contract-submit--short {
  background: var(--negative);
  color: var(--on-negative);
}

.contract-trade .trade-chart-panel {
  background: transparent;
  border: 0;
  display: flex;
  flex-direction: column;
  grid-column: 2;
  grid-row: 1;
  height: 372px;
  min-width: 0;
  overflow: hidden;
}

.contract-book-status {
  align-items: flex-end;
  display: flex;
  flex-direction: column;
  gap: 2px;
  height: 30px;
  justify-content: center;
  min-width: 0;
}

.contract-book-status span,
.contract-book-status strong {
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 8px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-book-status span {
  color: var(--muted);
}

.contract-book-status strong {
  color: var(--text);
  font-weight: 650;
}

.contract-trade .trade-order-book {
  background: transparent;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.contract-mini-book {
  height: 342px;
  min-height: 342px;
}

.contract-mini-book :deep(.order-book__mini-header) {
  height: 18px;
}

.contract-mini-book :deep(.order-book__mini-row) {
  font-size: 9px;
  height: 23px;
}

.contract-mini-book :deep(.order-book__mini-mid) {
  height: 34px;
}

.contract-mini-book :deep(.order-book__mini-mid strong) {
  font-size: 15px;
}

.contract-mini-book :deep(.order-book__mini-ratio) {
  height: 16px;
}

.contract-mini-book :deep(.order-book__mini-precision) {
  font-size: 8px;
  height: 20px;
  margin-top: 0;
  padding-inline: 5px;
}

.contract-mini-book :deep(.order-book__mini-state) {
  height: 342px;
}

.contract-position-tabs {
  align-items: center;
  display: grid;
  gap: 16px;
  grid-template-columns: auto auto auto minmax(0, 1fr) 40px;
  min-height: 40px;
  padding: 4px 16px 0 20px;
}

.contract-position-tabs button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted);
  display: inline-flex;
  font-size: 11px;
  gap: 3px;
  justify-content: center;
  min-height: 36px;
  padding: 0;
  position: relative;
  white-space: nowrap;
}

.contract-position-tabs button.active {
  color: var(--text);
}

.contract-position-tabs button.active::after {
  background: var(--accent);
  border-radius: 1px;
  bottom: 0;
  content: "";
  height: 2px;
  left: 50%;
  position: absolute;
  transform: translateX(-50%);
  width: 18px;
}

.contract-position-empty {
  align-items: center;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 28px;
  text-align: center;
}

.contract-position-empty > span {
  align-items: center;
  background: var(--positive-soft);
  border-radius: 999px;
  color: var(--positive);
  display: inline-flex;
  height: 52px;
  justify-content: center;
  width: 52px;
}

.contract-position-empty strong {
  font-size: 12px;
  font-weight: 500;
  max-width: 260px;
}

.contract-position-empty button {
  background: transparent;
  border: 0;
  color: var(--positive);
  min-height: 40px;
}

.contract-position-list {
  display: grid;
  padding: 8px 20px 0;
}

.contract-position-list article {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 5px;
  min-height: 58px;
  padding: 8px 0;
}

.contract-position-list article > div {
  align-items: center;
  display: flex;
  font-size: 10px;
  gap: 8px;
  justify-content: space-between;
  min-width: 0;
}

.contract-position-list article > div:last-child {
  color: var(--muted);
  font-size: 9px;
}

.contract-pencil-header button:focus-visible,
.contract-pencil-module button:focus-visible,
.contract-position-tabs button:focus-visible,
.contract-position-empty button:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: 0;
}

@media (max-width: 340px) {
  .spot-pencil-header {
    padding-inline: 10px;
  }

  .spot-pair-control {
    gap: 5px;
    padding-left: 1px;
  }

  .spot-pair-control strong {
    font-size: 17px;
  }

  .spot-pencil-workspace {
    gap: 10px;
    grid-template-columns: minmax(0, 1fr) 124px;
    padding-inline: 12px;
  }

  .spot-mini-book {
    width: 124px;
  }

  .spot-field-shell {
    padding-inline: 9px;
  }

  .spot-field-shell input {
    font-size: 12px;
  }

  .spot-field-shell b,
  .spot-available-row,
  .spot-percentage button {
    font-size: 8px;
  }

  .spot-account-tabs {
    gap: 8px;
    padding-inline: 12px;
  }

  .spot-account-tabs button {
    font-size: 10px;
  }

  .spot-order-filter,
  .spot-chart-entry {
    padding-inline: 12px;
  }

  .trade-heading,
  .trade-quote,
  .trade-console {
    padding-inline: 12px;
  }

  .trade-quote {
    gap: 10px;
    grid-template-columns: minmax(0, 1fr) minmax(96px, .62fr);
  }

  .trade-quote > div:first-child strong {
    font-size: 29px;
  }

  .quote-stats span {
    gap: 4px;
    grid-template-columns: 1fr;
    padding-block: 4px;
  }

  .quote-stats b {
    max-width: 100%;
  }

  .trade-view .chart-panel__canvas {
    height: 240px;
    min-height: 240px;
  }

  .input-stack .field-shell {
    grid-template-columns: minmax(64px, .72fr) minmax(0, 1fr) auto;
    padding-inline: 9px;
  }

  .order-type-row button,
  .risk-chip {
    font-size: 9px;
  }

  .contract-pencil-header {
    padding-inline: 12px;
  }

  .contract-pencil-module {
    gap: 10px;
    grid-template-columns: minmax(0, 1fr) 124px;
    padding-inline: 12px;
  }

  .contract-mode-row,
  .contract-price-row {
    gap: 6px;
  }

  .contract-price-row {
    grid-template-columns: minmax(0, 1fr) 48px;
  }

  .contract-field,
  .contract-mode-row button,
  .contract-order-type {
    padding-inline: 7px;
  }

  .contract-position-tabs {
    gap: 8px;
    padding-left: 12px;
    padding-right: 8px;
  }

  .contract-position-tabs button {
    font-size: 9px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .trade-view *,
  .trade-view *::before,
  .trade-view *::after {
    animation-duration: .01ms !important;
    animation-iteration-count: 1 !important;
    scroll-behavior: auto !important;
    transition-duration: .01ms !important;
  }

  .trade-view button:active {
    transform: none;
  }
}
</style>
