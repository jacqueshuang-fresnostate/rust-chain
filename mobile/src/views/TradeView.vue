<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
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
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import OrderBookPanel from '@/components/OrderBookPanel.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchOrderBook } from '@/api/market'
import {
  fetchMarginProducts,
  placeMarginOrder,
  placeSpotOrder,
  updateMarginLeverage,
} from '@/api/trading'
import { formatPrice, normalizeSymbol } from '@/core/format'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'
import { useNavigationStore } from '@/stores/navigation'
import type { MarginProduct, OrderBookLevel } from '@/core/types'

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
const bids = ref<OrderBookLevel[]>([])
const asks = ref<OrderBookLevel[]>([])
const feedback = ref('')
const feedbackTone = ref<'success' | 'error'>('error')
const submitting = ref(false)
const settingsSaving = ref(false)
const depthLoading = ref(false)
const depthError = ref(false)
const productsLoading = ref(false)

const pairSymbol = computed(() => String(route.params.symbol || 'BTC_USDT').replace(/[_-]/g, '/').toUpperCase())
const ticker = computed(() => marketStore.tickerFor(pairSymbol.value))
const baseAsset = computed(() => pairSymbol.value.split('/')[0] || '')
const quoteAsset = computed(() => pairSymbol.value.split('/')[1] || 'USDT')
const selectedProduct = computed(() => products.value.find((product) => normalizeSymbol(product.symbol) === normalizeSymbol(pairSymbol.value)) || products.value[0])
const currentPrice = computed(() => ticker.value?.lastPrice ?? 0)
const isLive = computed(() => !marketStore.error && !!ticker.value)
const hasDepth = computed(() => bids.value.length > 0 || asks.value.length > 0)
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

async function retryMarket(): Promise<void> {
  await marketStore.refresh(true)
  await loadDepth()
}

function setQuantity(percent: number): void {
  const quoteBudget = 100 * percent
  quantity.value = mode.value === 'contract'
    ? String(quoteBudget)
    : currentPrice.value ? String(quoteBudget / currentPrice.value) : ''
}

function openPairPicker(): void {
  void router.push({ name: 'markets', query: { purpose: 'trade', mode: mode.value } })
}

function selectTradeMode(nextMode: 'spot' | 'contract'): void {
  mode.value = nextMode
  navigation.rememberTradeMode(nextMode)
  feedback.value = ''
  void router.replace({
    name: 'trade',
    params: { symbol: pairSymbol.value.replace('/', '_') },
    query: nextMode === 'contract' ? { mode: 'contract' } : undefined,
  })
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

async function submitOrder(): Promise<void> {
  feedback.value = ''
  const amount = Number(quantity.value)
  const submittedOrderType = mode.value === 'contract' ? 'market' : orderType.value
  const limitPrice = submittedOrderType === 'limit' ? Number(price.value) : currentPrice.value
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
  } catch (reason) {
    setFeedback(apiErrorMessage(reason, t('trade.orderFailed')))
  } finally {
    submitting.value = false
  }
}

onMounted(async () => {
  await marketStore.refresh()
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
})

watch(pairSymbol, (symbol) => {
  navigation.rememberTradeSymbol(symbol)
  void loadDepth()
}, { immediate: true })

watch(() => route.query.mode, (nextMode) => {
  mode.value = nextMode === 'contract' ? 'contract' : 'spot'
  navigation.rememberTradeMode(mode.value)
}, { immediate: true })

watch(currentPrice, (value) => {
  if (!price.value && value > 0) price.value = String(value)
}, { immediate: true })
</script>

<template>
  <main
    class="page trade-page"
    :class="`trade-page--${mode}`"
    :data-trade-mode="mode"
  >
    <nav class="trade-category" :aria-label="t('trade.category')">
      <button type="button" @click="router.push({ name: 'swap' })">
        {{ t('trade.swap') }}
      </button>
      <button
        type="button"
        :class="{ 'is-active': mode === 'spot' }"
        :aria-pressed="mode === 'spot'"
        @click="selectTradeMode('spot')"
      >
        {{ t('trade.spot') }}
      </button>
      <button
        type="button"
        :class="{ 'is-active': mode === 'contract' }"
        :aria-pressed="mode === 'contract'"
        @click="selectTradeMode('contract')"
      >
        {{ t('trade.contract') }}
      </button>
    </nav>

    <section class="trade-instrument">
      <button type="button" class="trade-pair__selector" @click="openPairPicker">
        <AssetMark :symbol="baseAsset" :src="ticker?.iconUrl" :size="34" />
        <span>
          <b>{{ baseAsset }}/{{ quoteAsset }}</b>
          <small v-if="ticker" :class="ticker.changePercent >= 0 ? 'up' : 'down'">
            {{ formatPrice(currentPrice) }}
            {{ ticker.changePercent >= 0 ? '+' : '' }}{{ ticker.changePercent.toFixed(2) }}%
          </small>
          <small v-else>--</small>
        </span>
        <ChevronDown :size="18" />
      </button>
      <div class="trade-mode-signal">
        <Activity v-if="mode === 'spot'" :size="16" />
        <ShieldCheck v-else :size="16" />
        <span>{{ mode === 'spot' ? t('trade.spot') : t('trade.perpetual') }}</span>
      </div>
    </section>

    <div class="page-content trade-page__content">
      <section v-if="mode === 'contract'" class="contract-settings" :aria-busy="productsLoading">
        <span>{{ t('trade.isolated') }}</span>
        <button
          type="button"
          :disabled="settingsSaving || productsLoading"
          @click="changeLeverage"
        >
          <b>{{ leverage }}x</b>
          <ChevronDown :size="15" />
        </button>
        <span v-if="selectedProduct">
          {{ t('trade.marginAsset', { asset: selectedProduct.marginAssetSymbol }) }}
        </span>
        <span v-else>{{ t('trade.unavailableContract') }}</span>
      </section>

      <div class="trade-columns">
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
            <button type="button" @click="openLogin">
              {{ session.isAuthenticated ? t('trade.loadBalance') : t('trade.viewAfterLogin') }}
              <Plus :size="14" />
            </button>
          </p>

          <button
            class="button button--full order-submit"
            :class="side === 'buy' ? 'button--primary' : 'button--danger'"
            type="button"
            :disabled="submitting"
            :aria-busy="submitting"
            @click="submitOrder"
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
          <OrderBookPanel :bids="bids" :asks="asks" :current-price="currentPrice" />
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

.trade-category {
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  min-width: 0;
  position: sticky;
  top: env(safe-area-inset-top);
  z-index: 18;
}

.trade-category button {
  background: transparent;
  border-bottom: 3px solid transparent;
  color: var(--muted);
  font-size: 14px;
  font-weight: 700;
  min-height: 48px;
  min-width: 0;
  padding: 0 8px;
}

.trade-category .is-active {
  border-bottom-color: var(--trade-accent);
  color: var(--ink);
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

.trade-page__content {
  min-width: 0;
  padding-top: 14px;
}

.contract-settings {
  align-items: center;
  border-block: 1px solid var(--line);
  display: flex;
  gap: 8px;
  margin-bottom: 14px;
  min-height: 54px;
  min-width: 0;
  overflow-x: auto;
}

.contract-settings button {
  align-items: center;
  background: var(--trade-accent-soft);
  border: 1px solid color-mix(in srgb, var(--trade-accent) 35%, var(--line));
  color: var(--ink);
  display: inline-flex;
  flex: 0 0 auto;
  gap: 4px;
  min-height: 44px;
  padding: 0 12px;
}

.contract-settings span {
  color: var(--muted);
  flex: 0 0 auto;
  font-size: 11px;
  white-space: nowrap;
}

.trade-columns {
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1.08fr) minmax(124px, .92fr);
  margin: 0 -20px;
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
  color: var(--on-positive, #fff);
}

.buy-sell .is-sell {
  background: var(--negative);
  color: var(--on-negative, #fff);
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
  border-color: var(--focus, #1677ff);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--focus, #1677ff) 15%, transparent);
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
  gap: 5px;
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
  background: var(--dark-surface);
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
  background: color-mix(in srgb, var(--dark-surface) 94%, transparent);
  color: #aeb6bf;
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
  background: #252b2e;
  color: #f5f7f8;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.book-state--error {
  color: #ff8199;
}

.trade-orders {
  border-top: 8px solid var(--soft);
  margin: 24px -20px 0;
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

  .trade-mode-signal {
    font-size: 9px;
    padding-inline: 7px;
  }

  .trade-page__content {
    padding-left: 14px;
    padding-right: 14px;
  }

  .trade-columns,
  .trade-orders {
    margin-left: -14px;
    margin-right: -14px;
  }

  .order-form {
    padding-left: 14px;
  }

  .trade-orders__entry {
    padding-inline: 14px;
  }
}

@media (max-width: 340px) {
  .trade-category button {
    font-size: 12px;
    padding-inline: 3px;
  }

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

  .order-form {
    padding-left: 12px;
  }

  .order-book-shell :deep(.order-book) {
    padding-inline: 8px;
  }

  .order-book-shell :deep(.order-book__row) {
    font-size: 9px;
  }
}
</style>
