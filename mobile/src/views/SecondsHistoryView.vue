<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft, CircleAlert, FileClock, LoaderCircle, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchSecondsOrders, type SecondsOrder } from '@/api/seconds'
import { formatAmount, formatPrice } from '@/core/format'
import { goBackOr } from '@/core/navigation'
import {
  createSecondsHistoryRequestLifecycle,
  filterSecondsHistoryOrdersByDirection,
  historicalSecondsOrders,
  secondsOrderProfitLossPresentation,
  secondsOrderStatusPresentation,
  type SecondsHistoryDirectionFilter,
} from '@/core/secondsOrder'
import { currentIntlLocale } from '@/i18n'
import { useSessionStore } from '@/stores/session'

const HISTORY_DIRECTION_FILTERS: Array<{ value: SecondsHistoryDirectionFilter; labelKey: string }> = [
  { value: 'all', labelKey: 'seconds.historyFilterAll' },
  { value: 'up', labelKey: 'seconds.bullish' },
  { value: 'down', labelKey: 'seconds.bearish' },
]

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const { t } = useI18n()
const orders = ref<SecondsOrder[]>([])
const activeDirection = ref<SecondsHistoryDirectionFilter>('all')
const loading = ref(session.isAuthenticated)
const error = ref('')
const requestLifecycle = createSecondsHistoryRequestLifecycle({
  isAuthenticated: () => session.isAuthenticated,
  fetchOrders: fetchSecondsOrders,
})

const historyOrders = computed(() => historicalSecondsOrders(orders.value))
const filteredHistoryOrders = computed(() => (
  filterSecondsHistoryOrdersByDirection(historyOrders.value, activeDirection.value)
))

const historyState = computed(() => {
  if (!session.isAuthenticated) return 'guest'
  if (loading.value) return 'loading'
  if (error.value) return 'error'
  if (!historyOrders.value.length) return 'empty'
  return filteredHistoryOrders.value.length ? 'list' : 'filtered-empty'
})

async function load(): Promise<void> {
  loading.value = session.isAuthenticated
  error.value = ''
  const result = await requestLifecycle.load()
  if (result.state === 'stale') return
  if (result.state === 'guest') {
    orders.value = []
    loading.value = false
    return
  }
  if (result.state === 'loaded') orders.value = result.orders
  else error.value = apiErrorMessage(result.error, t('seconds.historyLoadFailed'))
  loading.value = false
}

function closeHistory(): void {
  void goBackOr(router, route.meta.backFallback || '/seconds')
}

function historyOrderStatusPresentation(order: SecondsOrder) {
  return secondsOrderStatusPresentation({ status: order.status })
}

function orderStatusLabel(order: SecondsOrder): string {
  const presentation = historyOrderStatusPresentation(order)
  return presentation.translationKey ? t(presentation.translationKey) : presentation.source
}

function orderStatusTone(order: SecondsOrder): string {
  return `is-${historyOrderStatusPresentation(order).tone}`
}

function orderProfitLossTitle(order: SecondsOrder): string {
  return t(secondsOrderProfitLossPresentation(order).translationKey)
}

function orderProfitLossAmount(order: SecondsOrder): string {
  const presentation = secondsOrderProfitLossPresentation(order)
  if (presentation.amount === undefined) return '--'
  const sign = presentation.amount > 0 ? '+' : ''
  return `${sign}${formatAmount(presentation.amount)} ${order.stakeAssetSymbol}`
}

function orderProfitLossTone(order: SecondsOrder): string {
  return `is-${secondsOrderProfitLossPresentation(order).tone}`
}

function formatHistoryTime(value: unknown): string {
  const timestamp = Number(value)
  if (!Number.isFinite(timestamp) || timestamp <= 0) return '--'
  const normalized = timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
  const parts = new Intl.DateTimeFormat(currentIntlLocale(), {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23',
  }).formatToParts(new Date(normalized))
  const read = (type: string): string => parts.find((part) => part.type === type)?.value || ''
  const month = read('month')
  const day = read('day')
  const hour = read('hour')
  const minute = read('minute')
  return month && day && hour && minute ? `${month}/${day} ${hour}:${minute}` : '--'
}

watch(() => session.isAuthenticated, (authenticated) => {
  requestLifecycle.invalidate()
  if (authenticated) {
    void load()
    return
  }
  orders.value = []
  activeDirection.value = 'all'
  loading.value = false
  error.value = ''
})

onMounted(() => { void load() })
onBeforeUnmount(() => requestLifecycle.stop())
</script>

<template>
  <main
    class="page page--plain pencil-page seconds-page seconds-history-page"
    data-pencil-source="vZy6U x29z7"
    data-seconds-history="dedicated"
    data-responsive-range="320-448"
    :data-history-state="historyState"
    :aria-busy="session.isAuthenticated && loading"
  >
    <header class="seconds-history-header">
      <button
        class="seconds-history-back"
        type="button"
        :aria-label="t('common.back')"
        @click="closeHistory"
      >
        <ArrowLeft :size="24" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <h1>{{ t('seconds.historyPageTitle') }}</h1>
    </header>

    <nav
      class="seconds-history-filters"
      role="group"
      :aria-label="t('seconds.historyDirectionFilter')"
    >
      <button
        v-for="filter in HISTORY_DIRECTION_FILTERS"
        :key="filter.value"
        class="seconds-history-filter"
        :class="{ 'is-active': activeDirection === filter.value }"
        type="button"
        :aria-pressed="activeDirection === filter.value"
        @click="activeDirection = filter.value"
      >
        <span class="seconds-history-filter__surface">{{ t(filter.labelKey) }}</span>
      </button>
    </nav>

    <div class="seconds-history-content">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="seconds-history-login"
        :description="t('seconds.historyLoginDescription')"
      />

      <template v-else>
        <section v-if="loading" class="seconds-history-state" role="status">
          <span class="seconds-history-state__plate">
            <LoaderCircle :size="22" class="spin" aria-hidden="true" />
          </span>
          <strong>{{ t('seconds.historyLoading') }}</strong>
        </section>

        <section v-else-if="error" class="seconds-history-state seconds-history-state--error" role="alert">
          <span class="seconds-history-state__plate">
            <CircleAlert :size="22" aria-hidden="true" />
          </span>
          <strong>{{ t('common.serviceUnavailable') }}</strong>
          <p>{{ error }}</p>
          <button type="button" :disabled="loading" @click="load">
            <RefreshCw :size="17" aria-hidden="true" />
            <span>{{ t('common.retry') }}</span>
          </button>
        </section>

        <section v-else-if="filteredHistoryOrders.length" class="seconds-history-list" role="list">
          <article
            v-for="order in filteredHistoryOrders"
            :key="order.id"
            class="seconds-history-order"
            data-history-order="real"
            data-settlement-source="api-only"
            role="listitem"
          >
            <header class="seconds-history-order__header">
              <strong
                class="seconds-history-order__identity"
                :title="`${order.symbol} · ${t('seconds.historyDuration', { seconds: order.durationSeconds })}`"
              >
                {{ `${order.symbol} · ${t('seconds.historyDuration', { seconds: order.durationSeconds })}` }}
              </strong>
              <b
                class="seconds-history-order__profit-loss"
                :class="orderProfitLossTone(order)"
                :title="orderProfitLossAmount(order)"
              >
                <span class="sr-only">{{ orderProfitLossTitle(order) }}</span>
                <span>{{ orderProfitLossAmount(order) }}</span>
              </b>
            </header>

            <div class="seconds-history-order__meta">
              <strong class="seconds-history-order__direction" :class="order.direction">
                {{ t(order.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}
              </strong>
              <span class="seconds-history-order__status" :class="orderStatusTone(order)">
                {{ orderStatusLabel(order) }}
              </span>
              <time class="seconds-history-order__time">
                {{ formatHistoryTime(order.createdAt) }}
              </time>
            </div>

            <footer class="seconds-history-order__summary">
              <span class="seconds-history-order__summary-item">
                <span>{{ t('seconds.historyStake') }}</span>
                <strong :title="`${formatAmount(order.stakeAmount)} ${order.stakeAssetSymbol}`">
                  {{ formatAmount(order.stakeAmount) }} {{ order.stakeAssetSymbol }}
                </strong>
              </span>
              <i aria-hidden="true">·</i>
              <span class="seconds-history-order__summary-item">
                <span>{{ t('seconds.historyEntryPrice') }}</span>
                <strong :title="order.entryPrice !== undefined ? formatPrice(order.entryPrice) : '--'">
                  {{ order.entryPrice !== undefined ? formatPrice(order.entryPrice) : '--' }}
                </strong>
              </span>
              <i aria-hidden="true">·</i>
              <span class="seconds-history-order__summary-item">
                <span>{{ t('seconds.historySettlementPrice') }}</span>
                <strong :title="order.settlementPrice !== undefined ? formatPrice(order.settlementPrice) : '--'">
                  {{ order.settlementPrice !== undefined ? formatPrice(order.settlementPrice) : '--' }}
                </strong>
              </span>
            </footer>
          </article>
        </section>

        <section
          v-else-if="historyOrders.length"
          class="seconds-history-state seconds-history-state--filtered"
          role="status"
        >
          <span class="seconds-history-state__plate">
            <FileClock :size="22" aria-hidden="true" />
          </span>
          <strong>{{ t('seconds.historyFilterEmptyTitle') }}</strong>
          <p>{{ t('seconds.historyFilterEmptyDescription') }}</p>
        </section>

        <section v-else class="seconds-history-state seconds-history-state--empty" role="status">
          <span class="seconds-history-state__plate">
            <FileClock :size="22" aria-hidden="true" />
          </span>
          <strong>{{ t('seconds.historyEmptyTitle') }}</strong>
          <p>{{ t('seconds.historyEmptyDescription') }}</p>
        </section>
      </template>
    </div>
  </main>
</template>

<style scoped>
.page.seconds-history-page {
  --history-page-inset-left: max(16px, env(safe-area-inset-left));
  --history-page-inset-right: max(16px, env(safe-area-inset-right));
  align-content: start;
  background: var(--history-canvas);
  box-sizing: border-box;
  color: var(--history-text);
  display: grid;
  font-family: "Noto Sans SC", "PingFang SC", "Hiragino Sans GB", sans-serif;
  gap: 14px;
  grid-template-columns: minmax(0, 1fr);
  min-height: 100dvh;
  min-width: 0;
  overflow-x: clip;
  padding: calc(16px + env(safe-area-inset-top)) var(--history-page-inset-right) calc(16px + env(safe-area-inset-bottom)) var(--history-page-inset-left);
}

.seconds-history-header {
  align-items: center;
  display: flex;
  height: 52px;
  justify-content: space-between;
  min-height: 52px;
  min-width: 0;
}

.seconds-history-header h1 {
  color: var(--history-header-text);
  font-size: 24px;
  font-weight: 700;
  line-height: 34px;
  margin: 0;
  max-width: calc(100% - 52px);
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-history-back {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 12px;
  color: var(--history-back);
  display: grid;
  flex: 0 0 44px;
  height: 44px;
  min-height: 44px;
  padding: 0;
  place-items: center start;
  width: 44px;
}

.seconds-history-back:not(:disabled):active,
.seconds-history-filter:not(:disabled):active {
  transform: none;
}

.seconds-history-back:focus-visible,
.seconds-history-filter:focus-visible,
.seconds-history-state--error button:focus-visible {
  box-shadow: 0 0 0 3px var(--focus-ring);
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.seconds-history-filters {
  align-items: flex-start;
  display: flex;
  gap: 8px;
  height: 38px;
  justify-content: flex-start;
  min-height: 38px;
  min-width: 0;
}

.seconds-history-filter {
  align-items: flex-start;
  background: transparent;
  border: 0;
  border-radius: 16px;
  color: var(--history-filter-inactive-text);
  display: flex;
  flex: 0 0 auto;
  font-size: 13px;
  font-weight: 400;
  height: 44px;
  justify-content: center;
  line-height: 18px;
  min-height: 44px;
  min-width: 59px;
  padding: 0;
  position: relative;
  width: fit-content;
}

.seconds-history-filter__surface {
  align-items: center;
  background: var(--history-filter-inactive);
  border-radius: 16px;
  box-sizing: border-box;
  display: flex;
  height: 33px;
  justify-content: center;
  min-width: 59px;
  padding: 7px 16px;
  white-space: nowrap;
}

.seconds-history-filter.is-active {
  color: var(--history-positive);
  font-weight: 600;
}

.seconds-history-filter.is-active .seconds-history-filter__surface {
  background: var(--history-filter-active);
}

.seconds-history-content {
  min-width: 0;
}

.seconds-history-list {
  display: grid;
  gap: 14px;
  margin-left: calc(0px - var(--history-page-inset-left));
  margin-right: calc(0px - var(--history-page-inset-right));
  min-width: 0;
  width: auto;
}

.seconds-history-order {
  align-content: start;
  background: var(--history-card);
  border: 0;
  border-radius: 0;
  box-sizing: border-box;
  box-shadow: none;
  display: grid;
  gap: 8px;
  grid-template-rows: 23px 19px 17px;
  height: 142px;
  min-width: 0;
  overflow: hidden;
  padding: 14px 16px;
  width: 100%;
}

.seconds-history-order__header {
  align-items: center;
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(0, 1fr) minmax(82px, 42%);
  min-width: 0;
}

.seconds-history-order__identity {
  color: var(--history-text);
  display: block;
  font-size: 16px;
  font-weight: 600;
  line-height: 23px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-history-order__profit-loss {
  display: block;
  font-size: 15px;
  font-weight: 700;
  line-height: 21px;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-history-order__profit-loss.is-positive {
  color: var(--history-positive);
}

.seconds-history-order__profit-loss.is-negative {
  color: var(--history-negative);
}

.seconds-history-order__profit-loss.is-pending {
  color: var(--history-status);
}

.seconds-history-order__meta {
  align-items: center;
  display: grid;
  gap: 0;
  grid-template-columns: auto minmax(0, 1fr) auto;
  min-width: 0;
}

.seconds-history-order__direction {
  font-size: 13px;
  font-weight: 600;
  line-height: 19px;
  min-width: 27px;
  white-space: nowrap;
}

.seconds-history-order__direction.up {
  color: var(--history-positive);
}

.seconds-history-order__direction.down {
  color: var(--history-negative);
}

.seconds-history-order__status {
  color: var(--history-status);
  font-size: 13px;
  font-weight: 400;
  justify-self: center;
  line-height: 19px;
  max-width: 100%;
  min-width: 40px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-history-order__time {
  color: var(--history-time);
  font-size: 12px;
  font-weight: 400;
  line-height: 18px;
  white-space: nowrap;
  width: 65px;
}

.seconds-history-order__summary {
  align-items: center;
  color: var(--history-summary);
  display: flex;
  font-size: 12px;
  font-weight: 400;
  gap: 5px;
  height: 17px;
  line-height: 17px;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}

.seconds-history-order__summary > i {
  flex: 0 0 auto;
  font-style: normal;
}

.seconds-history-order__summary-item {
  align-items: center;
  display: inline-flex;
  flex: 0 1 auto;
  gap: 4px;
  min-width: 0;
  white-space: nowrap;
}

.seconds-history-order__summary-item > strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-history-order__summary-item > span {
  flex: 0 0 auto;
}

.seconds-history-order__summary-item > strong {
  color: inherit;
  font-weight: 400;
}

.seconds-history-state {
  align-items: center;
  background: var(--history-card);
  border-radius: 16px;
  box-sizing: border-box;
  color: var(--history-summary);
  display: flex;
  flex-direction: column;
  gap: 8px;
  justify-content: center;
  min-height: 142px;
  min-width: 0;
  padding: 16px;
  text-align: center;
}

.seconds-history-state__plate {
  align-items: center;
  background: var(--history-filter-inactive);
  border-radius: 50%;
  color: var(--history-status);
  display: flex;
  height: 40px;
  justify-content: center;
  width: 40px;
}

.seconds-history-state strong {
  color: var(--history-text);
  font-size: 14px;
  line-height: 20px;
}

.seconds-history-state p {
  font-size: 12px;
  line-height: 18px;
  margin: 0;
  max-width: 300px;
  overflow-wrap: anywhere;
}

.seconds-history-state--error .seconds-history-state__plate,
.seconds-history-state--error strong {
  color: var(--history-negative);
}

.seconds-history-state--error button {
  align-items: center;
  background: var(--history-filter-inactive);
  border: 0;
  border-radius: 10px;
  color: var(--history-positive);
  display: inline-flex;
  font-size: 12px;
  gap: 7px;
  justify-content: center;
  min-height: 44px;
  min-width: 96px;
  padding: 0 16px;
}

.seconds-history-login {
  background: var(--history-card);
  background-image: none;
  border: 0;
  border-radius: 16px;
  box-sizing: border-box;
  min-height: 142px;
  padding: 14px;
}

.seconds-history-login :deep(.login-required__icon) {
  background: var(--history-filter-active);
  border: 0;
  color: var(--history-positive);
}

.seconds-history-login :deep(.login-required__copy p) {
  color: var(--history-summary);
}

.seconds-history-login :deep(.button) {
  background: var(--history-filter-active);
  border-color: transparent;
  color: var(--history-positive);
  min-height: 44px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .seconds-history-header h1 {
    font-size: 22px;
  }

  .seconds-history-order__header {
    gap: 7px;
    grid-template-columns: minmax(0, 1fr) minmax(76px, 40%);
  }
}

@media (prefers-reduced-motion: reduce) {
  .seconds-history-page *,
  .seconds-history-page *::before,
  .seconds-history-page *::after {
    scroll-behavior: auto !important;
    transition: none !important;
  }

  .spin {
    animation: none;
  }
}
</style>
