<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import {
  CheckCircle2,
  CircleAlert,
  LoaderCircle,
  PackageOpen,
  X,
} from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { confirmPredictionQuote, fetchPredictionConfig, fetchPredictionMarkets, fetchPredictionOrders, requestPredictionQuote, type PredictionAsset, type PredictionMarket, type PredictionOrder, type PredictionOutcome, type PredictionQuote } from '@/api/prediction'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime, formatPercent } from '@/core/format'
import { localizePredictionMarketText, type PredictionTextKind } from '@/core/predictionLocale'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const route = useRoute()
const router = useRouter()
const { locale, t } = useI18n()
const markets = ref<PredictionMarket[]>([])
const assets = ref<PredictionAsset[]>([])
const accounts = ref<WalletAccount[]>([])
const orders = ref<PredictionOrder[]>([])
const selected = ref<PredictionMarket | null>(null)
const outcome = ref<PredictionOutcome>('yes')
const assetId = ref(0)
const amount = ref('')
const quote = ref<PredictionQuote | null>(null)
const loading = ref(false)
const quoting = ref(false)
const confirming = ref(false)
const error = ref('')
const success = ref('')
const marketTab = ref<'active' | 'settled'>('active')
const orderDialog = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
let previousBodyOverflow = ''
const TERMINAL_MARKET_STATUSES = new Set([
  'settled',
  'resolved',
  'won',
  'lost',
  'refunded',
  'cancelled',
  'canceled',
  'closed',
])

const selectedAsset = computed(() => assets.value.find((asset) => asset.assetId === assetId.value))
const selectedAccount = computed(() => accounts.value.find((account) => account.assetId === assetId.value))
const amountNumber = computed(() => Number(amount.value || 0))
const valid = computed(() => Number.isFinite(amountNumber.value) && amountNumber.value > 0 && amountNumber.value <= (selectedAccount.value?.available || 0))
const dialogOpen = computed(() => Boolean(selected.value))
const visibleMarkets = computed(() => markets.value.filter((market) => (
  marketTab.value === 'settled' ? isSettledMarket(market) : !isSettledMarket(market)
)))

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    const [nextMarkets, nextAssets] = await Promise.all([fetchPredictionMarkets(), fetchPredictionConfig()])
    markets.value = nextMarkets
    assets.value = nextAssets
    if (session.isAuthenticated) {
      const [wallets, history] = await Promise.all([fetchWalletAccounts(), fetchPredictionOrders()])
      accounts.value = wallets
      orders.value = history
    }
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('prediction.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openOrder(market: PredictionMarket, nextOutcome: PredictionOutcome): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: route.fullPath } })
    return
  }
  selected.value = market
  outcome.value = nextOutcome
  assetId.value = assets.value.find((asset) => accounts.value.some((account) => account.assetId === asset.assetId))?.assetId || assets.value[0]?.assetId || 0
  amount.value = ''
  quote.value = null
  error.value = ''
}

function closeOrder(): void {
  if (confirming.value) return
  selected.value = null
  quote.value = null
  error.value = ''
}

async function getQuote(): Promise<void> {
  if (!selected.value || !valid.value) {
    error.value = t('prediction.invalidAmount')
    return
  }
  quoting.value = true
  error.value = ''
  try { quote.value = await requestPredictionQuote({ marketId: selected.value.id, outcome: outcome.value, assetId: assetId.value, stakeAmount: amountNumber.value }) } catch (reason) { error.value = apiErrorMessage(reason, t('prediction.quoteFailed')) } finally { quoting.value = false }
}

async function confirm(): Promise<void> {
  if (!quote.value || quote.value.expiresAt <= Date.now()) {
    quote.value = null
    error.value = t('prediction.quoteExpired')
    return
  }
  confirming.value = true
  error.value = ''
  try {
    const createdOrder = await confirmPredictionQuote(quote.value.quoteId)
    orders.value = [createdOrder, ...orders.value.filter((order) => order.id !== createdOrder.id)]
    selected.value = null
    quote.value = null
    success.value = t('prediction.created')
    try {
      const [wallets, history] = await Promise.all([fetchWalletAccounts(), fetchPredictionOrders()])
      accounts.value = wallets
      orders.value = history
    } catch (reason) {
      error.value = apiErrorMessage(reason, t('prediction.refreshAfterOrderFailed'))
    }
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('prediction.confirmFailed'))
  } finally {
    confirming.value = false
  }
}

function marketText(value: string | undefined, kind: PredictionTextKind): string {
  return localizePredictionMarketText(value, locale.value, kind)
}

function outcomeLabel(value: string): string {
  return marketText(value, 'outcome') || value
}

function statusLabel(status: string): string {
  const keys: Record<string, string> = {
    pending: 'prediction.statusPending',
    active: 'prediction.statusActive',
    won: 'prediction.statusWon',
    lost: 'prediction.statusLost',
    settled: 'prediction.statusSettled',
    refunded: 'prediction.statusRefunded',
    cancelled: 'prediction.statusCancelled',
    canceled: 'prediction.statusCancelled',
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

function orderStatusLabel(order: PredictionOrder): string {
  const result = order.result?.toLowerCase()
  if (result === 'invalid' || order.refundAmount > 0) return t('prediction.statusRefunded')
  if (order.status.toLowerCase() === 'settled' && result) {
    return t(result === order.outcome.toLowerCase() ? 'prediction.statusWon' : 'prediction.statusLost')
  }
  return statusLabel(order.status)
}

function orderStatusTone(order: PredictionOrder): string {
  const result = order.result?.toLowerCase()
  if (result === 'invalid' || order.refundAmount > 0) return 'is-pending'
  if (order.status.toLowerCase() === 'settled' && result) {
    return result === order.outcome.toLowerCase() ? 'is-positive' : 'is-negative'
  }
  return statusTone(order.status)
}

function isSettledMarket(market: PredictionMarket): boolean {
  return [market.displayStatus, market.settlementStatus]
    .some((status) => TERMINAL_MARKET_STATUSES.has(status.trim().toLowerCase()))
}

function marketStatus(market: PredictionMarket): string {
  const settled = isSettledMarket(market)
  const status = settled
    ? market.settlementStatus || market.displayStatus
    : market.displayStatus || market.settlementStatus
  const normalized = status.trim().toLowerCase()
  if (['open', 'opened', 'trading', 'live'].includes(normalized)) return t('prediction.statusActive')
  return statusLabel(status || (settled ? 'settled' : 'active'))
}

function trapDialogFocus(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    closeOrder()
    return
  }
  if (event.key !== 'Tab' || !orderDialog.value) return
  const focusable = Array.from(orderDialog.value.querySelectorAll<HTMLElement>(
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

watch(dialogOpen, async (open) => {
  if (open) {
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    previousBodyOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    await nextTick()
    orderDialog.value?.querySelector<HTMLElement>('[data-dialog-cancel]')?.focus()
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
  <main
    class="page page--plain prediction-page"
    data-pencil-source="pU7Kz IcvzQ"
    data-prediction-workspace="live"
  >
    <PageHeader
      :back="true"
      :eyebrow="t('products.prediction')"
      :pencil="true"
      :subtitle="t('prediction.introDescription')"
      :title="t('prediction.title')"
    />
    <div class="page-content prediction-content">
      <div v-if="error && !dialogOpen" class="prediction-message prediction-message--error" role="alert">
        <CircleAlert :size="18" />
        <span>{{ error }}</span>
      </div>
      <div v-if="success" class="prediction-message prediction-message--success" role="status">
        <CheckCircle2 :size="18" />
        <span>{{ success }}</span>
      </div>
      <div v-if="loading" class="prediction-state" aria-live="polite">
        <LoaderCircle :size="24" class="spin" />
        <span>{{ t('prediction.loading') }}</span>
      </div>

      <template v-else>
        <nav class="prediction-tabs" :aria-label="t('prediction.market')">
          <button
            type="button"
            :class="{ active: marketTab === 'active' }"
            :aria-pressed="marketTab === 'active'"
            @click="marketTab = 'active'"
          >
            {{ t('prediction.statusActive') }}
          </button>
          <button
            type="button"
            :class="{ active: marketTab === 'settled' }"
            :aria-pressed="marketTab === 'settled'"
            @click="marketTab = 'settled'"
          >
            {{ t('prediction.statusSettled') }}
          </button>
        </nav>

        <div v-if="visibleMarkets.length" class="prediction-list" :data-market-tab="marketTab">
          <article
            v-for="market in visibleMarkets"
            :key="market.id"
            data-market-source="api"
            :data-market-status="isSettledMarket(market) ? 'settled' : 'active'"
          >
            <header>
              <strong>{{ marketText(market.category, 'category') || t('prediction.market') }}</strong>
              <span :class="{ settled: isSettledMarket(market) }">
                <i aria-hidden="true" />
                {{ marketStatus(market) }}
              </span>
            </header>
            <h2>{{ marketText(market.title, 'title') }}</h2>
            <div class="prediction-outcomes">
              <button
                type="button"
                :disabled="isSettledMarket(market)"
                @click="openOrder(market, 'yes')"
              >
                <b>{{ outcomeLabel(market.yesLabel) }}</b>
                <small class="numeric">{{ formatPercent(market.yesPrice * 100) }}</small>
              </button>
              <button
                type="button"
                :disabled="isSettledMarket(market)"
                @click="openOrder(market, 'no')"
              >
                <b>{{ outcomeLabel(market.noLabel) }}</b>
                <small class="numeric">{{ formatPercent(market.noPrice * 100) }}</small>
              </button>
            </div>
            <p v-if="market.description || market.endAt" class="prediction-market-meta">
              <span v-if="market.description">{{ marketText(market.description, 'description') }}</span>
              <time v-if="market.endAt" :datetime="new Date(market.endAt).toISOString()">
                {{ formatDateTime(market.endAt) }}
              </time>
            </p>
          </article>
        </div>
        <div v-else class="prediction-state prediction-state--empty">
          <PackageOpen :size="23" />
          <span>{{ t('prediction.noMarkets') }}</span>
        </div>

        <p class="prediction-note">
          <CircleAlert :size="14" aria-hidden="true" />
          <span>{{ session.isAuthenticated ? t('prediction.introDescription') : t('prediction.loginDescription') }}</span>
        </p>

        <section v-if="session.isAuthenticated" class="prediction-orders">
          <div class="section-heading"><span>{{ t('prediction.myPredictions') }}</span><b>{{ orders.length }}</b></div>
          <article v-for="order in orders" :key="order.id">
            <div>
              <strong>{{ marketText(order.marketTitle, 'title') }}</strong>
              <small v-if="order.orderNo" class="order-number">{{ t('prediction.orderNumber', { orderNo: order.orderNo }) }}</small>
              <small>{{ formatDateTime(order.createdAt) }} · {{ outcomeLabel(order.outcome) }}</small>
            </div>
            <span>
              <b class="numeric">{{ formatAmount(order.stakeAmount) }} {{ order.assetSymbol }}</b>
              <small :class="orderStatusTone(order)">{{ orderStatusLabel(order) }}</small>
            </span>
          </article>
          <div v-if="!orders.length" class="prediction-state prediction-state--empty">
            <PackageOpen :size="22" />
            <span>{{ t('prediction.noOrders') }}</span>
          </div>
        </section>
      </template>
    </div>

    <div v-if="selected" class="prediction-mask" @click.self="closeOrder">
      <section
        ref="orderDialog"
        class="prediction-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="prediction-order-title"
        @keydown="trapDialogFocus"
      >
        <header>
          <div>
            <strong id="prediction-order-title">{{ marketText(selected.title, 'title') }}</strong>
            <small>{{ t('prediction.chooseOutcome', { outcome: outcomeLabel(outcome === 'yes' ? selected.yesLabel : selected.noLabel) }) }}</small>
          </div>
          <button
            class="icon-button"
            type="button"
            :aria-label="t('common.close')"
            :disabled="confirming"
            data-dialog-cancel
            @click="closeOrder"
          >
            <X :size="21" />
          </button>
        </header>

        <label class="prediction-field">
          <span>{{ t('prediction.paymentAsset') }}</span>
          <select v-model="assetId" @change="quote = null">
            <option v-for="asset in assets" :key="asset.assetId" :value="asset.assetId">
              {{ t('prediction.assetAvailable', { asset: asset.assetSymbol, amount: formatAmount(accounts.find((item) => item.assetId === asset.assetId)?.available) }) }}
            </option>
          </select>
        </label>
        <label class="prediction-field">
          <span>{{ t('prediction.stakeAmount') }}</span>
          <div>
            <input v-model="amount" class="numeric" inputmode="decimal" placeholder="0.00" @input="quote = null" />
            <b>{{ selectedAsset?.assetSymbol || '' }}</b>
          </div>
        </label>

        <dl v-if="quote" class="prediction-quote">
          <div><dt>{{ t('prediction.estimatedShares') }}</dt><dd>{{ formatAmount(quote.shares) }}</dd></div>
          <div><dt>{{ t('prediction.theoreticalPayout') }}</dt><dd>{{ formatAmount(quote.theoreticalPayout) }} {{ quote.assetSymbol }}</dd></div>
          <div><dt>{{ t('common.fee') }}</dt><dd>{{ formatAmount(quote.feeAmount) }} {{ quote.assetSymbol }}</dd></div>
        </dl>

        <p v-if="error" class="dialog-feedback" role="alert">{{ error }}</p>
        <button
          v-if="quote"
          class="button button--primary button--full prediction-submit"
          type="button"
          :disabled="confirming"
          :aria-busy="confirming"
          @click="confirm"
        >
          {{ t(confirming ? 'prediction.confirming' : 'prediction.confirmOrder') }}
        </button>
        <button
          v-else
          class="button button--primary button--full prediction-submit"
          type="button"
          :disabled="quoting"
          :aria-busy="quoting"
          @click="getQuote"
        >
          {{ t(quoting ? 'prediction.quoting' : 'prediction.getQuote') }}
        </button>
      </section>
    </div>
  </main>
</template>

<style scoped>
.prediction-page {
  background: var(--page);
  color: var(--text);
  min-width: 0;
  overflow-x: clip;
}

.prediction-content {
  display: grid;
  gap: 14px;
  min-width: 0;
  padding: 6px 20px calc(20px + env(safe-area-inset-bottom));
}

.prediction-message {
  align-items: center;
  border: 1px solid currentColor;
  border-radius: 8px;
  display: grid;
  font-size: 11px;
  gap: 9px;
  grid-template-columns: auto minmax(0, 1fr);
  line-height: 16px;
  min-height: 44px;
  padding: 8px 11px;
}

.prediction-message--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.prediction-message--success {
  background: var(--positive-soft);
  color: var(--positive);
}

.prediction-tabs {
  align-items: start;
  display: flex;
  gap: 20px;
  min-height: 27px;
}

.prediction-tabs button {
  background: transparent;
  border: 0;
  color: var(--muted);
  display: grid;
  font-size: 12px;
  font-weight: 650;
  gap: 5px;
  justify-items: start;
  line-height: 18px;
  min-height: 27px;
  padding: 0;
}

.prediction-tabs button::after {
  background: transparent;
  border-radius: 50%;
  content: '';
  height: 2px;
  width: 18px;
}

.prediction-tabs button.active {
  color: var(--text);
}

.prediction-tabs button.active::after {
  background: var(--positive);
}

.prediction-tabs button:focus-visible {
  box-shadow: 0 0 0 3px var(--focus-ring);
  outline: 0;
}

.prediction-list {
  display: grid;
  min-width: 0;
}

.prediction-list article {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 10px;
  min-width: 0;
  padding: 12px 0 6px;
}

.prediction-list article > header {
  align-items: center;
  display: flex;
  gap: 8px;
  min-width: 0;
}

.prediction-list article > header strong {
  color: var(--text);
  font-size: 12px;
  line-height: 18px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.prediction-list article > header span {
  align-items: center;
  background: var(--positive-soft);
  border-radius: 50%;
  color: var(--positive);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 9px;
  font-weight: 650;
  gap: 4px;
  height: 20px;
  padding: 0 8px;
}

.prediction-list article > header span.settled {
  background: var(--soft);
  color: var(--muted);
}

.prediction-list article > header i {
  background: currentColor;
  border-radius: 50%;
  height: 4px;
  width: 4px;
}

.prediction-list h2 {
  color: var(--text);
  font-size: 14px;
  font-weight: 650;
  line-height: 20px;
  margin: 0;
  overflow-wrap: anywhere;
}

.prediction-outcomes {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  min-width: 0;
}

.prediction-outcomes button {
  align-items: center;
  background: var(--positive-soft);
  border: 0;
  border-radius: 50%;
  color: var(--positive);
  display: flex;
  gap: 8px;
  height: 38px;
  justify-content: center;
  min-height: 38px;
  min-width: 0;
  padding: 0 12px;
}

.prediction-outcomes button:last-child {
  background: var(--negative-soft);
  color: var(--negative);
}

.prediction-outcomes button:disabled {
  cursor: default;
  filter: saturate(.4);
  opacity: .66;
}

.prediction-outcomes button:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: 0;
}

.prediction-outcomes b {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.prediction-outcomes small {
  font-size: 10px;
  font-weight: 650;
}

.prediction-market-meta {
  color: var(--muted);
  display: flex;
  font-size: 10px;
  gap: 6px;
  justify-content: space-between;
  line-height: 15px;
  margin: 0;
  min-width: 0;
}

.prediction-market-meta span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.prediction-market-meta time {
  flex: 0 0 auto;
}

.prediction-state {
  align-content: center;
  color: var(--muted);
  display: grid;
  font-size: 11px;
  gap: 9px;
  justify-items: center;
  min-height: 148px;
  text-align: center;
}

.prediction-state--empty {
  min-height: 112px;
}

.prediction-note {
  align-items: flex-start;
  color: var(--muted);
  display: flex;
  font-size: 11px;
  gap: 8px;
  line-height: 16px;
  margin: 0;
  padding-top: 2px;
}

.prediction-note svg {
  flex: 0 0 auto;
  margin-top: 1px;
}

.prediction-orders {
  border-top: 1px solid var(--line);
  display: grid;
  margin-top: 48px;
  min-width: 0;
}

.prediction-orders .section-heading {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  font-size: 14px;
  justify-content: space-between;
  margin: 0;
  min-height: 52px;
}

.prediction-orders .section-heading b {
  color: var(--positive);
  font-size: 11px;
}

.prediction-orders article {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 14px;
  justify-content: space-between;
  min-height: 70px;
}

.prediction-orders article > div,
.prediction-orders article > span {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.prediction-orders strong,
.prediction-orders b {
  font-size: 12px;
  overflow-wrap: anywhere;
}

.prediction-orders small {
  color: var(--muted);
  font-size: 10px;
}

.prediction-orders .is-positive {
  color: var(--positive);
}

.prediction-orders .is-negative {
  color: var(--negative);
}

.prediction-orders .is-pending {
  color: var(--accent);
}

.prediction-orders article > span {
  flex: 0 0 auto;
  text-align: right;
}

.prediction-mask {
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

.prediction-dialog {
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 8px;
  box-shadow: var(--shadow-soft);
  display: grid;
  gap: 14px;
  max-height: calc(100dvh - max(32px, env(safe-area-inset-top)) - max(32px, env(safe-area-inset-bottom)));
  max-width: 520px;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 17px;
  width: 100%;
}

.prediction-dialog > header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.prediction-dialog > header > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.prediction-dialog > header strong {
  font-size: 17px;
  line-height: 1.35;
  overflow-wrap: anywhere;
}

.prediction-dialog > header small {
  color: var(--muted);
  font-size: 11px;
}

.prediction-field {
  background: var(--field-surface);
  border: 1px solid var(--line);
  border-radius: 8px;
  display: grid;
  min-width: 0;
  padding: 7px 11px 6px;
}

.prediction-field:focus-within {
  background: var(--surface-elevated);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.prediction-field > span {
  color: var(--muted);
  font-size: 10px;
}

.prediction-field > div {
  align-items: center;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 44px;
}

.prediction-field select,
.prediction-field input {
  background: transparent;
  border: 0;
  color: var(--ink);
  min-height: 44px;
  min-width: 0;
  outline: 0;
  width: 100%;
}

.prediction-field select {
  font-size: 12px;
}

.prediction-field input {
  font-size: 20px;
  font-weight: 750;
}

.prediction-field b {
  font-size: 12px;
}

.prediction-quote {
  border-block: 1px solid var(--line);
  display: grid;
  margin: 0;
}

.prediction-quote > div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 44px;
}

.prediction-quote > div:last-child {
  border-bottom: 0;
}

.prediction-quote dt,
.prediction-quote dd {
  font-size: 11px;
  margin: 0;
}

.prediction-quote dt {
  color: var(--muted);
}

.prediction-quote dd {
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

.prediction-submit {
  border-radius: 8px;
  min-height: 52px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 340px) {
  .prediction-content {
    padding-inline: 16px;
  }

  .prediction-market-meta {
    align-items: flex-start;
    flex-direction: column;
  }

  .prediction-orders article {
    align-items: flex-start;
    flex-direction: column;
    padding: 11px 0;
  }

  .prediction-orders article > span {
    align-items: center;
    display: flex;
    justify-content: space-between;
    width: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .prediction-page *,
  .prediction-page *::before,
  .prediction-page *::after {
    animation-duration: .01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: .01ms !important;
  }

  .spin {
    animation: none;
  }
}
</style>
