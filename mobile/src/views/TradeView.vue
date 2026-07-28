<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowUpRight,
  CheckCircle2,
  ChevronRight,
  Info,
  RefreshCcw,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  X,
} from 'lucide-vue-next'
import MobileMarketChart from '@/components/MobileMarketChart.vue'
import OrderBookPanel from '@/components/OrderBookPanel.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchKlines, fetchOrderBook } from '@/api/market'
import {
  fetchMarginProducts,
  fetchMarginWallets,
  placeMarginOrder,
  placeSpotOrder,
  updateMarginLeverage,
} from '@/api/trading'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatPrice, normalizeSymbol } from '@/core/format'
import { quantityForBalancePercentage } from '@/core/tradeForm'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'
import { useNavigationStore } from '@/stores/navigation'
import type { KlinePoint, MarginProduct, OrderBookLevel, WalletAccount } from '@/core/types'

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
const bids = ref<OrderBookLevel[]>([])
const asks = ref<OrderBookLevel[]>([])
const points = ref<KlinePoint[]>([])
const interval = ref('15m')
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

const pairSymbol = computed(() => String(route.params.symbol || 'BTC_USDT').replace(/[_-]/g, '/').toUpperCase())
const ticker = computed(() => marketStore.tickerFor(pairSymbol.value))
const baseAsset = computed(() => pairSymbol.value.split('/')[0] || '')
const quoteAsset = computed(() => pairSymbol.value.split('/')[1] || 'USDT')
const selectedProduct = computed(() => products.value.find((product) => normalizeSymbol(product.symbol) === normalizeSymbol(pairSymbol.value)))
const currentPrice = computed(() => ticker.value?.lastPrice ?? 0)
const isLive = computed(() => !marketStore.error && !!ticker.value)
const hasDepth = computed(() => bids.value.length > 0 || asks.value.length > 0)
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

function setFeedback(message: string, tone: 'success' | 'error' = 'error'): void {
  feedback.value = message
  feedbackTone.value = tone
}

async function loadDepth(): Promise<void> {
  depthLoading.value = true
  depthError.value = false
  try {
    const depth = await fetchOrderBook(pairSymbol.value)
    bids.value = depth.bids
    asks.value = depth.asks
  } catch {
    bids.value = []
    asks.value = []
    depthError.value = true
  } finally {
    depthLoading.value = false
  }
}

async function loadChart(): Promise<void> {
  chartLoading.value = true
  try {
    points.value = await fetchKlines(pairSymbol.value, interval.value)
  } catch {
    points.value = []
  } finally {
    chartLoading.value = false
  }
}

async function retryMarket(): Promise<void> {
  await marketStore.refresh(true)
  await Promise.all([loadDepth(), loadChart()])
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
    balancesLoading.value = false
    balancesError.value = false
    return
  }
  balancesLoading.value = true
  balancesError.value = false
  try {
    if (mode.value === 'contract') {
      marginWallets.value = (await fetchMarginWallets()).wallets
    } else {
      spotWallets.value = await fetchWalletAccounts()
    }
  } catch {
    if (mode.value === 'contract') marginWallets.value = []
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
  if (interval.value === value) return
  interval.value = value
  void loadChart()
}

function openPairPicker(): void {
  void router.push({ name: 'markets', query: { purpose: 'trade', mode: mode.value } })
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
  void Promise.all([loadDepth(), loadChart()])
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
    <div class="trade-heading">
      <button class="symbol-selector" type="button" :aria-label="t('markets.pickerTitle')" @click="openPairPicker">
        <span class="trade-symbol-copy">
          <small>{{ mode === 'contract' ? t('rootPrototype.perpetualPair') : t('rootPrototype.spotTrading') }}</small>
          <strong>{{ pairSymbol }}</strong>
        </span>
        <ChevronRight :size="16" aria-hidden="true" />
      </button>
      <div class="trade-tools">
        <button class="icon-button" type="button" :aria-label="t('markets.refresh')" @click="retryMarket">
          <RefreshCcw :size="17" aria-hidden="true" />
        </button>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('trade.settings')"
          @click="mode === 'contract' ? changeLeverage() : openOrders('spot')"
        >
          <Settings2 :size="17" aria-hidden="true" />
        </button>
      </div>
    </div>

    <div class="trade-quote" :aria-busy="marketStore.loading" data-market-quote="live">
      <div>
        <strong class="numeric" :class="(ticker?.changePercent || 0) >= 0 ? 'positive' : 'negative'">
          {{ ticker ? formatPrice(currentPrice) : '--' }}
        </strong>
        <span :class="(ticker?.changePercent || 0) >= 0 ? 'positive' : 'negative'">
          {{ ticker ? `${ticker.changePercent >= 0 ? '+' : ''}${ticker.changePercent.toFixed(2)}%` : '--' }}
        </span>
      </div>
      <div class="quote-stats">
        <template v-if="mode === 'contract'">
          <span>{{ t('rootPrototype.markPrice') }} <b class="numeric">{{ ticker ? formatPrice(currentPrice) : '--' }}</b></span>
          <span>{{ t('rootPrototype.fundingRate') }} <b class="positive numeric">--</b></span>
        </template>
        <template v-else>
          <span>{{ t('marketDetail.high24h') }} <b class="numeric">{{ ticker ? formatPrice(ticker.highPrice) : '--' }}</b></span>
          <span>{{ t('marketDetail.low24h') }} <b class="numeric">{{ ticker ? formatPrice(ticker.lowPrice) : '--' }}</b></span>
        </template>
      </div>
    </div>

    <div class="chart-panel trade-chart-panel" :aria-busy="chartLoading">
      <div class="chart-tools">
        <button
          v-for="time in ['1m', '15m', '1h', '4h', '1d']"
          :key="time"
          type="button"
          :class="{ active: interval === time }"
          @click="chooseInterval(time)"
        >
          {{ time }}
        </button>
        <button type="button" :aria-label="t('marketDetail.indicators')" @click="openOrders('history')">
          <SlidersHorizontal :size="15" aria-hidden="true" />
        </button>
      </div>
      <div class="chart-panel__canvas">
        <MobileMarketChart :points="points" :loading="chartLoading" />
      </div>
      <span class="live-price-line numeric">{{ ticker ? formatPrice(currentPrice) : '--' }}</span>
      <OrderBookPanel
        class="chart-semantic-summary"
        :asks="asks"
        :bids="bids"
        :current-price="currentPrice"
        :base-asset="baseAsset"
        :loading="depthLoading"
        :quote-asset="quoteAsset"
      />
      <p class="chart-semantic-summary">
        {{ pairSymbol }} · {{ interval }} · {{ ticker ? formatPrice(currentPrice) : t('common.marketUnavailable') }}
      </p>
    </div>

    <div class="trade-console" data-order-surface="live">
      <div class="order-surface-heading">
        <div>
          <span>{{ mode === 'contract' ? t('rootPrototype.perpetualOrderEyebrow') : t('rootPrototype.spotOrderEyebrow') }}</span>
          <strong>{{ mode === 'contract' ? t('rootPrototype.perpetualOrder') : t('rootPrototype.spotOrder') }}</strong>
        </div>
        <button class="risk-chip" type="button" @click="openOrders(mode === 'contract' ? 'positions' : 'spot')">
          {{ mode === 'contract' ? t('trade.positionsAndAssets') : t('trade.viewOpenOrders') }}
        </button>
      </div>

      <div v-if="mode === 'contract'" class="contract-settings" :aria-label="t('trade.settings')">
        <button type="button" :disabled="true">
          <span>{{ t('rootPrototype.marginMode') }}</span>
          <strong>{{ t('trade.isolated') }}</strong>
          <ChevronRight :size="14" aria-hidden="true" />
        </button>
        <button type="button" :disabled="settingsSaving || productsLoading" @click="changeLeverage">
          <span>{{ t('rootPrototype.leverage') }}</span>
          <strong>{{ leverage }}x</strong>
          <ChevronRight :size="14" aria-hidden="true" />
        </button>
      </div>

      <div class="side-switch">
        <button
          type="button"
          :class="{ active: side === 'buy', buy: side === 'buy' }"
          @click="side = 'buy'"
        >
          {{ mode === 'contract' ? t('rootPrototype.openLong') : t('trade.buy') }}
        </button>
        <button
          type="button"
          :class="{ active: side === 'sell', sell: side === 'sell' }"
          @click="side = 'sell'"
        >
          {{ mode === 'contract' ? t('rootPrototype.openShort') : t('trade.sell') }}
        </button>
      </div>

      <div class="order-type-row">
        <button
          type="button"
          :class="{ active: selectedOrderType === 'limit' }"
          :disabled="mode === 'contract'"
          @click="orderType = 'limit'"
        >
          {{ t('trade.limitOrder') }}
        </button>
        <button
          type="button"
          :class="{ active: selectedOrderType === 'market' }"
          :disabled="mode === 'contract'"
          @click="orderType = 'market'"
        >
          {{ t('trade.marketOrder') }}
        </button>
        <button type="button" :disabled="true">{{ mode === 'contract' ? t('rootPrototype.triggerOrder') : t('rootPrototype.takeProfitStopLoss') }}</button>
      </div>

      <div class="input-stack">
        <label class="field-shell">
          <span>{{ t('rootPrototype.orderPrice') }}</span>
          <input
            v-model="price"
            inputmode="decimal"
            :readonly="selectedOrderType === 'market'"
            :placeholder="selectedOrderType === 'market' ? t('trade.marketPrice') : t('trade.pricePlaceholder')"
          />
          <b>{{ quoteAsset }}</b>
        </label>
        <label class="field-shell">
          <span>
            {{ mode === 'contract'
              ? t('trade.marginField', { asset: availableAsset })
              : t('common.quantity') }}
          </span>
          <input v-model="quantity" inputmode="decimal" :placeholder="t('trade.quantityPlaceholder')" />
          <b>{{ mode === 'contract' ? availableAsset : baseAsset }}</b>
        </label>
        <label v-if="mode === 'contract'" class="field-shell">
          <span>{{ t('rootPrototype.estimatedNotional') }}</span>
          <input :value="contractNotionalValue" inputmode="decimal" readonly />
          <b>{{ availableAsset }}</b>
        </label>
      </div>

      <div class="amount-control">
        <input
          type="range"
          min="0"
          max="100"
          step="25"
          :value="percentage"
          :style="{ '--range-value': `${percentage}%` }"
          :aria-label="t('rootPrototype.balancePercentage')"
          @input="setQuantity(Number(($event.target as HTMLInputElement).value))"
        />
        <div class="percent-row">
          <button
            v-for="value in [0, 25, 50, 75, 100]"
            :key="value"
            type="button"
            :class="{ active: percentage === value }"
            @click="setQuantity(value)"
          >
            {{ value }}%
          </button>
        </div>
      </div>

      <div class="available-row">
        <span>{{ mode === 'contract' ? t('rootPrototype.availableMargin') : t('rootPrototype.spotAvailable') }}</span>
        <button v-if="!session.isAuthenticated" type="button" @click="openLogin">
          {{ t('trade.viewAfterLogin') }}
        </button>
        <button v-else-if="balancesError" type="button" :disabled="balancesLoading" @click="loadTradingBalances">
          {{ t('common.retry') }}
          <RefreshCcw :size="14" :class="{ spin: balancesLoading }" />
        </button>
        <strong v-else class="numeric">
          {{ balancesLoading ? t('trade.loadBalance') : `${formatAmount(availableBalance)} ${availableAsset}` }}
        </strong>
      </div>

      <div class="trade-helper">
        <Info :size="14" aria-hidden="true" />
        <span>
          {{ mode === 'contract'
            ? t('rootPrototype.contractHelper', { mode: t('trade.isolated'), leverage })
            : t('rootPrototype.spotHelper', { asset: baseAsset }) }}
        </span>
      </div>

      <p
        class="trade-feedback"
        :class="feedback ? (feedbackIsPositive ? 'positive' : 'negative') : ''"
        aria-live="polite"
      >
        {{ feedback || (balancesLoading ? t('trade.loadBalance') : '') }}
      </p>

      <button
        ref="reviewButton"
        class="submit-order"
        :class="side"
        type="button"
        :disabled="submitting || !isLive"
        @click="reviewOrder"
      >
        {{ submitting ? t('trade.submittingOrder') : orderButtonLabel }}
        <ArrowUpRight :size="18" aria-hidden="true" />
      </button>

      <div class="trade-disclaimer">
        <ShieldCheck :size="15" aria-hidden="true" />
        <span>{{ mode === 'contract' ? t('rootPrototype.contractRisk') : t('rootPrototype.spotSettlement') }}</span>
      </div>
    </div>

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
