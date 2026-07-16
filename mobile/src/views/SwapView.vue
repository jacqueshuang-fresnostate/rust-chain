<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ArrowDownUp, History, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { confirmConvertQuote, fetchConvertOrders, fetchConvertPairs, requestConvertQuote, type ConvertOrder, type ConvertPair, type ConvertQuote } from '@/api/swap'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
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

async function confirm(): Promise<void> {
  if (!quote.value || quoteExpired.value) {
    error.value = t('swap.expired')
    quote.value = null
    return
  }
  confirming.value = true
  error.value = ''
  try {
    await confirmConvertQuote(quote.value.quoteId)
    success.value = t('swap.completed')
    quote.value = null
    amount.value = ''
    const [wallets, history] = await Promise.all([fetchWalletAccounts(), fetchConvertOrders()])
    accounts.value = wallets
    orders.value = history
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('swap.confirmFailed'))
  } finally {
    confirming.value = false
  }
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain swap-page">
    <PageHeader :title="t('swap.title')"><template #actions><button class="icon-button" type="button" :aria-label="t('swap.refresh')" :disabled="loading" @click="load"><RefreshCw :size="21" :class="{ spin: loading }" /></button></template></PageHeader>
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('swap.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p><p v-if="success" class="success-message">{{ success }}</p><p v-if="loading" class="empty-state">{{ t('swap.loading') }}</p>
        <template v-else-if="selectedPair">
          <section class="swap-form"><label><span>{{ t('swap.pay') }}</span><div class="swap-input"><AssetMark :symbol="selectedPair.fromAssetSymbol" :size="30" /><select v-model="pairId" @change="quote = null"><option v-for="pair in pairs" :key="pair.id" :value="pair.id">{{ pair.fromAssetSymbol }}</option></select><input v-model="amount" inputmode="decimal" placeholder="0.00" @input="quote = null" /><button type="button" @click="useMaximum">{{ t('swap.all') }}</button></div><small>{{ t('swap.available', { amount: formatAmount(available), asset: selectedPair.fromAssetSymbol }) }}</small></label><button class="swap-direction" type="button" :aria-label="t('swap.direction')" @click="swapDirection"><ArrowDownUp :size="21" /></button><label><span>{{ t('swap.receive') }}</span><div class="swap-input swap-input--receive"><AssetMark :symbol="selectedPair.toAssetSymbol" :size="30" /><span>{{ selectedPair.toAssetSymbol }}</span><strong>{{ quote ? formatAmount(quote.toAmount) : '--' }}</strong></div></label></section>
          <section class="swap-meta"><div><span>{{ t('swap.minimum') }}</span><b>{{ formatAmount(selectedPair.minAmount) }} {{ selectedPair.fromAssetSymbol }}</b></div><div><span>{{ t('swap.feeRate') }}</span><b>{{ formatPrice(selectedPair.feeRate * 100) }}%</b></div><div v-if="quote"><span>{{ t('swap.referenceRate') }}</span><b>1 {{ selectedPair.fromAssetSymbol }} = {{ formatPrice(quote.rate) }} {{ selectedPair.toAssetSymbol }}</b></div></section>
          <button v-if="!quote" class="button button--primary button--full" type="button" :disabled="quoting" @click="getQuote">{{ quoting ? t('swap.quoting') : t('swap.getQuote') }}</button><button v-else class="button button--primary button--full" type="button" :disabled="confirming || quoteExpired" @click="confirm">{{ quoteExpired ? t('swap.quoteExpired') : confirming ? t('swap.confirming') : t('swap.confirm', { amount: formatAmount(quote.toAmount), asset: selectedPair.toAssetSymbol }) }}</button>
          <section class="swap-history"><div class="section-heading"><span>{{ t('swap.history') }}</span><History :size="20" /></div><article v-for="order in orders" :key="order.id" class="swap-history__row"><div><strong>{{ order.fromAssetSymbol || t('swap.asset') }} → {{ order.toAssetSymbol || t('swap.asset') }}</strong><small>{{ formatDateTime(order.createdAt) }}</small></div><span><b>{{ formatAmount(order.fromAmount) }} → {{ formatAmount(order.toAmount) }}</b><small>{{ order.status }}</small></span></article><p v-if="!orders.length" class="empty-state">{{ t('swap.emptyHistory') }}</p></section>
        </template>
        <p v-else class="empty-state">{{ t('swap.noPairs') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.swap-page .page-content { display: grid; gap: 18px; padding-bottom: 42px; padding-top: 16px; }.swap-form { background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); display: grid; gap: 10px; padding: 14px; }.swap-form label { display: grid; gap: 8px; }.swap-form label > span,.swap-form small { color: var(--muted); font-size: 12px; }.swap-input { align-items: center; background: white; border: 1px solid transparent; border-radius: var(--radius); display: grid; gap: 9px; grid-template-columns: 30px minmax(67px, auto) 1fr auto; min-height: 58px; padding: 0 10px; }.swap-input:focus-within { border-color: var(--accent); }.swap-input select { appearance: none; background: transparent; border: 0; color: var(--ink); font-size: 15px; font-weight: 700; max-width: 88px; outline: 0; }.swap-input input { background: transparent; border: 0; color: var(--ink); font-size: 21px; font-weight: 740; min-width: 0; outline: 0; text-align: right; width: 100%; }.swap-input button { background: transparent; color: var(--accent); font-size: 12px; font-weight: 720; padding: 5px 0 5px 5px; }.swap-input--receive { grid-template-columns: 30px 1fr auto; }.swap-input--receive span { font-size: 15px; font-weight: 700; }.swap-input--receive strong { font-size: 21px; }.swap-direction { align-items: center; background: var(--ink); border: 3px solid var(--soft); border-radius: 50%; color: white; display: inline-flex; height: 40px; justify-content: center; justify-self: center; margin: -4px 0; width: 40px; z-index: 1; }.swap-meta { border-top: 1px solid var(--line); display: grid; margin-top: -2px; }.swap-meta div { align-items: center; display: flex; justify-content: space-between; min-height: 38px; }.swap-meta span { color: var(--muted); font-size: 12px; }.swap-meta b { font-size: 13px; text-align: right; }.swap-history { border-top: 1px solid var(--line); margin-top: 6px; }.swap-history .section-heading { align-items: center; margin-top: 20px; }.swap-history__row { align-items: center; border-bottom: 1px solid var(--line); display: flex; justify-content: space-between; min-height: 62px; }.swap-history__row div,.swap-history__row > span { display: grid; gap: 5px; }.swap-history__row strong,.swap-history__row b { font-size: 13px; }.swap-history__row small { color: var(--muted); font-size: 11px; }.swap-history__row > span { text-align: right; }.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: 0; }.spin { animation: spin .8s linear infinite; }@keyframes spin { to { transform: rotate(360deg); } }
</style>
