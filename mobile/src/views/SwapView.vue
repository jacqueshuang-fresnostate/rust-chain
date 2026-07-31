<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  ArrowDownUp,
  CheckCircle2,
  CircleAlert,
  History,
  LoaderCircle,
  PackageOpen,
  RefreshCw,
  X,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { confirmConvertQuote, fetchConvertOrders, fetchConvertPairs, requestConvertQuote, type ConvertOrder, type ConvertPair, type ConvertQuote } from '@/api/swap'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import { useModalDialog } from '@/core/modalDialog'
import { useNavigationStore } from '@/stores/navigation'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const navigation = useNavigationStore()
const { t } = useI18n()
const pairs = ref<ConvertPair[]>([])
const accounts = ref<WalletAccount[]>([])
const orders = ref<ConvertOrder[]>([])
const pairId = ref(0)
const amount = ref('')
const quote = ref<ConvertQuote | null>(null)
const loading = ref(false)
const quoting = ref(false)
const confirming = ref(false)
const error = ref('')
const success = ref('')
const reviewOpen = ref(false)
const reviewDialog = ref<HTMLElement | null>(null)
const { trapFocus: trapReviewFocus } = useModalDialog(reviewOpen, reviewDialog, '[data-dialog-cancel]')

const selectedPair = computed(() => pairs.value.find((pair) => pair.id === pairId.value) || pairs.value[0])
const available = computed(() => accounts.value.find((account) => account.symbol === selectedPair.value?.fromAssetSymbol)?.available || 0)
const amountNumber = computed(() => Number(amount.value || 0))
const amountAllowed = computed(() => {
  const pair = selectedPair.value
  if (!pair || !Number.isFinite(amountNumber.value)) return false
  return amountNumber.value >= pair.minAmount && (!pair.maxAmount || amountNumber.value <= pair.maxAmount)
})
const quoteExpired = computed(() => !quote.value || quote.value.expiresAt <= Date.now())

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    pairs.value = await fetchConvertPairs()
    pairId.value = pairs.value[0]?.id || 0
    if (session.isAuthenticated) {
      const [wallets, history] = await Promise.all([fetchWalletAccounts(), fetchConvertOrders()])
      accounts.value = wallets
      orders.value = history
    }
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('swap.loadFailed'))
  } finally {
    loading.value = false
  }
}

function swapDirection(): void {
  const pair = selectedPair.value
  if (!pair) return
  const reversed = pairs.value.find((item) => item.fromAssetId === pair.toAssetId && item.toAssetId === pair.fromAssetId)
  if (reversed) pairId.value = reversed.id
  quote.value = null
}

function useMaximum(): void {
  amount.value = String(available.value)
  quote.value = null
}

async function getQuote(): Promise<void> {
  error.value = ''
  success.value = ''
  quote.value = null
  if (!session.isAuthenticated) return
  if (!selectedPair.value || !amountAllowed.value) {
    error.value = t('swap.invalidAmount')
    return
  }
  if (amountNumber.value > available.value) {
    error.value = t('swap.exceedsBalance')
    return
  }
  quoting.value = true
  try {
    quote.value = await requestConvertQuote(selectedPair.value, amountNumber.value)
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('swap.quoteFailed'))
  } finally {
    quoting.value = false
  }
}

function openReview(): void {
  if (!quote.value || quoteExpired.value) {
    error.value = t('swap.expired')
    quote.value = null
    return
  }
  error.value = ''
  reviewOpen.value = true
}

function closeReview(): void {
  if (confirming.value) return
  reviewOpen.value = false
  error.value = ''
}

async function confirm(): Promise<void> {
  if (!quote.value || quoteExpired.value) {
    error.value = t('swap.expired')
    quote.value = null
    reviewOpen.value = false
    return
  }
  confirming.value = true
  error.value = ''
  try {
    await confirmConvertQuote(quote.value.quoteId)
    success.value = t('swap.completed')
    quote.value = null
    amount.value = ''
    reviewOpen.value = false
    const [wallets, history] = await Promise.all([fetchWalletAccounts(), fetchConvertOrders()])
    accounts.value = wallets
    orders.value = history
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('swap.confirmFailed'))
  } finally {
    confirming.value = false
  }
}

function handleReviewKeydown(event: KeyboardEvent): void {
  trapReviewFocus(event, closeReview)
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain swap-page">
    <PageHeader
      :back="true"
      :eyebrow="t('products.title')"
      :fallback="navigation.lastTradePath"
      :subtitle="t('swap.loginDescription')"
      :title="t('swap.title')"
    >
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('swap.refresh')"
          :disabled="loading"
          @click="load"
        >
          <RefreshCw :size="20" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content swap-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('swap.loginDescription')" />
      <template v-else>
        <div v-if="error && !reviewOpen" class="swap-message swap-message--error" role="alert">
          <CircleAlert :size="18" />
          <span>{{ error }}</span>
        </div>
        <div v-if="success" class="swap-message swap-message--success" role="status">
          <CheckCircle2 :size="18" />
          <span>{{ success }}</span>
        </div>
        <div v-if="loading" class="swap-state" aria-live="polite">
          <LoaderCircle :size="24" class="spin" />
          <span>{{ t('swap.loading') }}</span>
        </div>
        <template v-else-if="selectedPair">
          <section class="swap-workspace">
            <label class="swap-field">
              <span>{{ t('swap.pay') }}</span>
              <div class="swap-input">
                <AssetMark :symbol="selectedPair.fromAssetSymbol" :size="32" />
                <select v-model="pairId" @change="quote = null">
                  <option v-for="pair in pairs" :key="pair.id" :value="pair.id">{{ pair.fromAssetSymbol }}</option>
                </select>
                <input v-model="amount" class="numeric" inputmode="decimal" placeholder="0.00" @input="quote = null" />
                <button type="button" @click="useMaximum">{{ t('swap.all') }}</button>
              </div>
              <small>{{ t('swap.available', { amount: formatAmount(available), asset: selectedPair.fromAssetSymbol }) }}</small>
            </label>

            <button class="swap-direction" type="button" :aria-label="t('swap.direction')" @click="swapDirection">
              <ArrowDownUp :size="20" />
            </button>

            <div class="swap-field">
              <span>{{ t('swap.receive') }}</span>
              <div class="swap-input swap-input--receive">
                <AssetMark :symbol="selectedPair.toAssetSymbol" :size="32" />
                <span>{{ selectedPair.toAssetSymbol }}</span>
                <strong class="numeric">{{ quote ? formatAmount(quote.toAmount) : '--' }}</strong>
              </div>
            </div>
          </section>

          <section class="swap-meta">
            <div><span>{{ t('swap.minimum') }}</span><b>{{ formatAmount(selectedPair.minAmount) }} {{ selectedPair.fromAssetSymbol }}</b></div>
            <div><span>{{ t('swap.feeRate') }}</span><b>{{ formatPrice(selectedPair.feeRate * 100) }}%</b></div>
            <div v-if="quote"><span>{{ t('swap.referenceRate') }}</span><b>1 {{ selectedPair.fromAssetSymbol }} = {{ formatPrice(quote.rate) }} {{ selectedPair.toAssetSymbol }}</b></div>
          </section>

          <button
            v-if="!quote"
            class="button button--primary button--full swap-submit"
            type="button"
            :disabled="quoting"
            :aria-busy="quoting"
            @click="getQuote"
          >
            {{ quoting ? t('swap.quoting') : t('swap.getQuote') }}
          </button>
          <button
            v-else
            class="button button--primary button--full swap-submit"
            type="button"
            :disabled="confirming || quoteExpired"
            :aria-busy="confirming"
            @click="openReview"
          >
            {{ quoteExpired ? t('swap.quoteExpired') : confirming ? t('swap.confirming') : t('swap.confirm', { amount: formatAmount(quote.toAmount), asset: selectedPair.toAssetSymbol }) }}
          </button>

          <section class="swap-history">
            <div class="section-heading">
              <span>{{ t('swap.history') }}</span>
              <History :size="19" />
            </div>
            <article v-for="order in orders" :key="order.id" class="swap-history__row">
              <div><strong>{{ order.fromAssetSymbol || t('swap.asset') }} → {{ order.toAssetSymbol || t('swap.asset') }}</strong><small>{{ formatDateTime(order.createdAt) }}</small></div>
              <span><b>{{ formatAmount(order.fromAmount) }} → {{ formatAmount(order.toAmount) }}</b><small>{{ order.status }}</small></span>
            </article>
            <div v-if="!orders.length" class="swap-state swap-state--empty">
              <PackageOpen :size="22" />
              <span>{{ t('swap.emptyHistory') }}</span>
            </div>
          </section>
        </template>
        <div v-else class="swap-state swap-state--empty">
          <PackageOpen :size="23" />
          <span>{{ t('swap.noPairs') }}</span>
        </div>
      </template>
    </div>

    <div v-if="reviewOpen && quote && selectedPair" class="swap-review-mask" @click.self="closeReview">
      <section
        ref="reviewDialog"
        class="swap-review"
        role="dialog"
        aria-modal="true"
        aria-labelledby="swap-review-title"
        aria-describedby="swap-review-description"
        @keydown="handleReviewKeydown"
      >
        <header>
          <div>
            <span>{{ t('swap.title') }}</span>
            <h2 id="swap-review-title">{{ t('swap.confirm', { amount: formatAmount(quote.toAmount), asset: selectedPair.toAssetSymbol }) }}</h2>
            <small id="swap-review-description">{{ t('swap.loginDescription') }}</small>
          </div>
          <button class="icon-button" type="button" :aria-label="t('common.close')" :disabled="confirming" @click="closeReview">
            <X :size="21" aria-hidden="true" />
          </button>
        </header>
        <dl class="swap-review__summary">
          <div><dt>{{ t('swap.pay') }}</dt><dd class="numeric">{{ formatAmount(quote.fromAmount) }} {{ selectedPair.fromAssetSymbol }}</dd></div>
          <div><dt>{{ t('swap.receive') }}</dt><dd class="numeric up">{{ formatAmount(quote.toAmount) }} {{ selectedPair.toAssetSymbol }}</dd></div>
          <div><dt>{{ t('swap.referenceRate') }}</dt><dd class="numeric">1 {{ selectedPair.fromAssetSymbol }} = {{ formatPrice(quote.rate) }} {{ selectedPair.toAssetSymbol }}</dd></div>
          <div><dt>{{ t('common.fee') }}</dt><dd class="numeric">{{ formatAmount(quote.feeAmount) }} {{ selectedPair.fromAssetSymbol }}</dd></div>
        </dl>
        <p v-if="error" class="swap-review__error" role="alert">{{ error }}</p>
        <div class="swap-review__actions">
          <button class="button button--secondary" type="button" :disabled="confirming" data-dialog-cancel @click="closeReview">
            {{ t('common.cancel') }}
          </button>
          <button class="button button--primary" type="button" :disabled="confirming || quoteExpired" :aria-busy="confirming" @click="confirm">
            {{ confirming ? t('swap.confirming') : t('common.confirm') }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.swap-page {
  background: var(--surface);
  min-width: 0;
}

.swap-content {
  display: grid;
  gap: 16px;
  min-width: 0;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
  padding-top: 14px;
}

.swap-message {
  align-items: center;
  border: 1px solid currentColor;
  display: grid;
  font-size: 12px;
  gap: 9px;
  grid-template-columns: auto minmax(0, 1fr);
  line-height: 1.45;
  min-height: 52px;
  padding: 8px 11px;
}

.swap-message--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.swap-message--success {
  background: var(--positive-soft);
  color: var(--positive);
}

.swap-state {
  align-content: center;
  color: var(--muted);
  display: grid;
  font-size: 12px;
  gap: 9px;
  justify-items: center;
  min-height: 150px;
  text-align: center;
}

.swap-state--empty {
  min-height: 116px;
}

.swap-workspace {
  background:
    linear-gradient(132deg, color-mix(in srgb, var(--accent) 8%, transparent), transparent 62%),
    var(--surface);
  border-block: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  display: grid;
  gap: 0;
  padding: 12px 0;
}

.swap-field {
  display: grid;
  gap: 7px;
  min-width: 0;
}

.swap-field > span,
.swap-field > small {
  color: var(--muted);
  font-size: 10px;
}

.swap-input {
  align-items: center;
  background: var(--field-surface);
  border: 1px solid var(--line);
  display: grid;
  gap: 9px;
  grid-template-columns: 32px minmax(62px, auto) minmax(0, 1fr) auto;
  min-height: 60px;
  min-width: 0;
  padding: 0 10px;
}

.swap-input:focus-within {
  background: var(--surface-elevated);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.swap-input select {
  appearance: none;
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 14px;
  font-weight: 750;
  min-height: 44px;
  min-width: 0;
  outline: 0;
  width: 100%;
}

.swap-input input {
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 22px;
  font-weight: 780;
  min-height: 44px;
  min-width: 0;
  outline: 0;
  text-align: right;
  width: 100%;
}

.swap-input button {
  background: transparent;
  color: var(--accent);
  font-size: 11px;
  font-weight: 800;
  min-height: 44px;
  padding: 0 2px 0 7px;
}

.swap-input--receive {
  grid-template-columns: 32px minmax(62px, 1fr) auto;
}

.swap-input--receive span {
  font-size: 14px;
  font-weight: 750;
}

.swap-input--receive strong {
  font-size: 22px;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.swap-direction {
  align-items: center;
  align-self: center;
  background: var(--accent);
  border: 3px solid var(--surface);
  border-radius: 50%;
  color: var(--on-accent);
  display: inline-flex;
  height: 46px;
  justify-content: center;
  justify-self: center;
  margin: -2px 0;
  min-height: 46px;
  min-width: 46px;
  width: 46px;
  z-index: 1;
}

.swap-meta {
  border-block: 1px solid var(--line);
  display: grid;
}

.swap-meta > div {
  align-items: center;
  display: flex;
  gap: 14px;
  justify-content: space-between;
  min-height: 44px;
}

.swap-meta > div + div {
  border-top: 1px solid var(--line);
}

.swap-meta span {
  color: var(--muted);
  font-size: 11px;
}

.swap-meta b {
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  overflow-wrap: anywhere;
  text-align: right;
}

.swap-submit {
  border-radius: 0;
  min-height: 52px;
}

.swap-history {
  border-top: 8px solid var(--soft);
  margin: 8px -20px 0;
  padding: 0 20px;
}

.swap-history .section-heading {
  border-bottom: 1px solid var(--line);
  font-size: 16px;
  margin: 0;
  min-height: 56px;
}

.swap-history .section-heading svg {
  color: var(--accent);
}

.swap-history__row {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 14px;
  justify-content: space-between;
  min-height: 68px;
}

.swap-history__row > div,
.swap-history__row > span {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.swap-history__row strong,
.swap-history__row b {
  font-size: 12px;
  overflow-wrap: anywhere;
}

.swap-history__row small {
  color: var(--muted);
  font-size: 10px;
}

.swap-history__row > span {
  flex: 0 0 auto;
  text-align: right;
}

.swap-review-mask {
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

.swap-review {
  background: var(--surface);
  border: 1px solid var(--line);
  border-top: 3px solid var(--accent);
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

.swap-review > header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.swap-review > header > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.swap-review > header span {
  color: var(--accent);
  font-size: 10px;
  font-weight: 800;
}

.swap-review h2 {
  font-size: 18px;
  line-height: 1.3;
  margin: 0;
}

.swap-review > header small {
  color: var(--muted);
  font-size: 10px;
  line-height: 1.4;
}

.swap-review__summary {
  border-block: 1px solid var(--line);
  display: grid;
  margin: 0;
}

.swap-review__summary > div {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 44px;
}

.swap-review__summary > div:last-child {
  border-bottom: 0;
}

.swap-review__summary dt,
.swap-review__summary dd {
  font-size: 11px;
  margin: 0;
}

.swap-review__summary dt {
  color: var(--muted);
  flex: 0 0 auto;
}

.swap-review__summary dd {
  font-weight: 750;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.swap-review__error {
  background: var(--negative-soft);
  border-left: 3px solid var(--negative);
  color: var(--negative);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
  padding: 8px 10px;
}

.swap-review__actions {
  display: grid;
  gap: 9px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.swap-review__actions .button {
  min-height: 48px;
  padding-inline: 10px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 390px) {
  .swap-content {
    padding-left: 14px;
    padding-right: 14px;
  }

  .swap-history {
    margin-left: -14px;
    margin-right: -14px;
    padding-inline: 14px;
  }
}

@media (max-width: 340px) {
  .swap-input {
    gap: 6px;
    grid-template-columns: 30px minmax(52px, auto) minmax(0, 1fr) auto;
    padding-inline: 8px;
  }

  .swap-input--receive {
    grid-template-columns: 30px minmax(52px, 1fr) auto;
  }

  .swap-input input,
  .swap-input--receive strong {
    font-size: 18px;
  }

  .swap-meta > div {
    align-items: flex-start;
    flex-direction: column;
    gap: 4px;
    justify-content: center;
    padding-block: 8px;
  }

  .swap-meta b {
    text-align: left;
  }

  .swap-review__summary > div {
    align-items: flex-start;
    flex-direction: column;
    gap: 4px;
    justify-content: center;
    padding-block: 8px;
  }

  .swap-review__summary dd {
    text-align: left;
  }

  .swap-review__actions {
    grid-template-columns: 1fr;
  }
}
</style>
