<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { CheckCircle2, CircleAlert, LoaderCircle, PackageOpen, RefreshCw, X } from 'lucide-vue-next'
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
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { MarginProduct, MarketPair } from '@/core/types'

type Tab = 'spot' | 'margin' | 'history'
type PendingAction =
  | { kind: 'spot'; order: SpotOrder }
  | { kind: 'spot-all' }
  | { kind: 'margin'; position: MarginPosition }
  | { kind: 'margin-cancel-all' }
  | { kind: 'margin-close-all' }

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
    const [nextOrders, closed, liquidated, canceled, nextProducts, nextPairs] = await Promise.all([
      fetchSpotOrderHistory(),
      fetchMarginPositions('closed'),
      fetchMarginPositions('liquidated'),
      fetchMarginPositions('canceled'),
      fetchMarginProducts(),
      fetchMarketPairs(),
    ])
    historyOrders.value = nextOrders
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

watch(activeTab, () => { void load() })
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
  if (route.query.tab === 'positions') activeTab.value = 'margin'
  else if (route.query.tab === 'history') activeTab.value = 'history'
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
  <main class="page page--plain orders-page" data-orders-workspace="live">
    <PageHeader
      :eyebrow="t('orders.category')"
      :title="t('orders.title')"
      :subtitle="t('orders.loginDescription')"
      :back="true"
    >
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('orders.refresh')"
          :disabled="loading"
          @click="load"
        >
          <RefreshCw :size="20" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content orders-content">
      <nav class="order-tabs" :aria-label="t('orders.category')">
        <button
          v-for="item in tabs"
          :key="item.value"
          type="button"
          :aria-pressed="activeTab === item.value"
          :class="{ 'is-active': activeTab === item.value }"
          @click="setTab(item.value)"
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
        <div v-if="error" class="orders-message orders-message--error" role="alert">
          <CircleAlert :size="18" />
          <span>{{ error }}</span>
          <button type="button" :aria-label="t('orders.refresh')" @click="load">
            <RefreshCw :size="17" />
          </button>
        </div>
        <div v-if="feedback" class="orders-message orders-message--success" role="status">
          <CheckCircle2 :size="18" />
          <span>{{ feedback }}</span>
        </div>

        <div v-if="loading" class="orders-state" aria-live="polite">
          <LoaderCircle :size="24" class="spin" />
          <span>{{ t('orders.loading') }}</span>
        </div>

        <template v-else-if="activeTab === 'spot'">
          <div class="order-toolbar">
            <span>{{ t('orders.currentOrders', { count: sortedSpotOrders.length }) }}</span>
            <button
              v-if="sortedSpotOrders.length"
              type="button"
              :disabled="actionId === 'spot-all'"
              @click="requestAction({ kind: 'spot-all' })"
            >
              {{ actionId === 'spot-all' ? t('orders.canceling') : t('orders.cancelAll') }}
            </button>
          </div>
          <div v-if="sortedSpotOrders.length" class="order-list">
            <article v-for="order in sortedSpotOrders" :key="order.id" class="order-card">
              <header>
                <strong>{{ displayPair(order.symbol) }}</strong>
                <span :class="order.side === 'buy' ? 'buy-tag' : 'sell-tag'">
                  {{ order.side === 'buy' ? t('orders.buy') : t('orders.sell') }}
                </span>
              </header>
              <dl>
                <div><dt>{{ t('orders.orderPrice') }}</dt><dd>{{ order.orderType === 'market' ? t('orders.marketPrice') : formatPrice(order.price) }}</dd></div>
                <div><dt>{{ t('orders.orderQuantity') }}</dt><dd>{{ formatAmount(order.quantity) }}</dd></div>
                <div><dt>{{ t('orders.filled') }}</dt><dd>{{ formatAmount(order.filledQuantity) }}</dd></div>
              </dl>
              <footer>
                <small>{{ formatDateTime(order.createdAt) }}</small>
                <button
                  class="button button--secondary"
                  type="button"
                  :disabled="actionId === `spot-${order.id}`"
                  @click="requestAction({ kind: 'spot', order })"
                >
                  {{ actionId === `spot-${order.id}` ? t('orders.processing') : t('orders.cancel') }}
                </button>
              </footer>
            </article>
          </div>
          <div v-else class="orders-state orders-state--empty">
            <PackageOpen :size="23" />
            <span>{{ t('orders.noSpotOrders') }}</span>
          </div>
        </template>

        <template v-else-if="activeTab === 'margin'">
          <div class="order-toolbar">
            <span>{{ t('orders.currentPositions', { count: openedPositions.length }) }}</span>
            <div>
              <button
                v-if="cancelablePositions.length"
                type="button"
                :disabled="actionId === 'margin-cancel-all'"
                @click="requestAction({ kind: 'margin-cancel-all' })"
              >
                {{ t('orders.cancelPending') }}
              </button>
              <button
                v-if="closablePositions.length"
                class="order-toolbar__danger"
                type="button"
                :disabled="actionId === 'margin-close-all'"
                @click="requestAction({ kind: 'margin-close-all' })"
              >
                {{ actionId === 'margin-close-all' ? t('orders.closing') : t('orders.closeAll') }}
              </button>
            </div>
          </div>
          <div v-if="openedPositions.length" class="order-list">
            <article v-for="position in openedPositions" :key="position.id" class="order-card">
              <header>
                <strong>{{ positionSymbol(position) }}</strong>
                <span :class="position.direction === 'long' ? 'buy-tag' : 'sell-tag'">
                  {{ position.direction === 'long' ? t('orders.long') : t('orders.short') }} {{ position.leverage }}x
                </span>
              </header>
              <dl>
                <div><dt>{{ t('orders.margin') }}</dt><dd>{{ formatAmount(position.marginAmount) }}</dd></div>
                <div><dt>{{ t('orders.entryPrice') }}</dt><dd>{{ position.entryPrice > 0 ? formatPrice(position.entryPrice) : t('orders.waitingFill') }}</dd></div>
                <div><dt>{{ position.realizedPnl >= 0 ? t('orders.realizedProfit') : t('orders.realizedLoss') }}</dt><dd :class="position.realizedPnl >= 0 ? 'up' : 'down'">{{ formatAmount(position.realizedPnl) }}</dd></div>
              </dl>
              <footer>
                <small>{{ position.marginMode === 'cross' ? t('orders.cross') : t('orders.isolated') }} · {{ t('orders.notionalValue', { amount: formatAmount(position.notionalAmount) }) }}</small>
                <button
                  class="button"
                  :class="position.entryPrice > 0 ? 'button--danger' : 'button--secondary'"
                  type="button"
                  :disabled="actionId === `margin-${position.id}`"
                  @click="requestAction({ kind: 'margin', position })"
                >
                  {{ actionId === `margin-${position.id}` ? t('orders.processing') : position.entryPrice > 0 ? t('orders.close') : t('orders.cancel') }}
                </button>
              </footer>
            </article>
          </div>
          <div v-else class="orders-state orders-state--empty">
            <PackageOpen :size="23" />
            <span>{{ t('orders.noPositions') }}</span>
          </div>
        </template>

        <template v-else>
          <section class="history-section">
            <h2>{{ t('orders.spotHistory') }}</h2>
            <article v-for="order in sortedHistoryOrders" :key="order.id" class="history-row">
              <div><strong>{{ displayPair(order.symbol) }} · {{ order.side === 'buy' ? t('orders.buy') : t('orders.sell') }}</strong><small>{{ formatDateTime(order.createdAt) }}</small></div>
              <span><b>{{ statusLabel(order.status) }}</b><small>{{ formatAmount(order.filledQuantity) }} / {{ formatAmount(order.quantity) }}</small></span>
            </article>
            <div v-if="!sortedHistoryOrders.length" class="orders-state orders-state--empty">
              <PackageOpen :size="22" />
              <span>{{ t('orders.noSpotHistory') }}</span>
            </div>
          </section>
          <section class="history-section">
            <h2>{{ t('orders.marginHistory') }}</h2>
            <article v-for="position in historyPositions" :key="position.id" class="history-row">
              <div><strong>{{ positionSymbol(position) }} · {{ position.direction === 'long' ? t('orders.long') : t('orders.short') }}</strong><small>{{ position.marginMode === 'cross' ? t('orders.cross') : t('orders.isolated') }} · {{ position.leverage }}x</small></div>
              <span><b :class="position.realizedPnl >= 0 ? 'up' : 'down'">{{ formatAmount(position.realizedPnl) }}</b><small>{{ statusLabel(position.status) }}</small></span>
            </article>
            <div v-if="!historyPositions.length" class="orders-state orders-state--empty">
              <PackageOpen :size="22" />
              <span>{{ t('orders.noMarginHistory') }}</span>
            </div>
          </section>
        </template>
      </template>
    </div>

    <div
      v-if="pendingAction"
      class="orders-mask"
      @click.self="closeConfirm"
    >
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
          <button
            class="icon-button"
            type="button"
            :aria-label="t('common.close')"
            :disabled="Boolean(actionId)"
            data-dialog-cancel
            @click="closeConfirm"
          >
            <X :size="21" />
          </button>
        </header>
        <p v-if="error" class="orders-dialog__error" role="alert">{{ error }}</p>
        <div class="orders-dialog__actions">
          <button
            class="button button--secondary"
            type="button"
            :disabled="Boolean(actionId)"
            @click="closeConfirm"
          >
            {{ t('common.cancel') }}
          </button>
          <button
            class="button button--danger"
            type="button"
            :disabled="Boolean(actionId)"
            :aria-busy="Boolean(actionId)"
            @click="confirmAction"
          >
            {{ actionId ? t('orders.processing') : pendingActionLabel }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.orders-page {
  background: var(--surface);
  min-width: 0;
}

.orders-content {
  min-width: 0;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
  padding-top: 0;
}

.order-tabs {
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0 -16px;
  padding: 0 16px;
}

.order-tabs button {
  background: transparent;
  border-bottom: 3px solid transparent;
  color: var(--muted);
  font-size: 13px;
  font-weight: 700;
  min-height: 50px;
  padding: 0 6px;
}

.order-tabs button.is-active {
  border-color: var(--accent);
  color: var(--ink);
}

.orders-login-state {
  margin-top: 12px;
}

.orders-message {
  align-items: center;
  border: 1px solid currentColor;
  display: grid;
  font-size: 12px;
  gap: 9px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  line-height: 1.45;
  margin-top: 14px;
  min-height: 52px;
  padding: 4px 5px 4px 11px;
}

.orders-message--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.orders-message--success {
  background: var(--positive-soft);
  color: var(--positive);
  grid-template-columns: auto minmax(0, 1fr);
}

.orders-message button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.orders-state {
  align-content: center;
  color: var(--muted);
  display: grid;
  font-size: 12px;
  gap: 9px;
  justify-items: center;
  min-height: 150px;
  text-align: center;
}

.orders-state--empty {
  min-height: 120px;
}

.order-toolbar {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  font-size: 12px;
  gap: 10px;
  justify-content: space-between;
  min-height: 60px;
}

.order-toolbar > span {
  color: var(--muted);
  font-weight: 650;
}

.order-toolbar > div {
  display: flex;
  gap: 7px;
}

.order-toolbar button {
  background: var(--soft);
  border: 1px solid var(--line);
  color: var(--muted-strong);
  font-size: 11px;
  font-weight: 750;
  min-height: 44px;
  padding: 0 11px;
}

.order-toolbar button:focus-visible,
.order-toolbar button:not(:disabled):hover {
  border-color: var(--focus);
}

.order-toolbar .order-toolbar__danger {
  background: var(--negative-soft);
  border-color: color-mix(in srgb, var(--negative) 28%, var(--line));
  color: var(--negative);
}

.order-list {
  border-bottom: 1px solid var(--line);
  display: grid;
}

.order-card {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 9px;
  padding: 12px 0;
}

.order-card:last-child {
  border-bottom: 0;
}

.order-card header,
.order-card footer {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.order-card header strong {
  font-size: 14px;
}

.buy-tag,
.sell-tag {
  border: 1px solid currentColor;
  font-size: 10px;
  font-weight: 800;
  padding: 4px 7px;
}

.buy-tag {
  background: var(--positive-soft);
  color: var(--positive);
}

.sell-tag {
  background: var(--negative-soft);
  color: var(--negative);
}

.order-card dl {
  border-block: 1px solid var(--line);
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0;
}

.order-card dl > div {
  display: grid;
  gap: 4px;
  min-width: 0;
  padding: 8px 7px;
}

.order-card dl > div + div {
  border-left: 1px solid var(--line);
}

.order-card dt {
  color: var(--muted);
  font-size: 10px;
}

.order-card dd {
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  font-weight: 700;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.order-card footer small {
  color: var(--muted);
  font-size: 10px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.order-card footer .button {
  border-radius: 0;
  flex: 0 0 auto;
  font-size: 11px;
  min-height: 44px;
  min-width: 90px;
  padding: 0 12px;
}

.history-section {
  border-top: 8px solid var(--soft);
  margin: 10px -16px 0;
  padding: 0 16px;
}

.history-section + .history-section {
  margin-top: 0;
}

.history-section h2 {
  border-bottom: 1px solid var(--line);
  font-size: 15px;
  margin: 0;
  min-height: 52px;
  padding-top: 17px;
}

.history-row {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 14px;
  justify-content: space-between;
  min-height: 68px;
}

.history-row div,
.history-row > span {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.history-row strong,
.history-row b {
  font-size: 12px;
  overflow-wrap: anywhere;
}

.history-row small {
  color: var(--muted);
  font-size: 10px;
}

.history-row > span {
  flex: 0 0 auto;
  text-align: right;
}

.orders-mask {
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

.orders-dialog {
  background: var(--surface);
  border: 1px solid var(--line);
  border-top: 3px solid var(--negative);
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

.orders-dialog > header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
}

.orders-dialog > header > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.orders-dialog > header strong {
  font-size: 18px;
}

.orders-dialog > header small {
  color: var(--muted);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.orders-dialog__error {
  background: var(--negative-soft);
  border-left: 3px solid var(--negative);
  color: var(--negative);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
  padding: 8px 10px;
}

.orders-dialog__actions {
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, .8fr) minmax(0, 1.2fr);
}

.orders-dialog__actions .button {
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
  .orders-content {
    padding-left: 14px;
    padding-right: 14px;
  }

  .order-tabs,
  .history-section {
    margin-left: -14px;
    margin-right: -14px;
    padding-inline: 14px;
  }
}

@media (max-width: 340px) {
  .order-tabs button {
    font-size: 11px;
    padding-inline: 2px;
  }

  .order-toolbar {
    align-items: stretch;
    flex-direction: column;
    padding: 9px 0;
  }

  .order-toolbar > div,
  .order-toolbar > button {
    align-self: stretch;
  }

  .order-toolbar button {
    flex: 1;
  }

  .order-card dl > div {
    padding-inline: 4px;
  }

  .order-card dt {
    font-size: 9px;
  }

  .order-card dd {
    font-size: 10px;
  }

  .orders-dialog__actions {
    grid-template-columns: 1fr;
  }
}
</style>
