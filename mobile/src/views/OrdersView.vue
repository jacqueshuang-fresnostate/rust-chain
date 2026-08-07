<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowLeftRight, CheckCircle2, CircleAlert, ClipboardList, History, LoaderCircle, RefreshCw, X } from 'lucide-vue-next'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchMarketPairs } from '@/api/market'
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
import { formatAmount, formatPrice } from '@/core/format'
import { useNavigationStore } from '@/stores/navigation'
import { useSessionStore } from '@/stores/session'
import type { MarginProduct, MarketPair } from '@/core/types'

type MarketTab = 'spot' | 'margin'
type StateTab = 'current' | 'history' | 'positions'
type PendingAction =
  | { kind: 'spot'; order: SpotOrder }
  | { kind: 'spot-all' }
  | { kind: 'margin'; position: MarginPosition }
  | { kind: 'margin-cancel-all' }
  | { kind: 'margin-close-all' }

const route = useRoute()
const router = useRouter()
const navigation = useNavigationStore()
const session = useSessionStore()
const { t } = useI18n()
const marketTab = ref<MarketTab>('spot')
const stateTab = ref<StateTab>('current')
const marketTabs = computed(() => [
  { value: 'spot' as const, label: t('orders.spot') },
  { value: 'margin' as const, label: t('orders.marginMarket') },
])
const stateTabs = computed(() => [
  { value: 'current' as const, label: t('orders.current') },
  { value: 'history' as const, label: t('orders.historyOrdersTab') },
  { value: 'positions' as const, label: t('orders.positions') },
])
const spotOrders = ref<SpotOrder[]>([])
const historyOrders = ref<SpotOrder[]>([])
const positions = ref<MarginPosition[]>([])
const historyPositions = ref<MarginPosition[]>([])
const products = ref<MarginProduct[]>([])
const pairs = ref<MarketPair[]>([])
const loading = ref(false)
const actionId = ref('')
const feedback = ref('')
const error = ref('')
const pendingAction = ref<PendingAction | null>(null)
const confirmDialog = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''

const openedPositions = computed(() => positions.value.filter((position) => position.status === 'opened'))
const cancelablePositions = computed(() => openedPositions.value.filter((position) => position.entryPrice <= 0))
const closablePositions = computed(() => openedPositions.value.filter((position) => position.entryPrice > 0))
const sortedSpotOrders = computed(() => [...spotOrders.value].sort((left, right) => (right.createdAt || 0) - (left.createdAt || 0)))
const sortedHistoryOrders = computed(() => [...historyOrders.value].sort((left, right) => (right.createdAt || 0) - (left.createdAt || 0)))
const emptyTitle = computed(() => {
  if (marketTab.value === 'spot') return t(stateTab.value === 'history' ? 'orders.noSpotHistory' : 'orders.noSpotOrders')
  if (stateTab.value === 'history') return t('orders.noMarginHistory')
  return t(stateTab.value === 'positions' ? 'orders.noPositions' : 'orders.noMarginOrders')
})
const pendingActionLabel = computed(() => {
  const action = pendingAction.value
  if (!action) return ''
  if (action.kind === 'spot') return t('orders.cancel')
  if (action.kind === 'spot-all') return t('orders.cancelAll')
  if (action.kind === 'margin') return action.position.entryPrice > 0 ? t('orders.close') : t('orders.cancel')
  if (action.kind === 'margin-cancel-all') return t('orders.cancelPending')
  return t('orders.closeAll')
})
const pendingActionSummary = computed(() => {
  const action = pendingAction.value
  if (!action) return ''
  if (action.kind === 'spot') {
    return `${displayPair(action.order.symbol)} · ${formatAmount(action.order.quantity)}`
  }
  if (action.kind === 'spot-all') {
    return t('orders.currentOrders', { count: sortedSpotOrders.value.length })
  }
  if (action.kind === 'margin') {
    return `${positionSymbol(action.position)} · ${action.position.leverage}x`
  }
  const count = action.kind === 'margin-cancel-all'
    ? cancelablePositions.value.length
    : closablePositions.value.length
  return t('orders.currentPositions', { count })
})

function productFor(position: MarginPosition): MarginProduct | undefined {
  return products.value.find((product) => product.id === position.productId || product.pairId === position.pairId)
}

function positionSymbol(position: MarginPosition): string {
  const product = productFor(position)
  if (product) return product.symbol
  const pair = pairs.value.find((candidate) => candidate.id === position.pairId)
  return pair?.symbol || t('orders.contractNumber', { id: position.productId })
}

function displayPair(symbol: string): string {
  return symbol.replace(/_/g, '/').replace(/-/g, '/')
}

function setMarketTab(tab: MarketTab): void {
  if (marketTab.value === tab) {
    void load()
    return
  }
  marketTab.value = tab
  if (tab === 'spot' && stateTab.value === 'positions') stateTab.value = 'current'
}

function setStateTab(tab: StateTab): void {
  if (stateTab.value === tab) {
    void load()
    return
  }
  stateTab.value = tab
}

function openHistory(): void {
  setStateTab('history')
}

function openSpotTrade(): void {
  void router.push({ name: 'trade', params: { symbol: navigation.lastTradeSymbol } })
}

function baseAsset(symbol: string): string {
  return displayPair(symbol).split('/')[0] || symbol
}

function quoteAsset(symbol: string): string {
  return displayPair(symbol).split('/')[1] || ''
}

function spotOrderTypeLabel(order: SpotOrder): string {
  return t(order.orderType === 'market' ? 'trade.marketOrderShort' : 'trade.limitOrderShort')
}

function spotOrderPriceLabel(order: SpotOrder): string {
  return order.price > 0 ? formatPrice(order.price) : t('orders.marketPrice')
}

function currentOrderStatusLabel(status: string): string {
  if (['submitted', 'pending', 'trading', 'open'].includes(status.trim().toLowerCase())) return t('orders.waitingFill')
  return statusLabel(status)
}

function positionDirectionLabel(position: MarginPosition): string {
  return t(position.direction === 'long' ? 'trade.openLong' : 'trade.openShort')
}

function currentPositionStatusLabel(position: MarginPosition): string {
  if (position.status.trim().toLowerCase() === 'opened') {
    return position.entryPrice > 0 ? t('orders.statusHolding') : t('orders.waitingFill')
  }
  return currentOrderStatusLabel(position.status)
}

function positionAmount(position: MarginPosition): string {
  const symbol = positionSymbol(position)
  const asset = baseAsset(symbol)
  if (position.entryPrice > 0 && position.notionalAmount > 0) {
    return `${formatAmount(position.notionalAmount / position.entryPrice)} ${asset}`
  }
  const product = productFor(position)
  return `${formatAmount(position.marginAmount)} ${product?.marginAssetSymbol || quoteAsset(symbol)}`.trim()
}

function statusTone(status: string): 'positive' | 'negative' | 'info' {
  const normalized = status.trim().toLowerCase()
  if (['completed', 'filled', 'closed'].includes(normalized)) return 'positive'
  if (['canceled', 'cancelled', 'liquidated', 'rejected'].includes(normalized)) return 'negative'
  return 'info'
}

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  feedback.value = ''
  error.value = ''
  try {
    if (marketTab.value === 'spot') {
      if (stateTab.value === 'current') spotOrders.value = await fetchOpenSpotOrders()
      else if (stateTab.value === 'history') historyOrders.value = await fetchSpotOrderHistory()
      return
    }
    if (stateTab.value !== 'history') {
      const [nextPositions, nextProducts, nextPairs] = await Promise.all([
        fetchMarginPositions('opened'),
        fetchMarginProducts(),
        fetchMarketPairs(),
      ])
      positions.value = nextPositions
      products.value = nextProducts
      pairs.value = nextPairs
      return
    }
    const [closed, liquidated, canceled, nextProducts, nextPairs] = await Promise.all([
      fetchMarginPositions('closed'),
      fetchMarginPositions('liquidated'),
      fetchMarginPositions('canceled'),
      fetchMarginProducts(),
      fetchMarketPairs(),
    ])
    historyPositions.value = [...closed, ...liquidated, ...canceled]
    products.value = nextProducts
    pairs.value = nextPairs
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('orders.loadFailed'))
  } finally {
    loading.value = false
  }
}

async function cancelSpot(order: SpotOrder): Promise<boolean> {
  actionId.value = `spot-${order.id}`
  error.value = ''
  try {
    await cancelSpotOrder(order.id)
    await load()
    feedback.value = t('orders.spotCanceled')
    return true
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('orders.spotCancelFailed'))
    return false
  } finally {
    actionId.value = ''
  }
}

async function cancelAllSpot(): Promise<boolean> {
  if (!spotOrders.value.length) return false
  actionId.value = 'spot-all'
  error.value = ''
  try {
    await cancelAllSpotOrders(spotOrders.value.map((order) => order.id))
    await load()
    feedback.value = t('orders.allSpotCanceled')
    return true
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('orders.allSpotCancelFailed'))
    return false
  } finally {
    actionId.value = ''
  }
}

async function actOnPosition(position: MarginPosition): Promise<boolean> {
  const shouldCancel = position.entryPrice <= 0
  actionId.value = `margin-${position.id}`
  error.value = ''
  try {
    if (shouldCancel) await cancelMarginPosition(position.id)
    else await closeMarginPosition(position.id)
    await load()
    feedback.value = shouldCancel ? t('orders.marginCanceled') : t('orders.closeSubmitted')
    return true
  } catch (reason) {
    error.value = apiErrorMessage(reason, shouldCancel ? t('orders.marginCancelFailed') : t('orders.closeFailed'))
    return false
  } finally {
    actionId.value = ''
  }
}

async function cancelAllMargin(): Promise<boolean> {
  if (!cancelablePositions.value.length) return false
  actionId.value = 'margin-cancel-all'
  error.value = ''
  try {
    await cancelAllMarginPositions()
    await load()
    feedback.value = t('orders.allPendingCanceled')
    return true
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('orders.batchCancelFailed'))
    return false
  } finally {
    actionId.value = ''
  }
}

async function closeAllMargin(): Promise<boolean> {
  if (!closablePositions.value.length) return false
  actionId.value = 'margin-close-all'
  error.value = ''
  try {
    await closeAllMarginPositions()
    await load()
    feedback.value = t('orders.allCloseSubmitted')
    return true
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('orders.allCloseFailed'))
    return false
  } finally {
    actionId.value = ''
  }
}

function requestAction(action: PendingAction): void {
  error.value = ''
  feedback.value = ''
  pendingAction.value = action
}

function closeConfirm(): void {
  if (actionId.value) return
  pendingAction.value = null
}

async function confirmAction(): Promise<void> {
  const action = pendingAction.value
  if (!action) return
  let completed = false
  if (action.kind === 'spot') completed = await cancelSpot(action.order)
  else if (action.kind === 'spot-all') completed = await cancelAllSpot()
  else if (action.kind === 'margin') completed = await actOnPosition(action.position)
  else if (action.kind === 'margin-cancel-all') completed = await cancelAllMargin()
  else completed = await closeAllMargin()
  if (completed) pendingAction.value = null
}

function trapDialogFocus(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    closeConfirm()
    return
  }
  if (event.key !== 'Tab' || !confirmDialog.value) return
  const focusable = Array.from(confirmDialog.value.querySelectorAll<HTMLElement>(
    'button:not([disabled]), [tabindex]:not([tabindex="-1"])',
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

watch([marketTab, stateTab], () => { void load() })
watch(pendingAction, async (action) => {
  if (action) {
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
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

onMounted(() => {
  if (route.query.tab === 'positions') {
    marketTab.value = 'margin'
    stateTab.value = 'positions'
  } else if (route.query.tab === 'history') {
    stateTab.value = 'history'
  } else if (route.query.tab === 'margin') {
    marketTab.value = 'margin'
  }
  void load()
})

onBeforeUnmount(() => {
  document.body.style.overflow = previousBodyOverflow
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
  <main
    class="page pencil-page pencil-root-page orders-pencil"
    data-orders-workspace="live"
    data-pencil-source="kcP5D A85if n6oGO t2GTW4 e5Qs1 hxe8l"
  >
    <PageHeader :back="false" :pencil="true" :title="t('orders.titleShort')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('orders.historyOrdersTab')" @click="openHistory">
          <History :size="19" />
        </button>
      </template>
    </PageHeader>

    <div class="pencil-content orders-pencil__content">
      <nav class="pencil-segmented orders-market-tabs" :aria-label="t('orders.category')">
        <button
          v-for="item in marketTabs"
          :key="item.value"
          type="button"
          :aria-pressed="marketTab === item.value"
          @click="setMarketTab(item.value)"
        >
          {{ item.label }}
        </button>
      </nav>
      <nav class="pencil-segmented orders-state-tabs" :aria-label="t('orders.stateCategory')">
        <button
          v-for="item in stateTabs"
          :key="item.value"
          type="button"
          :disabled="marketTab === 'spot' && item.value === 'positions'"
          :aria-pressed="stateTab === item.value"
          @click="setStateTab(item.value)"
        >
          {{ item.label }}
        </button>
      </nav>

      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="orders-login-state"
        :description="t('orders.loginDescription')"
      />
      <template v-else>
        <div v-if="error" class="pencil-message pencil-message--error" role="alert">
          <CircleAlert :size="18" />
          <span>{{ error }}</span>
          <button type="button" :aria-label="t('orders.refresh')" @click="load"><RefreshCw :size="17" /></button>
        </div>
        <div v-if="feedback" class="pencil-message pencil-message--success" role="status">
          <CheckCircle2 :size="18" />
          <span>{{ feedback }}</span>
        </div>

        <div v-if="loading" class="pencil-state orders-loading" aria-live="polite">
          <LoaderCircle :size="24" class="spin" />
          <span>{{ t('orders.loading') }}</span>
        </div>

        <template v-else-if="marketTab === 'spot' && stateTab === 'current'">
          <div v-if="sortedSpotOrders.length" class="orders-list">
            <article v-for="order in sortedSpotOrders" :key="order.id" class="orders-row">
              <header class="orders-row__head">
                <strong>{{ displayPair(order.symbol) }}</strong>
                <span :class="order.side === 'buy' ? 'orders-side-chip' : 'orders-side-chip orders-side-chip--negative'">
                  {{ order.side === 'buy' ? t('orders.buy') : t('orders.sell') }}
                </span>
                <button
                  class="orders-row__state"
                  :class="`is-${statusTone(order.status)}`"
                  type="button"
                  :aria-label="`${t('orders.cancel')} ${displayPair(order.symbol)}`"
                  :disabled="actionId === `spot-${order.id}`"
                  @click="requestAction({ kind: 'spot', order })"
                >
                  {{ actionId === `spot-${order.id}` ? t('orders.processing') : currentOrderStatusLabel(order.status) }}
                </button>
              </header>
              <div class="orders-row__summary">
                <span>{{ spotOrderTypeLabel(order) }}{{ order.side === 'buy' ? t('orders.buy') : t('orders.sell') }}</span>
                <strong class="pencil-numeric">
                  {{ formatAmount(order.quantity) }} {{ baseAsset(order.symbol) }}
                  <small>@ {{ spotOrderPriceLabel(order) }}</small>
                </strong>
              </div>
            </article>
            <div class="orders-batch-footer">
              <span>{{ t('orders.currentOrders', { count: sortedSpotOrders.length }) }}</span>
              <button type="button" :disabled="actionId === 'spot-all'" @click="requestAction({ kind: 'spot-all' })">
                {{ actionId === 'spot-all' ? t('orders.canceling') : t('orders.cancelAll') }}
              </button>
            </div>
          </div>
          <div v-else-if="!error" class="orders-empty-branch">
            <div class="orders-empty-state" role="status">
              <span class="orders-empty-state__plate"><ClipboardList :size="24" aria-hidden="true" /></span>
              <strong>{{ emptyTitle }}</strong>
              <span>{{ t('orders.emptyDescription') }}</span>
            </div>
            <button class="orders-empty-action" type="button" @click="openSpotTrade">
              <ArrowLeftRight :size="17" aria-hidden="true" />{{ t('orders.goTrade') }}
            </button>
          </div>
        </template>

        <template v-else-if="marketTab === 'spot' && stateTab === 'history'">
          <div v-if="sortedHistoryOrders.length" class="orders-list">
            <article v-for="order in sortedHistoryOrders" :key="order.id" class="orders-row orders-row--history">
              <header class="orders-row__head">
                <strong>{{ displayPair(order.symbol) }}</strong>
                <span :class="order.side === 'buy' ? 'orders-side-chip' : 'orders-side-chip orders-side-chip--negative'">
                  {{ order.side === 'buy' ? t('orders.buy') : t('orders.sell') }}
                </span>
                <span class="orders-row__state" :class="`is-${statusTone(order.status)}`">{{ statusLabel(order.status) }}</span>
              </header>
              <div class="orders-row__summary">
                <span>{{ spotOrderTypeLabel(order) }}{{ order.side === 'buy' ? t('orders.buy') : t('orders.sell') }}</span>
                <strong class="pencil-numeric">
                  {{ formatAmount(order.filledQuantity || order.quantity) }} {{ baseAsset(order.symbol) }}
                  <small>@ {{ spotOrderPriceLabel(order) }}</small>
                </strong>
              </div>
            </article>
          </div>
          <div v-else-if="!error" class="orders-empty-branch">
            <div class="orders-empty-state" role="status">
              <span class="orders-empty-state__plate"><ClipboardList :size="24" aria-hidden="true" /></span>
              <strong>{{ emptyTitle }}</strong>
              <span>{{ t('orders.emptyDescription') }}</span>
            </div>
            <button class="orders-empty-action" type="button" @click="openSpotTrade">
              <ArrowLeftRight :size="17" aria-hidden="true" />{{ t('orders.goTrade') }}
            </button>
          </div>
        </template>

        <template v-else-if="marketTab === 'margin' && stateTab !== 'history'">
          <div v-if="(stateTab === 'positions' ? closablePositions : cancelablePositions).length" class="orders-list">
            <article
              v-for="position in (stateTab === 'positions' ? closablePositions : cancelablePositions)"
              :key="position.id"
              class="orders-row"
            >
              <header class="orders-row__head">
                <strong>{{ positionSymbol(position) }}</strong>
                <span :class="position.direction === 'long' ? 'orders-side-chip' : 'orders-side-chip orders-side-chip--negative'">
                  {{ positionDirectionLabel(position) }}
                </span>
                <button
                  class="orders-row__state"
                  :class="`is-${statusTone(position.status)}`"
                  type="button"
                  :aria-label="`${position.entryPrice > 0 ? t('orders.close') : t('orders.cancel')} ${positionSymbol(position)}`"
                  :disabled="actionId === `margin-${position.id}`"
                  @click="requestAction({ kind: 'margin', position })"
                >
                  {{ actionId === `margin-${position.id}` ? t('orders.processing') : currentPositionStatusLabel(position) }}
                </button>
              </header>
              <div class="orders-row__summary">
                <span>{{ positionDirectionLabel(position) }} · {{ position.leverage }}x · {{ position.entryPrice > 0 ? t('orders.marketPrice') : t('trade.limitOrderLabel') }}</span>
                <strong class="pencil-numeric">
                  {{ positionAmount(position) }}
                  <small>@ {{ position.entryPrice > 0 ? formatPrice(position.entryPrice) : t('orders.waitingFill') }}</small>
                </strong>
              </div>
            </article>
            <div class="orders-batch-footer">
              <span>
                {{ stateTab === 'positions'
                  ? t('orders.currentPositions', { count: closablePositions.length })
                  : t('orders.currentOrders', { count: cancelablePositions.length }) }}
              </span>
              <button
                v-if="stateTab === 'current'"
                type="button"
                :disabled="actionId === 'margin-cancel-all'"
                @click="requestAction({ kind: 'margin-cancel-all' })"
              >
                {{ t('orders.cancelPending') }}
              </button>
              <button
                v-else
                class="is-danger"
                type="button"
                :disabled="actionId === 'margin-close-all'"
                @click="requestAction({ kind: 'margin-close-all' })"
              >
                {{ actionId === 'margin-close-all' ? t('orders.closing') : t('orders.closeAll') }}
              </button>
            </div>
          </div>
          <div v-else-if="!error" class="orders-empty-branch">
            <div class="orders-empty-state" role="status">
              <span class="orders-empty-state__plate"><ClipboardList :size="24" aria-hidden="true" /></span>
              <strong>{{ emptyTitle }}</strong>
              <span>{{ t('orders.emptyDescription') }}</span>
            </div>
            <button class="orders-empty-action" type="button" @click="openSpotTrade">
              <ArrowLeftRight :size="17" aria-hidden="true" />{{ t('orders.goTrade') }}
            </button>
          </div>
        </template>

        <template v-else-if="marketTab === 'margin' && stateTab === 'history'">
          <div v-if="historyPositions.length" class="orders-list">
            <article v-for="position in historyPositions" :key="position.id" class="orders-row orders-row--history">
              <header class="orders-row__head">
                <strong>{{ positionSymbol(position) }}</strong>
                <span :class="position.direction === 'long' ? 'orders-side-chip' : 'orders-side-chip orders-side-chip--negative'">
                  {{ positionDirectionLabel(position) }}
                </span>
                <span class="orders-row__state" :class="`is-${statusTone(position.status)}`">{{ statusLabel(position.status) }}</span>
              </header>
              <div class="orders-row__summary">
                <span>{{ positionDirectionLabel(position) }} · {{ position.leverage }}x · {{ position.marginMode === 'cross' ? t('orders.cross') : t('orders.isolated') }}</span>
                <strong class="pencil-numeric">
                  {{ positionAmount(position) }}
                  <small>@ {{ position.entryPrice > 0 ? formatPrice(position.entryPrice) : t('orders.waitingFill') }}</small>
                </strong>
              </div>
            </article>
          </div>
          <div v-else-if="!error" class="orders-empty-branch">
            <div class="orders-empty-state" role="status">
              <span class="orders-empty-state__plate"><ClipboardList :size="24" aria-hidden="true" /></span>
              <strong>{{ emptyTitle }}</strong>
              <span>{{ t('orders.emptyDescription') }}</span>
            </div>
            <button class="orders-empty-action" type="button" @click="openSpotTrade">
              <ArrowLeftRight :size="17" aria-hidden="true" />{{ t('orders.goTrade') }}
            </button>
          </div>
        </template>
      </template>
    </div>

    <div v-if="pendingAction" class="orders-mask" @click.self="closeConfirm">
      <section
        ref="confirmDialog"
        class="orders-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="orders-confirm-title"
        @keydown="trapDialogFocus"
      >
        <header>
          <div>
            <strong id="orders-confirm-title">{{ pendingActionLabel }}</strong>
            <small>{{ pendingActionSummary }}</small>
          </div>
          <button class="icon-button" type="button" :aria-label="t('common.close')" :disabled="Boolean(actionId)" data-dialog-cancel @click="closeConfirm">
            <X :size="21" />
          </button>
        </header>
        <p v-if="error" class="orders-dialog__error" role="alert">{{ error }}</p>
        <div class="orders-dialog__actions">
          <button class="button button--secondary" type="button" :disabled="Boolean(actionId)" @click="closeConfirm">{{ t('common.cancel') }}</button>
          <button class="button button--danger" type="button" :disabled="Boolean(actionId)" :aria-busy="Boolean(actionId)" @click="confirmAction">
            {{ actionId ? t('orders.processing') : pendingActionLabel }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.orders-pencil {
  --pencil-root-header-margin: 4px;
}

.orders-pencil__content {
  padding-bottom: 0;
  padding-top: 4px;
}

.orders-market-tabs {
  gap: 24px;
  height: 45px;
  min-height: 45px;
  padding-top: 8px;
}

.orders-market-tabs button {
  font-size: 18px;
  font-weight: 500;
  height: 34px;
  line-height: 26px;
  min-height: 34px;
  padding-bottom: 8px;
}

.orders-market-tabs button[aria-pressed='true'] {
  color: var(--positive);
  font-weight: 500;
}

.orders-state-tabs {
  height: 34px;
  margin-top: 4px;
  min-height: 34px;
  padding-top: 4px;
}

.orders-state-tabs button {
  font-size: 13px;
  height: 26px;
  min-height: 26px;
  padding-bottom: 7px;
}

.orders-state-tabs button[aria-pressed='true'] {
  font-weight: 500;
}

.orders-state-tabs button:disabled {
  opacity: .42;
}

.orders-list,
.orders-loading,
.orders-login-state,
.orders-pencil__content > .pencil-state {
  margin-top: 4px;
}

.orders-list {
  display: grid;
}

.orders-row {
  box-sizing: border-box;
  display: grid;
  gap: 8px;
  grid-template-rows: 20px 16px;
  height: 64px;
  min-height: 64px;
  padding: 10px 0;
}

.orders-row__head {
  align-items: center;
  display: grid;
  gap: 7px;
  grid-template-columns: minmax(0, auto) auto minmax(0, 1fr);
  height: 20px;
  min-width: 0;
}

.orders-row__head > strong {
  font-size: 14px;
  font-weight: 700;
  line-height: 20px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.orders-side-chip {
  align-items: center;
  background: var(--accent-soft);
  border-radius: 4px;
  color: var(--positive);
  display: inline-flex;
  font-size: 10px;
  font-weight: 500;
  height: 20px;
  justify-content: center;
  line-height: 14px;
  padding: 0 7px;
  white-space: nowrap;
}

.orders-side-chip--negative {
  background: var(--negative-soft);
  color: var(--negative);
}

.orders-row__state {
  background: transparent;
  border: 0;
  font-size: 11px;
  font-weight: 500;
  height: 20px;
  justify-self: end;
  line-height: 20px;
  min-height: 20px;
  overflow: hidden;
  padding: 0;
  position: relative;
  text-overflow: ellipsis;
  white-space: nowrap;
}

button.orders-row__state::before {
  content: '';
  inset: -12px -8px;
  position: absolute;
}

.orders-row__state.is-info { color: var(--focus); }
.orders-row__state.is-positive { color: var(--positive); }
.orders-row__state.is-negative { color: var(--negative); }

.orders-row__summary {
  align-items: center;
  color: var(--muted);
  display: grid;
  font-size: 11px;
  gap: 10px;
  grid-template-columns: minmax(0, 1fr) auto;
  height: 16px;
  line-height: 16px;
  min-width: 0;
}

.orders-row__summary > span,
.orders-row__summary > strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.orders-row__summary > strong {
  color: var(--ink);
  font-size: 12px;
  font-weight: 650;
  text-align: right;
}

.orders-row__summary small {
  color: var(--muted);
  font-family: inherit;
  font-size: 11px;
  font-weight: 500;
  margin-left: 4px;
}

.orders-batch-footer {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 11px;
  height: 44px;
  justify-content: space-between;
}

.orders-batch-footer button {
  background: transparent;
  color: var(--positive);
  font-size: 11px;
  font-weight: 650;
  min-height: 44px;
  padding: 0;
}

.orders-batch-footer button.is-danger {
  color: var(--negative);
}

.orders-loading {
  min-height: 180px;
}

.orders-empty-branch {
  display: grid;
  gap: 0;
  padding: 0 0 20px;
}

.orders-empty-state {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  gap: 12px;
  justify-content: center;
  min-height: 225px;
  padding: 48px 20px;
  text-align: center;
}

.orders-empty-state__plate {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: 50%;
  display: flex;
  height: 56px;
  justify-content: center;
  width: 56px;
}

.orders-empty-state strong {
  color: var(--ink);
  font-size: 15px;
  font-weight: 650;
  line-height: 20px;
}

.orders-empty-state > span:last-child {
  font-size: 11px;
  line-height: 17px;
  max-width: 300px;
}

.orders-empty-action {
  align-items: center;
  background: var(--accent);
  border: 0;
  border-radius: 4px;
  color: var(--on-accent);
  display: flex;
  font-size: 13px;
  font-weight: 650;
  gap: 8px;
  height: 50px;
  justify-content: center;
  min-height: 50px;
  width: 100%;
}

.orders-mask { align-items: end; background: var(--overlay); display: grid; inset: 0; position: fixed; z-index: var(--layer-overlay); }
.orders-dialog { background: var(--surface); border-radius: 18px 18px 0 0; box-shadow: 0 -14px 36px var(--shadow); padding: 18px 16px calc(18px + env(safe-area-inset-bottom)); width: 100%; }
.orders-dialog > header { align-items: center; display: flex; justify-content: space-between; }
.orders-dialog > header div { display: grid; gap: 5px; }
.orders-dialog > header small { color: var(--muted); font-size: 11px; }
.orders-dialog__error { color: var(--negative); font-size: 12px; }
.orders-dialog__actions { display: grid; gap: 10px; grid-template-columns: 1fr 1fr; margin-top: 18px; }
@media (max-width: 340px) {
  .orders-row__summary { gap: 6px; }
  .orders-row__summary > strong { font-size: 11px; }
}
</style>
