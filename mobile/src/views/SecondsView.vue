<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  CircleAlert,
  Clock3,
  Gauge,
  LoaderCircle,
  RefreshCw,
  X,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
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
import { formatAmount, formatDateTime } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
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
const amountNumber = computed(() => Number(amount.value || 0))
const payoutRate = computed(() => cycle.value?.payoutRate || 0)
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

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const currentProductId = selected.value?.id
    const [nextProducts, nextOrders, nextAccounts] = await Promise.all([
      fetchSecondsProducts(),
      fetchSecondsOrders(),
      fetchWalletAccounts(),
    ])
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

onMounted(() => { void load() })

onBeforeUnmount(() => {
  document.body.style.overflow = previousBodyOverflow
})
</script>

<template>
  <main class="page page--plain seconds-page">
    <PageHeader :title="t('seconds.title')">
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('common.refresh')"
          :disabled="loading || !session.isAuthenticated"
          @click="load"
        >
          <RefreshCw :size="20" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>

    <div class="page-content seconds-content">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        :description="t('seconds.loginDescription')"
      />

      <template v-else>
        <div v-if="error" class="seconds-message seconds-message--error" role="alert">
          <CircleAlert :size="18" />
          <span>{{ error }}</span>
          <button
            v-if="!confirmOpen"
            type="button"
            :aria-label="t('common.retry')"
            @click="load"
          >
            <RefreshCw :size="17" />
          </button>
        </div>
        <div v-if="success" class="seconds-message seconds-message--success" role="status">
          <CheckCircle2 :size="18" />
          <span>{{ success }}</span>
        </div>

        <div v-if="loading" class="seconds-loading" aria-live="polite">
          <LoaderCircle :size="24" class="spin" />
          <span>{{ t('seconds.loading') }}</span>
        </div>

        <template v-else>
          <section v-if="selected" class="seconds-market-board">
            <header>
              <div>
                <span>{{ t('seconds.title') }}</span>
                <strong>{{ selected.symbol }}</strong>
              </div>
              <div class="seconds-rate">
                <Gauge :size="17" />
                <span>{{ t('seconds.payoutRate') }}</span>
              </div>
            </header>
            <div class="seconds-reference">
              <span>{{ t('seconds.highest', { rate: highestRate(selected) }) }}</span>
              <strong>{{ (payoutRate * 100).toFixed(2) }}%</strong>
              <small>{{ t('seconds.introDescription') }}</small>
            </div>
            <dl class="seconds-market-facts">
              <div>
                <dt>{{ t('seconds.settledIn', { asset: selected.stakeAssetSymbol }) }}</dt>
                <dd>{{ selected.stakeAssetSymbol }}</dd>
              </div>
              <div>
                <dt>{{ t('seconds.term') }}</dt>
                <dd>{{ t('seconds.duration', { seconds: cycle?.durationSeconds || 0 }) }}</dd>
              </div>
              <div>
                <dt>{{ t('common.available') }}</dt>
                <dd>{{ formatAmount(account?.available) }}</dd>
              </div>
            </dl>
          </section>

          <div
            v-if="products.length"
            class="seconds-products"
            role="group"
            :aria-label="t('seconds.title')"
          >
            <button
              v-for="product in products"
              :key="product.id"
              type="button"
              :class="{ 'is-active': selected?.id === product.id }"
              :aria-pressed="selected?.id === product.id"
              @click="selectProduct(product)"
            >
              <AssetMark
                :symbol="product.symbol.split(/[\/_-]/)[0] || product.symbol"
                :size="30"
              />
              <span>
                <strong>{{ product.symbol }}</strong>
                <small>{{ t('seconds.highest', { rate: highestRate(product) }) }}</small>
              </span>
            </button>
          </div>

          <div v-if="!products.length" class="seconds-empty">
            <Gauge :size="24" />
            <span>{{ t('seconds.noProducts') }}</span>
          </div>

          <template v-if="selected">
            <section class="seconds-control-group">
              <div class="seconds-control-label">
                <strong>{{ t('seconds.direction') }}</strong>
                <small>{{ t('seconds.introDescription') }}</small>
              </div>
              <div class="seconds-direction-grid" :aria-label="t('seconds.direction')">
                <button
                  type="button"
                  class="is-up"
                  :class="{ 'is-active': direction === 'up' }"
                  :aria-pressed="direction === 'up'"
                  @click="setDirection('up')"
                >
                  <ArrowUp :size="18" />
                  <span>{{ t('seconds.bullish') }}</span>
                </button>
                <button
                  type="button"
                  class="is-down"
                  :class="{ 'is-active': direction === 'down' }"
                  :aria-pressed="direction === 'down'"
                  @click="setDirection('down')"
                >
                  <ArrowDown :size="18" />
                  <span>{{ t('seconds.bearish') }}</span>
                </button>
              </div>
            </section>

            <section class="seconds-control-group">
              <div class="seconds-control-label">
                <strong>{{ t('seconds.term') }}</strong>
                <small>{{ t('seconds.settlementSummary', { asset: selected.stakeAssetSymbol, count: selected.cycles.length }) }}</small>
              </div>
              <div class="seconds-duration-grid" :aria-label="t('seconds.term')">
                <button
                  v-for="item in selected.cycles"
                  :key="item.id"
                  type="button"
                  :class="{ 'is-active': cycle?.id === item.id }"
                  :aria-pressed="cycle?.id === item.id"
                  @click="selectCycle(item.id)"
                >
                  <Clock3 :size="16" />
                  <span>{{ t('seconds.duration', { seconds: item.durationSeconds }) }}</span>
                  <small>{{ (item.payoutRate * 100).toFixed(0) }}%</small>
                </button>
              </div>
            </section>

            <section class="seconds-control-group">
              <label class="seconds-amount-field">
                <span>{{ t('seconds.stakeAmount') }}</span>
                <div>
                  <input
                    v-model="amount"
                    class="numeric"
                    inputmode="decimal"
                    :aria-invalid="Boolean(amount) && !valid"
                    @input="setAmount(amount)"
                  />
                  <b>{{ selected.stakeAssetSymbol }}</b>
                </div>
              </label>
              <div v-if="quickAmounts.length" class="seconds-amount-presets">
                <button
                  v-for="value in quickAmounts"
                  :key="value"
                  type="button"
                  :aria-pressed="amountNumber === value"
                  @click="setAmount(value)"
                >
                  {{ formatAmount(value) }}
                </button>
              </div>
            </section>

            <dl class="seconds-order-summary">
              <div>
                <dt>{{ t('seconds.payoutRate') }}</dt>
                <dd>{{ (payoutRate * 100).toFixed(2) }}%</dd>
              </div>
              <div>
                <dt>{{ t('common.available') }}</dt>
                <dd>{{ formatAmount(account?.available) }} {{ selected.stakeAssetSymbol }}</dd>
              </div>
              <div>
                <dt>{{ t('seconds.term') }}</dt>
                <dd>{{ t('seconds.duration', { seconds: cycle?.durationSeconds || 0 }) }}</dd>
              </div>
            </dl>

            <p class="seconds-balance">
              {{ t('seconds.balanceMinimum', {
                available: formatAmount(account?.available),
                asset: selected.stakeAssetSymbol,
                minimum: formatAmount(cycle?.minStake),
              }) }}
            </p>

            <button
              ref="reviewButton"
              class="button button--primary button--full seconds-submit"
              type="button"
              :disabled="submitting"
              @click="reviewOrder"
            >
              {{ t('seconds.confirmOrder') }}
            </button>
          </template>

          <section class="seconds-orders">
            <div class="section-heading">
              <span>{{ t('seconds.myOrders') }}</span>
              <b>{{ orders.length }}</b>
            </div>
            <div v-if="orders.length" class="seconds-order-list">
              <article v-for="order in orders" :key="order.id">
                <div>
                  <strong>
                    {{ order.symbol }} ·
                    {{ t(order.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}
                  </strong>
                  <small>
                    {{ formatDateTime(order.createdAt) }} ·
                    {{ t('seconds.duration', { seconds: order.durationSeconds }) }}
                  </small>
                </div>
                <span>
                  <b>{{ formatAmount(order.stakeAmount) }} {{ order.stakeAssetSymbol }}</b>
                  <small :class="statusTone(order.status)">{{ statusLabel(order.status) }}</small>
                </span>
              </article>
            </div>
            <div v-else class="seconds-empty">
              <Clock3 :size="22" />
              <span>{{ t('seconds.noOrders') }}</span>
            </div>
          </section>
        </template>
      </template>
    </div>

    <div
      v-if="confirmOpen && selected && cycle"
      class="seconds-mask"
      @click.self="closeConfirm"
    >
      <section
        ref="confirmDialog"
        class="seconds-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="seconds-confirm-title"
        @keydown="trapDialogFocus"
      >
        <header>
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

        <dl>
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
            <dd>{{ (cycle.payoutRate * 100).toFixed(2) }}%</dd>
          </div>
        </dl>

        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <div class="dialog-actions">
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
            class="button button--primary"
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
.seconds-page {
  background: var(--surface);
  min-width: 0;
}

.seconds-content {
  display: grid;
  gap: 16px;
  min-width: 0;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
  padding-top: 14px;
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
    linear-gradient(310deg, color-mix(in srgb, var(--focus, #1677ff) 8%, transparent), transparent 48%),
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

.seconds-market-board header {
  align-items: flex-start;
  display: flex;
  gap: 10px;
  justify-content: space-between;
  min-width: 0;
}

.seconds-market-board header > div:first-child,
.seconds-reference {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.seconds-market-board header span,
.seconds-reference > span,
.seconds-reference small,
.seconds-market-facts dt {
  color: var(--muted);
  font-size: 10px;
}

.seconds-market-board header strong {
  font-size: 15px;
}

.seconds-rate {
  align-items: center;
  color: var(--positive);
  display: inline-flex;
  gap: 5px;
  min-height: 32px;
}

.seconds-rate span {
  color: inherit !important;
  font-weight: 750;
}

.seconds-reference strong {
  color: var(--ink);
  font-size: 36px;
  font-variant-numeric: tabular-nums;
  line-height: 1;
  overflow-wrap: anywhere;
}

.seconds-reference small {
  line-height: 1.45;
  margin-top: 3px;
}

.seconds-market-facts,
.seconds-order-summary {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0;
  min-width: 0;
}

.seconds-market-facts > div,
.seconds-order-summary > div {
  border-right: 1px solid var(--line);
  display: grid;
  gap: 5px;
  min-width: 0;
  padding: 10px 9px;
}

.seconds-market-facts > div {
  border-block: 1px solid var(--line);
}

.seconds-market-facts > div:last-child,
.seconds-order-summary > div:last-child {
  border-right: 0;
}

.seconds-market-facts dd,
.seconds-order-summary dd {
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  font-weight: 750;
  margin: 0;
  overflow-wrap: anywhere;
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

.seconds-control-label strong {
  font-size: 12px;
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

.seconds-direction-grid .is-up.is-active {
  background: color-mix(in srgb, var(--positive) 11%, var(--surface));
  border-color: var(--positive);
  box-shadow: inset 0 -3px 0 var(--positive);
  color: var(--positive);
}

.seconds-direction-grid .is-down.is-active {
  background: color-mix(in srgb, var(--negative) 11%, var(--surface));
  border-color: var(--negative);
  box-shadow: inset 0 -3px 0 var(--negative);
  color: var(--negative);
}

.seconds-duration-grid button.is-active,
.seconds-amount-presets button[aria-pressed="true"] {
  background: color-mix(in srgb, var(--focus, #1677ff) 8%, var(--surface));
  border-color: var(--focus, #1677ff);
  box-shadow: inset 0 -3px 0 var(--focus, #1677ff);
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
  border-color: var(--focus, #1677ff);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--focus, #1677ff) 15%, transparent);
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

.seconds-submit {
  border-radius: 0;
  min-height: 52px;
}

.seconds-orders {
  border-top: 8px solid var(--soft);
  margin: 8px -20px 0;
  min-width: 0;
  padding: 0 20px;
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
  background: rgb(5 10 16 / 62%);
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
  box-shadow: 0 24px 60px rgb(5 10 16 / 28%);
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
    linear-gradient(128deg, rgb(41 210 133 / 18%), transparent 44%),
    linear-gradient(310deg, rgb(0 126 255 / 14%), transparent 48%),
    #101714;
  border-color: #32433a;
  border-top-color: #35d98d;
  color: #f4f8f5;
}

:global(html[data-theme='dark']) .seconds-market-board .seconds-reference strong,
:global([data-theme='dark']) .seconds-market-board .seconds-reference strong,
:global(.theme-dark) .seconds-market-board .seconds-reference strong,
:global(html[data-theme='dark']) .seconds-market-board header strong,
:global([data-theme='dark']) .seconds-market-board header strong,
:global(.theme-dark) .seconds-market-board header strong {
  color: #f4f8f5;
}

:global(html[data-theme='dark']) .seconds-market-board header span,
:global([data-theme='dark']) .seconds-market-board header span,
:global(.theme-dark) .seconds-market-board header span,
:global(html[data-theme='dark']) .seconds-market-board .seconds-reference > span,
:global([data-theme='dark']) .seconds-market-board .seconds-reference > span,
:global(.theme-dark) .seconds-market-board .seconds-reference > span,
:global(html[data-theme='dark']) .seconds-market-board .seconds-reference small,
:global([data-theme='dark']) .seconds-market-board .seconds-reference small,
:global(.theme-dark) .seconds-market-board .seconds-reference small,
:global(html[data-theme='dark']) .seconds-market-board dt,
:global([data-theme='dark']) .seconds-market-board dt,
:global(.theme-dark) .seconds-market-board dt {
  color: #9fb0a7;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 390px) {
  .seconds-content {
    padding-left: 14px;
    padding-right: 14px;
  }

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

  .seconds-market-facts,
  .seconds-order-summary {
    grid-template-columns: 1fr;
  }

  .seconds-market-facts > div,
  .seconds-order-summary > div {
    align-items: center;
    border-bottom: 1px solid var(--line);
    border-right: 0;
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .seconds-market-facts > div:last-child,
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
