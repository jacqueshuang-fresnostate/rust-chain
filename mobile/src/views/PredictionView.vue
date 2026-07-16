<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { CircleDollarSign, X } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { confirmPredictionQuote, fetchPredictionConfig, fetchPredictionMarkets, fetchPredictionOrders, requestPredictionQuote, type PredictionAsset, type PredictionMarket, type PredictionOrder, type PredictionOutcome, type PredictionQuote } from '@/api/prediction'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime, formatPercent } from '@/core/format'
import { localizePredictionMarketText, type PredictionTextKind } from '@/core/predictionLocale'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
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

const selectedAsset = computed(() => assets.value.find((asset) => asset.assetId === assetId.value))
const selectedAccount = computed(() => accounts.value.find((account) => account.assetId === assetId.value))
const amountNumber = computed(() => Number(amount.value || 0))
const valid = computed(() => Number.isFinite(amountNumber.value) && amountNumber.value > 0 && amountNumber.value <= (selectedAccount.value?.available || 0))

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
  if (!session.isAuthenticated) return
  selected.value = market
  outcome.value = nextOutcome
  assetId.value = assets.value.find((asset) => accounts.value.some((account) => account.assetId === asset.assetId))?.assetId || assets.value[0]?.assetId || 0
  amount.value = ''
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
    await confirmPredictionQuote(quote.value.quoteId)
    selected.value = null
    quote.value = null
    success.value = t('prediction.created')
    const [wallets, history] = await Promise.all([fetchWalletAccounts(), fetchPredictionOrders()])
    accounts.value = wallets
    orders.value = history
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

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain prediction-page"><PageHeader :title="t('prediction.title')" /><div class="page-content"><p v-if="error" class="error-message">{{ error }}</p><p v-if="success" class="success-message">{{ success }}</p><p v-if="loading" class="empty-state">{{ t('prediction.loading') }}</p><template v-else><section class="prediction-intro"><CircleDollarSign :size="25" /><div><strong>{{ t('prediction.title') }}</strong><p>{{ t('prediction.introDescription') }}</p></div></section><div class="prediction-list"><article v-for="market in markets" :key="market.id"><span>{{ marketText(market.category, 'category') || t('prediction.market') }}</span><h2>{{ marketText(market.title, 'title') }}</h2><p v-if="market.description">{{ marketText(market.description, 'description') }}</p><div><button type="button" @click="openOrder(market, 'yes')"><b>{{ outcomeLabel(market.yesLabel) }}</b><small>{{ formatPercent(market.yesPrice * 100) }}</small></button><button type="button" @click="openOrder(market, 'no')"><b>{{ outcomeLabel(market.noLabel) }}</b><small>{{ formatPercent(market.noPrice * 100) }}</small></button></div></article></div><p v-if="!markets.length" class="empty-state">{{ t('prediction.noMarkets') }}</p><LoginRequiredState v-if="!session.isAuthenticated" :description="t('prediction.loginDescription')" /><section v-else class="prediction-orders"><div class="section-heading"><span>{{ t('prediction.myPredictions') }}</span></div><article v-for="order in orders" :key="order.id"><div><strong>{{ marketText(order.marketTitle, 'title') }}</strong><small>{{ formatDateTime(order.createdAt) }} · {{ outcomeLabel(order.outcome) }}</small></div><span><b>{{ formatAmount(order.stakeAmount) }} {{ order.assetSymbol }}</b><small>{{ statusLabel(order.status) }}</small></span></article><p v-if="!orders.length" class="empty-state">{{ t('prediction.noOrders') }}</p></section></template></div><div v-if="selected" class="prediction-mask" @click.self="selected = null"><section class="prediction-dialog" role="dialog" aria-modal="true"><header><div><strong>{{ marketText(selected.title, 'title') }}</strong><small>{{ t('prediction.chooseOutcome', { outcome: outcomeLabel(outcome === 'yes' ? selected.yesLabel : selected.noLabel) }) }}</small></div><button class="icon-button" type="button" :aria-label="t('common.close')" @click="selected = null"><X :size="21" /></button></header><label><span>{{ t('prediction.paymentAsset') }}</span><select v-model="assetId"><option v-for="asset in assets" :key="asset.assetId" :value="asset.assetId">{{ t('prediction.assetAvailable', { asset: asset.assetSymbol, amount: formatAmount(accounts.find((item) => item.assetId === asset.assetId)?.available) }) }}</option></select></label><label><span>{{ t('prediction.stakeAmount') }}</span><div class="prediction-amount"><input v-model="amount" inputmode="decimal" placeholder="0.00" /><b>{{ selectedAsset?.assetSymbol || '' }}</b></div></label><template v-if="quote"><dl><div><dt>{{ t('prediction.estimatedShares') }}</dt><dd>{{ formatAmount(quote.shares) }}</dd></div><div><dt>{{ t('prediction.theoreticalPayout') }}</dt><dd>{{ formatAmount(quote.theoreticalPayout) }} {{ quote.assetSymbol }}</dd></div><div><dt>{{ t('common.fee') }}</dt><dd>{{ formatAmount(quote.feeAmount) }} {{ quote.assetSymbol }}</dd></div></dl><button class="button button--primary button--full" type="button" :disabled="confirming" @click="confirm">{{ t(confirming ? 'prediction.confirming' : 'prediction.confirmOrder') }}</button></template><button v-else class="button button--primary button--full" type="button" :disabled="quoting" @click="getQuote">{{ t(quoting ? 'prediction.quoting' : 'prediction.getQuote') }}</button></section></div></main>
</template>

<style scoped>
.prediction-page .page-content { display: grid; gap: 18px; padding-bottom: 42px; padding-top: 16px; }.prediction-intro { align-items: center; background: #f7edff; border: 1px solid #e7d3f9; border-radius: var(--radius); color: #9a4ec2; display: flex; gap: 11px; padding: 15px; }.prediction-intro div { display: grid; gap: 4px; }.prediction-intro strong { color: var(--ink); font-size: 17px; }.prediction-intro p { color: var(--muted-strong); font-size: 12px; line-height: 1.4; margin: 0; }.prediction-list { display: grid; gap: 11px; }.prediction-list article { border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow-soft); padding: 14px; }.prediction-list article > span { color: var(--muted); font-size: 11px; }.prediction-list h2 { font-size: 16px; line-height: 1.4; margin: 6px 0; }.prediction-list p { color: var(--muted); display: -webkit-box; font-size: 12px; line-height: 1.45; margin: 0 0 12px; overflow: hidden; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }.prediction-list article > div { display: grid; gap: 9px; grid-template-columns: 1fr 1fr; }.prediction-list button { background: var(--soft); border-radius: var(--radius); color: var(--ink); display: grid; gap: 4px; min-height: 53px; text-align: left; padding: 9px 11px; }.prediction-list button:first-child { background: var(--positive-soft); color: var(--positive); }.prediction-list button:last-child { background: var(--negative-soft); color: var(--negative); }.prediction-list b { font-size: 14px; }.prediction-list small { font-size: 12px; }.prediction-orders { border-top: 1px solid var(--line); }.prediction-orders .section-heading { margin-top: 20px; }.prediction-orders article { align-items: center; border-bottom: 1px solid var(--line); display: flex; justify-content: space-between; min-height: 62px; }.prediction-orders article div,.prediction-orders article > span { display: grid; gap: 5px; }.prediction-orders strong,.prediction-orders b { font-size: 13px; }.prediction-orders small { color: var(--muted); font-size: 11px; }.prediction-orders article > span { text-align: right; }.prediction-mask { align-items: flex-end; background: rgb(15 23 42 / 42%); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: 60; }.prediction-dialog { background: white; border-radius: var(--radius); display: grid; gap: 14px; max-height: calc(100dvh - 32px - env(safe-area-inset-top) - env(safe-area-inset-bottom)); max-width: 520px; overflow-y: auto; overscroll-behavior: contain; padding: 17px; width: 100%; }.prediction-dialog header { align-items: center; display: flex; justify-content: space-between; }.prediction-dialog header div { display: grid; gap: 4px; min-width: 0; }.prediction-dialog header strong { font-size: 17px; line-height: 1.35; }.prediction-dialog header small { color: var(--muted); font-size: 12px; }.prediction-dialog label { display: grid; gap: 7px; }.prediction-dialog label > span { color: var(--muted); font-size: 13px; }.prediction-dialog select { background: var(--soft); border: 0; border-radius: var(--radius); color: var(--ink); font: inherit; min-height: 48px; padding: 0 12px; }.prediction-amount { align-items: center; background: var(--soft); border-radius: var(--radius); display: grid; grid-template-columns: 1fr auto; min-height: 52px; padding: 0 12px; }.prediction-amount input { background: transparent; border: 0; font-size: 20px; font-weight: 720; min-width: 0; outline: 0; width: 100%; }.prediction-amount b { font-size: 13px; }.prediction-dialog dl { border-top: 1px solid var(--line); display: grid; margin: 0; }.prediction-dialog dl div { align-items: center; display: flex; justify-content: space-between; min-height: 34px; }.prediction-dialog dt,.prediction-dialog dd { color: var(--muted); font-size: 12px; margin: 0; }.prediction-dialog dd { color: var(--ink); }.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: 0; }
</style>
