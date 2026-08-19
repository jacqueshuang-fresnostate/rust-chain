<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowLeft,
  ArrowLeftRight,
  CheckCircle2,
  ChevronDown,
  CirclePlus,
  Download,
  History,
  Info,
  Inbox,
  List,
  RefreshCcw,
  Share2,
  Star,
  TriangleAlert,
  X,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import ContractTradeSheets from '@/components/ContractTradeSheets.vue'
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
  createMarginOrderIdempotencyKey,
  fetchMarginSetting,
  fetchMarginProducts,
  fetchMarginWallets,
  placeMarginOrder,
  placeSpotOrder,
  type MarginPosition,
  updateMarginLeverage,
  updateMarginMode,
} from '@/api/trading'
import { fetchWalletAccounts } from '@/api/wallet'
import { publicMarketWebSocketUrl } from '@/config/app'
import { formatAmount, formatPrice, normalizeSymbol } from '@/core/format'
import {
  classifyMarginOrderBackendBoundaryError,
  createMarginOrderReview,
  type MarginOrderReview,
} from '@/core/marginOrderConfirmation'
import { useModalDialog } from '@/core/modalDialog'
import { goBackOr } from '@/core/navigation'
import {
  clampMarginShortcutAmount,
  marginShortcutAvailable,
  quantityForBalancePercentage,
  type MarginAmountValidation,
  validateMarginAmount,
} from '@/core/tradeForm'
import { currentIntlLocale } from '@/i18n'
import { useMarketStore } from '@/stores/market'
import { useMarketFavoritesStore } from '@/stores/marketFavorites'
import { useSessionStore } from '@/stores/session'
import { useNavigationStore } from '@/stores/navigation'
import type { KlinePoint, MarginProduct, OrderBookLevel, TradePrint, WalletAccount } from '@/core/types'

const route = useRoute()
const router = useRouter()
const marketStore = useMarketStore()
const marketFavorites = useMarketFavoritesStore()
const session = useSessionStore()
const navigation = useNavigationStore()
const { t } = useI18n()
const mode = ref<'spot' | 'contract'>(route.query.mode === 'contract' ? 'contract' : 'spot')
const side = ref<'buy' | 'sell'>('buy')
const orderType = ref<'limit' | 'market'>('limit')
const price = ref('')
const quantity = ref('')
const percentage = ref<number | null>(0)
const leverage = ref(5)
const marginMode = ref<'cross' | 'isolated'>('isolated')
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
const settingsError = ref('')
const contractSheet = ref<'pair' | 'leverage' | 'marginMode' | null>(null)
const depthLoading = ref(false)
const depthError = ref(false)
const chartLoading = ref(false)
const productsLoading = ref(false)
const productsError = ref(false)
const balancesLoading = ref(false)
const balancesError = ref(false)
const spotOrderTypeOpen = ref(false)
const spotOrderTypeDialog = ref<HTMLElement | null>(null)
const confirmOpen = ref(false)
const confirmDialog = ref<HTMLElement | null>(null)
const reviewButton = ref<HTMLButtonElement | null>(null)
const reviewedMarginOrder = ref<MarginOrderReview | null>(null)
let marketRequestVersion = 0
let marginProductsRequestVersion = 0
let marginSettingRequestVersion = 0
let viewActive = true
const { trapFocus: trapSpotOrderTypeFocus } = useModalDialog(
  spotOrderTypeOpen,
  spotOrderTypeDialog,
  '[data-order-type-current="true"]',
)
const {
  trapFocus: trapConfirmFocus,
  setReturnFocus: setConfirmReturnFocus,
} = useModalDialog(confirmOpen, confirmDialog, '[data-dialog-cancel]')

const pairSymbol = computed(() => String(route.params.symbol || 'BTC_USDT').replace(/[_-]/g, '/').toUpperCase())
const isSpotMode = computed(() => mode.value === 'spot')
const ticker = computed(() => marketStore.tickerFor(pairSymbol.value))
const baseAsset = computed(() => pairSymbol.value.split('/')[0] || '')
const quoteAsset = computed(() => pairSymbol.value.split('/')[1] || 'USDT')
const isFavorite = computed(() => marketFavorites.isFavorite(pairSymbol.value))
const favoriteSaving = computed(() => marketFavorites.isPending(pairSymbol.value))
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
const currentPrice = computed(() => ticker.value?.lastPrice ?? 0)
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
function createCurrentMarginOrderReview(idempotencyKey?: string): MarginOrderReview {
  return createMarginOrderReview({
    productId: selectedProduct.value?.id || 0,
    side: side.value,
    marginMode: marginMode.value,
    leverage: leverage.value,
    marginAmount: Number(quantity.value),
    idempotencyKey,
    minMargin: selectedProduct.value?.minMargin,
    maxMargin: selectedProduct.value?.maxMargin,
    referencePrice: effectivePrice.value,
  })
}

const marginOrderDraft = computed(() => createCurrentMarginOrderReview())
const contractOrderReview = computed(() => reviewedMarginOrder.value || marginOrderDraft.value)
const contractNotionalValue = computed(() => contractOrderReview.value.estimatedNotional)
const contractOrderQuantity = computed(() => contractOrderReview.value.estimatedQuantity)
const contractShortcutAvailable = computed(() => marginShortcutAvailable(
  availableBalance.value,
  selectedProduct.value?.maxMargin,
))
const contractOpenQuantity = computed(() => {
  if (currentPrice.value <= 0) return 0
  return (contractShortcutAvailable.value * leverage.value) / currentPrice.value
})
const marginRangeDescription = computed(() => {
  const product = selectedProduct.value
  if (!product) return ''
  const minimum = formatAmount(product.minMargin, 8)
  if (product.maxMargin === null) {
    return t('trade.marginRangeWithoutMaximum', { minimum, asset: availableAsset.value })
  }
  return t('trade.marginRangeWithMaximum', {
    minimum,
    maximum: formatAmount(product.maxMargin, 8),
    asset: availableAsset.value,
  })
})
const marginAmountError = computed(() => {
  if (mode.value !== 'contract' || !selectedProduct.value || !quantity.value.trim()) return ''
  return marginAmountValidationMessage(marginOrderDraft.value.marginAmountValidation)
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

function marginAmountValidationMessage(validation: MarginAmountValidation): string {
  if (validation.error === 'below-minimum') {
    return t('trade.marginBelowMinimum', {
      minimum: formatAmount(validation.minMargin, 8),
      asset: availableAsset.value,
    })
  }
  if (validation.error === 'above-maximum' && validation.maxMargin !== null) {
    return t('trade.marginAboveMaximum', {
      maximum: formatAmount(validation.maxMargin, 8),
      asset: availableAsset.value,
    })
  }
  if (validation.error === 'invalid') return t('trade.invalidMarginAmount')
  return ''
}

function marginOrderFailureMessage(reason: unknown): string {
  const sourceMessage = apiErrorMessage(reason, t('trade.orderFailed'))
  const boundary = classifyMarginOrderBackendBoundaryError(sourceMessage)
  if (boundary === 'below-minimum') {
    void loadMarginProducts({ preserveExistingOnError: true })
    return t('trade.marginMinimumChanged')
  }
  if (boundary === 'above-maximum') {
    void loadMarginProducts({ preserveExistingOnError: true })
    return t('trade.marginMaximumChanged')
  }
  return sourceMessage
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

async function loadMarginProducts(options: { preserveExistingOnError?: boolean } = {}): Promise<void> {
  const requestVersion = ++marginProductsRequestVersion
  if (mode.value !== 'contract' || !session.isAuthenticated) {
    marginSettingRequestVersion += 1
    products.value = []
    productsLoading.value = false
    productsError.value = false
    return
  }
  productsLoading.value = true
  productsError.value = false
  try {
    const nextProducts = await fetchMarginProducts()
    if (
      requestVersion !== marginProductsRequestVersion
      || mode.value !== 'contract'
      || !session.isAuthenticated
    ) return
    products.value = nextProducts
  } catch {
    if (requestVersion !== marginProductsRequestVersion) return
    if (!options.preserveExistingOnError) products.value = []
    productsError.value = true
  } finally {
    if (requestVersion === marginProductsRequestVersion) productsLoading.value = false
  }
}

function applyMarginProductDefaults(product: MarginProduct): void {
  const levels = product.leverageLevels.length ? product.leverageLevels : [product.maxLeverage || 1]
  leverage.value = levels.includes(5) ? 5 : levels[0] || 1
  marginMode.value = product.marginModes.includes(product.marginMode)
    ? product.marginMode
    : product.marginModes[0] || 'isolated'
}

async function syncMarginSetting(product: MarginProduct): Promise<void> {
  const requestVersion = ++marginSettingRequestVersion
  applyMarginProductDefaults(product)
  try {
    const setting = await fetchMarginSetting(product.id)
    if (
      requestVersion !== marginSettingRequestVersion
      || selectedProduct.value?.id !== product.id
      || mode.value !== 'contract'
    ) return
    if (setting.leverage && product.leverageLevels.includes(setting.leverage)) {
      leverage.value = setting.leverage
    }
    if (setting.marginMode && product.marginModes.includes(setting.marginMode)) {
      marginMode.value = setting.marginMode
    }
  } catch (reason) {
    if (requestVersion !== marginSettingRequestVersion || selectedProduct.value?.id !== product.id) return
    setFeedback(apiErrorMessage(reason, t('trade.marginSettingLoadFailed')))
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
    maximum: mode.value === 'contract' ? selectedProduct.value?.maxMargin : null,
    mode: mode.value,
    percentage: percent / 100,
    price: effectivePrice.value,
    side: side.value,
  })
  const roundedQuantity = Number(nextQuantity.toFixed(8))
  const safeQuantity = mode.value === 'contract'
    ? clampMarginShortcutAmount(roundedQuantity, availableBalance.value, selectedProduct.value?.maxMargin)
    : roundedQuantity
  quantity.value = safeQuantity > 0 ? String(safeQuantity) : ''
}

function clearPercentageSelection(): void {
  percentage.value = null
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

function openContractSheet(sheet: 'pair' | 'leverage' | 'marginMode'): void {
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  if (sheet !== 'pair' && !selectedProduct.value) {
    setFeedback(t('trade.unavailableContract'))
    return
  }
  settingsError.value = ''
  contractSheet.value = sheet
}

function closeContractSheet(): void {
  if (!settingsSaving.value) contractSheet.value = null
}

function selectContractPair(symbol: string): void {
  if (settingsSaving.value) return
  contractSheet.value = null
  const routeSymbol = symbol.replace('/', '_')
  navigation.rememberTradeSymbol(routeSymbol)
  navigation.rememberTradeMode('contract')
  void router.replace({
    name: 'trade',
    params: { symbol: routeSymbol },
    query: { mode: 'contract' },
  })
}

function toggleFavorite(): void {
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  void marketFavorites.toggle(pairSymbol.value)
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

function openSpotOrderTypeSheet(): void {
  if (confirmOpen.value) return
  spotOrderTypeOpen.value = true
}

function closeSpotOrderTypeSheet(): void {
  spotOrderTypeOpen.value = false
}

function selectSpotOrderType(type: 'limit' | 'market'): void {
  orderType.value = type
  closeSpotOrderTypeSheet()
}

function handleSpotOrderTypeKeydown(event: KeyboardEvent): void {
  trapSpotOrderTypeFocus(event, closeSpotOrderTypeSheet)
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

async function applyContractLeverage(nextLeverage: number): Promise<void> {
  const product = selectedProduct.value
  if (!product) {
    setFeedback(t('trade.unavailableContract'))
    return
  }
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  if (!product.leverageLevels.includes(nextLeverage)) {
    settingsError.value = t('trade.invalidLeverageSelection')
    return
  }
  settingsSaving.value = true
  settingsError.value = ''
  feedback.value = ''
  try {
    await updateMarginLeverage(product.id, nextLeverage)
    leverage.value = nextLeverage
    contractSheet.value = null
    setFeedback(t('trade.leverageChanged'), 'success')
  } catch (reason) {
    settingsError.value = apiErrorMessage(reason, t('trade.leverageChangeFailed'))
    setFeedback(settingsError.value)
  } finally {
    settingsSaving.value = false
  }
}

async function applyContractMarginMode(nextMode: 'cross' | 'isolated'): Promise<void> {
  const product = selectedProduct.value
  if (!product) {
    setFeedback(t('trade.unavailableContract'))
    return
  }
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  if (!product.marginModes.includes(nextMode)) {
    settingsError.value = t('trade.invalidMarginModeSelection')
    return
  }
  settingsSaving.value = true
  settingsError.value = ''
  feedback.value = ''
  try {
    await updateMarginMode(product.id, nextMode)
    marginMode.value = nextMode
    contractSheet.value = null
    setFeedback(t('trade.marginModeChanged'), 'success')
  } catch (reason) {
    settingsError.value = apiErrorMessage(reason, t('trade.marginModeChangeFailed'))
    setFeedback(settingsError.value)
  } finally {
    settingsSaving.value = false
  }
}

function reviewOrder(event?: Event): void {
  feedback.value = ''
  const orderAmount = Number(quantity.value)
  if (spotOrderTypeOpen.value) return
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
  if (mode.value === 'contract' && !marginOrderDraft.value.marginAmountValidation.isValid) {
    setFeedback(marginAmountValidationMessage(marginOrderDraft.value.marginAmountValidation))
    return
  }
  if (!Number.isFinite(orderAmount) || orderAmount <= 0 || !Number.isFinite(effectivePrice.value) || effectivePrice.value <= 0) {
    setFeedback(t('trade.invalidOrder'))
    return
  }
  const trigger = event?.currentTarget
  setConfirmReturnFocus(trigger instanceof HTMLElement ? trigger : reviewButton.value)
  reviewedMarginOrder.value = mode.value === 'contract'
    ? createCurrentMarginOrderReview(createMarginOrderIdempotencyKey())
    : null
  confirmOpen.value = true
}

function reviewContractOrder(nextSide: 'buy' | 'sell', event?: Event): void {
  side.value = nextSide
  reviewOrder(event)
}

function closeConfirm(): void {
  if (submitting.value) return
  confirmOpen.value = false
  reviewedMarginOrder.value = null
}

async function submitOrder(): Promise<void> {
  if (submitting.value) return
  feedback.value = ''
  const submittedMode = mode.value
  const review = submittedMode === 'contract' ? reviewedMarginOrder.value : null
  const orderAmount = review?.request.marginAmount ?? Number(quantity.value)
  const submittedOrderType = submittedMode === 'contract' ? 'market' : orderType.value
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

  if (submittedMode === 'contract') {
    const product = selectedProduct.value
    if (!review || !product || review.request.productId !== product.id) {
      setFeedback(t('trade.unavailableContract'))
      return
    }
    const requestMarginValidation = validateMarginAmount({
      amount: review.request.marginAmount,
      minMargin: product.minMargin,
      maxMargin: product.maxMargin,
    })
    if (!requestMarginValidation.isValid) {
      setFeedback(marginAmountValidationMessage(requestMarginValidation))
      return
    }
    if (!review.isValid) {
      setFeedback(t('trade.invalidOrder'))
      return
    }
  }

  submitting.value = true
  try {
    if (submittedMode === 'spot') {
      await placeSpotOrder({
        symbol: pairSymbol.value,
        side: side.value,
        type: submittedOrderType,
        price: limitPrice,
        quantity: orderAmount,
      })
    } else {
      if (!review) return
      await placeMarginOrder(review.request)
    }
    setFeedback(t('trade.orderSubmitted'), 'success')
    quantity.value = ''
    percentage.value = 0
    confirmOpen.value = false
    reviewedMarginOrder.value = null
    await loadTradingBalances()
  } catch (reason) {
    setFeedback(submittedMode === 'contract'
      ? marginOrderFailureMessage(reason)
      : apiErrorMessage(reason, t('trade.orderFailed')))
  } finally {
    submitting.value = false
  }
}

function trapDialogFocus(event: KeyboardEvent): void {
  trapConfirmFocus(event, closeConfirm)
}

onMounted(async () => {
  await marketStore.refresh()
  if (viewActive) marketStore.startLiveUpdates('trade')
})

watch(pairSymbol, (symbol) => {
  navigation.rememberTradeSymbol(symbol)
  marketDataPanel.value = 'orderBook'
  spotChartOpen.value = false
  void loadMarketData()
}, { immediate: true })

watch(() => route.query.mode, (nextMode) => {
  mode.value = nextMode === 'contract' ? 'contract' : 'spot'
  if (mode.value === 'contract') closeSpotOrderTypeSheet()
  else {
    contractSheet.value = null
    settingsError.value = ''
  }
  navigation.rememberTradeMode(mode.value)
  percentage.value = 0
  quantity.value = ''
}, { immediate: true })

watch(() => [mode.value, session.isAuthenticated, selectedProduct.value?.id] as const, () => {
  const product = selectedProduct.value
  if (mode.value !== 'contract' || !session.isAuthenticated || !product) {
    marginSettingRequestVersion += 1
    return
  }
  void syncMarginSetting(product)
})

watch([mode, () => session.isAuthenticated], () => {
  void loadMarginProducts()
  void loadTradingBalances()
}, { immediate: true })

watch(currentPrice, (value) => {
  if (!price.value && value > 0) price.value = String(value)
}, { immediate: true })

watch(submitting, async (busy) => {
  if (!busy || !confirmOpen.value) return
  await nextTick()
  if (submitting.value && confirmOpen.value) confirmDialog.value?.focus()
})

onBeforeUnmount(() => {
  viewActive = false
  marginProductsRequestVersion += 1
  marginSettingRequestVersion += 1
  marketStore.stopLiveUpdates('trade')
  detailStreamSession.stop()
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
          <AssetMark :symbol="baseAsset" :src="ticker?.iconUrl" :fallback-src="ticker?.baseIconUrl" :size="24" />
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
            :aria-busy="favoriteSaving"
            :disabled="favoriteSaving"
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
            :aria-label="t('trade.orderTypeTrigger', { type: orderType === 'limit' ? t('trade.limitOrderShort') : t('trade.marketOrderShort') })"
            aria-haspopup="dialog"
            :aria-expanded="spotOrderTypeOpen"
            aria-controls="spot-order-type-dialog"
            @click="openSpotOrderTypeSheet"
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

      <div class="spot-account-workspace">
        <nav class="spot-account-tabs" :aria-label="t('orders.category')">
          <button type="button" @click="openOrders('spot')">
            {{ t('trade.orders') }} <ChevronDown :size="12" aria-hidden="true" />
          </button>
          <span
            id="spot-holdings-label"
            class="spot-account-current active"
            aria-current="true"
          >
            {{ t('orders.positions') }}
          </span>
          <button type="button" :aria-label="t('trade.orderHistory')" @click="openOrders('history')">
            <History :size="19" aria-hidden="true" />
          </button>
        </nav>

        <section
          id="spot-holdings-panel"
          class="spot-holdings-panel"
          role="region"
          aria-labelledby="spot-holdings-label"
        >
          <div class="spot-holdings-context">
            <span><i aria-hidden="true" />{{ t('trade.onlyCurrent') }}</span>
            <button type="button" @click="openAssets">{{ t('common.viewAll') }}</button>
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
      </div>

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
            <button
              class="contract-pair-selector"
              type="button"
              :aria-label="t('markets.pickerTitle')"
              aria-haspopup="dialog"
              :aria-expanded="contractSheet === 'pair'"
              aria-controls="contract-pair-dialog"
              @click="openContractSheet('pair')"
            >
              <AssetMark :symbol="baseAsset" :src="ticker?.iconUrl" :fallback-src="ticker?.baseIconUrl" :size="24" />
              <span class="contract-pair-selector__copy">
                <span>
                  <strong>{{ pairSymbol.replace('/', '') }}</strong>
                  <ChevronDown :size="14" aria-hidden="true" />
                </span>
                <small class="numeric" :class="(ticker?.changePercent || 0) >= 0 ? 'positive' : 'negative'">
                  {{ t('trade.perpetualShort') }}
                  {{ ticker ? `${ticker.changePercent >= 0 ? '+' : ''}${ticker.changePercent.toFixed(2)}%` : '--' }}
                </small>
              </span>
            </button>
          </div>
          <button
            class="contract-header-control"
            :class="{ active: isFavorite }"
            type="button"
            :aria-label="t(isFavorite ? 'rootPrototype.removeFavorite' : 'rootPrototype.addFavorite', { symbol: pairSymbol })"
            :aria-pressed="isFavorite"
            :aria-busy="favoriteSaving"
            :disabled="favoriteSaving"
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
              <span>{{ t('trade.fundingAndCountdown') }}</span>
              <strong class="numeric">-- / --</strong>
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
                :mini-levels="6"
                :show-mini-precision="false"
              />
            </div>
            <p class="chart-semantic-summary">
              {{ pairSymbol }} · {{ ticker ? formatPrice(currentPrice) : t('common.marketUnavailable') }}
            </p>
          </div>

          <div class="trade-console">
            <div class="contract-open-close" role="group" :aria-label="t('orders.stateCategory')">
              <button type="button" class="active" aria-pressed="true">
                {{ t('trade.openPositionShort') }}
              </button>
              <button type="button" aria-pressed="false" @click="openOrders('positions')">
                {{ t('trade.closePositionShort') }}
              </button>
            </div>

            <div class="contract-mode-row" :aria-label="t('trade.settings')">
              <button
                type="button"
                aria-haspopup="dialog"
                :aria-expanded="contractSheet === 'marginMode'"
                aria-controls="contract-marginMode-dialog"
                :disabled="settingsSaving || productsLoading || !selectedProduct"
                @click="openContractSheet('marginMode')"
              >
                <span>{{ t(marginMode === 'cross' ? 'trade.cross' : 'trade.isolated') }}</span>
                <ChevronDown :size="12" aria-hidden="true" />
              </button>
              <button
                type="button"
                aria-haspopup="dialog"
                :aria-expanded="contractSheet === 'leverage'"
                aria-controls="contract-leverage-dialog"
                :disabled="settingsSaving || productsLoading || !selectedProduct"
                @click="openContractSheet('leverage')"
              >
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
              <button
                type="button"
                :aria-label="t('marketDetail.latestPrice')"
                :disabled="currentPrice <= 0"
                @click="price = String(currentPrice || '')"
              >
                {{ t('trade.bestBidOffer') }}
              </button>
            </div>

            <label
              class="contract-field contract-amount-field"
              :class="{ 'is-invalid': marginAmountError }"
              :data-margin-validation="marginAmountError ? 'invalid' : 'ready'"
            >
              <span class="sr-only">{{ t('trade.marginField', { asset: availableAsset }) }}</span>
              <input
                v-model="quantity"
                class="numeric"
                inputmode="decimal"
                :placeholder="t('trade.marginAmountShort')"
                :aria-describedby="marginRangeDescription ? 'contract-margin-range' : undefined"
                :aria-errormessage="marginAmountError ? 'contract-margin-error' : undefined"
                :aria-invalid="marginAmountError ? 'true' : 'false'"
                @input="clearPercentageSelection"
              />
              <b>{{ availableAsset }}</b>
            </label>

            <div v-if="marginRangeDescription" class="contract-margin-guidance">
              <p id="contract-margin-range">{{ marginRangeDescription }}</p>
              <p
                v-if="marginAmountError"
                id="contract-margin-error"
                class="contract-margin-error"
                role="alert"
              >
                {{ marginAmountError }}
              </p>
            </div>

            <div class="contract-percentage">
              <div class="percent-row" role="group" :aria-label="t('rootPrototype.balancePercentage')">
                <button
                  v-for="value in [0, 25, 50, 75, 100]"
                  :key="value"
                  type="button"
                  :class="{ active: percentage === value }"
                  :aria-label="value === 100 ? t('trade.marginMaximumShortcut') : `${value}%`"
                  :aria-pressed="percentage === value"
                  :disabled="submitting || (session.isAuthenticated && (productsLoading || balancesLoading || !selectedProduct))"
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
                <dd v-if="!session.isAuthenticated" class="contract-balance-control">
                  <button type="button" @click="openLogin">{{ t('trade.viewAfterLogin') }}</button>
                </dd>
                <dd v-else-if="balancesError" class="contract-balance-control">
                  <button type="button" :disabled="balancesLoading" @click="loadTradingBalances">{{ t('common.retry') }}</button>
                </dd>
                <dd v-else class="numeric contract-balance-control">
                  <button
                    type="button"
                    class="contract-balance-action"
                    :disabled="balancesLoading"
                    @click="openAssets"
                  >
                    <span>{{ balancesLoading ? t('trade.loadBalance') : `${formatAmount(availableBalance)} ${availableAsset}` }}</span>
                    <CirclePlus v-if="!balancesLoading" :size="11" aria-hidden="true" />
                  </button>
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
              :disabled="submitting || productsLoading || !isLive || (session.isAuthenticated && !selectedProduct)"
              @click="reviewContractOrder('buy', $event)"
            >
              {{ t('trade.longActionCompact') }}
            </button>
            <button
              class="contract-submit contract-submit--short"
              type="button"
              :disabled="submitting || productsLoading || !isLive || (session.isAuthenticated && !selectedProduct)"
              @click="reviewContractOrder('sell', $event)"
            >
              {{ t('trade.shortActionCompact') }}
            </button>
          </div>
        </section>

        <nav class="contract-position-tabs" :aria-label="t('orders.category')">
          <button class="active" type="button" @click="openOrders(mode === 'contract' ? 'positions' : 'spot')">
            {{ t('orders.positions') }} ({{ visibleMarginPositions.length }})
          </button>
          <button type="button" @click="openOrders('positions')">
            {{ t('trade.contractOrdersTab') }} (0) <ChevronDown :size="12" aria-hidden="true" />
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
          <span><Inbox :size="24" aria-hidden="true" /></span>
          <strong>{{ session.isAuthenticated ? t('orders.noPositions') : t('trade.ordersLoginHint') }}</strong>
          <button v-if="!session.isAuthenticated" type="button" @click="openLogin">{{ t('common.loginNow') }}</button>
        </section>
      </section>
    </template>

    <ContractTradeSheets
      v-if="!isSpotMode"
      :open="contractSheet"
      :pair-symbol="pairSymbol"
      :product="selectedProduct"
      :products="products"
      :leverage="leverage"
      :margin-mode="marginMode"
      :saving="settingsSaving"
      :error="settingsError"
      :products-loading="productsLoading"
      :products-error="productsError"
      @close="closeContractSheet"
      @select-pair="selectContractPair"
      @apply-leverage="applyContractLeverage"
      @apply-margin-mode="applyContractMarginMode"
      @retry-products="loadMarginProducts"
    />

    <Teleport to="body">
      <div v-if="isSpotMode && spotOrderTypeOpen" class="spot-order-type-layer">
        <button
          class="spot-order-type-overlay"
          type="button"
          :aria-label="t('common.close')"
          tabindex="-1"
          @click="closeSpotOrderTypeSheet"
        />
        <section
          id="spot-order-type-dialog"
          ref="spotOrderTypeDialog"
          class="spot-order-type-sheet"
          role="dialog"
          aria-modal="true"
          aria-labelledby="spot-order-type-title"
          aria-describedby="spot-order-type-hint"
          tabindex="-1"
          @keydown="handleSpotOrderTypeKeydown"
        >
          <span class="spot-order-type-sheet__grab" aria-hidden="true" />
          <header class="spot-order-type-sheet__header">
            <div>
              <h2 id="spot-order-type-title">{{ t('trade.orderTypeSheetTitle') }}</h2>
              <p id="spot-order-type-hint">{{ t('trade.orderTypeSheetHint') }}</p>
            </div>
            <button
              class="spot-order-type-sheet__close"
              type="button"
              :aria-label="t('common.close')"
              @click="closeSpotOrderTypeSheet"
            >
              <X :size="20" aria-hidden="true" />
            </button>
          </header>
          <div class="spot-order-type-options" role="group" :aria-label="t('trade.orderTypeSheetTitle')">
            <button
              type="button"
              :class="{ active: orderType === 'limit' }"
              :aria-pressed="orderType === 'limit'"
              :data-order-type-current="orderType === 'limit'"
              @click="selectSpotOrderType('limit')"
            >
              <span class="spot-order-type-option__icon" aria-hidden="true"><List :size="20" /></span>
              <span class="spot-order-type-option__copy">
                <strong>{{ t('trade.limitOrderShort') }}</strong>
                <small>{{ t('trade.limitOrderDescription') }}</small>
              </span>
              <CheckCircle2 v-if="orderType === 'limit'" :size="20" aria-hidden="true" />
            </button>
            <button
              type="button"
              :class="{ active: orderType === 'market' }"
              :aria-pressed="orderType === 'market'"
              :data-order-type-current="orderType === 'market'"
              @click="selectSpotOrderType('market')"
            >
              <span class="spot-order-type-option__icon" aria-hidden="true"><ArrowLeftRight :size="20" /></span>
              <span class="spot-order-type-option__copy">
                <strong>{{ t('trade.marketOrderShort') }}</strong>
                <small>{{ t('trade.marketOrderDescription') }}</small>
              </span>
              <CheckCircle2 v-if="orderType === 'market'" :size="20" aria-hidden="true" />
            </button>
          </div>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div
        v-if="confirmOpen"
        class="confirmation-layer"
        :class="{ 'contract-order-confirm-layer': !isSpotMode }"
        :data-order-confirm-mode="mode"
      >
        <button
          class="confirmation-overlay-dismiss"
          type="button"
          :aria-label="t('common.close')"
          :disabled="submitting"
          tabindex="-1"
          @click="closeConfirm"
        />

        <section
          v-if="isSpotMode"
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
          <p>{{ pairSymbol }} · {{ formatAmount(Number(quantity || 0)) }} {{ baseAsset }}</p>
          <div class="confirmation-detail">
            <span>{{ t('common.price') }} {{ formatPrice(effectivePrice) }} {{ quoteAsset }}</span>
            <span>
              {{ t('common.amount') }}
              {{ formatAmount(Number(amountValue) || 0) }}
              {{ quoteAsset }}
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

        <section
          v-else
          ref="confirmDialog"
          class="contract-order-confirm"
          role="dialog"
          aria-modal="true"
          aria-labelledby="contract-order-confirm-title"
          aria-describedby="contract-order-confirm-risk"
          :aria-busy="submitting"
          tabindex="-1"
          @keydown="trapDialogFocus"
        >
          <div class="contract-order-confirm__top">
            <span class="contract-order-confirm__grab" aria-hidden="true" />

            <header class="contract-order-confirm__header">
              <div>
                <h2 id="contract-order-confirm-title">{{ t('trade.contractOrderConfirmTitle') }}</h2>
                <p>{{ t('trade.contractOrderConfirmHint') }}</p>
              </div>
              <button
                data-dialog-cancel
                class="contract-order-confirm__close"
                type="button"
                :disabled="submitting"
                :aria-label="t('common.close')"
                @click="closeConfirm"
              >
                <X :size="20" aria-hidden="true" />
              </button>
            </header>
          </div>

          <div class="contract-order-confirm__body">
            <section class="contract-order-confirm__identity" :aria-label="pairSymbol">
              <AssetMark
                :symbol="baseAsset"
                :src="ticker?.iconUrl"
                :fallback-src="ticker?.baseIconUrl"
                :size="36"
              />
              <div>
                <strong>{{ pairSymbol }}</strong>
                <span>{{ t('trade.perpetualShort') }} · {{ t('trade.marketOrderShort') }}</span>
              </div>
              <span
                class="contract-order-confirm__direction"
                :class="contractOrderReview.request.side === 'long' ? 'is-long' : 'is-short'"
              >
                {{ t(contractOrderReview.request.side === 'long' ? 'orders.long' : 'orders.short') }}
              </span>
            </section>

            <dl class="contract-order-confirm__details">
              <div>
                <dt>{{ t('rootPrototype.marginMode') }}</dt>
                <dd>{{ t(contractOrderReview.request.marginMode === 'cross' ? 'trade.cross' : 'trade.isolated') }}</dd>
              </div>
              <div>
                <dt>{{ t('rootPrototype.leverage') }}</dt>
                <dd class="numeric">{{ contractOrderReview.request.leverage }}x</dd>
              </div>
              <div>
                <dt>{{ t('trade.contractReferencePrice') }}</dt>
                <dd class="numeric">{{ formatPrice(contractOrderReview.referencePrice) }} {{ quoteAsset }}</dd>
              </div>
              <div>
                <dt>{{ t('trade.contractMarginCommitted') }}</dt>
                <dd class="numeric">{{ formatAmount(contractOrderReview.request.marginAmount) }} {{ availableAsset }}</dd>
              </div>
              <div>
                <dt>{{ t('rootPrototype.estimatedNotional') }}</dt>
                <dd class="numeric">{{ formatAmount(contractNotionalValue) }} {{ availableAsset }}</dd>
              </div>
              <div>
                <dt>{{ t('trade.contractEstimatedQuantity') }}</dt>
                <dd class="numeric">{{ formatAmount(contractOrderQuantity) }} {{ baseAsset }}</dd>
              </div>
            </dl>

            <aside id="contract-order-confirm-risk" class="contract-order-confirm__risk">
              <TriangleAlert :size="18" aria-hidden="true" />
              <div>
                <strong>{{ t('trade.marketExecutionRiskTitle') }}</strong>
                <p>{{ t('trade.marketExecutionRiskDescription') }}</p>
              </div>
            </aside>
          </div>

          <footer class="contract-order-confirm__actions">
            <p
              v-if="feedback && !feedbackIsPositive"
              class="contract-order-confirm__error"
              role="alert"
              aria-live="assertive"
            >
              {{ feedback }}
            </p>
            <button
              class="contract-order-confirm__submit"
              type="button"
              :disabled="submitting || productsLoading"
              @click="submitOrder"
            >
              <CheckCircle2 v-if="!submitting" :size="18" aria-hidden="true" />
              {{ submitting
                ? t('trade.submittingOrder')
                : t('trade.confirmContractOrder', { direction: t(contractOrderReview.request.side === 'long' ? 'orders.long' : 'orders.short') }) }}
            </button>
          </footer>
        </section>
      </div>
    </Teleport>
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
  height: 44px;
  min-width: 0;
  padding: 0 12px;
}

.spot-trade .spot-type-field {
  min-height: 44px;
}

.spot-type-field[aria-expanded='true'] {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
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

.spot-order-type-layer {
  align-items: end;
  box-sizing: border-box;
  color: var(--ink);
  display: grid;
  height: 100vh;
  height: 100dvh;
  inset: 0;
  isolation: isolate;
  min-width: 0;
  overflow: hidden;
  overscroll-behavior: contain;
  position: fixed;
  width: 100%;
  z-index: 90;
}

.spot-order-type-overlay {
  background: var(--overlay);
  border: 0;
  height: 100%;
  inset: 0;
  padding: 0;
  position: absolute;
  width: 100%;
  z-index: 0;
}

.spot-order-type-sheet {
  background: var(--surface-elevated);
  border: 1px solid color-mix(in srgb, var(--ink) 14%, transparent);
  border-bottom: 0;
  border-radius: 20px 20px 0 0;
  box-shadow: 0 -18px 48px color-mix(in srgb, var(--ink) 24%, transparent);
  box-sizing: border-box;
  display: grid;
  gap: 12px;
  justify-self: center;
  max-height: calc(100dvh - max(16px, env(safe-area-inset-top)));
  max-width: 448px;
  min-width: 0;
  overflow: hidden;
  padding:
    8px max(16px, env(safe-area-inset-right))
    calc(16px + env(safe-area-inset-bottom))
    max(16px, env(safe-area-inset-left));
  position: relative;
  width: 100%;
  z-index: 1;
}

.spot-order-type-sheet__grab {
  background: color-mix(in srgb, var(--ink) 26%, transparent);
  border-radius: 999px;
  display: block;
  height: 4px;
  justify-self: center;
  width: 40px;
}

.spot-order-type-sheet__header {
  align-items: center;
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) 44px;
  min-width: 0;
}

.spot-order-type-sheet__header > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.spot-order-type-sheet__header h2,
.spot-order-type-sheet__header p {
  margin: 0;
  overflow-wrap: anywhere;
}

.spot-order-type-sheet__header h2 {
  font-size: 18px;
  line-height: 24px;
}

.spot-order-type-sheet__header p {
  color: var(--muted-strong);
  font-size: 11px;
  line-height: 16px;
}

.spot-order-type-sheet__close {
  align-items: center;
  background: color-mix(in srgb, var(--ink) 6%, var(--surface-elevated));
  border: 1px solid color-mix(in srgb, var(--ink) 14%, transparent);
  border-radius: 999px;
  color: var(--muted-strong);
  display: flex;
  height: 44px;
  justify-content: center;
  padding: 0;
  width: 44px;
}

.spot-order-type-options {
  display: grid;
  gap: 8px;
  min-width: 0;
}

.spot-order-type-options > button {
  align-items: center;
  background: var(--field-surface);
  border: 1px solid color-mix(in srgb, var(--ink) 14%, transparent);
  border-radius: 12px;
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-columns: 36px minmax(0, 1fr) 20px;
  min-height: 64px;
  min-width: 0;
  padding: 10px 12px;
  text-align: left;
  width: 100%;
}

.spot-order-type-options > button.active {
  background: var(--positive-soft);
  border-color: var(--positive);
}

.spot-order-type-option__icon {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid color-mix(in srgb, var(--ink) 14%, transparent);
  border-radius: 10px;
  color: var(--muted-strong);
  display: flex;
  height: 36px;
  justify-content: center;
  width: 36px;
}

.spot-order-type-options > button.active .spot-order-type-option__icon,
.spot-order-type-options > button.active > svg {
  color: var(--positive);
}

.spot-order-type-option__copy {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.spot-order-type-option__copy strong {
  font-size: 14px;
  line-height: 20px;
}

.spot-order-type-option__copy small {
  color: var(--muted-strong);
  font-size: 11px;
  line-height: 16px;
  overflow-wrap: anywhere;
}

.spot-order-type-layer button:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
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

.spot-account-tabs :is(button, .spot-account-current) {
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

.spot-account-tabs .spot-account-current {
  overflow: hidden;
  text-overflow: ellipsis;
}

.spot-account-tabs .active {
  color: var(--text);
  font-weight: 660;
}

.spot-account-tabs .active::after {
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

.spot-holdings-panel {
  min-height: 232px;
}

.spot-holdings-context {
  align-items: center;
  border-top: 1px solid color-mix(in srgb, var(--line) 66%, transparent);
  display: flex;
  font-size: 10px;
  height: 34px;
  justify-content: space-between;
  min-height: 34px;
  padding: 0 16px;
}

.spot-holdings-context > span {
  align-items: center;
  color: var(--muted);
  display: inline-flex;
  gap: 7px;
}

.spot-holdings-context i {
  border: 1px solid var(--border);
  border-radius: 999px;
  height: 12px;
  width: 12px;
}

.spot-holdings-context button {
  background: transparent;
  border: 0;
  color: var(--text);
  font-size: 10px;
  min-height: 44px;
  min-width: 44px;
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
  min-height: 44px;
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

.confirmation-layer[data-order-confirm-mode='spot'] {
  --page: var(--surface);
  --surface-2: var(--soft);
  --text: var(--ink);
  --cyan: var(--focus);
}

.confirmation-sheet {
  padding-bottom: calc(18px + env(safe-area-inset-bottom));
}

.contract-order-confirm-layer,
.contract-order-confirm-layer * {
  box-sizing: border-box;
}

.contract-order-confirm-layer {
  align-items: end;
  bottom: 0;
  display: grid;
  height: 100vh;
  height: 100dvh;
  isolation: isolate;
  left: auto;
  max-width: none;
  overflow: hidden;
  overscroll-behavior: contain;
  position: fixed;
  right: 5.5vw;
  top: 0;
  width: min(100%, 448px);
  z-index: var(--layer-overlay, 80);
}

.contract-order-confirm-layer .confirmation-overlay-dismiss {
  background: rgb(7 17 13 / 64%);
}

.contract-order-confirm-layer .confirmation-overlay-dismiss:disabled {
  opacity: 1;
}

.contract-order-confirm {
  --confirm-page: #ffffff;
  --confirm-canvas: #f7f9f8;
  --confirm-raised: #eef2f0;
  --confirm-line: #ccd5d0;
  --confirm-line-strong: #aebbb4;
  --confirm-text: #111714;
  --confirm-muted: #68736d;
  --confirm-accent: #43efa9;
  --confirm-accent-strong: #087b52;
  --confirm-accent-soft: #d9f9eb;
  --confirm-negative: #d54732;
  --confirm-negative-soft: #fff0ec;
  --confirm-warning: #e79a2b;
  --confirm-warning-surface: rgb(255 180 84 / 9%);
  --confirm-warning-line: rgb(255 180 84 / 30%);
  background: var(--confirm-page);
  border: 0;
  border-radius: 22px 22px 0 0;
  border-top: 1px solid var(--confirm-line);
  box-shadow: 0 -10px 28px rgb(7 17 13 / 20%);
  color: var(--confirm-text);
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  height: min(620px, calc(100vh - max(12px, env(safe-area-inset-top, 0px))));
  height: min(620px, calc(100dvh - max(12px, env(safe-area-inset-top, 0px))));
  justify-self: center;
  max-height: calc(100vh - max(12px, env(safe-area-inset-top, 0px)));
  max-height: calc(100dvh - max(12px, env(safe-area-inset-top, 0px)));
  max-width: 448px;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  overscroll-behavior: auto;
  padding:
    11px max(16px, env(safe-area-inset-right, 0px))
    calc(18px + env(safe-area-inset-bottom, 0px))
    max(16px, env(safe-area-inset-left, 0px));
  position: relative;
  row-gap: 12px;
  width: 100%;
  z-index: 1;
}

html[data-theme='dark'] .contract-order-confirm {
  --confirm-page: #0c100e;
  --confirm-canvas: #070a09;
  --confirm-raised: #121714;
  --confirm-line: #29342e;
  --confirm-line-strong: #3a4a42;
  --confirm-text: #f2f7f4;
  --confirm-muted: #95a19a;
  --confirm-accent: #43efa9;
  --confirm-accent-strong: #61f1b6;
  --confirm-accent-soft: #103326;
  --confirm-negative: #ff7860;
  --confirm-negative-soft: #391a20;
  --confirm-warning: #f1b95c;
  --confirm-warning-surface: rgb(255 180 84 / 10%);
  --confirm-warning-line: rgb(255 180 84 / 28%);
  box-shadow: 0 -10px 28px rgb(0 0 0 / 64%);
}

.contract-order-confirm__top {
  display: grid;
  grid-template-rows: 14px minmax(44px, auto);
  min-width: 0;
  row-gap: 12px;
}

.contract-order-confirm__grab {
  align-items: center;
  display: flex;
  height: 14px;
  justify-content: center;
  width: 100%;
}

.contract-order-confirm__grab::before {
  background: var(--confirm-muted);
  border-radius: 2px;
  content: '';
  height: 4px;
  opacity: .72;
  width: 38px;
}

.contract-order-confirm__header {
  align-items: center;
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) 44px;
  min-height: 44px;
  min-width: 0;
}

.contract-order-confirm__header > div {
  min-width: 0;
}

.contract-order-confirm__header h2,
.contract-order-confirm__header p {
  margin: 0;
  overflow-wrap: anywhere;
}

.contract-order-confirm__header h2 {
  font-size: 19px;
  font-weight: 750;
  letter-spacing: -.02em;
  line-height: 1.15;
}

.contract-order-confirm__header p {
  color: var(--confirm-muted);
  font-size: 10px;
  font-weight: 500;
  line-height: 1.3;
  margin-top: 3px;
}

.contract-order-confirm__close {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 50%;
  color: var(--confirm-text);
  display: inline-flex;
  height: 44px;
  justify-content: center;
  padding: 0;
  position: relative;
  width: 44px;
  z-index: 0;
}

.contract-order-confirm__close::before {
  background: var(--confirm-canvas);
  border: 1px solid var(--confirm-line);
  border-radius: 50%;
  content: '';
  inset: 4px;
  position: absolute;
  z-index: -1;
}

.contract-order-confirm__body {
  align-content: start;
  display: grid;
  gap: 12px;
  min-height: 0;
  min-width: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 1px;
  scrollbar-width: none;
}

.contract-order-confirm__body::-webkit-scrollbar {
  display: none;
}

.contract-order-confirm__identity {
  align-items: center;
  background: var(--confirm-canvas);
  border: 1px solid var(--confirm-line);
  border-radius: 12px;
  display: grid;
  gap: 10px;
  grid-template-columns: 36px minmax(0, 1fr) auto;
  min-height: 70px;
  min-width: 0;
  padding: 11px 12px;
}

.contract-order-confirm__identity > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.contract-order-confirm__identity strong {
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 15px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-order-confirm__identity > div > span {
  color: var(--confirm-muted);
  font-size: 10px;
  line-height: 1.3;
}

.contract-order-confirm__direction {
  align-items: center;
  border: 1px solid transparent;
  border-radius: 999px;
  display: inline-flex;
  font-size: 11px;
  font-weight: 700;
  justify-content: center;
  min-height: 28px;
  padding: 5px 10px;
  white-space: nowrap;
}

.contract-order-confirm__direction.is-long {
  background: var(--confirm-accent-soft);
  border-color: var(--confirm-accent);
  color: var(--confirm-accent-strong);
}

.contract-order-confirm__direction.is-short {
  background: var(--confirm-negative-soft);
  border-color: color-mix(in srgb, var(--confirm-negative) 48%, transparent);
  color: var(--confirm-negative);
}

.contract-order-confirm__details {
  background: var(--confirm-canvas);
  border: 1px solid var(--confirm-line);
  border-radius: 12px;
  margin: 0;
  min-width: 0;
  overflow: hidden;
  padding: 0 12px;
}

.contract-order-confirm__details > div {
  align-items: center;
  border-bottom: 1px solid var(--confirm-line);
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 39px;
  min-width: 0;
  padding: 7px 0;
}

.contract-order-confirm__details > div:last-child {
  border-bottom: 0;
}

.contract-order-confirm__details dt {
  color: var(--confirm-muted);
  font-size: 11px;
  line-height: 1.3;
  min-width: 0;
}

.contract-order-confirm__details dd {
  font-size: 12px;
  font-weight: 650;
  line-height: 1.3;
  margin: 0;
  max-width: 190px;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.contract-order-confirm__risk {
  align-items: flex-start;
  background: var(--confirm-warning-surface);
  border: 1px solid var(--confirm-warning-line);
  border-radius: 10px;
  color: var(--confirm-text);
  display: grid;
  gap: 9px;
  grid-template-columns: 18px minmax(0, 1fr);
  min-width: 0;
  padding: 10px;
}

.contract-order-confirm__risk > svg {
  color: var(--confirm-warning);
  margin-top: 1px;
}

.contract-order-confirm__risk > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.contract-order-confirm__risk strong {
  font-size: 11px;
  font-weight: 700;
  line-height: 1.3;
}

.contract-order-confirm__risk p {
  color: var(--confirm-muted);
  font-size: 10px;
  line-height: 1.45;
  margin: 0;
  overflow-wrap: anywhere;
}

.contract-order-confirm__actions {
  display: grid;
  gap: 8px;
  min-width: 0;
}

.contract-order-confirm__error {
  background: var(--confirm-negative-soft);
  border: 1px solid color-mix(in srgb, var(--confirm-negative) 42%, transparent);
  border-radius: 9px;
  color: var(--confirm-negative);
  font-size: 10px;
  line-height: 1.4;
  margin: 0;
  max-height: 96px;
  max-height: min(96px, 18dvh);
  overflow-x: hidden;
  overflow-y: auto;
  overflow-wrap: anywhere;
  overscroll-behavior: contain;
  padding: 8px 10px;
}

.contract-order-confirm__submit {
  align-items: center;
  background: var(--confirm-accent);
  border: 0;
  border-radius: 24px;
  color: #07110d;
  display: inline-flex;
  font-size: 14px;
  font-weight: 750;
  gap: 7px;
  height: 48px;
  justify-content: center;
  min-width: 0;
  padding: 0 16px;
  width: 100%;
}

.contract-order-confirm__submit:disabled {
  background: var(--confirm-raised);
  color: var(--confirm-muted);
}

.contract-order-confirm button:focus-visible {
  outline: 2px solid var(--confirm-accent);
  outline-offset: 2px;
}

.contract-order-confirm__close:focus-visible {
  outline: 0;
}

.contract-order-confirm__close:focus-visible::before {
  box-shadow: 0 0 0 2px var(--confirm-accent);
}

.contract-trade {
  --contract-control-border: color-mix(in srgb, var(--line-strong) 86%, var(--text) 14%);
  --contract-control-surface: linear-gradient(
    180deg,
    color-mix(in srgb, var(--surface) 94%, var(--text) 6%),
    var(--surface-elevated)
  );
  --contract-control-shadow:
    inset 0 1px 0 color-mix(in srgb, var(--text) 10%, transparent),
    0 2px 6px color-mix(in srgb, var(--ink) 12%, transparent);
  --contract-control-shadow-pressed:
    inset 0 2px 4px color-mix(in srgb, var(--ink) 16%, transparent),
    0 1px 2px color-mix(in srgb, var(--ink) 10%, transparent);
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
  height: 61px;
  padding: 8px 20px;
  position: sticky;
  top: 0;
  z-index: var(--layer-sticky-header);
}

.contract-header-control {
  align-items: center;
  background: var(--contract-control-surface);
  border: 1px solid var(--contract-control-border);
  border-radius: 999px;
  box-shadow: var(--contract-control-shadow);
  color: var(--text);
  display: inline-flex;
  height: 44px;
  justify-content: center;
  margin-inline: -2px;
  padding: 0;
  width: 44px;
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
  display: inline-flex;
  gap: 4px;
  justify-content: center;
  min-height: 44px;
  min-width: 0;
  padding: 0 4px;
}

.contract-pair-selector__copy {
  display: grid;
  gap: 2px;
  justify-items: start;
  min-width: 0;
}

.contract-pair-selector__copy > span {
  align-items: center;
  display: inline-flex;
  gap: 4px;
  min-width: 0;
}

.contract-pair-selector strong {
  font-size: 17px;
  font-weight: 750;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-pair-selector small {
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 10px;
  font-weight: 650;
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-pair-selector[aria-expanded='true'] {
  color: var(--positive);
}

.contract-pair-selector :deep(.asset-mark) {
  border: 0;
  box-shadow: var(--asset-mark-shadow, none);
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
  background: transparent !important;
  border: 0;
  display: grid;
  gap: 8px;
  grid-column: 1;
  grid-row: 1;
  min-width: 0;
  padding: 0;
}

.contract-open-close {
  background: color-mix(in srgb, var(--surface-elevated) 88%, var(--text) 12%);
  border: 1px solid var(--contract-control-border);
  border-radius: 8px;
  box-shadow: inset 0 1px 0 color-mix(in srgb, var(--text) 8%, transparent);
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
  font-size: 12px;
  font-weight: 650;
  height: 44px;
  margin-block: -7px;
  min-width: 0;
  padding: 0 4px;
  position: relative;
  z-index: 0;
}

.contract-open-close button::before {
  border-radius: 6px;
  content: "";
  inset: 7px 0;
  position: absolute;
  z-index: -1;
}

.contract-open-close button.active {
  background: transparent;
  color: var(--on-positive);
}

.contract-open-close button.active::before {
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--accent) 84%, white 16%),
    var(--accent)
  );
  box-shadow: inset 0 1px 0 color-mix(in srgb, white 50%, transparent);
}

.contract-mode-row {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  height: 36px;
}

.contract-mode-row button,
.contract-order-type {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 8px;
  color: var(--text);
  display: flex;
  height: 44px;
  justify-content: space-between;
  margin-block: -4px;
  min-width: 0;
  padding: 0 10px;
  position: relative;
  z-index: 0;
}

.contract-mode-row button::before,
.contract-order-type::before {
  background: var(--contract-control-surface);
  border: 1px solid var(--contract-control-border);
  border-radius: 8px;
  box-shadow: var(--contract-control-shadow);
  content: "";
  inset: 4px 0;
  pointer-events: none;
  position: absolute;
  z-index: -1;
}

.contract-mode-row button[aria-expanded='true'] {
  color: var(--positive);
}

.contract-mode-row button[aria-expanded='true']::before {
  border-color: var(--accent);
}

.contract-price-row > button {
  align-items: center;
  background: var(--contract-control-surface);
  border: 1px solid var(--contract-control-border);
  border-radius: 8px;
  box-shadow: var(--contract-control-shadow);
  color: var(--text);
  display: flex;
  height: 44px;
  justify-content: center;
  min-width: 0;
  padding: 0 5px;
}

.contract-mode-row button:disabled,
.contract-order-type:disabled {
  color: var(--muted);
  cursor: not-allowed;
  opacity: .58;
}

.contract-mode-row button:disabled::before,
.contract-order-type:disabled::before {
  box-shadow: none;
}

.contract-mode-row span,
.contract-order-type span {
  font-size: 12px;
  font-weight: 650;
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
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 12px;
  font-weight: 650;
}

.contract-field {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  display: grid;
  min-width: 0;
}

.contract-field:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.contract-field.is-invalid,
.contract-field.is-invalid:focus-within {
  border-color: var(--negative);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--negative) 16%, transparent);
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

.contract-amount-field input {
  font-size: 11px;
  grid-column: 1;
  grid-row: 1;
  height: 38px;
  padding: 0;
}

.contract-amount-field b {
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 11px;
  grid-column: 2;
  grid-row: 1;
}

.contract-margin-guidance {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.contract-margin-guidance p {
  color: var(--muted);
  font-size: 9px;
  line-height: 1.35;
  margin: 0;
  min-width: 0;
  overflow-wrap: anywhere;
}

.contract-margin-guidance .contract-margin-error {
  color: var(--negative);
  font-weight: 650;
}

.contract-percentage {
  display: grid;
  min-height: 92px;
  min-width: 0;
}

.contract-trade .contract-percentage .percent-row {
  display: grid;
  gap: 4px;
  grid-auto-rows: 44px;
  grid-template-columns: repeat(3, minmax(44px, 1fr));
  height: auto;
  margin: 0;
  min-width: 0;
}

.contract-percentage button {
  background: transparent;
  border: 0;
  color: var(--muted);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 10px;
  height: 44px;
  min-height: 44px;
  min-width: 44px;
  padding: 0 3px;
  position: relative;
  z-index: 0;
}

.contract-percentage button::before {
  background: var(--contract-control-surface);
  border: 1px solid var(--contract-control-border);
  border-radius: 8px;
  box-shadow: var(--contract-control-shadow);
  content: "";
  inset: 5px 2px;
  pointer-events: none;
  position: absolute;
  z-index: -1;
}

.contract-percentage button.active {
  color: var(--positive);
  font-weight: 750;
}

.contract-percentage button.active::before {
  background: color-mix(in srgb, var(--accent) 18%, var(--surface-elevated));
  border-color: color-mix(in srgb, var(--accent) 72%, var(--contract-control-border));
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, var(--accent) 34%, transparent),
    0 2px 7px color-mix(in srgb, var(--accent) 14%, transparent);
}

.contract-trade .contract-percentage .percent-row button::after {
  display: none;
}

.contract-trade .contract-percentage .percent-row button {
  background: transparent;
  border: 0;
  color: var(--muted);
  min-height: 44px;
  min-width: 44px;
}

.contract-trade .contract-percentage .percent-row button.active {
  background: transparent;
  border: 0;
  color: var(--positive);
}

.contract-percentage button:disabled::before {
  box-shadow: none;
}

.contract-tpsl {
  align-items: center;
  display: flex;
  font-size: 11px;
  font-weight: 500;
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
  gap: 4px;
  grid-template-rows: 44px 18px;
  margin: 0;
}

.contract-balance-rows > div {
  align-items: center;
  display: flex;
  font-size: 10px;
  gap: 8px;
  justify-content: space-between;
  min-height: 0;
  min-width: 0;
}

.contract-balance-rows dt {
  color: var(--muted);
  flex: 0 0 auto;
}

.contract-balance-rows dd {
  margin: 0;
  min-width: 0;
}

.contract-balance-rows dd:not(.contract-balance-control) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-balance-control {
  overflow: visible;
}

.contract-balance-rows button {
  background: var(--contract-control-surface);
  border: 1px solid var(--contract-control-border);
  border-radius: 999px;
  box-shadow: var(--contract-control-shadow);
  color: var(--text);
  font-size: 10px;
  min-height: 44px;
  min-width: 44px;
  padding: 0 10px;
}

.contract-balance-rows .contract-balance-action {
  align-items: center;
  display: inline-flex;
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  gap: 3px;
  height: 44px;
  justify-content: flex-end;
  max-width: 100%;
}

.contract-balance-action > span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, white 44%, transparent),
    0 4px 10px color-mix(in srgb, var(--ink) 18%, transparent);
  font-size: 14px;
  font-weight: 750;
  height: 46px;
  min-width: 0;
  padding: 0 8px;
  width: 100%;
}

.contract-submit--long,
.contract-trade .contract-submit--long.submit-order {
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--accent) 86%, white 14%),
    var(--accent)
  );
  color: var(--on-positive);
  min-height: 46px;
}

.contract-submit--short {
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--negative) 84%, white 16%),
    var(--negative)
  );
  color: var(--on-negative);
}

.contract-submit:disabled {
  background: var(--contract-control-surface);
  box-shadow: none;
  color: var(--muted);
  cursor: not-allowed;
  opacity: .58;
}

:is(
  .contract-header-control,
  .contract-open-close button,
  .contract-mode-row button,
  .contract-order-type,
  .contract-price-row > button,
  .contract-percentage button,
  .contract-balance-action,
  .contract-submit
) {
  transition:
    transform 120ms ease,
    box-shadow 120ms ease,
    border-color 120ms ease,
    color 120ms ease,
    opacity 120ms ease;
}

:is(
  .contract-header-control,
  .contract-open-close button,
  .contract-mode-row button,
  .contract-price-row > button,
  .contract-percentage button,
  .contract-balance-action,
  .contract-submit
):active:not(:disabled) {
  box-shadow: var(--contract-control-shadow-pressed);
  transform: translateY(1px);
}

:is(
  .contract-mode-row button,
  .contract-percentage button
):active:not(:disabled)::before {
  box-shadow: var(--contract-control-shadow-pressed);
}

:is(
  .contract-header-control,
  .contract-open-close button,
  .contract-mode-row button,
  .contract-order-type,
  .contract-price-row > button,
  .contract-percentage button,
  .contract-balance-action,
  .contract-submit
):disabled {
  box-shadow: none;
  cursor: not-allowed;
  opacity: .58;
  transform: none;
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
  height: 26px;
  justify-content: center;
  min-width: 0;
}

.contract-book-status span,
.contract-book-status strong {
  font-size: 8px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-book-status span {
  color: var(--muted);
  font-family: var(--font-geist-sans), var(--font-family), sans-serif;
}

.contract-book-status strong {
  color: var(--text);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 9px;
  font-weight: 650;
}

.contract-trade .trade-order-book {
  background: transparent;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.contract-mini-book {
  height: 346px;
  min-height: 346px;
}

.contract-mini-book :deep(.order-book__mini-header) {
  height: 18px;
}

.contract-mini-book :deep(.order-book__mini-row) {
  font-size: 11px;
  height: 22.8px;
}

.contract-mini-book :deep(.order-book__mini-mid) {
  height: 34px;
}

.contract-mini-book :deep(.order-book__mini-mid strong) {
  font-size: 15px;
}

.contract-mini-book :deep(.order-book__mini-ratio) {
  gap: 6px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  height: 16px;
}

.contract-mini-book :deep(.order-book__mini-ratio)::before {
  background: linear-gradient(
    90deg,
    var(--accent) 0 var(--mini-bid-ratio),
    var(--negative) var(--mini-bid-ratio) 100%
  );
  border-radius: 2px;
  content: "";
  grid-column: 2;
  height: 4px;
}

.contract-mini-book :deep(.order-book__mini-ratio span:first-child) {
  grid-column: 1;
}

.contract-mini-book :deep(.order-book__mini-ratio span:last-child) {
  grid-column: 3;
}

.contract-mini-book :deep(.order-book__mini-precision) {
  font-size: 8px;
  height: 20px;
  margin-top: 0;
  padding-inline: 5px;
}

.contract-mini-book :deep(.order-book__mini-state) {
  height: 346px;
}

.contract-position-tabs {
  align-items: center;
  display: grid;
  gap: 18px;
  grid-template-columns: auto auto auto minmax(0, 1fr) 40px;
  height: 37px;
  min-height: 37px;
  padding: 8px 20px 4px;
}

.contract-position-tabs button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted);
  display: inline-flex;
  font-size: 12px;
  gap: 3px;
  justify-content: center;
  height: 44px;
  margin-block: -5px;
  padding: 0;
  position: relative;
  white-space: nowrap;
}

.contract-position-tabs button.active {
  color: var(--text);
  font-size: 13px;
  font-weight: 650;
}

.contract-position-tabs button.active::after {
  background: var(--accent);
  border-radius: 1px;
  bottom: 5px;
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
  font-size: 13px;
  font-weight: 650;
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
  box-shadow: 0 0 0 3px var(--focus-ring);
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

@media (max-width: 820px) {
  .contract-order-confirm-layer {
    right: 0;
    width: 100%;
  }
}

@media (max-width: 340px) {
  .contract-order-confirm {
    padding-left: max(12px, env(safe-area-inset-left, 0px));
    padding-right: max(12px, env(safe-area-inset-right, 0px));
    row-gap: 10px;
  }

  .contract-order-confirm__body {
    gap: 10px;
  }

  .contract-order-confirm__top {
    row-gap: 10px;
  }

  .contract-order-confirm__identity {
    gap: 8px;
    grid-template-columns: 36px minmax(0, 1fr) auto;
    padding-inline: 9px;
  }

  .contract-order-confirm__direction {
    font-size: 10px;
    padding-inline: 8px;
  }

  .contract-order-confirm__details {
    padding-inline: 9px;
  }

  .contract-order-confirm__details > div {
    gap: 8px;
  }

  .contract-order-confirm__details dd {
    font-size: 11px;
    max-width: 154px;
  }

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

  .spot-account-tabs :is(button, .spot-account-current) {
    font-size: 10px;
  }

  .spot-holdings-context,
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

@media (prefers-reduced-motion: no-preference) {
  .contract-order-confirm {
    animation: contract-order-confirm-enter 240ms cubic-bezier(.2, .8, .2, 1) both;
  }
}

@keyframes contract-order-confirm-enter {
  from {
    opacity: .8;
    transform: translateY(24px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@media (prefers-reduced-motion: reduce) {
  .trade-view *,
  .trade-view *::before,
  .trade-view *::after,
  .spot-order-type-layer,
  .spot-order-type-layer *,
  .spot-order-type-layer *::before,
  .spot-order-type-layer *::after,
  .contract-order-confirm-layer,
  .contract-order-confirm-layer *,
  .contract-order-confirm-layer *::before,
  .contract-order-confirm-layer *::after {
    animation-duration: .01ms !important;
    animation-iteration-count: 1 !important;
    scroll-behavior: auto !important;
    transition-duration: .01ms !important;
  }

  .trade-view button:active,
  .spot-order-type-layer button:active,
  .contract-order-confirm-layer button:active {
    transform: none;
  }

  .trade-view .contract-header-control:active:not(:disabled),
  .trade-view .contract-pencil-module button:active:not(:disabled) {
    transform: none;
  }
}
</style>
