<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ArrowUpRight, CalendarDays, CircleDollarSign, Clock3, ReceiptText, ShieldCheck } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  createNewCoinPurchase,
  fetchNewCoinProject,
  subscribeNewCoin,
  type NewCoinProject,
} from '@/api/newCoin'
import { fetchMarketTickers } from '@/api/market'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import type { MarketTicker, WalletAccount } from '@/core/types'
import { useSessionStore } from '@/stores/session'

const props = defineProps<{ symbol: string }>()
const router = useRouter()
const { t } = useI18n()
const session = useSessionStore()
const project = ref<NewCoinProject | null>(null)
const accounts = ref<WalletAccount[]>([])
const tickers = ref<MarketTicker[]>([])
const quoteAssetId = ref(0)
const amount = ref('')
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const success = ref('')

const lifecycle = computed(() => project.value?.lifecycleStatus.toLowerCase() || '')
const canSubscribe = computed(() => lifecycle.value === 'subscription')
const canPurchase = computed(() => lifecycle.value === 'listed' && Boolean(project.value?.postListingPurchaseEnabled && project.value?.postListingPairId))
const selectedTicker = computed(() => tickers.value.find((ticker) => ticker.id === project.value?.postListingPairId))
const quoteSymbol = computed(() => canPurchase.value ? selectedTicker.value?.quote || t('newCoin.quoteAsset') : 'USDT')
const selectedAccount = computed(() => accounts.value.find((account) => account.assetId === quoteAssetId.value))
const amountNumber = computed(() => Number(amount.value || 0))
const executionPrice = computed(() => selectedTicker.value?.lastPrice || project.value?.issuePrice || 0)
const paymentAmount = computed(() => canPurchase.value ? amountNumber.value * executionPrice.value : amountNumber.value)
const estimatedQuantity = computed(() => canSubscribe.value && project.value?.issuePrice ? amountNumber.value / project.value.issuePrice : amountNumber.value)
const canSubmit = computed(() => {
  if (!project.value || !selectedAccount.value || !Number.isFinite(amountNumber.value) || amountNumber.value <= 0) return false
  if (paymentAmount.value > selectedAccount.value.available) return false
  if (canSubscribe.value) return estimatedQuantity.value > 0
  return canPurchase.value && executionPrice.value > 0
})

const lifecycleLabel = computed(() => {
  const keys: Record<string, string> = {
    subscription: 'newCoin.subscriptionOpen',
    distribution: 'newCoin.waitingDistribution',
    listed: 'newCoin.listed',
    closed: 'newCoin.closed',
  }
  const key = keys[lifecycle.value]
  return key ? t(key) : project.value?.lifecycleStatus || '--'
})

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    const nextProject = await fetchNewCoinProject(props.symbol)
    project.value = nextProject
    const requests: [Promise<WalletAccount[]>, Promise<MarketTicker[]>] = [
      session.isAuthenticated ? fetchWalletAccounts() : Promise.resolve([]),
      nextProject.postListingPurchaseEnabled ? fetchMarketTickers() : Promise.resolve([]),
    ]
    const [nextAccounts, nextTickers] = await Promise.all(requests)
    accounts.value = nextAccounts
    tickers.value = nextTickers
    selectDefaultAccount()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.projectLoadFailed'))
  } finally {
    loading.value = false
  }
}

function selectDefaultAccount(): void {
  const matching = accounts.value.find((account) => account.symbol === quoteSymbol.value)
  quoteAssetId.value = matching?.assetId || accounts.value.find((account) => account.symbol === 'USDT')?.assetId || accounts.value[0]?.assetId || 0
}

function setAmount(value: number): void {
  const available = selectedAccount.value?.available || 0
  const next = Math.max(0, Math.min(available * value, available))
  amount.value = next ? String(Number(next.toFixed(8))) : ''
}

async function submit(): Promise<void> {
  if (!project.value || !canSubmit.value) {
    error.value = t('newCoin.invalidAmount')
    return
  }
  submitting.value = true
  error.value = ''
  try {
    if (canSubscribe.value) {
      await subscribeNewCoin({
        symbol: project.value.symbol,
        quoteAssetId: quoteAssetId.value,
        quoteAmount: amountNumber.value,
        issuePrice: project.value.issuePrice,
      })
      success.value = t('newCoin.subscriptionSubmitted')
    } else if (canPurchase.value && project.value.postListingPairId) {
      await createNewCoinPurchase({
        symbol: project.value.symbol,
        pairId: project.value.postListingPairId,
        price: executionPrice.value,
        quantity: amountNumber.value,
      })
      success.value = t('newCoin.purchaseSubmitted')
    }
    amount.value = ''
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t(canPurchase.value ? 'newCoin.purchaseFailed' : 'newCoin.subscriptionFailed'))
  } finally {
    submitting.value = false
  }
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain new-coin-detail-page">
    <PageHeader :title="t('newCoin.projectTitle', { symbol: props.symbol.toUpperCase() })">
      <template #actions><button class="icon-button" type="button" :aria-label="t('newCoin.records')" @click="router.push({ name: 'new-coin-records' })"><ReceiptText :size="20" /></button></template>
    </PageHeader>
    <div class="page-content">
      <p v-if="error" class="error-message">{{ error }}</p>
      <p v-if="success" class="success-message">{{ success }}</p>
      <p v-if="loading" class="empty-state">{{ t('newCoin.loadingProject') }}</p>
      <template v-else-if="project">
        <section class="project-hero"><AssetMark :symbol="project.symbol" :size="56" /><div><span>{{ lifecycleLabel }}</span><h1>{{ project.symbol }}</h1><p>{{ t('newCoin.projectDescription') }}</p></div></section>

        <section class="project-metrics surface"><article><span>{{ t('newCoin.issuePrice') }}</span><strong>{{ formatPrice(project.issuePrice) }}</strong><small>{{ t('newCoin.referenceAsset', { asset: quoteSymbol }) }}</small></article><article><span>{{ t('newCoin.plannedIssue') }}</span><strong>{{ formatAmount(project.totalSupply) }}</strong><small>{{ project.symbol }}</small></article><article><span>{{ t('newCoin.currentStage') }}</span><strong>{{ lifecycleLabel }}</strong><small>{{ project.status || 'active' }}</small></article></section>

        <section class="project-section"><div class="section-heading"><span>{{ t('newCoin.rules') }}</span></div><dl class="detail-list"><div><dt><CalendarDays :size="17" />{{ t('newCoin.listingTime') }}</dt><dd>{{ formatDateTime(project.listedAt) }}</dd></div><div><dt><Clock3 :size="17" />{{ t('newCoin.unlockMethod') }}</dt><dd>{{ project.unlockType || '--' }}</dd></div><div v-if="project.fixedUnlockAt"><dt><ArrowUpRight :size="17" />{{ t('newCoin.estimatedUnlock') }}</dt><dd>{{ formatDateTime(project.fixedUnlockAt) }}</dd></div><div v-else-if="project.relativeUnlockSeconds"><dt><ArrowUpRight :size="17" />{{ t('newCoin.unlockPeriod') }}</dt><dd>{{ t('newCoin.days', { days: Math.ceil(project.relativeUnlockSeconds / 86400) }) }}</dd></div><div><dt><ShieldCheck :size="17" />{{ t('newCoin.unlockFee') }}</dt><dd>{{ project.unlockFeeEnabled ? `${formatAmount(project.unlockFeeRate)} ${project.unlockFeeBasis || ''}` : t('newCoin.none') }}</dd></div></dl></section>

        <LoginRequiredState v-if="!session.isAuthenticated" :description="t('newCoin.detailLoginDescription')" />
        <section v-else-if="canSubscribe || canPurchase" class="entry-card surface"><header><div><span>{{ t(canSubscribe ? 'newCoin.subscribe' : 'newCoin.postListingPurchase') }}</span><h2>{{ t(canSubscribe ? 'newCoin.subscribeTitle' : 'newCoin.purchaseTitle') }}</h2></div><CircleDollarSign :size="23" /></header><p>{{ canSubscribe ? t('newCoin.subscribeDescription') : t('newCoin.purchaseDescription', { price: formatPrice(executionPrice), asset: quoteSymbol }) }}</p><label><span>{{ t('newCoin.paymentAsset') }}</span><select v-model="quoteAssetId"><option v-for="account in accounts" :key="account.assetId" :value="account.assetId">{{ t('newCoin.assetAvailable', { asset: account.symbol, amount: formatAmount(account.available) }) }}</option></select></label><label><span>{{ t(canSubscribe ? 'newCoin.subscriptionAmount' : 'newCoin.purchaseQuantity', { asset: project.symbol }) }}</span><div class="entry-amount"><input v-model="amount" inputmode="decimal" placeholder="0.00" /><b>{{ canSubscribe ? selectedAccount?.symbol || quoteSymbol : project.symbol }}</b></div></label><div class="quick-values"><button v-for="value in [0.25, 0.5, 0.75, 1]" :key="value" type="button" @click="setAmount(value)">{{ value === 1 ? t('newCoin.maximum') : `${value * 100}%` }}</button></div><dl><div><dt>{{ t(canSubscribe ? 'newCoin.estimatedSubscription' : 'newCoin.estimatedPayment') }}</dt><dd>{{ formatAmount(canSubscribe ? estimatedQuantity : paymentAmount) }} {{ canSubscribe ? project.symbol : selectedAccount?.symbol || quoteSymbol }}</dd></div><div><dt>{{ t('newCoin.availableBalance') }}</dt><dd>{{ formatAmount(selectedAccount?.available) }} {{ selectedAccount?.symbol }}</dd></div></dl><button class="button button--primary button--full" type="button" :disabled="submitting || !canSubmit" @click="submit">{{ submitting ? t('common.submitting') : t(canSubscribe ? 'newCoin.subscribeAsset' : 'newCoin.purchaseAsset', { asset: project.symbol }) }}</button></section>
        <section v-else class="stage-note"><Clock3 :size="19" /><div><strong>{{ t('newCoin.stageUnavailable') }}</strong><p>{{ t('newCoin.stageUnavailableDescription') }}</p></div></section>
      </template>
    </div>
  </main>
</template>

<style scoped>
.new-coin-detail-page .page-content { display: grid; gap: 20px; padding-bottom: 42px; padding-top: 16px; }
.project-hero { align-items: center; background: #edf6f3; border: 1px solid #d4e9e2; border-radius: var(--radius); display: flex; gap: 14px; padding: 17px; }
.project-hero > div { display: grid; gap: 4px; min-width: 0; }
.project-hero span { color: var(--positive); font-size: 12px; font-weight: 700; }
.project-hero h1 { font-size: 24px; margin: 0; }
.project-hero p { color: var(--muted-strong); font-size: 12px; line-height: 1.45; margin: 0; }
.project-metrics { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); padding: 0; }
.project-metrics article { display: grid; gap: 5px; min-height: 106px; padding: 16px 12px; }
.project-metrics article + article { border-left: 1px solid var(--line); }
.project-metrics span,.project-metrics small { color: var(--muted); font-size: 11px; }
.project-metrics strong { font-size: 15px; line-height: 1.25; overflow-wrap: anywhere; }
.project-section { border-top: 1px solid var(--line); padding-top: 2px; }
.project-section .section-heading { margin-top: 15px; }
.detail-list { display: grid; margin: 0; }
.detail-list div { align-items: center; border-bottom: 1px solid var(--line); display: flex; justify-content: space-between; min-height: 52px; gap: 18px; }
.detail-list dt { align-items: center; color: var(--muted-strong); display: inline-flex; font-size: 13px; gap: 8px; }
.detail-list dt svg { color: var(--accent); }
.detail-list dd { color: var(--ink); font-size: 13px; margin: 0; overflow-wrap: anywhere; text-align: right; }
.entry-card { display: grid; gap: 13px; padding: 17px; }
.entry-card header { align-items: center; display: flex; justify-content: space-between; }
.entry-card header > div { display: grid; gap: 4px; }
.entry-card header span { color: var(--positive); font-size: 12px; font-weight: 700; }
.entry-card h2 { font-size: 19px; margin: 0; }
.entry-card header > svg { color: var(--accent); }
.entry-card > p { color: var(--muted); font-size: 12px; line-height: 1.45; margin: -2px 0 2px; }
.entry-card label { display: grid; gap: 7px; }
.entry-card label > span { color: var(--muted); font-size: 13px; }
.entry-card select { background: var(--soft); border: 0; border-radius: var(--radius); color: var(--ink); font: inherit; min-height: 48px; padding: 0 12px; }
.entry-amount { align-items: center; background: var(--soft); border-radius: var(--radius); display: grid; grid-template-columns: 1fr auto; min-height: 54px; padding: 0 13px; }
.entry-amount input { background: transparent; border: 0; color: var(--ink); font-size: 21px; font-weight: 720; min-width: 0; outline: 0; width: 100%; }
.entry-amount b { font-size: 13px; }
.quick-values { display: grid; gap: 8px; grid-template-columns: repeat(4, minmax(0, 1fr)); }
.quick-values button { background: white; border: 1px solid var(--line); border-radius: var(--radius); color: var(--ink); font-size: 12px; min-height: 34px; }
.entry-card dl { border-top: 1px solid var(--line); display: grid; margin: 0; }
.entry-card dl div { align-items: center; display: flex; justify-content: space-between; min-height: 34px; }
.entry-card dt,.entry-card dd { color: var(--muted); font-size: 12px; margin: 0; }
.entry-card dd { color: var(--ink); font-weight: 650; text-align: right; }
.stage-note { align-items: flex-start; background: var(--soft); border-radius: var(--radius); display: flex; gap: 10px; padding: 15px; }
.stage-note > svg { color: var(--muted-strong); flex: 0 0 auto; margin-top: 2px; }
.stage-note div { display: grid; gap: 5px; }
.stage-note strong { font-size: 14px; }
.stage-note p { color: var(--muted); font-size: 12px; line-height: 1.45; margin: 0; }
.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: 0; }
</style>
