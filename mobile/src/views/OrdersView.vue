<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { RefreshCw } from 'lucide-vue-next'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  cancelAllMarginPositions,
  cancelAllSpotOrders,
  cancelMarginPosition,
  cancelSpotOrder,
  closeAllMarginPositions,
  closeMarginPosition,
  fetchMarginPositions,
  fetchMarginProducts,
  fetchOpenSpotOrders,
  fetchSpotOrderHistory,
  type MarginPosition,
  type SpotOrder,
} from '@/api/trading'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { MarginProduct } from '@/core/types'

type Tab = 'spot' | 'margin' | 'history'

const route = useRoute()
const session = useSessionStore()
const { t } = useI18n()
const activeTab = ref<Tab>('spot')
const tabs = computed(() => [
  { value: 'spot' as const, label: t('orders.spotOrders') },
  { value: 'margin' as const, label: t('orders.marginPositions') },
  { value: 'history' as const, label: t('orders.history') },
])
const spotOrders = ref<SpotOrder[]>([])
const historyOrders = ref<SpotOrder[]>([])
const positions = ref<MarginPosition[]>([])
const historyPositions = ref<MarginPosition[]>([])
const products = ref<MarginProduct[]>([])
const loading = ref(false)
const actionId = ref('')
const feedback = ref('')
const error = ref('')

const openedPositions = computed(() => positions.value.filter((position) => position.status === 'opened'))
const cancelablePositions = computed(() => openedPositions.value.filter((position) => position.entryPrice <= 0))
const closablePositions = computed(() => openedPositions.value.filter((position) => position.entryPrice > 0))
const sortedSpotOrders = computed(() => [...spotOrders.value].sort((left, right) => (right.createdAt || 0) - (left.createdAt || 0)))
const sortedHistoryOrders = computed(() => [...historyOrders.value].sort((left, right) => (right.createdAt || 0) - (left.createdAt || 0)))

function productFor(position: MarginPosition): MarginProduct | undefined {
  return products.value.find((product) => product.id === position.productId)
}

function positionSymbol(position: MarginPosition): string {
  const product = productFor(position)
  if (product) return product.symbol
  return position.symbol.includes('/') ? position.symbol : t('orders.contractNumber', { id: position.productId })
}

function displayPair(symbol: string): string {
  return symbol.replace(/_/g, '/').replace(/-/g, '/')
}

function setTab(tab: Tab): void {
  activeTab.value = tab
}

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  feedback.value = ''
  error.value = ''
  try {
    if (activeTab.value === 'spot') {
      spotOrders.value = await fetchOpenSpotOrders()
      return
    }
    if (activeTab.value === 'margin') {
      const [nextPositions, nextProducts] = await Promise.all([fetchMarginPositions('opened'), fetchMarginProducts()])
      positions.value = nextPositions
      products.value = nextProducts
      return
    }
    const [nextOrders, closed, liquidated, canceled, nextProducts] = await Promise.all([
      fetchSpotOrderHistory(),
      fetchMarginPositions('closed'),
      fetchMarginPositions('liquidated'),
      fetchMarginPositions('canceled'),
      fetchMarginProducts(),
    ])
    historyOrders.value = nextOrders
    historyPositions.value = [...closed, ...liquidated, ...canceled]
    products.value = nextProducts
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('orders.loadFailed'))
  } finally {
    loading.value = false
  }
}

async function cancelSpot(order: SpotOrder): Promise<void> {
  actionId.value = `spot-${order.id}`
  error.value = ''
  try {
    await cancelSpotOrder(order.id)
    feedback.value = t('orders.spotCanceled')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('orders.spotCancelFailed'))
  } finally {
    actionId.value = ''
  }
}

async function cancelAllSpot(): Promise<void> {
  if (!spotOrders.value.length) return
  actionId.value = 'spot-all'
  error.value = ''
  try {
    await cancelAllSpotOrders(spotOrders.value.map((order) => order.id))
    feedback.value = t('orders.allSpotCanceled')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('orders.allSpotCancelFailed'))
  } finally {
    actionId.value = ''
  }
}

async function actOnPosition(position: MarginPosition): Promise<void> {
  const shouldCancel = position.entryPrice <= 0
  actionId.value = `margin-${position.id}`
  error.value = ''
  try {
    if (shouldCancel) await cancelMarginPosition(position.id)
    else await closeMarginPosition(position.id)
    feedback.value = shouldCancel ? t('orders.marginCanceled') : t('orders.closeSubmitted')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, shouldCancel ? t('orders.marginCancelFailed') : t('orders.closeFailed'))
  } finally {
    actionId.value = ''
  }
}

async function cancelAllMargin(): Promise<void> {
  if (!cancelablePositions.value.length) return
  actionId.value = 'margin-cancel-all'
  error.value = ''
  try {
    await cancelAllMarginPositions()
    feedback.value = t('orders.allPendingCanceled')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('orders.batchCancelFailed'))
  } finally {
    actionId.value = ''
  }
}

async function closeAllMargin(): Promise<void> {
  if (!closablePositions.value.length) return
  actionId.value = 'margin-close-all'
  error.value = ''
  try {
    await closeAllMarginPositions()
    feedback.value = t('orders.allCloseSubmitted')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('orders.allCloseFailed'))
  } finally {
    actionId.value = ''
  }
}

watch(activeTab, () => { void load() })
onMounted(() => {
  if (route.query.tab === 'positions') activeTab.value = 'margin'
  else if (route.query.tab === 'history') activeTab.value = 'history'
  void load()
})

function statusLabel(status: string): string {
  const keyByStatus: Record<string, string> = {
    submitted: 'orders.statusSubmitted',
    pending: 'orders.statusPending',
    trading: 'orders.statusTrading',
    open: 'orders.statusTrading',
    partially_filled: 'orders.statusPartiallyFilled',
    completed: 'orders.statusCompleted',
    filled: 'orders.statusCompleted',
    canceled: 'orders.statusCanceled',
    cancelled: 'orders.statusCanceled',
    closed: 'orders.statusClosed',
    liquidated: 'orders.statusLiquidated',
    rejected: 'orders.statusRejected',
  }
  const key = keyByStatus[status.trim().toLowerCase()]
  return key ? t(key) : status
}
</script>

<template>
  <main class="page page--plain orders-page">
    <PageHeader :title="t('orders.title')">
      <template #actions><button class="icon-button" type="button" :aria-label="t('orders.refresh')" :disabled="loading" @click="load"><RefreshCw :size="21" :class="{ spin: loading }" /></button></template>
    </PageHeader>
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('orders.loginDescription')" />
      <template v-else>
        <nav class="order-tabs" :aria-label="t('orders.category')"><button v-for="item in tabs" :key="item.value" type="button" :class="{ 'is-active': activeTab === item.value }" @click="setTab(item.value)">{{ item.label }}</button></nav>
        <p v-if="error" class="error-message">{{ error }}</p><p v-if="feedback" class="success-message">{{ feedback }}</p>
        <p v-if="loading" class="empty-state">{{ t('orders.loading') }}</p>

        <template v-else-if="activeTab === 'spot'">
          <div class="order-toolbar"><span>{{ t('orders.currentOrders', { count: sortedSpotOrders.length }) }}</span><button v-if="sortedSpotOrders.length" type="button" :disabled="actionId === 'spot-all'" @click="cancelAllSpot">{{ actionId === 'spot-all' ? t('orders.canceling') : t('orders.cancelAll') }}</button></div>
          <div v-if="sortedSpotOrders.length" class="order-list"><article v-for="order in sortedSpotOrders" :key="order.id" class="order-card"><header><strong>{{ displayPair(order.symbol) }}</strong><span :class="order.side === 'buy' ? 'buy-tag' : 'sell-tag'">{{ order.side === 'buy' ? t('orders.buy') : t('orders.sell') }}</span></header><dl><div><dt>{{ t('orders.orderPrice') }}</dt><dd>{{ order.orderType === 'market' ? t('orders.marketPrice') : formatPrice(order.price) }}</dd></div><div><dt>{{ t('orders.orderQuantity') }}</dt><dd>{{ formatAmount(order.quantity) }}</dd></div><div><dt>{{ t('orders.filled') }}</dt><dd>{{ formatAmount(order.filledQuantity) }}</dd></div></dl><footer><small>{{ formatDateTime(order.createdAt) }}</small><button class="button button--secondary" type="button" :disabled="actionId === `spot-${order.id}`" @click="cancelSpot(order)">{{ actionId === `spot-${order.id}` ? t('orders.processing') : t('orders.cancel') }}</button></footer></article></div><p v-else class="empty-state">{{ t('orders.noSpotOrders') }}</p>
        </template>

        <template v-else-if="activeTab === 'margin'">
          <div class="order-toolbar"><span>{{ t('orders.currentPositions', { count: openedPositions.length }) }}</span><div><button v-if="cancelablePositions.length" type="button" :disabled="actionId === 'margin-cancel-all'" @click="cancelAllMargin">{{ t('orders.cancelPending') }}</button><button v-if="closablePositions.length" class="order-toolbar__danger" type="button" :disabled="actionId === 'margin-close-all'" @click="closeAllMargin">{{ actionId === 'margin-close-all' ? t('orders.closing') : t('orders.closeAll') }}</button></div></div>
          <div v-if="openedPositions.length" class="order-list"><article v-for="position in openedPositions" :key="position.id" class="order-card"><header><strong>{{ positionSymbol(position) }}</strong><span :class="position.direction === 'long' ? 'buy-tag' : 'sell-tag'">{{ position.direction === 'long' ? t('orders.long') : t('orders.short') }} {{ position.leverage }}x</span></header><dl><div><dt>{{ t('orders.margin') }}</dt><dd>{{ formatAmount(position.marginAmount) }}</dd></div><div><dt>{{ t('orders.entryPrice') }}</dt><dd>{{ position.entryPrice > 0 ? formatPrice(position.entryPrice) : t('orders.waitingFill') }}</dd></div><div><dt>{{ position.realizedPnl >= 0 ? t('orders.realizedProfit') : t('orders.realizedLoss') }}</dt><dd :class="position.realizedPnl >= 0 ? 'up' : 'down'">{{ formatAmount(position.realizedPnl) }}</dd></div></dl><footer><small>{{ position.marginMode === 'cross' ? t('orders.cross') : t('orders.isolated') }} · {{ t('orders.notionalValue', { amount: formatAmount(position.notionalAmount) }) }}</small><button class="button" :class="position.entryPrice > 0 ? 'button--danger' : 'button--secondary'" type="button" :disabled="actionId === `margin-${position.id}`" @click="actOnPosition(position)">{{ actionId === `margin-${position.id}` ? t('orders.processing') : position.entryPrice > 0 ? t('orders.close') : t('orders.cancel') }}</button></footer></article></div><p v-else class="empty-state">{{ t('orders.noPositions') }}</p>
        </template>

        <template v-else>
          <div class="history-section"><h2>{{ t('orders.spotHistory') }}</h2><article v-for="order in sortedHistoryOrders" :key="order.id" class="history-row"><div><strong>{{ displayPair(order.symbol) }} · {{ order.side === 'buy' ? t('orders.buy') : t('orders.sell') }}</strong><small>{{ formatDateTime(order.createdAt) }}</small></div><span><b>{{ statusLabel(order.status) }}</b><small>{{ formatAmount(order.filledQuantity) }} / {{ formatAmount(order.quantity) }}</small></span></article><p v-if="!sortedHistoryOrders.length" class="empty-state">{{ t('orders.noSpotHistory') }}</p></div>
          <div class="history-section"><h2>{{ t('orders.marginHistory') }}</h2><article v-for="position in historyPositions" :key="position.id" class="history-row"><div><strong>{{ positionSymbol(position) }} · {{ position.direction === 'long' ? t('orders.long') : t('orders.short') }}</strong><small>{{ position.marginMode === 'cross' ? t('orders.cross') : t('orders.isolated') }} · {{ position.leverage }}x</small></div><span><b :class="position.realizedPnl >= 0 ? 'up' : 'down'">{{ formatAmount(position.realizedPnl) }}</b><small>{{ statusLabel(position.status) }}</small></span></article><p v-if="!historyPositions.length" class="empty-state">{{ t('orders.noMarginHistory') }}</p></div>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.orders-page { background: var(--background); }.orders-page .page-content { background: var(--surface); min-height: calc(100dvh - 56px); padding-bottom: 36px; }.order-tabs { border-bottom: 1px solid var(--line); display: flex; gap: 25px; margin: 0 -20px; overflow: auto; padding: 0 20px; }.order-tabs button { background: transparent; border-bottom: 2px solid transparent; color: var(--muted); flex: 0 0 auto; font-size: 15px; font-weight: 680; min-height: 50px; padding: 0; }.order-tabs .is-active { border-color: var(--ink); color: var(--ink); font-weight: 760; }.order-toolbar { align-items: center; display: flex; font-size: 13px; justify-content: space-between; min-height: 58px; }.order-toolbar > span { color: var(--muted); }.order-toolbar > button,.order-toolbar div { display: flex; gap: 8px; }.order-toolbar button { background: var(--soft); border: 1px solid var(--line); border-radius: 18px; color: var(--muted-strong); font-size: 12px; font-weight: 700; min-height: 32px; padding: 0 11px; }.order-toolbar .order-toolbar__danger { color: var(--negative); }.order-list { display: grid; gap: 10px; }.order-card { border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow-soft); padding: 14px; }.order-card header,.order-card footer { align-items: center; display: flex; justify-content: space-between; }.order-card header strong { font-size: 16px; }.buy-tag,.sell-tag { border-radius: 4px; font-size: 11px; font-weight: 720; padding: 4px 7px; }.buy-tag { background: var(--positive-soft); color: var(--positive); }.sell-tag { background: var(--negative-soft); color: var(--negative); }.order-card dl { display: grid; gap: 8px; grid-template-columns: repeat(3, 1fr); margin: 14px 0; }.order-card dl div { display: grid; gap: 4px; min-width: 0; }.order-card dt { color: var(--muted); font-size: 11px; }.order-card dd { font-size: 13px; font-variant-numeric: tabular-nums; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.order-card footer { border-top: 1px solid var(--line); padding-top: 11px; }.order-card footer small { color: var(--muted); font-size: 11px; overflow: hidden; padding-right: 8px; text-overflow: ellipsis; white-space: nowrap; }.order-card footer .button { font-size: 12px; min-height: 34px; padding: 0 11px; }.success-message { color: var(--positive); font-size: 13px; margin: 12px 0 0; }.history-section { border-top: 1px solid var(--line); }.history-section + .history-section { margin-top: 26px; }.history-section h2 { font-size: 18px; margin: 20px 0 6px; }.history-row { align-items: center; border-bottom: 1px solid var(--line); display: flex; justify-content: space-between; min-height: 63px; }.history-row div,.history-row > span { display: grid; gap: 5px; }.history-row strong,.history-row b { font-size: 13px; }.history-row small { color: var(--muted); font-size: 11px; }.history-row > span { text-align: right; }.spin { animation: spin .8s linear infinite; }@keyframes spin { to { transform: rotate(360deg); } }
</style>
