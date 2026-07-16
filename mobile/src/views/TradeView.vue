<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronDown, Plus } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import OrderBookPanel from '@/components/OrderBookPanel.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchOrderBook } from '@/api/market'
import { fetchMarginProducts, placeMarginOrder, placeSpotOrder, updateMarginLeverage } from '@/api/trading'
import { createFallbackDepth, fallbackTickers } from '@/data/fallback'
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

const pairSymbol = computed(() => String(route.params.symbol || 'BTC_USDT').replace(/[_-]/g, '/').toUpperCase())
const ticker = computed(() => marketStore.tickerFor(pairSymbol.value) || fallbackTickers.find((item) => normalizeSymbol(item.symbol) === normalizeSymbol(pairSymbol.value)) || fallbackTickers[0])
const selectedProduct = computed(() => products.value.find((product) => normalizeSymbol(product.symbol) === normalizeSymbol(pairSymbol.value)) || products.value[0])
const currentPrice = computed(() => ticker.value.lastPrice)
const isLive = computed(() => !marketStore.sampleData && marketStore.tickers.length > 0)
const orderButtonLabel = computed(() => {
  if (mode.value === 'contract') return side.value === 'buy' ? t('trade.longAction', { leverage: leverage.value }) : t('trade.shortAction', { leverage: leverage.value })
  return side.value === 'buy' ? t('trade.buyAsset', { asset: ticker.value.base }) : t('trade.sellAsset', { asset: ticker.value.base })
})
const feedbackIsPositive = computed(() => feedbackTone.value === 'success')

function setFeedback(message: string, tone: 'success' | 'error' = 'error'): void {
  feedback.value = message
  feedbackTone.value = tone
}

async function loadDepth(): Promise<void> {
  try {
    const depth = await fetchOrderBook(pairSymbol.value)
    bids.value = depth.bids
    asks.value = depth.asks
  } catch {
    const depth = createFallbackDepth(currentPrice.value)
    bids.value = depth.bids
    asks.value = depth.asks
  }
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
    setFeedback(t('trade.demoDisabled'))
    return
  }
  if (!Number.isFinite(amount) || amount <= 0 || !Number.isFinite(limitPrice) || limitPrice <= 0) {
    setFeedback(t('trade.invalidOrder'))
    return
  }

  submitting.value = true
  try {
    if (mode.value === 'spot') {
      await placeSpotOrder({ symbol: pairSymbol.value, side: side.value, type: submittedOrderType, price: limitPrice, quantity: amount })
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
  } catch (error) {
    setFeedback(error instanceof Error ? error.message : t('trade.orderFailed'))
  } finally {
    submitting.value = false
  }
}

onMounted(async () => {
  await marketStore.refresh()
  try {
    products.value = await fetchMarginProducts()
    const product = selectedProduct.value
    if (product) {
      leverage.value = product.leverageLevels.includes(5) ? 5 : product.leverageLevels[0] || 1
      marginMode.value = 'isolated'
    }
  } catch {
    products.value = []
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
  if (!price.value) price.value = String(value)
}, { immediate: true })
</script>

<template>
  <main class="page trade-page">
    <nav class="trade-category" :aria-label="t('trade.category')"><button type="button" @click="router.push({ name: 'swap' })">{{ t('trade.swap') }}</button><button :class="{ 'is-active': mode === 'spot' }" type="button" @click="selectTradeMode('spot')">{{ t('trade.spot') }}</button><button :class="{ 'is-active': mode === 'contract' }" type="button" @click="selectTradeMode('contract')">{{ t('trade.contract') }}</button></nav>
    <section class="trade-pair">
      <button type="button" class="trade-pair__selector" @click="openPairPicker"><AssetMark :symbol="ticker.base" :src="ticker.iconUrl" :size="30" /><span><b>{{ ticker.base }}/{{ ticker.quote }}</b><small :class="ticker.changePercent >= 0 ? 'up' : 'down'">{{ formatPrice(currentPrice) }} {{ ticker.changePercent >= 0 ? '+' : '' }}{{ ticker.changePercent.toFixed(2) }}%</small></span><ChevronDown :size="18" /></button>
    </section>

    <div class="page-content trade-page__content">
      <div class="trade-mode"><button type="button" :class="{ 'is-active': mode === 'spot' }" @click="selectTradeMode('spot')">{{ t('trade.spot') }}</button><button type="button" :class="{ 'is-active': mode === 'contract' }" @click="selectTradeMode('contract')">{{ t('trade.perpetual') }}</button></div>
      <div v-if="mode === 'contract'" class="contract-settings"><span>{{ t('trade.isolated') }}</span><button type="button" :disabled="settingsSaving" @click="changeLeverage">{{ leverage }}x <ChevronDown :size="15" /></button><span v-if="selectedProduct">{{ t('trade.marginAsset', { asset: selectedProduct.marginAssetSymbol }) }}</span></div>

      <div class="trade-columns">
        <section class="order-form">
          <div class="buy-sell"><button type="button" :class="{ 'is-buy': side === 'buy' }" @click="side = 'buy'">{{ t('trade.buy') }}</button><button type="button" :class="{ 'is-sell': side === 'sell' }" @click="side = 'sell'">{{ t('trade.sell') }}</button></div>
          <button v-if="mode === 'spot'" class="order-type" type="button" @click="orderType = orderType === 'limit' ? 'market' : 'limit'">{{ orderType === 'limit' ? t('trade.limitOrder') : t('trade.marketOrder') }} <ChevronDown :size="16" /></button>
          <div v-else class="order-type">{{ t('trade.marketOrder') }}</div>
          <label v-if="mode === 'spot'" class="trade-field"><span>{{ t('trade.priceField', { asset: ticker.quote }) }}</span><input v-model="price" class="input" :disabled="orderType === 'market'" inputmode="decimal" :placeholder="t('trade.pricePlaceholder')" /><b v-if="orderType === 'market'">{{ t('trade.marketPrice') }}</b></label>
          <div v-else class="trade-field trade-field--market"><span>{{ t('trade.priceField', { asset: ticker.quote }) }}</span><b>{{ t('trade.marketPrice') }}</b></div>
          <label class="trade-field"><span>{{ mode === 'contract' ? t('trade.marginField', { asset: ticker.quote }) : t('trade.quantityField', { asset: ticker.base }) }}</span><input v-model="quantity" class="input" inputmode="decimal" :placeholder="t('trade.quantityPlaceholder')" /></label>
          <div class="percent-row"><button v-for="item in [0.25, 0.5, 0.75, 1]" :key="item" type="button" @click="setQuantity(item)">{{ item === 1 ? t('trade.maximum') : `${item * 100}%` }}</button></div>
          <p class="trade-balance">{{ t('common.available') }} <button type="button" @click="openLogin">{{ session.isAuthenticated ? t('trade.loadBalance') : t('trade.viewAfterLogin') }} <Plus :size="14" /></button></p>
          <button class="button button--full" :class="side === 'buy' ? 'button--primary' : 'button--danger'" type="button" :disabled="submitting" @click="submitOrder">{{ submitting ? t('trade.submittingOrder') : orderButtonLabel }}</button>
          <p v-if="feedback" class="trade-feedback" :class="feedbackIsPositive ? 'up' : 'down'">{{ feedback }}</p>
        </section>

        <OrderBookPanel :bids="bids" :asks="asks" :current-price="currentPrice" />
      </div>

      <section class="trade-orders"><header><button class="is-active" type="button" @click="openOrders('spot')">{{ t('trade.orders') }}</button><button type="button" @click="openOrders('positions')">{{ t('trade.positionsAndAssets') }}</button><button type="button" @click="openOrders('history')">{{ t('trade.orderHistory') }}</button></header><LoginRequiredState v-if="!session.isAuthenticated" :description="t('trade.ordersLoginHint')" /><button v-else class="trade-orders__entry" type="button" @click="openOrders(mode === 'contract' ? 'positions' : 'spot')">{{ mode === 'contract' ? t('trade.viewPositions') : t('trade.viewOpenOrders') }}</button></section>
      <p v-if="marketStore.sampleData" class="sample-note">{{ t('trade.demoDisabled') }}</p>
    </div>
  </main>
</template>

<style scoped>
.trade-page { padding-top: env(safe-area-inset-top); }.trade-category { border-bottom: 1px solid var(--line); display: flex; gap: 26px; overflow-x: auto; padding: 0 20px; }.trade-category button { background: transparent; color: var(--muted); flex: 0 0 auto; font-size: 17px; font-weight: 650; min-height: 48px; padding: 0; }.trade-category .is-active { color: var(--ink); font-weight: 760; }
.trade-pair { align-items: center; border-bottom: 1px solid var(--line); display: flex; justify-content: space-between; min-height: 66px; padding: 0 12px 0 20px; }.trade-pair__selector { align-items: center; background: transparent; display: flex; gap: 9px; min-width: 0; text-align: left; }.trade-pair__selector > span { display: grid; min-width: 0; }.trade-pair__selector b { font-size: 19px; }.trade-pair__selector small { font-size: 12px; margin-top: 3px; }.trade-pair__selector > svg { color: var(--muted); }
.trade-page__content { padding-top: 16px; }.trade-mode { background: var(--soft); border-radius: 24px; display: grid; grid-template-columns: 1fr 1fr; padding: 4px; }.trade-mode button { background: transparent; border-radius: 19px; color: var(--muted); font-size: 14px; font-weight: 700; min-height: 35px; }.trade-mode .is-active { background: white; box-shadow: 0 1px 4px rgb(15 23 42 / 10%); color: var(--ink); }
.contract-settings { align-items: center; display: flex; gap: 8px; margin: 13px 0; overflow: auto; }.contract-settings button { align-items: center; background: var(--soft); border-radius: 6px; color: var(--ink); display: inline-flex; font-size: 13px; font-weight: 700; gap: 3px; min-height: 34px; padding: 0 10px; }.contract-settings span { color: var(--muted); font-size: 12px; white-space: nowrap; }
.trade-columns { display: grid; gap: 14px; grid-template-columns: minmax(0, 1.05fr) minmax(0, .95fr); margin: 16px -20px 0; }.order-form { padding-left: 20px; }.buy-sell { background: var(--soft); border-radius: 24px; display: grid; grid-template-columns: 1fr 1fr; padding: 4px; }.buy-sell button { background: transparent; border-radius: 19px; color: var(--muted); font-weight: 740; min-height: 35px; }.buy-sell .is-buy { background: var(--positive); color: white; }.buy-sell .is-sell { background: var(--negative); color: white; }
.order-type { align-items: center; background: var(--soft); border-radius: 6px; color: var(--ink); display: flex; font-size: 13px; font-weight: 700; justify-content: space-between; margin-top: 12px; min-height: 42px; padding: 0 12px; width: 100%; }.trade-field { display: grid; margin-top: 10px; position: relative; }.trade-field > span { color: var(--muted); font-size: 11px; left: 12px; position: absolute; top: 7px; z-index: 1; }.trade-field .input { font-size: 16px; min-height: 58px; padding: 19px 12px 3px; }.trade-field b { color: var(--muted); font-size: 13px; position: absolute; right: 12px; top: 24px; }.trade-field--market { background: var(--soft); border-radius: 6px; min-height: 58px; }
.percent-row { display: grid; gap: 6px; grid-template-columns: repeat(4, 1fr); margin-top: 10px; }.percent-row button { background: white; border: 1px solid var(--line); border-radius: 6px; color: var(--muted-strong); font-size: 11px; min-height: 31px; }.trade-balance { align-items: center; color: var(--muted); display: flex; font-size: 12px; justify-content: space-between; margin: 12px 0; }.trade-balance button { align-items: center; background: transparent; color: var(--muted-strong); display: inline-flex; gap: 3px; padding: 0; }.order-form .button { font-size: 14px; min-height: 45px; padding: 0 6px; }.trade-feedback { font-size: 12px; line-height: 1.45; margin: 8px 0 0; }
.trade-columns :deep(.order-book) { border-radius: 0; padding: 0 12px 12px; }.trade-columns :deep(.order-book__row) { font-size: 11px; }.trade-columns :deep(.order-book__last strong) { font-size: 15px; }
.trade-orders { border-top: 1px solid var(--line); margin: 24px -20px 0; }.trade-orders header { display: flex; gap: 20px; overflow: auto; padding: 0 20px; }.trade-orders header button { background: transparent; border-bottom: 2px solid transparent; color: var(--muted); flex: 0 0 auto; font-size: 15px; min-height: 48px; padding: 0; }.trade-orders header .is-active { border-color: var(--ink); color: var(--ink); font-weight: 750; }.trade-orders__entry { background: transparent; color: var(--muted-strong); font-size: 14px; min-height: 74px; padding: 0 20px; text-align: left; width: 100%; }.sample-note { background: #fff8e6; border-radius: 6px; color: #8a5a00; font-size: 12px; margin: 13px 0 0; padding: 8px 10px; }
@media (max-width: 390px) { .trade-columns { gap: 10px; }.trade-page__content { padding-left: 14px; padding-right: 14px; }.trade-columns { margin-left: -14px; margin-right: -14px; }.order-form { padding-left: 14px; }.trade-orders { margin-left: -14px; margin-right: -14px; } }
</style>
