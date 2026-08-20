<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowLeft,
  ArrowLeftRight,
  CandlestickChart,
  CheckCircle2,
  ChevronDown,
  CirclePlus,
  Download,
  Ellipsis,
  History,
  Info,
  Inbox,
  List,
  RefreshCcw,
  Share2,
  SlidersHorizontal,
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
  cancelMarginPosition,
  closeAllMarginPositions,
  closeMarginPosition,
  createMarginOrderIdempotencyKey,
  fetchMarginPositionRisk,
  fetchMarginSetting,
  fetchMarginProducts,
  fetchMarginWallets,
  placeMarginOrder,
  placeSpotOrder,
  type MarginPosition,
  type MarginPositionRisk,
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
import {
  isFilledMarginPosition,
  isPendingMarginPosition,
  marginLimitPriceFromBbo,
  preferredMarginOrderType,
} from '@/core/marginOrder'
import {
  resolveMarginPositionRiskMetrics,
  type MarginPositionRiskMetrics,
} from '@/core/marginRiskMetrics'
import { useModalDialog } from '@/core/modalDialog'
import { goBackOr } from '@/core/navigation'
import {
  clampMarginShortcutAmount,
  marginShortcutAvailable,
  quantityForBalancePercentage,
  type MarginAmountValidation,
  type MarginLimitPriceValidation,
  validateMarginAmount,
} from '@/core/tradeForm'
import { currentIntlLocale } from '@/i18n'
import { useMarketStore } from '@/stores/market'
import { useMarketFavoritesStore } from '@/stores/marketFavorites'
import { useSessionStore } from '@/stores/session'
import { useNavigationStore } from '@/stores/navigation'
import type { KlinePoint, MarginOrderType, MarginProduct, OrderBookLevel, TradePrint, WalletAccount } from '@/core/types'

type PositionActionType = 'close' | 'market-close-all' | 'cancel'

interface PositionActionState {
  id: string
  type: PositionActionType
}

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
const contractOrderType = ref<MarginOrderType | null>(null)
const contractLimitPrice = ref('')
const quantity = ref('')
const percentage = ref<number | null>(0)
const leverage = ref(5)
const marginMode = ref<'cross' | 'isolated'>('isolated')
const products = ref<MarginProduct[]>([])
const spotWallets = ref<WalletAccount[]>([])
const marginWallets = ref<WalletAccount[]>([])
const marginPositions = ref<MarginPosition[]>([])
const marginRiskSnapshots = ref<Record<string, MarginPositionRisk>>({})
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
const contractSheet = ref<'pair' | 'leverage' | 'marginMode' | 'orderType' | null>(null)
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
const contractOpenMode = ref<'open' | 'close'>('open')
const contractWorkspaceTab = ref<'orders' | 'positions' | 'strategy'>('positions')
const currentPairOnly = ref(true)
const contractMoreOpen = ref(false)
const contractMoreButton = ref<HTMLButtonElement | null>(null)
const contractMoreMenu = ref<HTMLElement | null>(null)
const contractWorkspace = ref<HTMLElement | null>(null)
const positionActionSaving = ref<PositionActionState | null>(null)
const armedPositionAction = ref<PositionActionState | null>(null)
const bulkCloseArmed = ref(false)
const bulkCloseSaving = ref(false)
let marketRequestVersion = 0
let marginProductsRequestVersion = 0
let marginSettingRequestVersion = 0
let marginRiskRequestVersion = 0
let marginRiskRefreshTimer: number | undefined
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
const filledMarginPositions = computed(() => marginPositions.value.filter((position) => (
  !['closed', 'liquidated', 'cancelled', 'canceled'].includes(position.status.toLowerCase())
  && isFilledMarginPosition(position)
)))
const pendingMarginOrders = computed(() => marginPositions.value.filter((position) => (
  !['closed', 'liquidated', 'cancelled', 'canceled'].includes(position.status.toLowerCase())
  && isPendingMarginPosition(position)
)))
const visibleMarginPositions = computed(() => {
  const product = selectedProduct.value
  if (!currentPairOnly.value) return filledMarginPositions.value
  if (!product) return []
  return filledMarginPositions.value.filter((position) => position.productId === product.id)
})
const visiblePendingMarginOrders = computed(() => {
  const product = selectedProduct.value
  if (!currentPairOnly.value) return pendingMarginOrders.value
  if (!product) return []
  return pendingMarginOrders.value.filter((position) => position.productId === product.id)
})
const currentPrice = computed(() => ticker.value?.lastPrice ?? 0)
const isLive = computed(() => currentPrice.value > 0 && (!marketStore.error || liveDetailActive.value))
const selectedOrderType = computed(() => mode.value === 'contract' ? contractOrderType.value : orderType.value)
const contractOrderTypeLabel = computed(() => {
  if (contractOrderType.value === 'limit') return t('trade.limitOrderShort')
  if (contractOrderType.value === 'market') return t('trade.marketOrderShort')
  return t('trade.orderTypeUnavailableShort')
})
const effectivePrice = computed(() => {
  if (mode.value === 'contract') {
    return contractOrderType.value === 'limit' ? Number(contractLimitPrice.value) : currentPrice.value
  }
  return orderType.value === 'limit' ? Number(price.value) : currentPrice.value
})
const contractPriceValue = computed({
  get: () => contractOrderType.value === 'limit'
    ? contractLimitPrice.value
    : currentPrice.value > 0 ? formatPrice(currentPrice.value) : '',
  set: (value: string) => {
    if (contractOrderType.value === 'limit') contractLimitPrice.value = value
  },
})
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
    orderType: contractOrderType.value,
    limitPrice: contractLimitPrice.value,
    pricePrecision: selectedProduct.value?.pricePrecision,
    idempotencyKey,
    minMargin: selectedProduct.value?.minMargin,
    maxMargin: selectedProduct.value?.maxMargin,
    referencePrice: currentPrice.value,
  })
}

const marginOrderDraft = computed(() => createCurrentMarginOrderReview())
const contractLimitPriceValidation = computed(() => marginOrderDraft.value.limitPriceValidation)
const contractSuggestedLimitPrice = computed(() => marginLimitPriceFromBbo({
  side: side.value,
  bids: bids.value,
  asks: asks.value,
  latestPrice: currentPrice.value,
}))
const contractOrderReview = computed(() => reviewedMarginOrder.value || marginOrderDraft.value)
const contractNotionalValue = computed(() => contractOrderReview.value.estimatedNotional)
const contractOrderQuantity = computed(() => contractOrderReview.value.estimatedQuantity)
const contractShortcutAvailable = computed(() => marginShortcutAvailable(
  availableBalance.value,
  selectedProduct.value?.maxMargin,
))
const contractBookPrecision = computed(() => {
  const precision = selectedProduct.value?.pricePrecision
  if (precision === null || precision === undefined) return '--'
  if (precision <= 6) return (10 ** -precision).toFixed(precision)
  return `1e-${precision}`
})
const contractInterestSummary = computed(() => {
  const rate = selectedProduct.value?.hourlyInterestRate
  if (rate === undefined || !Number.isFinite(rate)) return '-- / --'
  return `${(rate * 100).toFixed(4)}% / 1h`
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

function marginLimitPriceValidationMessage(validation: MarginLimitPriceValidation): string {
  if (validation.error === 'required') return t('trade.marginLimitPriceRequired')
  if (validation.error === 'precision' && validation.pricePrecision !== null) {
    return t('trade.marginLimitPricePrecision', { precision: validation.pricePrecision })
  }
  if (validation.error === 'precision-unavailable') return t('trade.marginPricePrecisionUnavailable')
  if (validation.error === 'invalid') return t('trade.marginLimitPriceInvalid')
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
  if (mode.value !== 'contract') {
    marginSettingRequestVersion += 1
    products.value = []
    contractOrderType.value = null
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
    ) return
    products.value = nextProducts
    const nextSelected = nextProducts.find((product) => (
      normalizeSymbol(product.symbol) === normalizeSymbol(pairSymbol.value)
    ))
    contractOrderType.value = preferredMarginOrderType(
      contractOrderType.value,
      nextSelected?.orderTypes || [],
    )
  } catch {
    if (requestVersion !== marginProductsRequestVersion) return
    if (!options.preserveExistingOnError) {
      products.value = []
      contractOrderType.value = null
    }
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
  contractOrderType.value = preferredMarginOrderType(contractOrderType.value, product.orderTypes)
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
    marginRiskSnapshots.value = {}
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
      await loadMarginPositionRisks(margin.positions)
    } else {
      spotWallets.value = await fetchWalletAccounts()
      marginPositions.value = []
      marginRiskSnapshots.value = {}
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

async function loadMarginPositionRisks(positions = filledMarginPositions.value): Promise<void> {
  const requestVersion = ++marginRiskRequestVersion
  if (!session.isAuthenticated || mode.value !== 'contract' || !positions.length) {
    if (!positions.length) marginRiskSnapshots.value = {}
    return
  }
  const eligible = positions.filter((position) => {
    const product = products.value.find((item) => item.id === position.productId)
    return product?.positionRiskSupported !== false && isFilledMarginPosition(position)
  })
  if (!eligible.length) return
  const results = await Promise.allSettled(eligible.map((position) => fetchMarginPositionRisk(position.id)))
  if (requestVersion !== marginRiskRequestVersion || mode.value !== 'contract') return
  const activeIds = new Set(positions.map((position) => position.id))
  const next = Object.fromEntries(
    Object.entries(marginRiskSnapshots.value).filter(([positionId]) => activeIds.has(positionId)),
  )
  results.forEach((result) => {
    if (result.status === 'fulfilled') next[result.value.positionId] = result.value
  })
  marginRiskSnapshots.value = next
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

function openContractSheet(sheet: 'pair' | 'leverage' | 'marginMode' | 'orderType'): void {
  if (sheet === 'pair') {
    settingsError.value = ''
    contractSheet.value = sheet
    return
  }
  if (!selectedProduct.value) {
    setFeedback(t('trade.unavailableContract'))
    return
  }
  if (sheet === 'orderType' && !selectedProduct.value?.orderTypes.length) {
    setFeedback(t('trade.marginOrderTypeUnavailable'))
    return
  }
  if (sheet === 'orderType') {
    settingsError.value = ''
    contractSheet.value = sheet
    return
  }
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  settingsError.value = ''
  contractSheet.value = sheet
}

function selectContractOrderType(nextOrderType: MarginOrderType): void {
  const product = selectedProduct.value
  if (!product?.orderTypes.includes(nextOrderType) || settingsSaving.value) return
  contractOrderType.value = nextOrderType
  if (nextOrderType === 'limit' && !contractLimitPrice.value.trim()) fillContractLimitPrice()
  contractSheet.value = null
}

function fillContractLimitPrice(): void {
  if (contractOrderType.value !== 'limit') return
  const nextPrice = contractSuggestedLimitPrice.value
  if (nextPrice !== null) contractLimitPrice.value = String(nextPrice)
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
  contractMoreOpen.value = false
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
  contractMoreOpen.value = false
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

function openOrders(tab: 'spot' | 'margin' | 'positions' | 'history' = 'spot'): void {
  contractMoreOpen.value = false
  void router.push({ name: 'orders', query: { tab } })
}

function openContractChart(): void {
  contractMoreOpen.value = false
  void router.push({
    name: 'market-detail',
    params: { symbol: pairSymbol.value.replace('/', '_') },
  })
}

function toggleContractMore(): void {
  if (contractMoreOpen.value) {
    closeContractMore()
    return
  }
  openContractMore('first')
}

function openContractMore(target: 'first' | 'last'): void {
  contractMoreOpen.value = true
  void nextTick(() => {
    const items = contractMoreItems()
    const item = target === 'last' ? items.at(-1) : items[0]
    item?.focus()
  })
}

function closeContractMore(restoreFocus = true): void {
  if (!contractMoreOpen.value) return
  contractMoreOpen.value = false
  if (restoreFocus) void nextTick(() => contractMoreButton.value?.focus())
}

function contractMoreItems(): HTMLButtonElement[] {
  if (!contractMoreMenu.value) return []
  return Array.from(
    contractMoreMenu.value.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)'),
  )
}

function handleContractMoreButtonKeydown(event: KeyboardEvent): void {
  if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
  event.preventDefault()
  openContractMore(event.key === 'ArrowUp' ? 'last' : 'first')
}

function handleContractMoreKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    closeContractMore()
    return
  }

  const items = contractMoreItems()
  if (!items.length) return
  const currentIndex = Math.max(0, items.indexOf(document.activeElement as HTMLButtonElement))
  let targetIndex: number | null = null
  if (event.key === 'ArrowDown') targetIndex = (currentIndex + 1) % items.length
  if (event.key === 'ArrowUp') targetIndex = (currentIndex - 1 + items.length) % items.length
  if (event.key === 'Home') targetIndex = 0
  if (event.key === 'End') targetIndex = items.length - 1
  if (targetIndex === null) return
  event.preventDefault()
  items[targetIndex]?.focus()
}

function toggleCurrentPairScope(): void {
  if (positionActionSaving.value || bulkCloseSaving.value) return
  currentPairOnly.value = !currentPairOnly.value
  armedPositionAction.value = null
  bulkCloseArmed.value = false
}

function selectContractWorkspaceTab(tab: 'orders' | 'positions' | 'strategy'): void {
  if (tab === 'strategy' && !selectedProduct.value?.strategyOrdersSupported) return
  contractWorkspaceTab.value = tab
  contractOpenMode.value = tab === 'positions' ? 'close' : 'open'
  armedPositionAction.value = null
  bulkCloseArmed.value = false
}

function selectContractOpenMode(nextMode: 'open' | 'close'): void {
  contractOpenMode.value = nextMode
  if (nextMode === 'close') {
    contractWorkspaceTab.value = 'positions'
    void nextTick(() => contractWorkspace.value?.scrollIntoView({
      behavior: window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth',
      block: 'start',
    }))
  }
}

function productForPosition(position: MarginPosition): MarginProduct | undefined {
  return products.value.find((product) => product.id === position.productId)
}

function symbolForPosition(position: MarginPosition): string {
  return productForPosition(position)?.symbol || pairSymbol.value
}

function logoForPosition(position: MarginPosition): string | undefined {
  const product = productForPosition(position)
  return product?.logoUrl || marketStore.tickerFor(product?.symbol || pairSymbol.value)?.iconUrl
}

function riskForPosition(position: MarginPosition): MarginPositionRisk | undefined {
  return marginRiskSnapshots.value[position.id]
}

function resolvePositionRiskDisplayMetrics(position: MarginPosition): MarginPositionRiskMetrics {
  const risk = riskForPosition(position)
  return resolveMarginPositionRiskMetrics({
    marginMode: position.marginMode,
    direction: position.direction,
    entryPrice: position.entryPrice,
    notionalAmount: position.notionalAmount,
    marginAmount: position.marginAmount,
    interestAmount: position.interestAmount,
    serverMaintenanceMarginRate: risk?.maintenanceMarginRate,
    productMaintenanceMarginRate: productForPosition(position)?.maintenanceMarginRate,
    serverEstimatedLiquidationPrice: risk?.estimatedLiquidationPrice,
  })
}

const marginRiskDisplayMetricsByPositionId = computed<Record<string, MarginPositionRiskMetrics>>(() => {
  const result: Record<string, MarginPositionRiskMetrics> = {}
  marginPositions.value.forEach((position) => {
    result[position.id] = resolvePositionRiskDisplayMetrics(position)
  })
  return result
})

function positionRiskDisplayMetrics(position: MarginPosition): MarginPositionRiskMetrics {
  return marginRiskDisplayMetricsByPositionId.value[position.id]
}

function estimatedLiquidationPriceDisplay(position: MarginPosition): string {
  const metrics = positionRiskDisplayMetrics(position)
  if (metrics.liquidationRiskScope === 'account') return t('trade.crossAccountRisk')
  return metrics.estimatedLiquidationPrice === null
    ? '--'
    : formatPrice(metrics.estimatedLiquidationPrice)
}

function formatRate(value: number | null | undefined, digits = 2): string {
  return value === null || value === undefined || !Number.isFinite(value)
    ? '--'
    : `${(value * 100).toFixed(digits)}%`
}

function liquidationDistanceWidth(position: MarginPosition): string {
  const distance = riskForPosition(position)?.liquidationDistanceRate
  if (distance === null || distance === undefined || !Number.isFinite(distance)) return '0%'
  return `${Math.min(100, Math.max(4, distance * 100))}%`
}

async function performPositionAction(position: MarginPosition, action: PositionActionType): Promise<void> {
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  if (positionActionSaving.value || bulkCloseSaving.value) return
  if (armedPositionAction.value?.id !== position.id || armedPositionAction.value.type !== action) {
    bulkCloseArmed.value = false
    armedPositionAction.value = { id: position.id, type: action }
    return
  }

  const closesPosition = action === 'close' || action === 'market-close-all'
  positionActionSaving.value = { id: position.id, type: action }
  feedback.value = ''
  try {
    if (closesPosition) await closeMarginPosition(position.id)
    else await cancelMarginPosition(position.id)
    setFeedback(t(closesPosition ? 'trade.positionClosed' : 'trade.marginOrderCanceled'), 'success')
    armedPositionAction.value = null
    await loadTradingBalances()
  } catch (reason) {
    setFeedback(apiErrorMessage(reason, t(closesPosition ? 'trade.positionCloseFailed' : 'trade.marginOrderCancelFailed')))
  } finally {
    positionActionSaving.value = null
  }
}

async function performBulkClose(): Promise<void> {
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  if (
    !visibleMarginPositions.value.length
    || positionActionSaving.value
    || bulkCloseSaving.value
    || !selectedProduct.value?.bulkCloseSupported
  ) return
  if (!bulkCloseArmed.value) {
    armedPositionAction.value = null
    bulkCloseArmed.value = true
    return
  }
  bulkCloseSaving.value = true
  feedback.value = ''
  try {
    const result = await closeAllMarginPositions(currentPairOnly.value ? selectedProduct.value?.id : undefined)
    if (result.failures.length) {
      setFeedback(t('trade.positionsPartiallyClosed', {
        succeeded: result.positions.length,
        failed: result.failures.length,
      }))
    } else {
      setFeedback(t('trade.positionsClosed'), 'success')
    }
    bulkCloseArmed.value = false
    await loadTradingBalances()
  } catch (reason) {
    setFeedback(apiErrorMessage(reason, t('trade.positionsCloseFailed')))
  } finally {
    bulkCloseSaving.value = false
  }
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
  if (mode.value === 'contract' && contractOrderType.value === null) {
    setFeedback(t('trade.marginOrderTypeUnavailable'))
    return
  }
  if (
    mode.value === 'contract'
    && contractOrderType.value === 'limit'
    && !contractLimitPriceValidation.value.isValid
  ) {
    setFeedback(marginLimitPriceValidationMessage(contractLimitPriceValidation.value))
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
  const submittedOrderType = orderType.value
  const limitPrice = effectivePrice.value
  if (!session.isAuthenticated) {
    openLogin()
    return
  }

  if (submittedMode === 'spot') {
    if (!isLive.value) {
      setFeedback(t('trade.marketUnavailable'))
      return
    }
    if (!Number.isFinite(orderAmount) || orderAmount <= 0 || !Number.isFinite(limitPrice) || limitPrice <= 0) {
      setFeedback(t('trade.invalidOrder'))
      return
    }
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
  marginRiskRefreshTimer = window.setInterval(() => {
    if (viewActive && mode.value === 'contract' && session.isAuthenticated) {
      void loadMarginPositionRisks()
    }
  }, 5_000)
})

watch(pairSymbol, (symbol) => {
  navigation.rememberTradeSymbol(symbol)
  marketDataPanel.value = 'orderBook'
  spotChartOpen.value = false
  contractMoreOpen.value = false
  armedPositionAction.value = null
  bulkCloseArmed.value = false
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
  contractOpenMode.value = 'open'
  contractWorkspaceTab.value = 'positions'
  contractMoreOpen.value = false
}, { immediate: true })

watch(() => [mode.value, session.isAuthenticated, selectedProduct.value?.id] as const, () => {
  const product = selectedProduct.value
  if (mode.value !== 'contract' || !session.isAuthenticated || !product) {
    marginSettingRequestVersion += 1
    return
  }
  void syncMarginSetting(product)
})

watch(() => selectedProduct.value?.id ?? null, async (productId, previousProductId) => {
  if (productId === previousProductId) return
  contractLimitPrice.value = ''
  await nextTick()
  if (contractOrderType.value === 'limit') fillContractLimitPrice()
})

watch([mode, () => session.isAuthenticated], () => {
  void loadMarginProducts()
  void loadTradingBalances()
}, { immediate: true })

watch(currentPrice, (value) => {
  if (!price.value && value > 0) price.value = String(value)
  if (
    mode.value === 'contract'
    && contractOrderType.value === 'limit'
    && !contractLimitPrice.value.trim()
    && value > 0
  ) {
    fillContractLimitPrice()
  }
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
  marginRiskRequestVersion += 1
  if (marginRiskRefreshTimer !== undefined) window.clearInterval(marginRiskRefreshTimer)
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
        data-pencil-source="cjzfi p6GfgT"
        data-instrument-hero="pair-price"
        data-market-quote="live"
        data-order-surface="live"
        :data-contract-state="visibleMarginPositions.length ? 'positions' : 'empty'"
      >
        <header class="contract-pencil-header">
          <div class="contract-header-identity">
            <button class="contract-header-control" type="button" :aria-label="t('common.back')" @click="goBack">
              <ArrowLeft :size="22" aria-hidden="true" />
            </button>
            <button
              class="contract-pair-selector"
              type="button"
              :aria-label="t('markets.pickerTitle')"
              aria-haspopup="dialog"
              :aria-expanded="contractSheet === 'pair'"
              aria-controls="contract-pair-dialog"
              @click="openContractSheet('pair')"
            >
              <AssetMark :symbol="baseAsset" :src="selectedProduct?.logoUrl || ticker?.iconUrl" :fallback-src="ticker?.baseIconUrl" :size="28" />
              <span class="contract-pair-selector__copy">
                <span>
                  <strong>{{ pairSymbol.replace('/', '') }}</strong>
                  <small class="contract-product-badge">{{ t('trade.perpetualShort') }}</small>
                  <ChevronDown :size="14" aria-hidden="true" />
                </span>
                <small class="contract-pair-market numeric" :class="(ticker?.changePercent || 0) >= 0 ? 'positive' : 'negative'">
                  {{ ticker ? formatPrice(currentPrice) : '--' }} ·
                  {{ ticker ? `${ticker.changePercent >= 0 ? '+' : ''}${ticker.changePercent.toFixed(2)}%` : '--' }}
                </small>
              </span>
            </button>
          </div>
          <div class="contract-header-actions">
            <button class="contract-header-control" type="button" :aria-label="t('marketDetail.chart')" @click="openContractChart">
              <CandlestickChart :size="19" aria-hidden="true" />
            </button>
            <button
              ref="contractMoreButton"
              class="contract-header-control"
              type="button"
              :aria-label="t('marketDetail.actions')"
              :aria-expanded="contractMoreOpen"
              aria-haspopup="menu"
              @click="toggleContractMore"
              @keydown="handleContractMoreButtonKeydown"
            >
              <Ellipsis :size="20" aria-hidden="true" />
            </button>
          </div>
          <button
            v-if="contractMoreOpen"
            class="contract-more-dismiss"
            type="button"
            :aria-label="t('common.close')"
            tabindex="-1"
            @click="closeContractMore()"
          />
          <div
            v-if="contractMoreOpen"
            ref="contractMoreMenu"
            class="contract-more-menu"
            role="menu"
            @keydown="handleContractMoreKeydown"
          >
            <button
              type="button"
              role="menuitem"
              :aria-busy="favoriteSaving"
              :disabled="favoriteSaving"
              @click="toggleFavorite"
            >
              <Star :size="16" :fill="isFavorite ? 'currentColor' : 'none'" aria-hidden="true" />
              {{ t(isFavorite ? 'rootPrototype.removeFavorite' : 'rootPrototype.addFavorite', { symbol: pairSymbol }) }}
            </button>
            <button type="button" role="menuitem" @click="openOrders('margin')">
              <List :size="16" aria-hidden="true" />{{ t('trade.viewOpenOrders') }}
            </button>
            <button type="button" role="menuitem" @click="openAssets">
              <ArrowLeftRight :size="16" aria-hidden="true" />{{ t('nav.assets') }}
            </button>
          </div>
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
              <span>{{ t('trade.hourlyInterestAndCycle') }}</span>
              <strong class="numeric">{{ contractInterestSummary }}</strong>
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
                :mini-ask-levels="6"
                :mini-bid-levels="7"
                :mini-precision="contractBookPrecision"
                :show-mini-precision="true"
              />
            </div>
            <p class="chart-semantic-summary">
              {{ pairSymbol }} · {{ ticker ? formatPrice(currentPrice) : t('common.marketUnavailable') }}
            </p>
          </div>

          <div class="trade-console">
            <div class="contract-open-close" role="group" :aria-label="t('orders.stateCategory')">
              <button
                type="button"
                :class="{ active: contractOpenMode === 'open' }"
                :aria-pressed="contractOpenMode === 'open'"
                @click="selectContractOpenMode('open')"
              >
                {{ t('trade.openPositionShort') }}
              </button>
              <button
                type="button"
                :class="{ active: contractOpenMode === 'close' }"
                :aria-pressed="contractOpenMode === 'close'"
                @click="selectContractOpenMode('close')"
              >
                {{ t('trade.closePositionShort') }}
              </button>
            </div>

            <div class="contract-mode-row contract-settings-row" :aria-label="t('trade.settings')">
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
              <button
                class="contract-order-type"
                type="button"
                :class="{ active: contractOrderType !== null }"
                aria-haspopup="dialog"
                :aria-expanded="contractSheet === 'orderType'"
                aria-controls="contract-orderType-dialog"
                :aria-label="contractOrderType === null ? t('trade.marginOrderTypeUnavailable') : t('trade.orderTypeTrigger', { type: contractOrderTypeLabel })"
                :disabled="settingsSaving || productsLoading || !selectedProduct?.orderTypes.length"
                @click="openContractSheet('orderType')"
              >
                <span>{{ contractOrderTypeLabel }}</span>
                <ChevronDown :size="12" aria-hidden="true" />
              </button>
            </div>

            <div class="contract-price-row">
              <label
                class="contract-field contract-price-field"
                :class="{ 'is-invalid': contractOrderType === 'limit' && !contractLimitPriceValidation.isValid }"
              >
                <span>{{ t('trade.priceField', { asset: quoteAsset }) }}</span>
                <input
                  v-model="contractPriceValue"
                  class="numeric"
                  :placeholder="contractOrderType === 'limit' ? t('trade.pricePlaceholder') : t('trade.marketPrice')"
                  autocomplete="off"
                  enterkeyhint="done"
                  inputmode="decimal"
                  :spellcheck="false"
                  :readonly="contractOrderType !== 'limit'"
                  :aria-invalid="contractOrderType === 'limit' && !contractLimitPriceValidation.isValid ? 'true' : 'false'"
                  :aria-errormessage="contractOrderType === 'limit' && !contractLimitPriceValidation.isValid ? 'contract-limit-price-error' : undefined"
                  @input="clearPercentageSelection"
                />
              </label>
              <button
                type="button"
                :aria-label="t('marketDetail.latestPrice')"
                :disabled="contractOrderType !== 'limit' || contractSuggestedLimitPrice === null"
                @click="fillContractLimitPrice"
              >
                {{ t('trade.bestBidOffer') }}
              </button>
            </div>
            <p
              v-if="contractOrderType === 'limit' && !contractLimitPriceValidation.isValid"
              id="contract-limit-price-error"
              class="contract-limit-price-error sr-only"
              role="alert"
            >
              {{ marginLimitPriceValidationMessage(contractLimitPriceValidation) }}
            </p>

            <label
              class="contract-field contract-amount-field"
              :class="{ 'is-invalid': marginAmountError }"
              :data-margin-validation="marginAmountError ? 'invalid' : 'ready'"
            >
              <span>{{ t('trade.marginField', { asset: availableAsset }) }}</span>
              <input
                v-model="quantity"
                class="numeric"
                autocomplete="off"
                enterkeyhint="done"
                inputmode="decimal"
                placeholder="0"
                :spellcheck="false"
                :aria-describedby="marginRangeDescription ? 'contract-margin-range' : undefined"
                :aria-errormessage="marginAmountError ? 'contract-margin-error' : undefined"
                :aria-invalid="marginAmountError ? 'true' : 'false'"
                @input="clearPercentageSelection"
              />
              <b>{{ availableAsset }}</b>
            </label>

            <div v-if="marginRangeDescription" class="contract-margin-guidance sr-only">
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
              <div
                class="percent-row"
                role="group"
                :aria-label="t('rootPrototype.balancePercentage')"
                :style="{ '--percentage-progress': `${percentage ?? 0}%` }"
              >
                <button
                  v-for="value in [0, 25, 50, 75, 100]"
                  :key="value"
                  type="button"
                  :class="{ active: percentage === value, passed: (percentage ?? 0) >= value }"
                  :aria-label="value === 100 ? t('trade.marginMaximumShortcut') : `${value}%`"
                  :aria-pressed="percentage === value"
                  :disabled="submitting || (session.isAuthenticated && (productsLoading || balancesLoading || !selectedProduct))"
                  @click="setQuantity(value)"
                >
                  <span class="sr-only">{{ value }}%</span>
                </button>
              </div>
            </div>

            <div class="contract-available-row">
              <span>{{ t('trade.available') }}</span>
              <button v-if="!session.isAuthenticated" type="button" @click="openLogin">
                {{ t('trade.viewAfterLogin') }}
              </button>
              <button v-else-if="balancesError" type="button" :disabled="balancesLoading" @click="loadTradingBalances">
                {{ t('common.retry') }}
              </button>
              <button v-else class="numeric" type="button" :disabled="balancesLoading" @click="openAssets">
                {{ balancesLoading ? t('trade.loadBalance') : `${formatAmount(availableBalance)} ${availableAsset}` }}
                <CirclePlus v-if="!balancesLoading" :size="11" aria-hidden="true" />
              </button>
            </div>

            <div class="contract-tpsl" :aria-disabled="!selectedProduct?.takeProfitStopLossSupported">
              <span aria-hidden="true" />
              {{ t('rootPrototype.takeProfitStopLoss') }}
              <small v-if="!selectedProduct?.takeProfitStopLossSupported">{{ t('trade.featureUnavailableShort') }}</small>
            </div>

            <dl class="contract-open-meta contract-open-meta--long">
              <div><dt>{{ t('trade.openLongAvailable') }}</dt><dd class="numeric">{{ formatAmount(contractOrderQuantity) }} {{ baseAsset }}</dd></div>
              <div><dt>{{ t('orders.margin') }}</dt><dd class="numeric">{{ formatAmount(Number(quantity) || 0) }} {{ availableAsset }}</dd></div>
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
              :disabled="contractOpenMode === 'close' || submitting || productsLoading || !isLive || (session.isAuthenticated && (!selectedProduct || !contractOrderType || (contractOrderType === 'limit' && !contractLimitPriceValidation.isValid)))"
              @click="reviewContractOrder('buy', $event)"
            >
              {{ t('trade.longActionCompact', { leverage }) }}
            </button>
            <dl class="contract-open-meta contract-open-meta--short">
              <div><dt>{{ t('trade.openShortAvailable') }}</dt><dd class="numeric">{{ formatAmount(contractOrderQuantity) }} {{ baseAsset }}</dd></div>
              <div><dt>{{ t('orders.margin') }}</dt><dd class="numeric">{{ formatAmount(Number(quantity) || 0) }} {{ availableAsset }}</dd></div>
            </dl>
            <button
              class="contract-submit contract-submit--short"
              type="button"
              :disabled="contractOpenMode === 'close' || submitting || productsLoading || !isLive || (session.isAuthenticated && (!selectedProduct || !contractOrderType || (contractOrderType === 'limit' && !contractLimitPriceValidation.isValid)))"
              @click="reviewContractOrder('sell', $event)"
            >
              {{ t('trade.shortActionCompact', { leverage }) }}
            </button>
          </div>
        </section>

        <nav ref="contractWorkspace" class="contract-position-tabs" role="tablist" :aria-label="t('orders.category')">
          <button
            id="contract-orders-tab"
            type="button"
            role="tab"
            :class="{ active: contractWorkspaceTab === 'orders' }"
            :aria-selected="contractWorkspaceTab === 'orders'"
            aria-controls="contract-workspace-panel"
            @click="selectContractWorkspaceTab('orders')"
          >
            {{ t('trade.contractOrdersTab') }} ({{ visiblePendingMarginOrders.length }})
          </button>
          <button
            id="contract-positions-tab"
            type="button"
            role="tab"
            :class="{ active: contractWorkspaceTab === 'positions' }"
            :aria-selected="contractWorkspaceTab === 'positions'"
            aria-controls="contract-workspace-panel"
            @click="selectContractWorkspaceTab('positions')"
          >
            {{ t('trade.positionAssetsTab', { count: visibleMarginPositions.length }) }}
          </button>
          <button
            id="contract-strategy-tab"
            type="button"
            role="tab"
            :class="{ active: contractWorkspaceTab === 'strategy' }"
            :aria-selected="contractWorkspaceTab === 'strategy'"
            :aria-disabled="!selectedProduct?.strategyOrdersSupported"
            :disabled="!selectedProduct?.strategyOrdersSupported"
            aria-controls="contract-workspace-panel"
            @click="selectContractWorkspaceTab('strategy')"
          >
            {{ t('trade.strategyOrdersTab') }} (0)
          </button>
          <span aria-hidden="true" />
          <button type="button" :aria-label="t('orders.history')" @click="openOrders('history')">
            <History :size="17" aria-hidden="true" />
          </button>
        </nav>

        <section
          id="contract-workspace-panel"
          class="contract-workspace-panel"
          role="tabpanel"
          :aria-labelledby="`contract-${contractWorkspaceTab}-tab`"
        >
          <div v-if="contractWorkspaceTab === 'positions'" class="contract-workspace-tools">
            <button
              class="contract-current-pair"
              type="button"
              :aria-pressed="currentPairOnly"
              :disabled="bulkCloseSaving || positionActionSaving !== null"
              @click="toggleCurrentPairScope"
            >
              <span aria-hidden="true" />{{ t('trade.onlyCurrent') }}
            </button>
            <button
              class="contract-close-all"
              type="button"
              :class="{ armed: bulkCloseArmed }"
              :aria-busy="bulkCloseSaving"
              :disabled="bulkCloseSaving || positionActionSaving !== null || !visibleMarginPositions.length || !selectedProduct?.bulkCloseSupported"
              @click="performBulkClose"
            >
              {{ bulkCloseSaving ? t('orders.processing') : bulkCloseArmed ? t('trade.confirmCloseAll') : t('orders.closeAll') }}
            </button>
            <button
              class="contract-filter-control"
              type="button"
              :aria-label="t('trade.positionFilter')"
              :aria-pressed="currentPairOnly"
              :disabled="bulkCloseSaving || positionActionSaving !== null"
              @click="toggleCurrentPairScope"
            >
              <SlidersHorizontal :size="15" aria-hidden="true" />
            </button>
          </div>

          <div v-if="contractWorkspaceTab === 'positions' && visibleMarginPositions.length" class="contract-position-list">
            <article v-for="position in visibleMarginPositions" :key="position.id" class="contract-position-card">
              <header>
                <div class="contract-position-identity">
                  <AssetMark
                    :symbol="symbolForPosition(position).split('/')[0] || baseAsset"
                    :src="logoForPosition(position)"
                    :size="24"
                  />
                  <div>
                    <strong>{{ symbolForPosition(position).replace('/', '') }}</strong>
                    <span :class="position.direction === 'long' ? 'positive' : 'negative'">
                      {{ t(position.direction === 'long' ? 'orders.long' : 'orders.short') }}
                    </span>
                    <span>{{ t(position.marginMode === 'cross' ? 'trade.cross' : 'trade.isolated') }}</span>
                    <span class="numeric">{{ position.leverage }}x</span>
                  </div>
                </div>
                <div class="contract-position-pnl" :class="(riskForPosition(position)?.unrealizedPnl || 0) >= 0 ? 'positive' : 'negative'">
                  <small>{{ t('trade.unrealizedPnl') }}</small>
                  <strong class="numeric">{{ riskForPosition(position) ? formatAmount(riskForPosition(position)!.unrealizedPnl) : '--' }}</strong>
                  <span class="numeric">{{ formatRate(riskForPosition(position)?.returnRate) }}</span>
                </div>
              </header>

              <dl class="contract-position-metrics">
                <div><dt>{{ t('common.quantity') }}</dt><dd class="numeric">{{ riskForPosition(position) ? formatAmount(riskForPosition(position)!.positionQuantity) : '--' }}</dd></div>
                <div><dt>{{ t('orders.margin') }}</dt><dd class="numeric">{{ formatAmount(position.marginAmount) }}</dd></div>
                <div><dt>{{ t('trade.maintenanceMarginRate') }}</dt><dd class="numeric">{{ formatRate(positionRiskDisplayMetrics(position).maintenanceMarginRate) }}</dd></div>
                <div><dt>{{ t('orders.entryPrice') }}</dt><dd class="numeric">{{ position.entryPrice ? formatPrice(position.entryPrice) : '--' }}</dd></div>
                <div><dt>{{ t('trade.markPrice') }}</dt><dd class="numeric">{{ riskForPosition(position) ? formatPrice(riskForPosition(position)!.markPrice) : '--' }}</dd></div>
                <div><dt>{{ t('trade.estimatedLiquidationPrice') }}</dt><dd class="numeric">{{ estimatedLiquidationPriceDisplay(position) }}</dd></div>
              </dl>

              <div class="contract-liquidation-distance">
                <span>{{ t('trade.liquidationDistance') }}</span>
                <i><b :style="{ width: liquidationDistanceWidth(position) }" /></i>
                <strong class="numeric">{{ formatRate(riskForPosition(position)?.liquidationDistanceRate) }}</strong>
              </div>
              <div class="contract-position-actions" role="group" :aria-label="t('trade.positionActions')">
                <button
                  data-position-action="take-profit-stop-loss"
                  type="button"
                  :disabled="!productForPosition(position)?.takeProfitStopLossSupported || positionActionSaving !== null || bulkCloseSaving"
                >
                  <span>{{ t('rootPrototype.takeProfitStopLoss') }}</span>
                  <small v-if="!productForPosition(position)?.takeProfitStopLossSupported">{{ t('trade.featureUnavailableShort') }}</small>
                </button>
                <button
                  class="contract-position-action"
                  data-position-action="close"
                  type="button"
                  :class="{ armed: armedPositionAction?.id === position.id && armedPositionAction.type === 'close' }"
                  :aria-busy="positionActionSaving?.id === position.id && positionActionSaving.type === 'close'"
                  :disabled="positionActionSaving !== null || bulkCloseSaving"
                  @click="performPositionAction(position, 'close')"
                >
                  <span>{{ positionActionSaving?.id === position.id && positionActionSaving.type === 'close' ? t('orders.processing') : armedPositionAction?.id === position.id && armedPositionAction.type === 'close' ? t('trade.confirmClosePosition') : t('trade.closePositionShort') }}</span>
                </button>
                <button
                  class="contract-position-market-close-all"
                  data-position-action="market-close-all"
                  type="button"
                  :class="{ armed: armedPositionAction?.id === position.id && armedPositionAction.type === 'market-close-all' }"
                  :aria-busy="positionActionSaving?.id === position.id && positionActionSaving.type === 'market-close-all'"
                  :disabled="positionActionSaving !== null || bulkCloseSaving"
                  @click="performPositionAction(position, 'market-close-all')"
                >
                  <span>{{ positionActionSaving?.id === position.id && positionActionSaving.type === 'market-close-all' ? t('orders.processing') : armedPositionAction?.id === position.id && armedPositionAction.type === 'market-close-all' ? t('trade.confirmMarketCloseAll') : t('trade.marketCloseAll') }}</span>
                </button>
              </div>
            </article>
          </div>

          <div v-else-if="contractWorkspaceTab === 'orders' && visiblePendingMarginOrders.length" class="contract-order-list">
            <article v-for="position in visiblePendingMarginOrders" :key="position.id" class="contract-pending-card">
              <div class="contract-pending-identity">
                <AssetMark :symbol="symbolForPosition(position).split('/')[0] || baseAsset" :src="logoForPosition(position)" :size="24" />
                <span>
                  <strong>{{ symbolForPosition(position).replace('/', '') }}</strong>
                  <small :class="position.direction === 'long' ? 'positive' : 'negative'">
                    {{ t(position.direction === 'long' ? 'orders.long' : 'orders.short') }} · {{ t('trade.limitOrderShort') }}
                  </small>
                </span>
              </div>
              <dl>
                <div><dt>{{ t('trade.marginLimitPrice') }}</dt><dd class="numeric">{{ position.limitPrice || '--' }}</dd></div>
                <div><dt>{{ t('orders.margin') }}</dt><dd class="numeric">{{ formatAmount(position.marginAmount) }} {{ productForPosition(position)?.marginAssetSymbol || availableAsset }}</dd></div>
              </dl>
              <button
                type="button"
                :class="{ armed: armedPositionAction?.id === position.id && armedPositionAction.type === 'cancel' }"
                :aria-busy="positionActionSaving?.id === position.id && positionActionSaving.type === 'cancel'"
                :disabled="positionActionSaving !== null || bulkCloseSaving"
                @click="performPositionAction(position, 'cancel')"
              >
                {{ positionActionSaving?.id === position.id && positionActionSaving.type === 'cancel' ? t('orders.processing') : armedPositionAction?.id === position.id && armedPositionAction.type === 'cancel' ? t('trade.confirmCancelOrder') : t('orders.cancel') }}
              </button>
            </article>
          </div>

          <div v-else class="contract-position-empty" role="status">
            <span><Inbox :size="22" aria-hidden="true" /></span>
            <strong v-if="contractWorkspaceTab === 'strategy'">{{ t('trade.strategyOrdersUnavailable') }}</strong>
            <strong v-else-if="!session.isAuthenticated">{{ t('trade.ordersLoginHint') }}</strong>
            <strong v-else>{{ t(contractWorkspaceTab === 'orders' ? 'orders.noMarginOrders' : 'orders.noPositions') }}</strong>
            <button v-if="!session.isAuthenticated" type="button" @click="openLogin">{{ t('common.loginNow') }}</button>
          </div>
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
      :order-type="contractOrderType"
      :saving="settingsSaving"
      :error="settingsError"
      :products-loading="productsLoading"
      :products-error="productsError"
      @close="closeContractSheet"
      @select-pair="selectContractPair"
      @apply-leverage="applyContractLeverage"
      @apply-margin-mode="applyContractMarginMode"
      @select-order-type="selectContractOrderType"
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
                <p>{{ t('trade.contractOrderConfirmHint', { type: t(contractOrderReview.request.orderType === 'limit' ? 'trade.limitOrderShort' : 'trade.marketOrderShort') }) }}</p>
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
                <span>{{ t('trade.perpetualShort') }} · {{ t(contractOrderReview.request.orderType === 'limit' ? 'trade.limitOrderShort' : 'trade.marketOrderShort') }}</span>
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
                <dt>{{ t('trade.marginOrderType') }}</dt>
                <dd>{{ t(contractOrderReview.request.orderType === 'limit' ? 'trade.limitOrderShort' : 'trade.marketOrderShort') }}</dd>
              </div>
              <div v-if="contractOrderReview.request.orderType === 'limit'">
                <dt>{{ t('trade.marginLimitPrice') }}</dt>
                <dd class="numeric">{{ formatPrice(Number(contractOrderReview.request.price || 0)) }} {{ quoteAsset }}</dd>
              </div>
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
                <strong>{{ t(contractOrderReview.request.orderType === 'limit' ? 'trade.limitExecutionRiskTitle' : 'trade.marketExecutionRiskTitle') }}</strong>
                <p>{{ t(contractOrderReview.request.orderType === 'limit' ? 'trade.limitExecutionRiskDescription' : 'trade.marketExecutionRiskDescription') }}</p>
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
  --contract-bg: #f7f9f8;
  --contract-surface: #ffffff;
  --contract-surface-soft: #edf2ef;
  --contract-line: #dce5e0;
  --contract-line-strong: #c4d0ca;
  --contract-control-line: #ccd5d0;
  --contract-text: #111714;
  --contract-muted: #68736d;
  --contract-accent: #43efa9;
  --contract-positive: #159a6d;
  --contract-negative: #e94f37;
  --contract-position-action-surface: #ffffff;
  --contract-position-action-border: #087b52;
  --contract-position-action-text: #087b52;
  background: var(--contract-bg);
  color: var(--contract-text);
  min-height: 100dvh;
  overscroll-behavior-y: none;
  padding-bottom: calc(24px + env(safe-area-inset-bottom));
}

html[data-theme='dark'] .contract-trade {
  --contract-bg: #070a09;
  --contract-surface: #0c100e;
  --contract-surface-soft: #111713;
  --contract-line: #29342e;
  --contract-line-strong: #3b4841;
  --contract-control-line: #29342e;
  --contract-text: #f2f7f4;
  --contract-muted: #95a19a;
  --contract-positive: #61f1b6;
  --contract-negative: #ff654a;
  --contract-position-action-surface: #121714;
  --contract-position-action-border: #202923;
  --contract-position-action-text: var(--contract-text);
}

.contract-pencil-surface {
  background: var(--contract-bg);
  color: var(--contract-text);
  min-height: 100dvh;
  min-width: 0;
}

.contract-pencil-header {
  align-items: center;
  background: color-mix(in srgb, var(--contract-bg) 96%, transparent);
  border-bottom: 1px solid color-mix(in srgb, var(--contract-line) 76%, transparent);
  display: grid;
  grid-template-columns: minmax(0, 1fr) 74px;
  height: calc(58px + env(safe-area-inset-top));
  padding: env(safe-area-inset-top) 14px 0;
  position: sticky;
  top: 0;
  z-index: 70;
}

.contract-header-identity,
.contract-header-actions,
.contract-pair-selector,
.contract-pair-selector__copy > span {
  align-items: center;
  display: flex;
  min-width: 0;
}

.contract-header-identity {
  height: 58px;
}

.contract-header-actions {
  display: grid;
  grid-template-columns: repeat(2, 37px);
  height: 58px;
}

.contract-header-control {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 12px;
  color: var(--contract-text);
  display: grid;
  height: 44px;
  justify-content: center;
  padding: 0;
  width: 44px;
}

.contract-header-actions .contract-header-control {
  margin-inline: -3px;
}

.contract-header-control:active:not(:disabled) {
  background: var(--contract-surface-soft);
  transform: scale(.94);
}

.contract-pair-selector {
  background: transparent;
  border: 0;
  color: var(--contract-text);
  gap: 8px;
  height: 44px;
  margin-left: -3px;
  max-width: 190px;
  padding: 0 3px;
}

.contract-pair-selector :deep(.asset-mark) {
  border: 1px solid color-mix(in srgb, var(--contract-accent) 32%, var(--contract-line));
  box-shadow: none;
  flex: 0 0 auto;
}

.contract-pair-selector__copy {
  display: grid;
  gap: 2px;
  justify-items: start;
  min-width: 0;
}

.contract-pair-selector__copy > span {
  gap: 4px;
  width: 100%;
}

.contract-pair-selector strong {
  font-family: var(--font-geist-sans), var(--font-family), sans-serif;
  font-size: 17px;
  font-weight: 760;
  letter-spacing: -.35px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-product-badge {
  background: color-mix(in srgb, var(--contract-accent) 18%, var(--contract-surface));
  border-radius: 999px;
  color: var(--contract-positive);
  flex: 0 0 auto;
  font-size: 8px;
  font-weight: 720;
  line-height: 16px;
  padding: 0 6px;
}

.contract-pair-selector__copy .contract-pair-market {
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 9px;
  font-weight: 620;
  max-width: 146px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-pair-selector[aria-expanded='true'] {
  color: var(--contract-positive);
}

.contract-more-dismiss {
  background: transparent;
  border: 0;
  inset: 0;
  padding: 0;
  position: fixed;
  z-index: 71;
}

.contract-more-menu {
  -webkit-backdrop-filter: blur(18px) saturate(135%);
  backdrop-filter: blur(18px) saturate(135%);
  background: color-mix(in srgb, var(--contract-surface) 94%, transparent);
  border: 1px solid var(--contract-line);
  border-radius: 14px;
  box-shadow: 0 18px 42px color-mix(in srgb, #000 22%, transparent);
  display: grid;
  min-width: 188px;
  overflow: hidden;
  padding: 6px;
  position: absolute;
  right: 12px;
  top: calc(env(safe-area-inset-top) + 52px);
  z-index: 72;
}

.contract-more-menu button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 9px;
  color: var(--contract-text);
  display: flex;
  font-size: 12px;
  gap: 10px;
  min-height: 44px;
  padding: 0 10px;
  text-align: left;
}

.contract-more-menu button:active {
  background: var(--contract-surface-soft);
}

.contract-pencil-module {
  align-items: start;
  background: var(--contract-bg);
  display: grid;
  gap: 10px;
  grid-template-columns: 202px minmax(150px, 1fr);
  height: 460px;
  min-width: 0;
  overflow: hidden;
  padding: 2px 14px 8px;
}

.contract-trade .trade-console {
  background: transparent !important;
  border: 0;
  grid-column: 1;
  grid-row: 1;
  height: 450px;
  min-width: 0;
  padding: 0;
  position: relative;
}

.contract-open-close {
  background: var(--contract-surface);
  border: 1px solid var(--contract-control-line);
  border-radius: 7px;
  display: grid;
  gap: 3px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  height: 30px;
  left: 0;
  padding: 2px;
  position: absolute;
  right: 0;
  top: 0;
}

.contract-open-close button {
  background: transparent;
  border: 0;
  border-radius: 5px;
  color: var(--contract-muted);
  font-size: 11px;
  font-weight: 680;
  height: 24px;
  min-height: 24px;
  min-width: 0;
  padding: 0 4px;
}

.contract-open-close button.active {
  background: var(--contract-accent);
  box-shadow: none;
  color: #07110d;
}

.contract-mode-row {
  display: grid;
  gap: 5px;
  grid-template-columns: 54px 48px minmax(0, 1fr);
  height: 32px;
  left: 0;
  position: absolute;
  right: 0;
  top: 36px;
}

.contract-mode-row button,
.contract-order-type {
  align-items: center;
  background: var(--contract-surface);
  border: 1px solid var(--contract-control-line);
  border-radius: 7px;
  color: var(--contract-text);
  display: flex;
  font-size: 10px;
  gap: 2px;
  height: 32px;
  justify-content: center;
  min-height: 32px;
  min-width: 0;
  padding: 0 5px;
}

.contract-mode-row button[aria-expanded='true'],
.contract-order-type[aria-expanded='true'] {
  border-color: var(--contract-accent);
  color: var(--contract-positive);
}

.contract-mode-row button:disabled,
.contract-order-type:disabled {
  color: var(--contract-muted);
  opacity: .56;
}

.contract-mode-row span,
.contract-order-type span {
  font-size: 10px;
  font-weight: 650;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-order-type {
  width: 100%;
}

.contract-order-type > span {
  flex: 1;
  text-align: center;
}

.contract-price-row {
  display: grid;
  gap: 6px;
  grid-template-columns: minmax(0, 138px) 58px;
  height: 56px;
  left: 0;
  min-width: 0;
  position: absolute;
  right: 0;
  top: 74px;
}

.contract-price-row > button {
  background: var(--contract-surface);
  border: 1px solid transparent;
  border-radius: 8px;
  color: var(--contract-text);
  font-family: var(--font-geist-sans), var(--font-family), sans-serif;
  font-size: 12px;
  font-weight: 650;
  height: 56px;
  min-width: 0;
  padding: 0 4px;
}

.contract-field {
  align-items: center;
  background: var(--contract-surface);
  border: 1px solid transparent;
  border-radius: 8px;
  display: grid;
  min-width: 0;
}

.contract-field:focus-within {
  border-color: var(--contract-accent);
  box-shadow:
    inset 0 0 0 1px color-mix(in srgb, var(--contract-accent) 30%, transparent),
    0 0 0 2px color-mix(in srgb, var(--contract-accent) 13%, transparent);
}

.contract-field.is-invalid,
.contract-field.is-invalid:focus-within {
  border-color: var(--contract-negative);
  box-shadow:
    inset 0 0 0 1px color-mix(in srgb, var(--contract-negative) 26%, transparent),
    0 0 0 2px color-mix(in srgb, var(--contract-negative) 11%, transparent);
}

.contract-field > span {
  color: var(--contract-muted);
  font-size: 9px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-field input {
  background: transparent;
  border: 0;
  box-shadow: none;
  color: var(--contract-text);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
  caret-color: var(--contract-accent);
}

.contract-field input::placeholder {
  color: var(--contract-muted);
  opacity: .72;
}

.contract-field input[readonly] {
  cursor: default;
}

.contract-price-field {
  align-content: center;
  gap: 3px;
  grid-template-rows: 13px 22px;
  height: 56px;
  padding: 7px 10px;
}

.contract-price-field input {
  font-size: 17px;
  font-weight: 700;
  height: 22px;
  line-height: 22px;
}

.contract-amount-field {
  align-content: center;
  column-gap: 6px;
  grid-template-columns: minmax(0, 1fr) auto;
  grid-template-rows: 13px 20px;
  height: 46px;
  left: 0;
  padding: 5px 10px 4px;
  position: absolute;
  right: 0;
  top: 136px;
}

.contract-amount-field input {
  font-size: 15px;
  font-weight: 700;
  grid-column: 1;
  grid-row: 2;
  height: 20px;
  line-height: 20px;
}

.contract-amount-field b {
  align-self: center;
  color: var(--contract-muted);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 10px;
  grid-column: 2;
  grid-row: 1 / span 2;
}

.contract-percentage {
  height: 32px;
  left: 0;
  position: absolute;
  right: 0;
  top: 188px;
}

.contract-trade .contract-percentage .percent-row {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  height: 32px;
  margin: 0;
  min-width: 0;
  position: relative;
}

.contract-trade .contract-percentage .percent-row::before,
.contract-trade .contract-percentage .percent-row::after {
  border-radius: 999px;
  content: '';
  height: 4px;
  left: 8px;
  position: absolute;
  right: 8px;
  top: 8px;
}

.contract-trade .contract-percentage .percent-row::before {
  background: var(--contract-control-line);
}

.contract-trade .contract-percentage .percent-row::after {
  background: var(--contract-accent);
  right: auto;
  width: calc((100% - 16px) * var(--percentage-progress) / 100);
}

.contract-percentage button,
.contract-trade .contract-percentage .percent-row button {
  align-items: end;
  background: transparent;
  border: 0;
  color: var(--contract-muted);
  display: grid;
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 8px;
  height: 44px;
  justify-items: center;
  margin-top: -6px;
  min-height: 44px;
  min-width: 0;
  padding: 0 0 4px;
  position: relative;
  z-index: 1;
}

.contract-percentage button::before {
  background: var(--contract-surface);
  border: 1px solid var(--contract-control-line);
  border-radius: 50%;
  content: '';
  height: 12px;
  left: 50%;
  position: absolute;
  top: 10px;
  transform: translateX(-50%);
  width: 12px;
}

.contract-trade .contract-percentage .percent-row button::after {
  content: '';
  height: 44px;
  left: 50%;
  position: absolute;
  top: 0;
  transform: translateX(-50%);
  width: 44px;
}

.contract-percentage button.passed::before,
.contract-percentage button.active::before {
  background: var(--contract-accent);
  border: 2px solid var(--contract-accent);
}

.contract-percentage button.active {
  background: transparent;
  border-color: transparent;
  box-shadow: none;
  color: var(--contract-text);
  font-weight: 760;
}

.contract-trade .contract-percentage .percent-row button:focus-visible {
  box-shadow: none;
  outline: 0;
}

.contract-trade .contract-percentage .percent-row button:focus-visible::before {
  box-shadow:
    0 0 0 2px var(--contract-bg),
    0 0 0 4px var(--contract-accent);
}

.contract-available-row {
  align-items: center;
  color: var(--contract-muted);
  display: flex;
  font-size: 9px;
  height: 13px;
  justify-content: space-between;
  left: 0;
  position: absolute;
  right: 0;
  top: 226px;
}

.contract-available-row button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--contract-text);
  display: inline-flex;
  font-size: 9px;
  gap: 3px;
  height: 32px;
  justify-content: flex-end;
  margin-right: -4px;
  max-width: 150px;
  overflow: hidden;
  padding: 0 4px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-tpsl {
  align-items: center;
  color: var(--contract-text);
  display: flex;
  font-size: 10px;
  gap: 5px;
  height: 16px;
  left: 0;
  position: absolute;
  right: 0;
  top: 245px;
}

.contract-tpsl > span {
  border: 1.5px solid var(--contract-text);
  border-radius: 3px;
  height: 16px;
  width: 16px;
}

.contract-tpsl small {
  color: var(--contract-muted);
  font-size: 8px;
  margin-left: auto;
}

.contract-open-meta {
  display: grid;
  gap: 1px;
  height: 28px;
  left: 0;
  margin: 0;
  position: absolute;
  right: 0;
}

.contract-open-meta--long { top: 267px; }
.contract-open-meta--short { top: 349px; }

.contract-open-meta > div {
  align-items: center;
  display: flex;
  font-size: 9px;
  justify-content: space-between;
  min-width: 0;
}

.contract-open-meta dt { color: var(--contract-muted); }
.contract-open-meta dd {
  color: var(--contract-text);
  margin: 0;
  max-width: 132px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-submit {
  border: 0;
  border-radius: 21px;
  box-shadow: none;
  font-size: 14px;
  font-weight: 760;
  height: 42px;
  left: 0;
  min-height: 42px;
  padding: 0 8px;
  position: absolute;
  right: 0;
  width: 100%;
}

.contract-submit--long,
.contract-trade .contract-submit--long.submit-order {
  background: var(--contract-accent);
  color: #07130e;
  min-height: 42px;
  top: 301px;
}

.contract-submit--short {
  background: var(--contract-negative);
  color: #fff;
  top: 383px;
}

.contract-submit:disabled {
  background: var(--contract-surface-soft);
  color: var(--contract-muted);
  opacity: .62;
}

.contract-feedback {
  bottom: 0;
  display: -webkit-box;
  font-size: 8px;
  left: 2px;
  line-height: 10px;
  margin: 0;
  max-height: 20px;
  overflow: hidden;
  overflow-wrap: anywhere;
  position: absolute;
  right: 2px;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.contract-trade .trade-chart-panel {
  background: transparent;
  border: 0;
  display: flex;
  flex-direction: column;
  grid-column: 2;
  grid-row: 1;
  height: 450px;
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
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-book-status span { color: var(--contract-muted); }
.contract-book-status strong {
  color: var(--contract-text);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 9px;
}

.contract-trade .trade-order-book {
  background: transparent;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.contract-mini-book {
  height: 424px;
  min-height: 424px;
}

.contract-mini-book :deep(.order-book__mini-header) {
  color: var(--contract-muted);
  height: 18px;
}

.contract-mini-book :deep(.order-book__mini-row) {
  color: var(--contract-text);
  font-size: 9px;
  height: 22px;
}

.contract-mini-book :deep(.order-book__mini-mid) { height: 41px; }
.contract-mini-book :deep(.order-book__mini-mid strong) {
  color: var(--contract-positive);
  font-size: 15px;
}
.contract-mini-book :deep(.order-book__mini-mid small) { color: var(--contract-muted); }
.contract-mini-book :deep(.order-book__mini-ratio) {
  color: var(--contract-muted);
  gap: 5px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  height: 16px;
}
.contract-mini-book :deep(.order-book__mini-ratio)::before {
  background: linear-gradient(90deg, var(--contract-accent) 0 var(--mini-bid-ratio), var(--contract-negative) var(--mini-bid-ratio) 100%);
  border-radius: 2px;
  content: '';
  grid-column: 2;
  height: 3px;
}
.contract-mini-book :deep(.order-book__mini-ratio span:first-child) { grid-column: 1; }
.contract-mini-book :deep(.order-book__mini-ratio span:last-child) { grid-column: 3; }
.contract-mini-book :deep(.order-book__mini-precision) {
  color: var(--contract-muted);
  height: 24px;
  padding-inline: 1px;
}
.contract-mini-book :deep(.order-book__mini-precision-value) { background: var(--contract-surface-soft); }
.contract-mini-book :deep(.order-book__mini-state) { height: 398px; }

.contract-position-tabs {
  align-items: center;
  border-bottom: 1px solid var(--contract-line);
  border-top: 1px solid var(--contract-line);
  display: grid;
  gap: 4px;
  grid-template-columns: auto auto auto minmax(0, 1fr) 44px;
  height: 44px;
  min-height: 44px;
  padding: 0 10px 0 14px;
  scroll-margin-top: calc(58px + env(safe-area-inset-top));
}

.contract-position-tabs button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--contract-muted);
  display: inline-flex;
  font-size: 11px;
  gap: 3px;
  height: 44px;
  justify-content: center;
  min-width: 44px;
  padding: 0 5px;
  position: relative;
  white-space: nowrap;
}

.contract-position-tabs button.active {
  color: var(--contract-text);
  font-weight: 720;
}

.contract-position-tabs button.active::after {
  background: var(--contract-accent);
  border-radius: 2px;
  bottom: 0;
  content: '';
  height: 2px;
  left: 50%;
  position: absolute;
  transform: translateX(-50%);
  width: 22px;
}

.contract-position-tabs button[aria-disabled='true'] { opacity: .58; }

.contract-workspace-panel {
  background: var(--contract-bg);
  min-height: 330px;
  padding: 0 16px calc(20px + env(safe-area-inset-bottom));
}

.contract-workspace-tools {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) auto 42px;
  height: 42px;
}

.contract-workspace-tools button {
  background: transparent;
  border: 0;
  color: var(--contract-muted);
  font-size: 10px;
  min-height: 42px;
}

.contract-current-pair {
  align-items: center;
  display: flex;
  gap: 7px;
  justify-self: start;
  padding: 0;
}

.contract-current-pair > span {
  border: 1px solid var(--contract-line-strong);
  border-radius: 3px;
  height: 13px;
  position: relative;
  width: 13px;
}

.contract-current-pair[aria-pressed='true'] > span {
  background: var(--contract-text);
  border-color: var(--contract-text);
}

.contract-current-pair[aria-pressed='true'] > span::after {
  border-bottom: 1.5px solid var(--contract-bg);
  border-right: 1.5px solid var(--contract-bg);
  content: '';
  height: 6px;
  left: 4px;
  position: absolute;
  top: 1px;
  transform: rotate(45deg);
  width: 3px;
}

.contract-close-all {
  color: var(--contract-text) !important;
  font-weight: 650;
  padding: 0 5px;
}
.contract-close-all.armed { color: var(--contract-negative) !important; }
.contract-filter-control { display: grid; place-items: center; padding: 0; }

.contract-position-list,
.contract-order-list {
  display: grid;
  gap: 10px;
}

.contract-position-card,
.contract-pending-card {
  background: var(--contract-surface);
  border: 1px solid var(--contract-line);
  border-radius: 12px;
  min-width: 0;
}

.contract-position-card {
  display: grid;
  gap: 12px;
  grid-template-rows: auto auto auto 44px;
  min-height: 272px;
  padding: 14px 12px 12px;
}

.contract-position-card > header {
  align-items: start;
  display: flex;
  justify-content: space-between;
  min-width: 0;
}

.contract-position-identity {
  align-items: center;
  display: flex;
  gap: 8px;
  min-width: 0;
}

.contract-position-identity > div {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  min-width: 0;
}

.contract-position-identity strong {
  flex-basis: 100%;
  font-size: 13px;
  line-height: 16px;
}

.contract-position-identity span {
  background: var(--contract-surface-soft);
  border-radius: 3px;
  color: var(--contract-muted);
  font-size: 8px;
  line-height: 16px;
  padding: 0 5px;
}

.contract-position-pnl {
  display: grid;
  justify-items: end;
  min-width: 95px;
}

.contract-position-pnl small { color: var(--contract-muted); font-size: 8px; }
.contract-position-pnl strong { font-size: 16px; line-height: 20px; }
.contract-position-pnl span { font-size: 9px; }

.contract-position-metrics {
  display: grid;
  gap: 12px 8px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0;
}

.contract-position-metrics div { min-width: 0; }
.contract-position-metrics dt {
  color: var(--contract-muted);
  font-size: 8px;
  margin-bottom: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.contract-position-metrics dd {
  color: var(--contract-text);
  font-size: 10px;
  font-weight: 650;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-liquidation-distance {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: auto minmax(0, 1fr) auto;
}

.contract-liquidation-distance span,
.contract-liquidation-distance strong {
  color: var(--contract-muted);
  font-size: 8px;
}

.contract-liquidation-distance i {
  background: var(--contract-line);
  border-radius: 999px;
  height: 4px;
  overflow: hidden;
}
.contract-liquidation-distance b {
  background: linear-gradient(90deg, var(--contract-accent), #f0c85d);
  border-radius: inherit;
  display: block;
  height: 100%;
}

.contract-position-actions {
  display: grid;
  gap: 10px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  height: 44px;
  min-width: 0;
  padding-inline-end: 1px;
  width: 100%;
}

.contract-position-actions button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 12px;
  color: var(--contract-position-action-text);
  display: flex;
  flex-direction: column;
  font-size: 14px;
  font-weight: 600;
  gap: 2px;
  height: 44px;
  justify-content: center;
  line-height: 17px;
  min-height: 44px;
  min-width: 0;
  overflow-wrap: anywhere;
  padding: 3px 4px;
  position: relative;
  transition: color 120ms ease;
  white-space: normal;
}

.contract-position-actions button::before {
  background: var(--contract-position-action-surface);
  border: 1px solid var(--contract-position-action-border);
  border-radius: 12px;
  content: '';
  inset: 1px 0;
  pointer-events: none;
  position: absolute;
  transition: background-color 120ms ease, border-color 120ms ease, transform 120ms ease;
  z-index: 0;
}

.contract-position-actions button > * {
  position: relative;
  z-index: 1;
}

.contract-position-actions button:focus-visible {
  z-index: 1;
}

.contract-position-actions button:active:not(:disabled)::before {
  background: color-mix(in srgb, var(--contract-position-action-surface) 82%, var(--contract-position-action-border));
  transform: translateY(1px);
}

.contract-position-actions button:disabled {
  opacity: .58;
}

.contract-position-actions small {
  font-size: 8px;
  font-weight: 560;
  line-height: 10px;
}

.contract-pending-card > button {
  background: var(--contract-surface-soft);
  border: 1px solid var(--contract-line);
  border-radius: 8px;
  color: var(--contract-text);
  font-size: 10px;
  font-weight: 650;
  min-height: 36px;
}
.contract-position-action.armed,
.contract-position-market-close-all.armed,
.contract-pending-card > button.armed {
  color: var(--contract-negative);
}
.contract-position-action.armed::before,
.contract-position-market-close-all.armed::before {
  border-color: var(--contract-negative);
}

.contract-pending-card {
  align-items: center;
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 108px;
  padding: 12px;
}
.contract-pending-identity { align-items: center; display: flex; gap: 8px; min-width: 0; }
.contract-pending-identity > span { display: grid; min-width: 0; }
.contract-pending-identity strong { font-size: 12px; }
.contract-pending-identity small { font-size: 9px; }
.contract-pending-card dl {
  display: grid;
  gap: 4px;
  grid-column: 1;
  margin: 0;
}
.contract-pending-card dl > div { display: flex; font-size: 9px; gap: 8px; justify-content: space-between; }
.contract-pending-card dt { color: var(--contract-muted); }
.contract-pending-card dd { margin: 0; }
.contract-pending-card > button { grid-column: 2; grid-row: 1 / span 2; min-width: 64px; }

.contract-position-empty {
  align-items: center;
  color: var(--contract-muted);
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 220px;
  padding-top: 52px;
  text-align: center;
}
.contract-position-empty > span {
  background: var(--contract-surface-soft);
  border-radius: 50%;
  color: var(--contract-muted);
  display: grid;
  height: 46px;
  place-items: center;
  width: 46px;
}
.contract-position-empty strong { font-size: 11px; font-weight: 620; max-width: 260px; }
.contract-position-empty button { background: transparent; border: 0; color: var(--contract-positive); min-height: 40px; }

.contract-pencil-header button:focus-visible,
.contract-pencil-module button:focus-visible,
.contract-position-tabs button:focus-visible,
.contract-workspace-panel button:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

@media (max-width: 359px) {
  .contract-pencil-header { padding-inline: 10px; }
  .contract-pair-selector { gap: 5px; max-width: 170px; }
  .contract-product-badge { display: none; }
  .contract-pencil-module {
    gap: 8px;
    grid-template-columns: minmax(0, 1fr) 132px;
    padding-inline: 12px;
  }
  .contract-mode-row { grid-template-columns: 48px 42px minmax(0, 1fr); }
  .contract-price-row { grid-template-columns: minmax(0, 1fr) 48px; }
  .contract-mode-row button,
  .contract-order-type { padding-inline: 3px; }
  .contract-position-tabs { gap: 0; padding-inline: 8px 5px; }
  .contract-position-tabs button { font-size: 9px; padding-inline: 3px; }
  .contract-workspace-panel { padding-inline: 12px; }
  .contract-position-card { padding-inline: 10px; }
  .contract-position-metrics { gap-inline: 5px; }
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
