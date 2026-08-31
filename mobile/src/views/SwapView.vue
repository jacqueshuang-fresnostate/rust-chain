<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  ArrowDownUp,
  Check,
  CheckCircle2,
  ChevronDown,
  CircleAlert,
  History,
  LoaderCircle,
  PackageOpen,
  Search,
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
import {
  decimalCompare,
  decimalTextFromBoundary,
  decimalWithinRange,
  positiveDecimalInput,
} from '@/core/decimal'
import { convertStatusPresentation } from '@/core/financialEnumPresentation'
import { useModalDialog } from '@/core/modalDialog'
import {
  buildSwapAvailableBalanceMap,
  buildSwapPickerAssetLogos,
  resolveReverseSwapPair,
  resolveSelectedSwapPair,
  resolveSwapPickerPair,
  swapPairSelectionKey,
} from '@/core/swapAssetLogos'
import { useNavigationStore } from '@/stores/navigation'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const navigation = useNavigationStore()
const { t } = useI18n()
const pairs = ref<ConvertPair[]>([])
const accounts = ref<WalletAccount[]>([])
const orders = ref<ConvertOrder[]>([])
const pairSelectionKey = ref('')
const amount = ref('')
const quote = ref<ConvertQuote | null>(null)
const loading = ref(false)
const quoting = ref(false)
const confirming = ref(false)
const error = ref('')
const success = ref('')
const reviewOpen = ref(false)
const reviewDialog = ref<HTMLElement | null>(null)
const pickerOpen = ref(false)
const pickerDialog = ref<HTMLElement | null>(null)
const pickerSide = ref<'from' | 'to'>('from')
const pickerFilter = ref<'popular' | 'holding' | 'all'>('popular')
const pickerQuery = ref('')
const historySection = ref<HTMLElement | null>(null)
const { trapFocus: trapReviewFocus } = useModalDialog(reviewOpen, reviewDialog, '[data-dialog-cancel]')
const { trapFocus: trapPickerFocus } = useModalDialog(pickerOpen, pickerDialog, '[data-picker-search]')

const selectedPair = computed(() => resolveSelectedSwapPair(pairs.value, pairSelectionKey.value))
const availableBySymbol = computed(() => buildSwapAvailableBalanceMap(accounts.value))
const availableBalance = (symbol: string): number => availableBySymbol.value.get(symbol.trim().toUpperCase()) || 0
const available = computed(() => selectedPair.value ? availableBalance(selectedPair.value.fromAssetSymbol) : 0)
const selectedAccount = computed(() => accounts.value.find((account) => (
  account.symbol === selectedPair.value?.fromAssetSymbol
)))
const availableText = computed(() => decimalTextFromBoundary(
  selectedAccount.value?.availableText ?? selectedAccount.value?.available,
  { allowNegative: false },
))
const amountText = computed(() => positiveDecimalInput(amount.value))
const amountAllowed = computed(() => {
  const pair = selectedPair.value
  return Boolean(pair && decimalWithinRange(amountText.value, {
    minimum: pair.minAmountText ?? pair.minAmount,
    maximum: pair.maxAmountText ?? pair.maxAmount,
  }))
})
const quoteExpired = computed(() => !quote.value || quote.value.expiresAt <= Date.now())
const pickerAssets = computed(() => {
  const needle = pickerQuery.value.trim().toUpperCase()
  const assets = buildSwapPickerAssetLogos(pairs.value, pickerSide.value).map((asset) => ({
    ...asset,
    balance: availableBalance(asset.symbol),
  })).filter((asset) => (
    (!needle || asset.symbol.includes(needle))
    && (pickerFilter.value !== 'holding' || asset.balance > 0)
  ))
  return pickerFilter.value === 'popular' && !needle ? assets.slice(0, 6) : assets
})

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    const nextPairs = await fetchConvertPairs()
    const nextSelectedPair = resolveSelectedSwapPair(nextPairs, pairSelectionKey.value)
    pairs.value = nextPairs
    pairSelectionKey.value = nextSelectedPair ? swapPairSelectionKey(nextSelectedPair) : ''
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
  const reversed = resolveReverseSwapPair(pairs.value, pair)
  if (reversed) pairSelectionKey.value = swapPairSelectionKey(reversed)
  quote.value = null
  error.value = ''
  success.value = ''
}

function useMaximum(): void {
  amount.value = availableText.value ? String(availableText.value) : ''
  quote.value = null
}

function openPicker(side: 'from' | 'to'): void {
  pickerSide.value = side
  pickerFilter.value = 'popular'
  pickerQuery.value = ''
  pickerOpen.value = true
}

function closePicker(): void {
  pickerOpen.value = false
}

function selectPickerAsset(symbol: string): void {
  const pair = resolveSwapPickerPair(pairs.value, pickerSide.value, symbol, selectedPair.value)
  if (!pair) return
  pairSelectionKey.value = swapPairSelectionKey(pair)
  quote.value = null
  error.value = ''
  success.value = ''
  closePicker()
}

function openHistory(): void {
  historySection.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
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
  if (amountText.value && availableText.value && decimalCompare(amountText.value, availableText.value) > 0) {
    error.value = t('swap.exceedsBalance')
    return
  }
  quoting.value = true
  try {
    const requestAmount = amountText.value
    if (!requestAmount) throw new TypeError('invalid convert amount')
    quote.value = await requestConvertQuote(selectedPair.value, requestAmount)
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

function handlePickerKeydown(event: KeyboardEvent): void {
  trapPickerFocus(event, closePicker)
}

function orderStatusLabel(status: string): string {
  const presentation = convertStatusPresentation(status)
  return t(presentation.translationKey, { source: presentation.source || '--' })
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain pencil-page swap-pencil" data-pencil-source="x9T4CL eXdnN sf288 xvVss">
    <PageHeader :back="true" :fallback="navigation.lastTradePath" :pencil="true" :title="t('swap.title')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('swap.history')" @click="openHistory"><History :size="18" /></button>
      </template>
    </PageHeader>

    <div class="pencil-content swap-pencil__content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('swap.loginDescription')" />
      <template v-else>
        <div v-if="error && !reviewOpen" class="pencil-message pencil-message--error" role="alert"><CircleAlert :size="18" /><span>{{ error }}</span></div>
        <div v-if="success" class="pencil-message pencil-message--success" role="status"><CheckCircle2 :size="18" /><span>{{ success }}</span></div>
        <div v-if="loading" class="pencil-state" aria-live="polite"><LoaderCircle :size="24" class="spin" /><span>{{ t('swap.loading') }}</span></div>

        <template v-else-if="selectedPair">
          <section class="swap-workspace-pencil">
            <div class="swap-card">
              <div class="swap-card__header">
                <span>{{ t('swap.pay') }}</span>
                <span class="swap-card__balance">
                  <small>{{ t('swap.available', { amount: formatAmount(available), asset: selectedPair.fromAssetSymbol }) }}</small>
                  <button type="button" @click="useMaximum">{{ t('swap.all') }}</button>
                </span>
              </div>
              <div class="swap-card__main">
                <input v-model="amount" class="pencil-numeric" inputmode="decimal" placeholder="0.00" @input="quote = null" />
                <button class="swap-asset-button" type="button" @click="openPicker('from')">
                  <AssetMark :symbol="selectedPair.fromAssetSymbol" :src="selectedPair.fromAssetLogoUrl" :size="28" />
                  <strong>{{ selectedPair.fromAssetSymbol }}</strong>
                  <ChevronDown :size="16" />
                </button>
              </div>
            </div>

            <button class="swap-direction-pencil" type="button" :aria-label="t('swap.direction')" @click="swapDirection"><ArrowDownUp :size="19" /></button>

            <div class="swap-card">
              <div class="swap-card__header"><span>{{ t('swap.receive') }}</span><small>{{ t('swap.quoteResult') }}</small></div>
              <div class="swap-card__main">
                <strong class="pencil-numeric swap-receive-value">{{ quote ? formatAmount(quote.toAmount) : t('swap.awaitingQuote') }}</strong>
                <button class="swap-asset-button" type="button" @click="openPicker('to')">
                  <AssetMark :symbol="selectedPair.toAssetSymbol" :src="selectedPair.toAssetLogoUrl" :size="28" />
                  <strong>{{ selectedPair.toAssetSymbol }}</strong>
                  <ChevronDown :size="16" />
                </button>
              </div>
            </div>
          </section>

          <dl class="swap-meta-pencil">
            <div>
              <dt>{{ t('swap.referenceRate') }}</dt>
              <dd class="pencil-numeric">{{ quote ? `1 ${selectedPair.fromAssetSymbol} = ${formatPrice(quote.rate)} ${selectedPair.toAssetSymbol}` : t('swap.afterQuote') }}</dd>
            </div>
            <div><dt>{{ t('swap.feeRate') }}</dt><dd class="pencil-numeric">{{ formatPrice(selectedPair.feeRate * 100) }}%</dd></div>
            <div>
              <dt>{{ t(quote ? 'swap.validUntil' : 'swap.minimum') }}</dt>
              <dd class="pencil-numeric">{{ quote ? formatDateTime(quote.expiresAt) : `${formatAmount(selectedPair.minAmount)} ${selectedPair.fromAssetSymbol}` }}</dd>
            </div>
          </dl>

          <button
            v-if="!quote"
            class="pencil-primary pencil-primary--full swap-submit-pencil"
            type="button"
            :disabled="quoting"
            :aria-busy="quoting"
            @click="getQuote"
          >
            {{ quoting ? t('swap.quoting') : t('swap.getQuote') }}
          </button>
          <button
            v-else
            class="pencil-primary pencil-primary--full swap-submit-pencil"
            type="button"
            :disabled="confirming || quoteExpired"
            :aria-busy="confirming"
            @click="openReview"
          >
            {{ quoteExpired ? t('swap.quoteExpired') : confirming ? t('swap.confirming') : t('swap.confirm', { amount: formatAmount(quote.toAmount), asset: selectedPair.toAssetSymbol }) }}
          </button>

          <p class="swap-helper-pencil">
            {{ t('swap.spotWalletNote') }}
          </p>

          <section ref="historySection" class="pencil-section swap-history-pencil">
            <div class="pencil-section__heading">
              <h2>{{ t('swap.history') }}</h2>
              <button type="button" :disabled="loading" @click="load">{{ t('swap.all') }}</button>
            </div>
            <div v-if="orders.length" class="pencil-list">
              <article v-for="order in orders" :key="order.id" class="pencil-row swap-history-row">
                <span class="pencil-row__copy"><strong>{{ order.fromAssetSymbol || t('swap.asset') }} → {{ order.toAssetSymbol || t('swap.asset') }}</strong><small>{{ formatDateTime(order.createdAt) }}</small></span>
                <span class="pencil-row__value"><strong class="pencil-numeric">{{ formatAmount(order.fromAmount) }} → {{ formatAmount(order.toAmount) }}</strong><small>{{ orderStatusLabel(order.status) }}</small></span>
              </article>
            </div>
            <div v-else class="pencil-state"><PackageOpen :size="22" /><span>{{ t('swap.emptyHistory') }}</span></div>
          </section>
        </template>
        <div v-else-if="!error" class="pencil-state"><PackageOpen :size="23" /><span>{{ t('swap.noPairs') }}</span></div>
      </template>
    </div>

    <div v-if="pickerOpen && selectedPair" class="pencil-sheet-mask" @click.self="closePicker">
      <section ref="pickerDialog" class="pencil-sheet swap-picker" role="dialog" aria-modal="true" :aria-label="t('swap.selectAsset')" @keydown="handlePickerKeydown">
        <div class="pencil-sheet__handle" />
        <header>
          <h2>{{ t('swap.selectAsset') }}</h2>
          <button class="icon-button" type="button" :aria-label="t('common.close')" @click="closePicker"><X :size="20" /></button>
        </header>
        <label class="swap-picker__search">
          <Search :size="17" />
          <input v-model="pickerQuery" data-picker-search type="search" :placeholder="t('swap.searchAsset')" />
        </label>
        <nav class="pencil-segmented swap-picker__tabs" :aria-label="t('swap.assetFilter')">
          <button type="button" :aria-pressed="pickerFilter === 'popular'" @click="pickerFilter = 'popular'">{{ t('markets.popular') }}</button>
          <button type="button" :aria-pressed="pickerFilter === 'holding'" @click="pickerFilter = 'holding'">{{ t('swap.holdings') }}</button>
          <button type="button" :aria-pressed="pickerFilter === 'all'" @click="pickerFilter = 'all'">{{ t('swap.allAssets') }}</button>
        </nav>
        <div v-if="pickerAssets.length" class="pencil-list swap-picker__list">
          <button
            v-for="asset in pickerAssets"
            :key="asset.symbol"
            class="pencil-row"
            :class="{ 'is-selected': asset.symbol === (pickerSide === 'from' ? selectedPair.fromAssetSymbol : selectedPair.toAssetSymbol) }"
            type="button"
            :aria-pressed="asset.symbol === (pickerSide === 'from' ? selectedPair.fromAssetSymbol : selectedPair.toAssetSymbol)"
            @click="selectPickerAsset(asset.symbol)"
          >
            <AssetMark :symbol="asset.symbol" :src="asset.logoUrl" :size="38" />
            <span class="pencil-row__copy"><strong>{{ asset.symbol }}</strong><small class="pencil-numeric">{{ t('swap.available', { amount: formatAmount(asset.balance), asset: asset.symbol }) }}</small></span>
            <span class="pencil-row__value">
              <Check v-if="asset.symbol === (pickerSide === 'from' ? selectedPair.fromAssetSymbol : selectedPair.toAssetSymbol)" :size="17" class="up" />
            </span>
          </button>
        </div>
        <div v-else class="pencil-state"><PackageOpen :size="22" /><span>{{ t('swap.noMatchingAssets') }}</span></div>
      </section>
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
          <button class="icon-button" type="button" :aria-label="t('common.close')" :disabled="confirming" @click="closeReview"><X :size="21" /></button>
        </header>
        <dl class="swap-review__summary">
          <div><dt>{{ t('swap.pay') }}</dt><dd class="pencil-numeric">{{ formatAmount(quote.fromAmount) }} {{ selectedPair.fromAssetSymbol }}</dd></div>
          <div><dt>{{ t('swap.receive') }}</dt><dd class="pencil-numeric up">{{ formatAmount(quote.toAmount) }} {{ selectedPair.toAssetSymbol }}</dd></div>
          <div><dt>{{ t('swap.referenceRate') }}</dt><dd class="pencil-numeric">1 {{ selectedPair.fromAssetSymbol }} = {{ formatPrice(quote.rate) }} {{ selectedPair.toAssetSymbol }}</dd></div>
          <div><dt>{{ t('common.fee') }}</dt><dd class="pencil-numeric">{{ formatAmount(quote.feeAmount) }} {{ selectedPair.fromAssetSymbol }}</dd></div>
        </dl>
        <p v-if="error" class="swap-review__error" role="alert">{{ error }}</p>
        <div class="swap-review__actions">
          <button class="pencil-secondary" type="button" :disabled="confirming" data-dialog-cancel @click="closeReview">{{ t('common.cancel') }}</button>
          <button class="pencil-primary" type="button" :disabled="confirming || quoteExpired" :aria-busy="confirming" @click="confirm">{{ confirming ? t('swap.confirming') : t('common.confirm') }}</button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.swap-pencil {
  --pencil-header-height: 58px;
  --pencil-header-inline: 14px;
}

.swap-pencil :deep(.pencil-page-header) {
  height: var(--pencil-header-height);
  min-height: var(--pencil-header-height);
  padding-block: 7px;
}

.swap-pencil__content {
  min-height: 639px;
  padding-bottom: calc(12px + env(safe-area-inset-bottom));
  padding-top: 0;
}

.swap-workspace-pencil {
  height: 252px;
  position: relative;
}

.swap-card {
  background: var(--surface-2);
  border: 1px solid transparent;
  border-radius: 14px;
  box-sizing: border-box;
  display: grid;
  gap: 2px;
  height: 90px;
  left: 0;
  padding: 10px 14px 8px;
  position: absolute;
  right: 0;
  top: 8px;
}

.swap-card:nth-of-type(2) {
  top: 162px;
}

.swap-card:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.swap-card__header,
.swap-card__balance {
  align-items: center;
  display: flex;
}

.swap-card__header {
  justify-content: space-between;
  min-width: 0;
}

.swap-card__header > span:first-child,
.swap-card__header small {
  color: var(--muted);
  font-size: 10px;
  font-weight: 500;
}

.swap-card__balance {
  gap: 7px;
  justify-content: flex-end;
  min-width: 0;
}

.swap-card__balance small {
  max-width: 190px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.swap-card__balance button {
  background: transparent;
  color: var(--positive);
  font-size: 10px;
  font-weight: 600;
  min-height: 28px;
  padding: 0;
}

.swap-card__main {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) auto;
  min-width: 0;
}

.swap-card__main > input {
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 28px;
  font-weight: 650;
  height: 44px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.swap-card__main > input:focus-visible {
  outline: 0;
}

.swap-asset-button {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: 999px;
  color: var(--ink);
  display: flex;
  gap: 6px;
  height: 44px;
  padding: 0 10px 0 7px;
}

.swap-asset-button:focus-visible,
.swap-card__balance button:focus-visible,
.swap-history-pencil button:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.swap-asset-button strong {
  font-size: 13px;
  font-weight: 650;
}

.swap-receive-value {
  font-size: 26px;
  font-weight: 650;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.swap-direction-pencil {
  align-items: center;
  align-self: center;
  background: transparent;
  border: 0;
  border-radius: 50%;
  color: var(--ink);
  display: flex;
  height: 44px;
  justify-content: center;
  justify-self: center;
  padding: 0;
  position: absolute;
  top: 108px;
  left: calc(50% - 22px);
  width: 44px;
  z-index: 1;
}

.swap-direction-pencil::before {
  background: var(--surface);
  border: 1px solid var(--line-strong);
  border-radius: 50%;
  content: '';
  height: 40px;
  inset: 2px;
  position: absolute;
  width: 40px;
}

.swap-direction-pencil > svg {
  position: relative;
  z-index: 1;
}

.swap-direction-pencil:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.swap-meta-pencil {
  display: grid;
  height: 87px;
  margin: 12px 0 0;
}

.swap-meta-pencil > div {
  align-items: center;
  display: flex;
  font-size: 11px;
  justify-content: space-between;
  min-height: 29px;
}

.swap-meta-pencil dt {
  color: var(--muted);
}

.swap-meta-pencil dd {
  color: var(--ink);
  font-size: 10px;
  font-weight: 600;
  margin: 0;
  max-width: 72%;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.swap-submit-pencil {
  margin-top: 12px;
  min-height: 56px;
}

.swap-helper-pencil {
  color: var(--muted);
  font-size: 10px;
  line-height: 18px;
  margin: 10px 0 0;
  min-height: 20px;
}

.swap-history-pencil {
  height: 168px;
  margin-top: 10px;
  scroll-margin-top: 60px;
}

.swap-history-pencil .pencil-section__heading {
  min-height: 30px;
}

.swap-history-pencil .pencil-section__heading h2 {
  font-size: 14px;
}

.swap-history-pencil .pencil-section__heading button {
  color: var(--muted);
  min-height: 24px;
}

.swap-history-row {
  grid-template-columns: minmax(0, 1fr) minmax(92px, auto);
  min-height: 69px;
}

.swap-history-row .pencil-row__copy strong,
.swap-history-row .pencil-row__value strong {
  font-size: 11px;
}

.swap-history-row .pencil-row__value {
  align-items: end;
  display: grid;
  gap: 3px;
}

.swap-history-row .pencil-row__value small {
  color: var(--positive);
}

.swap-picker {
  overflow-x: hidden;
}

.swap-picker :deep(.icon-button) {
  background: var(--surface-2);
  border: 0;
  border-radius: 50%;
  box-shadow: none;
  color: var(--ink);
  height: 44px;
  min-height: 44px;
  padding: 0;
  width: 44px;
}

.swap-picker__search {
  align-items: center;
  background: var(--surface-2);
  border: 1px solid transparent;
  border-radius: 12px;
  display: flex;
  gap: 9px;
  height: 52px;
  padding: 0 12px;
}

.swap-picker__search:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.swap-picker__search input {
  background: transparent;
  border: 0;
  color: var(--ink);
  height: 50px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.swap-picker__search input:focus-visible {
  outline: 0;
}

.swap-picker__tabs {
  gap: 22px;
  margin-top: 0;
  min-height: 38px;
}

.swap-picker__tabs button {
  font-size: 12px;
  min-height: 38px;
}

.swap-picker__list {
  margin: 0 -8px;
}

.swap-picker__list .pencil-row {
  border-radius: 12px;
  grid-template-columns: 38px minmax(0, 1fr) 22px;
  min-height: 64px;
  padding: 0 8px;
}

.swap-picker .pencil-sheet__handle {
  margin-bottom: 12px;
}

.swap-picker > header {
  height: 46px;
  min-height: 46px;
}

.swap-picker__list .pencil-row.is-selected {
  background: var(--accent-soft);
}

.swap-picker__list .pencil-row__value {
  color: var(--positive);
  min-width: 22px;
}

.swap-pencil :deep(.asset-mark) {
  border: 0;
  box-shadow: none;
}

.swap-review-mask {
  align-items: end;
  background: var(--overlay);
  display: grid;
  inset: 0;
  position: fixed;
  z-index: var(--layer-overlay);
}

.swap-review {
  background: var(--surface-elevated);
  border-radius: 20px 20px 0 0;
  box-shadow: none;
  padding: 18px 16px calc(18px + env(safe-area-inset-bottom));
  width: 100%;
}

.swap-review > header {
  align-items: start;
  display: flex;
  justify-content: space-between;
}

.swap-review > header div {
  display: grid;
  gap: 4px;
}

.swap-review > header span,
.swap-review > header small {
  color: var(--muted);
  font-size: 10px;
}

.swap-review h2 {
  font-size: 18px;
  margin: 0;
}

.swap-review__summary {
  display: grid;
  gap: 10px;
  margin: 18px 0;
}

.swap-review__summary > div {
  align-items: center;
  display: flex;
  font-size: 11px;
  justify-content: space-between;
}

.swap-review__summary dt {
  color: var(--muted);
}

.swap-review__summary dd {
  margin: 0;
  text-align: right;
}

.swap-review__error {
  color: var(--negative);
  font-size: 11px;
}

.swap-review__actions {
  display: grid;
  gap: 10px;
  grid-template-columns: 1fr 1fr;
}

@media (max-width: 340px) {
  .swap-card {
    padding-inline: 12px;
  }

  .swap-card__balance small {
    max-width: 136px;
  }

  .swap-card__main > input,
  .swap-receive-value {
    font-size: 23px;
  }

  .swap-asset-button {
    padding-right: 7px;
  }

  .swap-meta-pencil dd {
    max-width: 64%;
  }
}
</style>
