<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  CircleAlert,
  Clock3,
  LoaderCircle,
  RefreshCw,
  X,
} from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  fetchSecondsOrders,
  fetchSecondsProducts,
  openSecondsOrder,
  type SecondsCycle,
  type SecondsOrder,
  type SecondsProduct,
} from '@/api/seconds'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const marketStore = useMarketStore()
const router = useRouter()
const { t } = useI18n()
const products = ref<SecondsProduct[]>([])
const orders = ref<SecondsOrder[]>([])
const accounts = ref<WalletAccount[]>([])
const selected = ref<SecondsProduct | null>(null)
const selectedCycleId = ref(0)
const direction = ref<'up' | 'down'>('up')
const amount = ref('')
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const success = ref('')
const confirmOpen = ref(false)
const confirmDialog = ref<HTMLElement | null>(null)
const reviewButton = ref<HTMLButtonElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''

const cycle = computed<SecondsCycle | undefined>(() => (
  selected.value?.cycles.find((item) => item.id === selectedCycleId.value)
  || selected.value?.cycles[0]
))
const account = computed(() => accounts.value.find((item) => item.assetId === selected.value?.stakeAssetId))
const selectedTicker = computed(() => marketStore.tickerFor(selected.value?.symbol || ''))
const amountNumber = computed(() => Number(amount.value || 0))
const payoutRate = computed(() => cycle.value?.payoutRate || 0)
const payoutCoefficient = computed(() => 1 + payoutRate.value)
const estimatedPayout = computed(() => (
  Number.isFinite(amountNumber.value) && amountNumber.value > 0
    ? amountNumber.value * (1 + payoutRate.value)
    : 0
))
const valid = computed(() => Boolean(
  cycle.value
  && Number.isFinite(amountNumber.value)
  && amountNumber.value >= cycle.value.minStake
  && (!cycle.value.maxStake || amountNumber.value <= cycle.value.maxStake)
  && amountNumber.value <= (account.value?.available || 0),
))
const quickAmounts = computed(() => {
  const activeCycle = cycle.value
  if (!activeCycle) return []
  const upperBound = Math.min(
    activeCycle.maxStake || Number.POSITIVE_INFINITY,
    account.value?.available || 0,
  )
  return [...new Set([
    activeCycle.minStake,
    activeCycle.minStake * 2,
    activeCycle.minStake * 5,
    upperBound,
  ])]
    .filter((value) => Number.isFinite(value) && value >= activeCycle.minStake && value <= upperBound)
    .slice(0, 4)
})
const quickAmountSlots = computed(() => (
  quickAmounts.value.length ? quickAmounts.value : [0, 0, 0, 0]
))

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    const currentProductId = selected.value?.id
    const productsRequest = fetchSecondsProducts()
    const [nextProducts, nextOrders, nextAccounts] = session.isAuthenticated
      ? await Promise.all([productsRequest, fetchSecondsOrders(), fetchWalletAccounts()])
      : [await productsRequest, [], []] as [SecondsProduct[], SecondsOrder[], WalletAccount[]]
    products.value = nextProducts
    orders.value = nextOrders
    accounts.value = nextAccounts
    selected.value = nextProducts.find((product) => product.id === currentProductId) || nextProducts[0] || null
    if (selected.value) {
      const stillAvailable = selected.value.cycles.some((item) => item.id === selectedCycleId.value)
      if (!stillAvailable) selectedCycleId.value = selected.value.cycles[0]?.id || 0
      if (!amount.value) amount.value = String(cycle.value?.minStake || '')
    }
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('seconds.loadFailed'))
  } finally {
    loading.value = false
  }
}

function selectProduct(product: SecondsProduct): void {
  selected.value = product
  selectedCycleId.value = product.cycles[0]?.id || 0
  direction.value = 'up'
  amount.value = String(product.cycles[0]?.minStake || '')
  error.value = ''
  success.value = ''
}

function selectProductFromEvent(event: Event): void {
  const productId = Number((event.target as HTMLSelectElement).value)
  const product = products.value.find((item) => item.id === productId)
  if (product) selectProduct(product)
}

function selectCycle(cycleId: number): void {
  selectedCycleId.value = cycleId
  amount.value = String(cycle.value?.minStake || '')
  error.value = ''
  success.value = ''
}

function setDirection(nextDirection: 'up' | 'down'): void {
  direction.value = nextDirection
  error.value = ''
  success.value = ''
}

function setAmount(value: string | number): void {
  amount.value = String(value)
  error.value = ''
  success.value = ''
}

function reviewOrder(): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/seconds' } })
    return
  }
  if (!selected.value || !cycle.value || !valid.value) {
    error.value = t('seconds.invalidAmount')
    return
  }
  error.value = ''
  confirmOpen.value = true
}

function closeConfirm(): void {
  if (submitting.value) return
  confirmOpen.value = false
}

async function submit(): Promise<void> {
  if (!session.isAuthenticated) {
    error.value = t('seconds.loginDescription')
    return
  }
  if (!selected.value || !cycle.value || !valid.value) {
    error.value = t('seconds.invalidAmount')
    return
  }
  submitting.value = true
  error.value = ''
  try {
    await openSecondsOrder({
      productId: selected.value.id,
      durationSeconds: cycle.value.durationSeconds,
      direction: direction.value,
      stakeAmount: amountNumber.value,
    })
    amount.value = ''
    success.value = t('seconds.created')
    confirmOpen.value = false
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('seconds.orderFailed'))
  } finally {
    submitting.value = false
  }
}

function statusLabel(status: string): string {
  const keys: Record<string, string> = {
    pending: 'seconds.statusPending',
    active: 'seconds.statusActive',
    won: 'seconds.statusWon',
    lost: 'seconds.statusLost',
    settled: 'seconds.statusSettled',
    cancelled: 'seconds.statusCancelled',
    canceled: 'seconds.statusCancelled',
  }
  const key = keys[status.toLowerCase()]
  return key ? t(key) : status
}

function statusTone(status: string): string {
  const normalized = status.toLowerCase()
  if (normalized === 'won' || normalized === 'settled') return 'is-positive'
  if (normalized === 'lost' || normalized === 'cancelled' || normalized === 'canceled') return 'is-negative'
  return 'is-pending'
}

function highestRate(product: SecondsProduct): string {
  const highest = Math.max(0, ...product.cycles.map((item) => item.payoutRate * 100))
  return highest.toFixed(0)
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

onMounted(() => {
  void Promise.all([load(), marketStore.refresh()])
})

onBeforeUnmount(() => {
  document.body.style.overflow = previousBodyOverflow
})
</script>

<template>
  <main class="secondary-view page page--plain page--prototype-grid seconds-page">
    <PageHeader
      :back="true"
      :eyebrow="t('seconds.scene')"
      :title="t('seconds.title')"
      :subtitle="t('seconds.context')"
    >
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('common.refresh')"
          :disabled="loading"
          @click="load"
        >
          <RefreshCw :size="20" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>

    <div class="secondary-content page-content seconds-content">
      <section
        class="seconds-workspace"
        data-seconds-workspace="live"
        :class="{ 'seconds-guest': !session.isAuthenticated }"
      >
        <section
          class="seconds-market-board"
          :data-seconds-market="selected ? 'live' : loading ? 'loading' : 'empty'"
          :aria-busy="loading || marketStore.loading"
        >
          <header>
            <div>
              <span>{{ t('seconds.workbenchTitle') }}</span>
              <strong>{{ selected?.symbol || '--/--' }}</strong>
            </div>
            <span class="seconds-market-change">
              <ArrowUp v-if="(selectedTicker?.changePercent || 0) >= 0" :size="15" />
              <ArrowDown v-else :size="15" />
              {{ selectedTicker ? `${selectedTicker.changePercent >= 0 ? '+' : ''}${selectedTicker.changePercent.toFixed(2)}%` : '--' }}
            </span>
          </header>

          <div class="seconds-reference-price">
            <span>{{ t('seconds.referencePrice') }}</span>
            <strong>{{ selectedTicker ? formatPrice(selectedTicker.lastPrice) : '--' }}</strong>
            <small>
              {{ selected
                ? `${selected.stakeAssetSymbol} · ${selectedTicker ? t('common.liveData') : t('common.marketUnavailable')}`
                : '--' }}
            </small>
          </div>

          <dl class="seconds-round-context">
            <div>
              <dt>{{ t('seconds.currentRound') }}</dt>
              <dd>--</dd>
            </div>
            <div>
              <dt>{{ t('seconds.settlementWindow') }}</dt>
              <dd>{{ cycle ? t('seconds.duration', { seconds: cycle.durationSeconds }) : '--' }}</dd>
            </div>
            <div>
              <dt>{{ t('seconds.payoutCoefficient') }}</dt>
              <dd>{{ cycle ? `${payoutCoefficient.toFixed(2)}x` : '--' }}</dd>
            </div>
          </dl>
        </section>

        <label class="field seconds-pair-field" :data-field-state="selected ? 'complete' : 'idle'">
          <span>{{ t('marketDetail.market') }}</span>
          <select
            :value="selected?.id || ''"
            :disabled="loading || !products.length"
            @change="selectProductFromEvent"
          >
            <option v-if="!products.length" value="">{{ loading ? t('seconds.loading') : t('seconds.noProducts') }}</option>
            <option v-for="product in products" :key="product.id" :value="product.id">
              {{ product.symbol }} · {{ t('seconds.highest', { rate: highestRate(product) }) }}
            </option>
          </select>
        </label>

        <section class="seconds-control-group" :aria-labelledby="'seconds-direction-label'">
          <div class="seconds-control-label">
            <span id="seconds-direction-label">{{ t('seconds.direction') }}</span>
            <small>{{ t('seconds.directionHelper') }}</small>
          </div>
          <div class="seconds-direction-grid" role="group" :aria-label="t('seconds.direction')">
            <button
              type="button"
              class="up"
              :class="{ active: direction === 'up' }"
              :aria-pressed="direction === 'up'"
              :disabled="loading || !selected"
              @click="setDirection('up')"
            >
              <ArrowUp :size="18" />
              <span>{{ t('seconds.bullish') }}</span>
            </button>
            <button
              type="button"
              class="down"
              :class="{ active: direction === 'down' }"
              :aria-pressed="direction === 'down'"
              :disabled="loading || !selected"
              @click="setDirection('down')"
            >
              <ArrowDown :size="18" />
              <span>{{ t('seconds.bearish') }}</span>
            </button>
          </div>
        </section>

        <section class="seconds-control-group" :aria-labelledby="'seconds-duration-label'">
          <div class="seconds-control-label">
            <span id="seconds-duration-label">{{ t('seconds.term') }}</span>
            <small>{{ t('seconds.durationHelper') }}</small>
          </div>
          <div class="seconds-duration-grid" role="group" :aria-label="t('seconds.term')">
            <template v-if="selected?.cycles.length">
              <button
                v-for="item in selected.cycles"
                :key="item.id"
                type="button"
                :class="{ active: cycle?.id === item.id }"
                :aria-pressed="cycle?.id === item.id"
                @click="selectCycle(item.id)"
              >
                <Clock3 :size="16" />
                <span>{{ t('seconds.duration', { seconds: item.durationSeconds }) }}</span>
              </button>
            </template>
            <template v-else>
              <button v-for="slot in 3" :key="slot" type="button" disabled>
                <Clock3 :size="16" />
                <span>--</span>
              </button>
            </template>
          </div>
        </section>

        <label
          class="field seconds-amount-field"
          :data-field-state="amount && !valid ? 'invalid' : amount && valid ? 'complete' : 'idle'"
        >
          <span>{{ t('seconds.stakeAmount') }}</span>
          <div>
            <input
              v-model="amount"
              class="numeric"
              inputmode="decimal"
              :disabled="loading || !selected"
              :aria-invalid="Boolean(amount) && !valid"
              @input="setAmount(amount)"
            />
            <b>{{ selected?.stakeAssetSymbol || '--' }}</b>
          </div>
        </label>

        <div class="seconds-amount-presets" role="group" :aria-label="t('seconds.stakeAmount')">
          <button
            v-for="(value, index) in quickAmountSlots"
            :key="`${value}-${index}`"
            type="button"
            :aria-pressed="value > 0 && amountNumber === value"
            :disabled="loading || !selected || value <= 0"
            @click="setAmount(value)"
          >
            {{ value > 0 ? formatAmount(value) : '--' }}
          </button>
        </div>

        <dl class="seconds-order-summary">
          <div>
            <dt>{{ t('seconds.estimatedPayout') }}</dt>
            <dd>{{ selected ? `${formatAmount(estimatedPayout)} ${selected.stakeAssetSymbol}` : '--' }}</dd>
          </div>
          <div>
            <dt>{{ t('seconds.availableBalance') }}</dt>
            <dd>
              {{ selected && session.isAuthenticated && account
                ? `${formatAmount(account.available)} ${selected.stakeAssetSymbol}`
                : '--' }}
            </dd>
          </div>
          <div>
            <dt>{{ t('seconds.localResult') }}</dt>
            <dd>{{ success ? t('seconds.resultConfirmed') : t('seconds.resultPending') }}</dd>
          </div>
        </dl>

        <p v-if="selected" class="seconds-balance">
          {{ t('seconds.balanceMinimum', {
            available: session.isAuthenticated && account ? formatAmount(account.available) : '--',
            asset: selected.stakeAssetSymbol,
            minimum: formatAmount(cycle?.minStake),
          }) }}
        </p>

        <div class="seconds-feedback" aria-live="polite">
          <div v-if="error" class="seconds-message seconds-message--error" role="alert">
            <CircleAlert :size="18" />
            <span>{{ error }}</span>
            <button type="button" :aria-label="t('common.retry')" @click="load">
              <RefreshCw :size="17" />
            </button>
          </div>
          <div
            v-else-if="success"
            class="seconds-message seconds-message--success"
            data-session-feedback="created"
            role="status"
          >
            <CheckCircle2 :size="18" />
            <span>{{ success }}</span>
          </div>
          <span v-else-if="loading">
            <LoaderCircle :size="15" class="spin" />
            {{ t('seconds.loading') }}
          </span>
          <span v-else-if="!session.isAuthenticated">{{ t('seconds.loginDescription') }}</span>
          <span v-else>{{ t('seconds.introDescription') }}</span>
        </div>

        <button
          ref="reviewButton"
          class="button button--primary button--full seconds-submit"
          type="button"
          :disabled="submitting || loading || !selected"
          @click="reviewOrder"
        >
          {{ t('seconds.confirmOrder') }}
        </button>

        <section class="seconds-session-records seconds-orders" :aria-label="t('seconds.myOrders')">
          <h2 class="group-title">{{ t('seconds.myOrders') }} · {{ session.isAuthenticated ? orders.length : '--' }}</h2>
          <template v-if="orders.length">
            <article v-for="order in orders.slice(0, 3)" :key="order.id">
              <div>
                <strong>
                  {{ order.symbol }} ·
                  {{ t(order.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}
                </strong>
                <span :class="statusTone(order.status)">{{ statusLabel(order.status) }}</span>
              </div>
              <p>
                {{ formatAmount(order.stakeAmount) }} {{ order.stakeAssetSymbol }} ·
                {{ t('seconds.duration', { seconds: order.durationSeconds }) }} ·
                {{ formatDateTime(order.createdAt) }}
              </p>
            </article>
          </template>
          <p v-else>{{ session.isAuthenticated ? t('seconds.noOrders') : t('seconds.loginDescription') }}</p>
        </section>
      </section>
    </div>

    <div
      v-if="confirmOpen && selected && cycle"
      class="confirmation-layer seconds-mask"
      @click.self="closeConfirm"
    >
      <section
        ref="confirmDialog"
        class="confirmation-sheet seconds-dialog"
        role="dialog"
        aria-modal="true"
        :aria-busy="submitting"
        aria-labelledby="seconds-confirm-title"
        aria-describedby="seconds-confirm-summary"
        tabindex="-1"
        @keydown="trapDialogFocus"
      >
        <header>
          <span class="confirmation-icon"><CheckCircle2 :size="20" /></span>
          <div>
            <strong id="seconds-confirm-title">{{ t('seconds.confirmOrder') }}</strong>
            <small>{{ selected.symbol }} · {{ t('seconds.settledIn', { asset: selected.stakeAssetSymbol }) }}</small>
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

        <p id="seconds-confirm-summary">
          {{ selected.symbol }} · {{ t(direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }} ·
          {{ formatAmount(amountNumber) }} {{ selected.stakeAssetSymbol }}
        </p>

        <dl class="confirmation-detail">
          <div>
            <dt>{{ t('seconds.direction') }}</dt>
            <dd>{{ t(direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}</dd>
          </div>
          <div>
            <dt>{{ t('seconds.term') }}</dt>
            <dd>{{ t('seconds.duration', { seconds: cycle.durationSeconds }) }}</dd>
          </div>
          <div>
            <dt>{{ t('seconds.stakeAmount') }}</dt>
            <dd>{{ formatAmount(amountNumber) }} {{ selected.stakeAssetSymbol }}</dd>
          </div>
          <div>
            <dt>{{ t('seconds.payoutRate') }}</dt>
            <dd>
              {{ (cycle.payoutRate * 100).toFixed(2) }}% ·
              {{ formatAmount(estimatedPayout) }} {{ selected.stakeAssetSymbol }}
            </dd>
          </div>
          <div>
            <dt>{{ t('marketDetail.latestPrice') }}</dt>
            <dd>{{ selectedTicker ? formatPrice(selectedTicker.lastPrice) : '--' }}</dd>
          </div>
        </dl>

        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <div class="confirmation-actions dialog-actions">
          <button
            type="button"
            class="button button--secondary"
            :disabled="submitting"
            @click="closeConfirm"
          >
            {{ t('common.cancel') }}
          </button>
          <button
            type="button"
            class="button button--primary confirmation-primary"
            :disabled="submitting"
            :aria-busy="submitting"
            @click="submit"
          >
            {{ submitting ? t('common.submitting') : t('seconds.confirmOrder') }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.seconds-workspace {
  display: grid;
  gap: 16px;
  min-width: 0;
}

.seconds-guest {
  align-content: start;
}

.seconds-message {
  align-items: center;
  border: 1px solid currentColor;
  display: grid;
  font-size: 12px;
  gap: 9px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  line-height: 1.45;
  min-height: 52px;
  padding: 4px 5px 4px 11px;
}

.seconds-message--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.seconds-message--success {
  background: var(--positive-soft);
  color: var(--positive);
  grid-template-columns: auto minmax(0, 1fr);
}

.seconds-message button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.seconds-loading,
.seconds-empty {
  align-content: center;
  color: var(--muted);
  display: grid;
  font-size: 12px;
  gap: 9px;
  justify-items: center;
  min-height: 132px;
  text-align: center;
}

.seconds-market-board {
  background:
    linear-gradient(128deg, color-mix(in srgb, var(--positive) 11%, transparent), transparent 45%),
    linear-gradient(310deg, color-mix(in srgb, var(--focus) 8%, transparent), transparent 48%),
    var(--surface);
  border: 1px solid var(--line);
  border-top: 3px solid var(--positive);
  color: var(--ink);
  display: grid;
  gap: 14px;
  min-width: 0;
  overflow: hidden;
  padding: 16px;
  position: relative;
}

.seconds-market-board::after {
  bottom: 9px;
  color: color-mix(in srgb, var(--ink) 25%, transparent);
  content: 'LOCAL / SHORT CYCLE';
  font-size: 8px;
  pointer-events: none;
  position: absolute;
  right: 12px;
}

.seconds-market-board header {
  align-items: flex-start;
  display: flex;
  gap: 10px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-market-board header > div:first-child,
.seconds-reference-price {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.seconds-market-board header span,
.seconds-reference-price > span,
.seconds-reference-price small,
.seconds-round-context dt {
  color: var(--muted);
  font-size: 10px;
}

.seconds-market-board header strong {
  font-size: 15px;
}

.seconds-market-change {
  align-items: center;
  color: var(--positive);
  display: inline-flex;
  font-variant-numeric: tabular-nums;
  gap: 5px;
  min-height: 32px;
}

.seconds-market-change {
  color: var(--positive) !important;
  font-weight: 750;
}

.seconds-reference-price strong {
  color: var(--ink);
  font-size: 36px;
  font-variant-numeric: tabular-nums;
  line-height: 1;
  overflow-wrap: anywhere;
}

.seconds-reference-price small {
  line-height: 1.45;
  margin-top: 3px;
}

.seconds-round-context,
.seconds-order-summary {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0;
  min-width: 0;
}

.seconds-round-context > div,
.seconds-order-summary > div {
  border-right: 1px solid var(--line);
  display: grid;
  gap: 5px;
  min-width: 0;
  padding: 10px 9px;
}

.seconds-round-context > div {
  border-block: 1px solid var(--line);
}

.seconds-round-context > div:last-child,
.seconds-order-summary > div:last-child {
  border-right: 0;
}

.seconds-round-context dd,
.seconds-order-summary dd {
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  font-weight: 750;
  margin: 0;
  overflow-wrap: anywhere;
}

.seconds-order-summary .seconds-payout {
  display: grid;
  gap: 3px;
}

.seconds-order-summary .seconds-payout strong {
  color: var(--ink);
  font-size: 11px;
}

.seconds-order-summary .seconds-payout small {
  color: var(--positive);
  font-size: 9px;
}

.seconds-products {
  display: grid;
  gap: 7px;
  grid-auto-columns: minmax(138px, 1fr);
  grid-auto-flow: column;
  min-width: 0;
  overflow-x: auto;
  padding-bottom: 2px;
}

.seconds-products button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line);
  color: var(--ink);
  display: grid;
  gap: 8px;
  grid-template-columns: 30px minmax(0, 1fr);
  min-height: 58px;
  min-width: 0;
  padding: 7px 9px;
  text-align: left;
}

.seconds-products button.is-active {
  background: color-mix(in srgb, var(--positive) 7%, var(--surface));
  border-color: var(--positive);
  box-shadow: inset 0 -3px 0 var(--positive);
}

.seconds-products span {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.seconds-products strong {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seconds-products small {
  color: var(--muted);
  font-size: 9px;
}

.seconds-control-group {
  display: grid;
  gap: 8px;
  min-width: 0;
}

.seconds-control-label {
  align-items: center;
  display: flex;
  gap: 8px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-control-label span {
  font-size: 12px;
  font-weight: 700;
}

.seconds-control-label small {
  color: var(--muted);
  font-size: 9px;
  line-height: 1.4;
  max-width: 66%;
  overflow-wrap: anywhere;
  text-align: right;
}

.seconds-direction-grid,
.seconds-duration-grid,
.seconds-amount-presets {
  display: grid;
  gap: 6px;
  min-width: 0;
}

.seconds-direction-grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.seconds-duration-grid {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.seconds-duration-grid button,
.seconds-direction-grid button,
.seconds-amount-presets button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line);
  color: var(--muted);
  display: inline-flex;
  font-size: 10px;
  font-weight: 750;
  gap: 5px;
  justify-content: center;
  min-height: 48px;
  min-width: 0;
  padding: 0 5px;
}

.seconds-duration-grid button {
  display: grid;
  gap: 2px;
  grid-template-columns: auto auto;
}

.seconds-duration-grid button small {
  color: inherit;
  font-size: 8px;
  grid-column: 1 / -1;
}

.seconds-direction-grid .up.active {
  background: color-mix(in srgb, var(--positive) 11%, var(--surface));
  border-color: var(--positive);
  box-shadow: inset 0 -3px 0 var(--positive);
  color: var(--positive);
}

.seconds-direction-grid .down.active {
  background: color-mix(in srgb, var(--negative) 11%, var(--surface));
  border-color: var(--negative);
  box-shadow: inset 0 -3px 0 var(--negative);
  color: var(--negative);
}

.seconds-duration-grid button.active,
.seconds-amount-presets button[aria-pressed="true"] {
  background: color-mix(in srgb, var(--focus) 8%, var(--surface));
  border-color: var(--focus);
  box-shadow: inset 0 -3px 0 var(--focus);
  color: var(--ink);
}

.seconds-amount-field {
  background: var(--soft);
  border: 1px solid var(--line);
  display: grid;
  min-width: 0;
  padding: 7px 11px 6px;
}

.seconds-amount-field:focus-within {
  background: var(--surface);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--focus) 15%, transparent);
}

.seconds-amount-field > span {
  color: var(--muted);
  font-size: 10px;
}

.seconds-amount-field > div {
  align-items: center;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 38px;
}

.seconds-amount-field input {
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 20px;
  font-weight: 750;
  min-width: 0;
  outline: 0;
  width: 100%;
}

.seconds-amount-field b {
  font-size: 12px;
}

.seconds-amount-presets {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.seconds-order-summary {
  border-block: 1px solid var(--line);
}

.seconds-order-summary dt {
  color: var(--muted);
  font-size: 9px;
}

.seconds-balance {
  color: var(--muted);
  font-size: 10px;
  line-height: 1.45;
  margin: -5px 0 0;
}

.seconds-feedback {
  align-content: start;
  display: grid;
  min-height: 76px;
}

.seconds-feedback > span {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 10px;
  gap: 6px;
}

.seconds-submit {
  border-radius: 0;
  min-height: 52px;
}

.seconds-session-records {
  display: grid;
  min-width: 0;
}

.seconds-session-records h2 {
  font-size: 11px;
}

.seconds-session-records article {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 6px;
  min-width: 0;
  padding: 12px 0;
}

.seconds-session-records article > div {
  display: flex;
  gap: 8px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-session-records article strong,
.seconds-session-records article span {
  font-size: 10px;
  overflow-wrap: anywhere;
}

.seconds-session-records article span {
  color: var(--positive);
  text-align: right;
}

.seconds-session-records article span.is-negative {
  color: var(--negative);
}

.seconds-session-records article span.is-pending {
  color: var(--accent);
}

.seconds-session-records article p,
.seconds-session-records > p {
  color: var(--muted);
  font-size: 9px;
  line-height: 1.5;
  margin: 0;
  overflow-wrap: anywhere;
}

.seconds-orders {
  border-top: 8px solid var(--soft);
  margin: 8px -16px 0;
  min-width: 0;
  padding: 0 16px;
}

.seconds-orders .section-heading {
  border-bottom: 1px solid var(--line);
  font-size: 16px;
  margin: 0;
  min-height: 56px;
}

.seconds-orders .section-heading b {
  color: var(--positive);
  font-size: 12px;
}

.seconds-order-list article {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 72px;
  min-width: 0;
}

.seconds-order-list article > div,
.seconds-order-list article > span {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.seconds-order-list article > span {
  justify-items: end;
  text-align: right;
}

.seconds-order-list strong,
.seconds-order-list b {
  font-size: 11px;
  overflow-wrap: anywhere;
}

.seconds-order-list small {
  color: var(--muted);
  font-size: 9px;
  line-height: 1.4;
}

.seconds-order-list small.is-positive {
  color: var(--positive);
}

.seconds-order-list small.is-negative {
  color: var(--negative);
}

.seconds-order-list small.is-pending {
  color: var(--accent);
}

.seconds-mask {
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
  z-index: 80;
}

.seconds-dialog {
  background: var(--surface);
  border: 1px solid var(--line);
  border-top: 3px solid var(--positive);
  box-shadow: var(--shadow-soft);
  display: grid;
  gap: 15px;
  max-height: calc(100dvh - max(32px, env(safe-area-inset-top)) - max(32px, env(safe-area-inset-bottom)));
  max-width: 520px;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 17px;
  width: 100%;
}

.seconds-dialog header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
}

.seconds-dialog header > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.seconds-dialog header strong {
  font-size: 18px;
}

.seconds-dialog header small {
  color: var(--muted);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.seconds-dialog dl {
  border-block: 1px solid var(--line);
  display: grid;
  margin: 0;
}

.seconds-dialog dl > div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 44px;
}

.seconds-dialog dl > div:last-child {
  border-bottom: 0;
}

.seconds-dialog dt,
.seconds-dialog dd {
  font-size: 12px;
  margin: 0;
}

.seconds-dialog dt {
  color: var(--muted);
}

.seconds-dialog dd {
  font-variant-numeric: tabular-nums;
  font-weight: 750;
  text-align: right;
}

.dialog-feedback {
  background: var(--negative-soft);
  border-left: 3px solid var(--negative);
  color: var(--negative);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
  padding: 8px 10px;
}

.dialog-actions {
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, .8fr) minmax(0, 1.2fr);
}

.dialog-actions .button {
  min-height: 48px;
  padding-inline: 10px;
}

:global(html[data-theme='dark']) .seconds-market-board,
:global([data-theme='dark']) .seconds-market-board,
:global(.theme-dark) .seconds-market-board {
  background:
    linear-gradient(128deg, color-mix(in srgb, var(--positive) 18%, transparent), transparent 44%),
    linear-gradient(310deg, color-mix(in srgb, var(--focus) 14%, transparent), transparent 48%),
    var(--dark-surface);
  border-color: color-mix(in srgb, var(--positive) 30%, var(--line));
  border-top-color: var(--positive);
  color: var(--ink);
}

:global(html[data-theme='dark']) .seconds-market-board .seconds-reference-price strong,
:global([data-theme='dark']) .seconds-market-board .seconds-reference-price strong,
:global(.theme-dark) .seconds-market-board .seconds-reference-price strong,
:global(html[data-theme='dark']) .seconds-market-board header strong,
:global([data-theme='dark']) .seconds-market-board header strong,
:global(.theme-dark) .seconds-market-board header strong {
  color: var(--ink);
}

:global(html[data-theme='dark']) .seconds-market-board header span,
:global([data-theme='dark']) .seconds-market-board header span,
:global(.theme-dark) .seconds-market-board header span,
:global(html[data-theme='dark']) .seconds-market-board .seconds-reference-price > span,
:global([data-theme='dark']) .seconds-market-board .seconds-reference-price > span,
:global(.theme-dark) .seconds-market-board .seconds-reference-price > span,
:global(html[data-theme='dark']) .seconds-market-board .seconds-reference-price small,
:global([data-theme='dark']) .seconds-market-board .seconds-reference-price small,
:global(.theme-dark) .seconds-market-board .seconds-reference-price small,
:global(html[data-theme='dark']) .seconds-market-board dt,
:global([data-theme='dark']) .seconds-market-board dt,
:global(.theme-dark) .seconds-market-board dt {
  color: var(--muted);
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 390px) {
  .seconds-orders {
    margin-left: -14px;
    margin-right: -14px;
    padding-inline: 14px;
  }

  .seconds-market-board {
    padding-inline: 13px;
  }
}

@media (max-width: 340px) {
  .seconds-control-label {
    align-items: flex-start;
    flex-direction: column;
  }

  .seconds-control-label small {
    max-width: none;
    text-align: left;
  }

  .seconds-round-context,
  .seconds-order-summary {
    grid-template-columns: 1fr;
  }

  .seconds-round-context > div,
  .seconds-order-summary > div {
    align-items: center;
    border-bottom: 1px solid var(--line);
    border-right: 0;
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .seconds-round-context > div:last-child,
  .seconds-order-summary > div:last-child {
    border-bottom: 0;
  }

  .seconds-duration-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .seconds-amount-presets {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .dialog-actions {
    grid-template-columns: 1fr;
  }
}
</style>
