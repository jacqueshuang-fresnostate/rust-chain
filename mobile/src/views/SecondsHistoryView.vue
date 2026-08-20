<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  ArrowDown,
  ArrowUp,
  CircleAlert,
  FileClock,
  LoaderCircle,
  RefreshCw,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchSecondsOrders, type SecondsOrder } from '@/api/seconds'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import {
  createSecondsHistoryRequestLifecycle,
  historicalSecondsOrders,
  secondsOrderProfitLossPresentation,
  secondsOrderStatusPresentation,
} from '@/core/secondsOrder'
import { useSessionStore } from '@/stores/session'

const session = useSessionStore()
const { t } = useI18n()
const orders = ref<SecondsOrder[]>([])
const loading = ref(session.isAuthenticated)
const error = ref('')
const requestLifecycle = createSecondsHistoryRequestLifecycle({
  isAuthenticated: () => session.isAuthenticated,
  fetchOrders: fetchSecondsOrders,
})

const historyOrders = computed(() => historicalSecondsOrders(orders.value))

const historyState = computed(() => {
  if (!session.isAuthenticated) return 'guest'
  if (loading.value) return 'loading'
  if (error.value) return 'error'
  return historyOrders.value.length ? 'list' : 'empty'
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

function orderStatusLabel(order: SecondsOrder): string {
  const presentation = secondsOrderStatusPresentation(order)
  return presentation.translationKey ? t(presentation.translationKey) : presentation.source
}

function orderStatusTone(order: SecondsOrder): string {
  return `is-${secondsOrderStatusPresentation(order).tone}`
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

watch(() => session.isAuthenticated, (authenticated) => {
  requestLifecycle.invalidate()
  if (authenticated) {
    void load()
    return
  }
  orders.value = []
  loading.value = false
  error.value = ''
})

onMounted(() => { void load() })
onBeforeUnmount(() => requestLifecycle.stop())
</script>

<template>
  <main
    class="page page--plain pencil-page seconds-page seconds-history-page"
    data-seconds-history="dedicated"
    data-responsive-range="320-448"
    :data-history-state="historyState"
    :aria-busy="session.isAuthenticated && loading"
  >
    <PageHeader
      :back="true"
      :eyebrow="t('seconds.title')"
      fallback="/seconds"
      :pencil="true"
      :subtitle="t('seconds.historyContext')"
      :title="t('seconds.historyTitle')"
    >
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('seconds.refreshHistory')"
          :aria-busy="loading"
          :disabled="loading || !session.isAuthenticated"
          @click="load"
        >
          <RefreshCw :size="18" :class="{ spin: loading }" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>

    <div class="page-content seconds-history-content">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="seconds-history-login"
        :description="t('seconds.historyLoginDescription')"
      />

      <template v-else>
        <section v-if="loading" class="seconds-history-state" role="status">
          <span class="seconds-history-state__plate">
            <LoaderCircle :size="24" class="spin" aria-hidden="true" />
          </span>
          <strong>{{ t('seconds.historyLoading') }}</strong>
        </section>

        <section v-else-if="error" class="seconds-history-state seconds-history-state--error" role="alert">
          <span class="seconds-history-state__plate">
            <CircleAlert :size="24" aria-hidden="true" />
          </span>
          <strong>{{ t('common.serviceUnavailable') }}</strong>
          <p>{{ error }}</p>
          <button type="button" :disabled="loading" @click="load">
            <RefreshCw :size="17" aria-hidden="true" />
            <span>{{ t('common.retry') }}</span>
          </button>
        </section>

        <section v-else-if="historyOrders.length" class="seconds-history-list" role="list">
          <article
            v-for="order in historyOrders"
            :key="order.id"
            class="seconds-history-order"
            data-history-order="real"
            data-settlement-source="api-only"
            role="listitem"
          >
            <header>
              <strong>{{ order.symbol }}</strong>
              <b class="seconds-history-order__status" :class="orderStatusTone(order)">
                {{ orderStatusLabel(order) }}
              </b>
            </header>
            <dl>
              <div class="seconds-history-order__profit-loss">
                <dt>{{ orderProfitLossTitle(order) }}</dt>
                <dd class="numeric" :class="orderProfitLossTone(order)">
                  {{ orderProfitLossAmount(order) }}
                </dd>
              </div>
              <div>
                <dt>{{ t('seconds.direction') }}</dt>
                <dd :class="order.direction">
                  <ArrowUp v-if="order.direction === 'up'" :size="14" aria-hidden="true" />
                  <ArrowDown v-else :size="14" aria-hidden="true" />
                  <span>{{ t(order.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}</span>
                </dd>
              </div>
              <div>
                <dt>{{ t('seconds.stakeAmount') }}</dt>
                <dd class="numeric">{{ formatAmount(order.stakeAmount) }} {{ order.stakeAssetSymbol }}</dd>
              </div>
              <div>
                <dt>{{ t('seconds.term') }}</dt>
                <dd>{{ t('seconds.duration', { seconds: order.durationSeconds }) }}</dd>
              </div>
              <div>
                <dt>{{ t('orders.entryPrice') }}</dt>
                <dd class="numeric">{{ order.entryPrice !== undefined ? formatPrice(order.entryPrice) : '--' }}</dd>
              </div>
              <div>
                <dt>{{ t('seconds.settlementPrice') }}</dt>
                <dd class="numeric">{{ order.settlementPrice !== undefined ? formatPrice(order.settlementPrice) : '--' }}</dd>
              </div>
              <div>
                <dt>{{ t('seconds.createdTime') }}</dt>
                <dd class="numeric">{{ formatDateTime(order.createdAt) }}</dd>
              </div>
            </dl>
          </article>
        </section>

        <section v-else class="seconds-history-state seconds-history-state--empty" role="status">
          <span class="seconds-history-state__plate">
            <FileClock :size="24" aria-hidden="true" />
          </span>
          <strong>{{ t('seconds.historyEmptyTitle') }}</strong>
          <p>{{ t('seconds.historyEmptyDescription') }}</p>
        </section>
      </template>
    </div>
  </main>
</template>

<style scoped>
.seconds-history-page {
  background: var(--page);
  color: var(--text);
  min-width: 0;
  overflow-x: clip;
}

.seconds-history-content {
  display: grid;
  min-width: 0;
  padding:
    8px
    max(20px, env(safe-area-inset-right))
    calc(24px + env(safe-area-inset-bottom))
    max(20px, env(safe-area-inset-left));
}

.seconds-history-list {
  display: grid;
  min-width: 0;
}

.seconds-history-order {
  border-bottom: 1px solid var(--hairline);
  display: grid;
  gap: 10px;
  min-width: 0;
  padding: 14px 0;
}

.seconds-history-order:first-child {
  border-top: 1px solid var(--hairline);
}

.seconds-history-order header {
  align-items: center;
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) minmax(0, 45%);
  min-width: 0;
}

.seconds-history-order header > strong {
  color: var(--text);
  font-size: 15px;
  font-weight: 750;
  line-height: 21px;
  min-width: 0;
  overflow-wrap: anywhere;
}

.seconds-history-order__status {
  font-size: 11px;
  font-weight: 650;
  line-height: 16px;
  max-width: 100%;
  overflow-wrap: anywhere;
  text-align: right;
}

.seconds-history-order__status.is-positive {
  color: var(--positive);
}

.seconds-history-order__status.is-negative {
  color: var(--negative);
}

.seconds-history-order__status.is-pending {
  color: var(--muted-strong);
}

.seconds-history-order dl {
  display: grid;
  gap: 8px 14px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin: 0;
  min-width: 0;
}

.seconds-history-order dl > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.seconds-history-order dl > .seconds-history-order__profit-loss {
  align-items: center;
  border-bottom: 1px solid var(--hairline);
  border-top: 1px solid var(--hairline);
  gap: 12px;
  grid-column: 1 / -1;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 48px;
  padding: 7px 0;
}

.seconds-history-order .seconds-history-order__profit-loss dt {
  color: var(--muted-strong);
  font-size: 12px;
  font-weight: 650;
}

.seconds-history-order .seconds-history-order__profit-loss dd {
  font-size: 16px;
  font-weight: 760;
  justify-content: flex-end;
  line-height: 22px;
  text-align: right;
}

.seconds-history-order__profit-loss dd.is-positive {
  color: var(--positive);
}

.seconds-history-order__profit-loss dd.is-negative {
  color: var(--negative);
}

.seconds-history-order__profit-loss dd.is-pending {
  color: var(--muted-strong);
}

.seconds-history-order dt,
.seconds-history-order dd {
  font-size: 11px;
  line-height: 16px;
  margin: 0;
  min-width: 0;
}

.seconds-history-order dt {
  color: var(--muted);
}

.seconds-history-order dd {
  align-items: center;
  color: var(--text);
  display: flex;
  font-weight: 650;
  gap: 4px;
  overflow-wrap: anywhere;
}

.seconds-history-order dd.up {
  color: var(--positive);
}

.seconds-history-order dd.down {
  color: var(--negative);
}

.seconds-history-state {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  gap: 12px;
  justify-content: center;
  min-height: 240px;
  min-width: 0;
  padding: 44px 12px;
  text-align: center;
}

.seconds-history-state__plate {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--muted);
  display: flex;
  height: 56px;
  justify-content: center;
  width: 56px;
}

.seconds-history-state strong {
  color: var(--text);
  font-size: 15px;
  line-height: 21px;
}

.seconds-history-state p {
  font-size: 11px;
  line-height: 17px;
  margin: 0;
  max-width: 300px;
  overflow-wrap: anywhere;
}

.seconds-history-state--error .seconds-history-state__plate,
.seconds-history-state--error strong {
  color: var(--negative);
}

.seconds-history-state--error button {
  align-items: center;
  background: transparent;
  border: 1px solid var(--line);
  color: var(--positive);
  display: inline-flex;
  font-size: 12px;
  gap: 7px;
  justify-content: center;
  min-height: 44px;
  min-width: 96px;
  padding: 0 16px;
}

.seconds-history-state--error button:focus-visible {
  box-shadow: 0 0 0 3px var(--focus-ring);
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.seconds-history-login {
  background: transparent;
  background-image: none;
  border: 0;
  border-top: 1px solid var(--hairline);
  gap: 10px;
  grid-template-columns: 34px minmax(0, 1fr) auto;
  min-height: 72px;
  padding: 10px 0;
}

.seconds-history-login :deep(.login-required__icon) {
  background: var(--accent-soft);
  border: 0;
  color: var(--positive);
  height: 34px;
  width: 34px;
}

.seconds-history-login :deep(.login-required__copy) {
  gap: 2px;
}

.seconds-history-login :deep(.login-required__copy strong) {
  font-size: 13px;
}

.seconds-history-login :deep(.login-required__copy p) {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.4;
}

.seconds-history-login :deep(.button) {
  min-height: 44px;
  padding-inline: 14px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .seconds-history-content {
    padding-left: max(16px, env(safe-area-inset-left));
    padding-right: max(16px, env(safe-area-inset-right));
  }

  .seconds-history-order dl {
    gap: 8px 10px;
  }

  .seconds-history-login {
    align-items: center;
    grid-template-columns: 34px minmax(0, 1fr);
  }

  .seconds-history-login :deep(.button) {
    grid-column: 2;
    justify-self: start;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
