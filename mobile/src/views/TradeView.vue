<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  Activity,
  ArrowUpRight,
  ChevronDown,
  CircleAlert,
  LoaderCircle,
  Plus,
  RefreshCw,
  ShieldCheck,
  X,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
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
    const amount = Number(value)
    quantity.value = Number.isFinite(amount) && amount > 0 && effectivePrice.value > 0
      ? String(Number((amount / effectivePrice.value).toFixed(8)))
      : ''
  },
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
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  const nextQuantity = quantityForBalancePercentage({
    available: availableBalance.value,
    mode: mode.value,
    percentage: percent,
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
  const amount = Number(quantity.value)
  if (!session.isAuthenticated) {
    openLogin()
    return
  }
  if (!isLive.value) {
    setFeedback(t('trade.marketUnavailable'))
    return
  }
  if (!Number.isFinite(amount) || amount <= 0 || !Number.isFinite(effectivePrice.value) || effectivePrice.value <= 0) {
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
  const amount = Number(quantity.value)
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
  if (!Number.isFinite(amount) || amount <= 0 || !Number.isFinite(limitPrice) || limitPrice <= 0) {
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
        quantity: amount,
      })
    } else {
      if (!selectedProduct.value) throw new Error(t('trade.unavailableContract'))
      await placeMarginOrder({
        productId: selectedProduct.value.id,
        side: side.value === 'buy' ? 'long' : 'short',
        marginMode: marginMode.value,
        leverage: leverage.value,
        marginAmount: amount,
      })
    }
    setFeedback(t('trade.orderSubmitted'), 'success')
    quantity.value = ''
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
    class="page trade-page"
    :class="`trade-page--${mode}`"
    :data-trade-mode="mode"
  >
    <section class="trade-instrument">
      <button type="button" class="trade-pair__selector" @click="openPairPicker">
        <AssetMark :symbol="baseAsset" :src="ticker?.iconUrl" :size="34" />
        <span>
          <b>{{ baseAsset }}/{{ quoteAsset }}</b>
          <small>{{ mode === 'contract' ? t('trade.perpetual') : t('marketDetail.spot') }}</small>
        </span>
        <ChevronDown :size="18" />
      </button>
      <div class="trade-mode-signal">
        <Activity v-if="mode === 'spot'" :size="16" />
        <ShieldCheck v-else :size="16" />
        <span>{{ mode === 'spot' ? t('trade.spot') : t('trade.perpetual') }}</span>
      </div>
    </section>

    <section class="trade-quote" :aria-busy="marketStore.loading" data-market-quote="live">
      <div class="trade-quote__price">
        <span>{{ t('marketDetail.latestPrice') }}</span>
        <strong
          v-if="ticker"
          class="numeric"
          :class="ticker.changePercent >= 0 ? 'up' : 'down'"
        >
          {{ formatPrice(currentPrice) }}
        </strong>
        <strong v-else class="numeric">--</strong>
        <small v-if="ticker" :class="ticker.changePercent >= 0 ? 'up' : 'down'">
          {{ ticker.changePercent >= 0 ? '+' : '' }}{{ ticker.changePercent.toFixed(2) }}%
        </small>
        <small v-else>{{ marketStore.loading ? t('common.loading') : t('common.marketUnavailable') }}</small>
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
          <dd class="numeric">{{ ticker ? formatAmount(ticker.volume) : '--' }}</dd>
        </div>
      </dl>
    </section>

    <div class="page-content trade-page__content">
      <LoginRequiredState
        v-if="mode === 'contract' && !session.isAuthenticated"
        :description="t('trade.contractLoginDescription')"
      />

      <section v-if="mode !== 'contract' || session.isAuthenticated" class="trade-chart-panel" :aria-busy="chartLoading">
        <header>
          <span>{{ t('marketDetail.market') }}</span>
          <small>{{ t('common.liveData') }}</small>
        </header>
        <nav :aria-label="t('marketDetail.indicators')">
          <button
            v-for="item in ['1m', '15m', '1h', '4h', '1d']"
            :key="item"
            type="button"
            :class="{ 'is-active': interval === item }"
            :aria-pressed="interval === item"
            @click="chooseInterval(item)"
          >
            {{ item }}
          </button>
        </nav>
        <div class="trade-chart-panel__canvas">
          <MobileMarketChart :points="points" :loading="chartLoading" />
        </div>
      </section>

      <section
        v-if="mode !== 'contract' || session.isAuthenticated"
        class="trade-console-heading"
        data-order-surface="live"
      >
        <div>
          <span>{{ mode === 'contract' ? t('trade.perpetual') : t('trade.spot') }}</span>
          <strong>{{ t('trade.orders') }}</strong>
        </div>
        <button
          type="button"
          @click="openOrders(mode === 'contract' ? 'positions' : 'spot')"
        >
          {{ mode === 'contract' ? t('trade.viewPositions') : t('trade.viewOpenOrders') }}
          <ArrowUpRight :size="15" />
        </button>
      </section>

      <section v-if="mode === 'contract' && session.isAuthenticated" class="contract-settings" :aria-busy="productsLoading">
        <div>
          <span>{{ t('trade.marginAsset', { asset: selectedProduct?.marginAssetSymbol || quoteAsset }) }}</span>
          <strong>{{ t('trade.isolated') }}</strong>
        </div>
        <button
          type="button"
          :disabled="settingsSaving || productsLoading || !selectedProduct"
          @click="changeLeverage"
        >
          <span>{{ t('trade.perpetual') }}</span>
          <b>{{ leverage }}x</b>
          <ChevronDown :size="15" />
        </button>
      </section>

      <div v-if="mode !== 'contract' || session.isAuthenticated" class="trade-columns">
        <section class="order-form">
          <div class="buy-sell" :aria-label="t('trade.category')">
            <button
              type="button"
              :class="{ 'is-buy': side === 'buy' }"
              :aria-pressed="side === 'buy'"
              @click="side = 'buy'"
            >
              {{ t('trade.buy') }}
            </button>
            <button
              type="button"
              :class="{ 'is-sell': side === 'sell' }"
              :aria-pressed="side === 'sell'"
              @click="side = 'sell'"
            >
              {{ t('trade.sell') }}
            </button>
          </div>

          <button
            v-if="mode === 'spot'"
            class="order-type"
            type="button"
            @click="orderType = orderType === 'limit' ? 'market' : 'limit'"
          >
            <span>{{ orderType === 'limit' ? t('trade.limitOrder') : t('trade.marketOrder') }}</span>
            <ChevronDown :size="16" />
          </button>
          <div v-else class="order-type order-type--fixed">
            <span>{{ t('trade.marketOrder') }}</span>
            <ShieldCheck :size="16" />
          </div>

          <label v-if="mode === 'spot'" class="trade-field">
            <span>{{ t('trade.priceField', { asset: quoteAsset }) }}</span>
            <input
              v-model="price"
              class="input numeric"
              :disabled="orderType === 'market'"
              inputmode="decimal"
              :placeholder="t('trade.pricePlaceholder')"
            />
            <b v-if="orderType === 'market'">{{ t('trade.marketPrice') }}</b>
          </label>
          <div v-else class="trade-field trade-field--market">
            <span>{{ t('trade.priceField', { asset: quoteAsset }) }}</span>
            <b>{{ t('trade.marketPrice') }}</b>
          </div>

          <label class="trade-field">
            <span>
              {{ mode === 'contract'
                ? t('trade.marginField', { asset: quoteAsset })
                : t('trade.quantityField', { asset: baseAsset }) }}
            </span>
            <input
              v-model="quantity"
              class="input numeric"
              inputmode="decimal"
              :placeholder="t('trade.quantityPlaceholder')"
            />
          </label>

          <label v-if="mode === 'spot'" class="trade-field">
            <span>{{ t('common.amount') }} ({{ quoteAsset }})</span>
            <input
              v-model="amountValue"
              class="input numeric"
              inputmode="decimal"
              :placeholder="t('common.amount')"
            />
          </label>

          <div class="percent-row">
            <button
              v-for="item in [0.25, 0.5, 0.75, 1]"
              :key="item"
              type="button"
              @click="setQuantity(item)"
            >
              {{ item === 1 ? t('trade.maximum') : `${item * 100}%` }}
            </button>
          </div>

          <p class="trade-balance">
            <span>{{ t('common.available') }}</span>
            <button v-if="!session.isAuthenticated" type="button" @click="openLogin">
              {{ t('trade.viewAfterLogin') }}
              <Plus :size="14" />
            </button>
            <button v-else-if="balancesError" type="button" :disabled="balancesLoading" @click="loadTradingBalances">
              {{ t('common.retry') }}
              <RefreshCw :size="14" :class="{ spin: balancesLoading }" />
            </button>
            <strong v-else class="numeric">
              {{ balancesLoading ? t('common.loading') : `${formatAmount(availableBalance)} ${availableAsset}` }}
            </strong>
          </p>

          <button
            ref="reviewButton"
            class="button button--full order-submit"
            :class="side === 'buy' ? 'button--primary' : 'button--danger'"
            type="button"
            :disabled="submitting"
            :aria-busy="submitting"
            @click="reviewOrder"
          >
            {{ submitting ? t('trade.submittingOrder') : orderButtonLabel }}
          </button>
          <p
            v-if="feedback"
            class="trade-feedback"
            :class="feedbackIsPositive ? 'is-success' : 'is-error'"
            :role="feedbackIsPositive ? 'status' : 'alert'"
          >
            {{ feedback }}
          </p>
        </section>

        <section class="order-book-shell" :aria-busy="depthLoading">
          <OrderBookPanel
            :bids="bids"
            :asks="asks"
            :current-price="currentPrice"
            :base-asset="baseAsset"
            :quote-asset="quoteAsset"
            :loading="depthLoading"
          />
          <div v-if="depthLoading && !hasDepth" class="book-state">
            <LoaderCircle :size="18" class="spin" />
            <span>{{ t('common.loading') }}</span>
          </div>
          <div v-else-if="depthError && !hasDepth" class="book-state book-state--error">
            <CircleAlert :size="18" />
            <span>{{ t('common.loadFailed') }}</span>
            <button type="button" :aria-label="t('common.retry')" @click="loadDepth">
              <RefreshCw :size="16" />
            </button>
          </div>
          <div v-else-if="!hasDepth" class="book-state">
            <span>{{ t('trade.marketUnavailable') }}</span>
          </div>
        </section>
      </div>

      <section class="trade-orders">
        <header>
          <button
            type="button"
            :class="{ 'is-active': mode === 'spot' }"
            @click="openOrders('spot')"
          >
            {{ t('trade.orders') }}
          </button>
          <button
            type="button"
            :class="{ 'is-active': mode === 'contract' }"
            @click="openOrders('positions')"
          >
            {{ t('trade.positionsAndAssets') }}
          </button>
          <button type="button" @click="openOrders('history')">
            {{ t('trade.orderHistory') }}
          </button>
        </header>
        <LoginRequiredState
          v-if="!session.isAuthenticated"
          :description="t('trade.ordersLoginHint')"
        />
        <button
          v-else
          class="trade-orders__entry"
          type="button"
          @click="openOrders(mode === 'contract' ? 'positions' : 'spot')"
        >
          <span>{{ mode === 'contract' ? t('trade.viewPositions') : t('trade.viewOpenOrders') }}</span>
          <ArrowUpRight :size="18" />
        </button>
      </section>

      <div v-if="marketStore.error" class="market-error" role="alert">
        <CircleAlert :size="18" />
        <span>{{ t('trade.marketUnavailable') }}</span>
        <button
          type="button"
          :disabled="marketStore.loading"
          :aria-label="t('common.retry')"
          @click="retryMarket"
        >
          <RefreshCw :size="17" :class="{ spin: marketStore.loading }" />
        </button>
      </div>
    </div>

    <div
      v-if="confirmOpen"
      class="trade-confirm-mask"
      @click.self="closeConfirm"
    >
      <section
        ref="confirmDialog"
        class="trade-confirm-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="trade-confirm-title"
        @keydown="trapDialogFocus"
      >
        <header>
          <div>
            <strong id="trade-confirm-title">{{ orderButtonLabel }}</strong>
            <small>{{ pairSymbol }} · {{ mode === 'contract' ? t('trade.perpetual') : t('trade.spot') }}</small>
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
        <dl>
          <div>
            <dt>{{ mode === 'contract' ? t('trade.marginField', { asset: quoteAsset }) : t('trade.quantityField', { asset: baseAsset }) }}</dt>
            <dd class="numeric">{{ quantity || '--' }}</dd>
          </div>
          <div>
            <dt>{{ t('trade.priceField', { asset: quoteAsset }) }}</dt>
            <dd class="numeric">{{ selectedOrderType === 'market' ? t('trade.marketPrice') : formatPrice(effectivePrice) }}</dd>
          </div>
          <div v-if="mode === 'spot'">
            <dt>{{ t('common.amount') }}</dt>
            <dd class="numeric">{{ amountValue || '--' }} {{ quoteAsset }}</dd>
          </div>
          <div v-else>
            <dt>{{ t('trade.isolated') }}</dt>
            <dd class="numeric">{{ leverage }}x</dd>
          </div>
        </dl>
        <p
          v-if="feedback"
          class="trade-feedback"
          :class="feedbackIsPositive ? 'is-success' : 'is-error'"
          :role="feedbackIsPositive ? 'status' : 'alert'"
        >
          {{ feedback }}
        </p>
        <div class="trade-confirm-actions">
          <button class="button button--secondary" type="button" :disabled="submitting" @click="closeConfirm">
            {{ t('common.cancel') }}
          </button>
          <button
            class="button"
            :class="side === 'buy' ? 'button--primary' : 'button--danger'"
            type="button"
            :disabled="submitting"
            :aria-busy="submitting"
            @click="submitOrder"
          >
            {{ submitting ? t('trade.submittingOrder') : orderButtonLabel }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.trade-page {
  --trade-accent: var(--positive);
  --trade-accent-soft: var(--positive-soft);
  background: var(--surface);
  min-width: 0;
}

.trade-page--contract {
  --trade-accent: var(--accent);
  --trade-accent-soft: color-mix(in srgb, var(--trade-accent) 12%, var(--surface));
}

.trade-instrument {
  align-items: center;
  background:
    linear-gradient(115deg, color-mix(in srgb, var(--trade-accent) 9%, transparent), transparent 54%),
    var(--surface);
  border-bottom: 1px solid var(--line);
  display: flex;
  justify-content: space-between;
  min-height: 72px;
  min-width: 0;
  padding: 8px 14px 8px 18px;
}

.trade-page--contract .trade-instrument {
  border-bottom-color: color-mix(in srgb, var(--trade-accent) 38%, var(--line));
}

.trade-pair__selector {
  align-items: center;
  background: transparent;
  color: var(--ink);
  display: flex;
  gap: 9px;
  min-height: 52px;
  min-width: 0;
  padding: 0;
  text-align: left;
}

.trade-pair__selector > span {
  display: grid;
  min-width: 0;
}

.trade-pair__selector b {
  font-size: 19px;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0;
}

.trade-pair__selector small {
  color: var(--muted);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  margin-top: 3px;
}

.trade-pair__selector > svg {
  color: var(--muted);
  flex: 0 0 auto;
}

.trade-mode-signal {
  align-items: center;
  background: var(--trade-accent-soft);
  border: 1px solid color-mix(in srgb, var(--trade-accent) 38%, var(--line));
  color: var(--trade-accent);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 10px;
  font-weight: 800;
  gap: 5px;
  min-height: 32px;
  padding: 0 9px;
  text-transform: uppercase;
}

.trade-quote {
  background:
    linear-gradient(112deg, color-mix(in srgb, var(--trade-accent) 9%, transparent), transparent 55%),
    var(--surface-elevated);
  border-bottom: 1px solid var(--line);
  border-top: 3px solid var(--trade-accent);
  display: grid;
  gap: 16px;
  grid-template-columns: minmax(0, 1.15fr) minmax(126px, .85fr);
  min-width: 0;
  padding: 16px 20px;
}

.trade-quote__price {
  display: grid;
  min-width: 0;
}

.trade-quote__price > span,
.trade-quote dt {
  color: var(--muted);
  font-size: 10px;
}

.trade-quote__price > strong {
  font-size: 30px;
  line-height: 1.08;
  margin-top: 4px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.trade-quote__price > small {
  font-size: 11px;
  margin-top: 5px;
}

.trade-quote dl {
  align-self: center;
  display: grid;
  gap: 7px;
  margin: 0;
  min-width: 0;
}

.trade-quote dl > div {
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) auto;
  min-width: 0;
}

.trade-quote dd {
  color: var(--muted-strong);
  font-size: 10px;
  margin: 0;
  max-width: 88px;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.trade-page__content {
  min-width: 0;
  padding-top: 14px;
}

.contract-settings {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin-bottom: 12px;
  min-width: 0;
}

.contract-settings > div,
.contract-settings button {
  align-content: center;
  background: var(--soft);
  border: 1px solid var(--line);
  display: grid;
  min-height: 58px;
  min-width: 0;
  padding: 7px 11px;
  text-align: left;
}

.contract-settings button {
  color: var(--ink);
  grid-template-columns: minmax(0, 1fr) auto auto;
  gap: 5px;
}

.contract-settings span {
  color: var(--muted);
  font-size: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-settings > div strong,
.contract-settings button b {
  color: var(--ink);
  font-size: 13px;
  margin-top: 4px;
}

.contract-settings button b {
  color: var(--trade-accent);
  margin-top: 0;
}

.trade-chart-panel {
  border-block: 1px solid var(--line);
  margin: 0 -16px 14px;
  min-width: 0;
}

.trade-chart-panel > header {
  align-items: center;
  display: flex;
  justify-content: space-between;
  min-height: 40px;
  padding: 0 20px;
}

.trade-chart-panel > header span {
  color: var(--ink);
  font-size: 12px;
  font-weight: 750;
}

.trade-chart-panel > header small {
  color: var(--trade-accent);
  font-size: 9px;
}

.trade-chart-panel > nav {
  border-block: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
}

.trade-chart-panel > nav button {
  background: transparent;
  border-bottom: 2px solid transparent;
  color: var(--muted);
  font-size: 10px;
  min-height: 44px;
  min-width: 0;
  padding: 0 3px;
}

.trade-chart-panel > nav button.is-active {
  background: var(--trade-accent-soft);
  border-color: var(--trade-accent);
  color: var(--trade-accent);
  font-weight: 800;
}

.trade-chart-panel__canvas {
  height: 224px;
  min-width: 0;
  overflow: hidden;
}

.trade-console-heading {
  align-items: center;
  border-top: 8px solid var(--soft);
  display: flex;
  justify-content: space-between;
  margin: 0 -16px 12px;
  min-height: 72px;
  min-width: 0;
  padding: 8px 20px 0;
}

.trade-console-heading > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.trade-console-heading > div span {
  color: var(--trade-accent);
  font-size: 9px;
  font-weight: 800;
  text-transform: uppercase;
}

.trade-console-heading > div strong {
  font-size: 17px;
}

.trade-console-heading > button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line-strong);
  color: var(--muted-strong);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 10px;
  font-weight: 750;
  gap: 5px;
  min-height: 44px;
  padding: 0 10px;
}

.trade-columns {
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1.08fr) minmax(124px, .92fr);
  margin: 0 -16px;
  min-width: 0;
}

.order-form {
  border-top: 3px solid var(--trade-accent);
  min-width: 0;
  padding: 10px 0 0 20px;
}

.buy-sell {
  background: var(--line);
  display: grid;
  gap: 1px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.buy-sell button {
  background: var(--surface);
  color: var(--muted);
  font-size: 13px;
  font-weight: 800;
  min-height: 48px;
  min-width: 0;
}

.buy-sell .is-buy {
  background: var(--positive);
  color: var(--on-positive);
}

.buy-sell .is-sell {
  background: var(--negative);
  color: var(--on-negative);
}

.order-type {
  align-items: center;
  background: var(--soft);
  border: 1px solid var(--line);
  color: var(--ink);
  display: flex;
  font-size: 12px;
  font-weight: 750;
  justify-content: space-between;
  margin-top: 10px;
  min-height: 44px;
  min-width: 0;
  padding: 0 11px;
  width: 100%;
}

.order-type--fixed {
  color: var(--trade-accent);
}

.trade-field {
  align-items: center;
  background: var(--soft);
  border: 1px solid var(--line);
  display: grid;
  margin-top: 9px;
  min-height: 58px;
  min-width: 0;
  position: relative;
}

.trade-field:focus-within {
  background: var(--surface);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--focus) 15%, transparent);
}

.trade-field > span {
  color: var(--muted);
  font-size: 10px;
  left: 11px;
  pointer-events: none;
  position: absolute;
  top: 7px;
  z-index: 1;
}

.trade-field .input {
  background: transparent;
  border: 0;
  border-radius: 0;
  color: var(--ink);
  font-size: 15px;
  font-weight: 700;
  min-height: 56px;
  min-width: 0;
  padding: 19px 10px 3px;
}

.trade-field .input:focus {
  background: transparent;
  box-shadow: none;
}

.trade-field .input:disabled {
  color: var(--muted);
  opacity: .76;
}

.trade-field b {
  color: var(--muted);
  font-size: 12px;
  position: absolute;
  right: 10px;
  top: 25px;
}

.trade-field--market {
  align-content: center;
}

.percent-row {
  display: grid;
  gap: 3px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  margin-top: 9px;
  min-width: 0;
}

.percent-row button {
  background: var(--surface);
  border: 1px solid var(--line);
  color: var(--muted-strong);
  font-size: 10px;
  min-height: 44px;
  min-width: 0;
  padding: 0 2px;
}

.percent-row button:focus-visible,
.percent-row button:hover {
  border-color: var(--trade-accent);
  color: var(--trade-accent);
}

.trade-balance {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 11px;
  justify-content: space-between;
  margin: 7px 0;
  min-height: 44px;
  min-width: 0;
}

.trade-balance button {
  align-items: center;
  background: transparent;
  color: var(--muted-strong);
  display: inline-flex;
  font-size: 10px;
  gap: 3px;
  min-height: 44px;
  min-width: 0;
  padding: 0;
}

.trade-balance strong {
  color: var(--ink);
  font-size: 10px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.order-submit {
  border-radius: 0;
  font-size: 13px;
  min-height: 52px;
  padding: 0 7px;
}

.trade-feedback {
  border-left: 3px solid currentColor;
  font-size: 11px;
  line-height: 1.45;
  margin: 9px 0 0;
  padding: 7px 8px;
}

.trade-feedback.is-success {
  background: var(--positive-soft);
  color: var(--positive);
}

.trade-feedback.is-error {
  background: var(--negative-soft);
  color: var(--negative);
}

.order-book-shell {
  background: var(--surface-elevated);
  min-height: 388px;
  min-width: 0;
  position: relative;
}

.order-book-shell :deep(.order-book) {
  min-height: 100%;
  padding: 12px 10px;
}

.order-book-shell :deep(.order-book__row) {
  font-size: 10px;
}

.order-book-shell :deep(.order-book__last strong) {
  font-size: 15px;
}

.book-state {
  align-content: center;
  background: color-mix(in srgb, var(--surface-elevated) 94%, transparent);
  color: var(--muted);
  display: grid;
  gap: 8px;
  inset: 38px 0 0;
  justify-items: center;
  padding: 16px 8px;
  position: absolute;
  text-align: center;
}

.book-state span {
  font-size: 10px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.book-state button {
  background: var(--surface-elevated);
  color: var(--ink);
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.book-state--error {
  color: var(--negative);
}

.trade-orders {
  border-top: 8px solid var(--soft);
  margin: 24px -16px 0;
  min-width: 0;
}

.trade-orders header {
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  min-width: 0;
}

.trade-orders header button {
  background: transparent;
  border-bottom: 3px solid transparent;
  color: var(--muted);
  font-size: 11px;
  min-height: 48px;
  min-width: 0;
  padding: 0 4px;
}

.trade-orders header .is-active {
  border-color: var(--trade-accent);
  color: var(--ink);
  font-weight: 800;
}

.trade-orders__entry {
  align-items: center;
  background: transparent;
  color: var(--muted-strong);
  display: flex;
  font-size: 13px;
  justify-content: space-between;
  min-height: 76px;
  padding: 0 20px;
  text-align: left;
  width: 100%;
}

.market-error {
  align-items: center;
  background: var(--negative-soft);
  border: 1px solid color-mix(in srgb, var(--negative) 28%, var(--line));
  color: var(--negative);
  display: grid;
  font-size: 11px;
  gap: 9px;
  grid-template-columns: auto minmax(0, 1fr) 44px;
  margin: 12px 0 0;
  min-height: 52px;
  padding: 4px 4px 4px 10px;
}

.market-error button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.trade-confirm-mask {
  align-items: flex-end;
  background: var(--overlay);
  display: flex;
  inset: 0;
  justify-content: center;
  padding:
    max(16px, env(safe-area-inset-top))
    16px
    max(16px, env(safe-area-inset-bottom));
  position: fixed;
  z-index: var(--layer-overlay);
}

.trade-confirm-dialog {
  background: var(--surface);
  border: 1px solid var(--line);
  border-top: 3px solid var(--trade-accent);
  box-shadow: var(--shadow-soft);
  display: grid;
  gap: 15px;
  max-height: calc(100dvh - max(32px, env(safe-area-inset-top)) - max(32px, env(safe-area-inset-bottom)));
  max-width: var(--app-max-width);
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 17px;
  width: 100%;
}

.trade-confirm-dialog > header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
}

.trade-confirm-dialog > header > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.trade-confirm-dialog > header strong {
  font-size: 18px;
}

.trade-confirm-dialog > header small {
  color: var(--muted);
  font-size: 10px;
  overflow-wrap: anywhere;
}

.trade-confirm-dialog dl {
  border-block: 1px solid var(--line);
  display: grid;
  margin: 0;
}

.trade-confirm-dialog dl > div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 44px;
}

.trade-confirm-dialog dl > div:last-child {
  border-bottom: 0;
}

.trade-confirm-dialog dt,
.trade-confirm-dialog dd {
  font-size: 11px;
  margin: 0;
}

.trade-confirm-dialog dt {
  color: var(--muted);
}

.trade-confirm-dialog dd {
  font-weight: 750;
  max-width: 62%;
  overflow-wrap: anywhere;
  text-align: right;
}

.trade-confirm-actions {
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, .8fr) minmax(0, 1.2fr);
}

.trade-confirm-actions .button {
  border-radius: 0;
  min-height: 48px;
  min-width: 0;
  padding-inline: 8px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 390px) {
  .trade-instrument {
    padding-left: 14px;
  }

  .trade-quote {
    padding-inline: 14px;
  }

  .trade-mode-signal {
    font-size: 9px;
    padding-inline: 7px;
  }

  .trade-page__content {
    padding-left: 14px;
    padding-right: 14px;
  }

  .trade-columns,
  .trade-chart-panel,
  .trade-console-heading,
  .trade-orders {
    margin-left: -14px;
    margin-right: -14px;
  }

  .order-form {
    padding-left: 14px;
  }

  .trade-chart-panel > header {
    padding-inline: 14px;
  }

  .trade-orders__entry {
    padding-inline: 14px;
  }

  .trade-console-heading {
    padding-inline: 14px;
  }
}

@media (max-width: 340px) {
  .trade-instrument {
    padding-inline: 12px;
  }

  .trade-pair__selector {
    gap: 6px;
  }

  .trade-pair__selector b {
    font-size: 16px;
  }

  .trade-mode-signal span {
    display: none;
  }

  .trade-mode-signal {
    min-height: 44px;
    min-width: 44px;
    padding: 0;
    justify-content: center;
  }

  .trade-columns {
    gap: 8px;
    grid-template-columns: minmax(0, 1fr) minmax(116px, .8fr);
  }

  .trade-quote {
    gap: 10px;
    grid-template-columns: minmax(0, 1fr) 112px;
    padding-inline: 12px;
  }

  .trade-quote__price > strong {
    font-size: 25px;
  }

  .trade-console-heading {
    padding-inline: 12px;
  }

  .trade-console-heading > button {
    max-width: 148px;
  }

  .contract-settings {
    gap: 6px;
  }

  .contract-settings > div,
  .contract-settings button {
    padding-inline: 8px;
  }

  .trade-chart-panel__canvas {
    height: 210px;
  }

  .order-form {
    padding-left: 12px;
  }

  .percent-row {
    grid-template-columns: repeat(2, minmax(44px, 1fr));
  }

  .order-book-shell :deep(.order-book) {
    padding-inline: 8px;
  }

  .order-book-shell :deep(.order-book__row) {
    font-size: 9px;
  }

  .trade-confirm-actions {
    grid-template-columns: 1fr;
  }
}
</style>
