<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Landmark, RefreshCw, X } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchEarnProducts, fetchEarnSubscriptions, redeemEarnSubscription, subscribeEarnProduct, type EarnProduct, type EarnSubscription } from '@/api/earn'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const { t } = useI18n()
const products = ref<EarnProduct[]>([])
const subscriptions = ref<EarnSubscription[]>([])
const accounts = ref<WalletAccount[]>([])
const selected = ref<EarnProduct | null>(null)
const amount = ref('')
const loading = ref(false)
const submitting = ref(false)
const actionId = ref(0)
const error = ref('')
const success = ref('')

const available = computed(() => accounts.value.find((account) => account.assetId === selected.value?.assetId)?.available || 0)
const amountNumber = computed(() => Number(amount.value || 0))
const canSubscribe = computed(() => {
  const product = selected.value
  return Boolean(product && Number.isFinite(amountNumber.value) && amountNumber.value >= product.minSubscribe && (!product.maxSubscribe || amountNumber.value <= product.maxSubscribe) && amountNumber.value <= available.value)
})

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextProducts, nextSubscriptions, nextAccounts] = await Promise.all([fetchEarnProducts(), fetchEarnSubscriptions(), fetchWalletAccounts()])
    products.value = nextProducts
    subscriptions.value = nextSubscriptions
    accounts.value = nextAccounts
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('earn.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openSubscribe(product: EarnProduct): void {
  selected.value = product
  amount.value = String(product.minSubscribe)
  success.value = ''
  error.value = ''
}

function useMaximum(): void {
  amount.value = String(available.value)
}

async function subscribe(): Promise<void> {
  if (!selected.value || !canSubscribe.value) {
    error.value = t('earn.invalidAmount')
    return
  }
  submitting.value = true
  error.value = ''
  try {
    await subscribeEarnProduct(selected.value.id, amountNumber.value)
    selected.value = null
    success.value = t('earn.subscribed')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('earn.subscribeFailed'))
  } finally {
    submitting.value = false
  }
}

async function redeem(subscription: EarnSubscription): Promise<void> {
  actionId.value = subscription.id
  error.value = ''
  try {
    await redeemEarnSubscription(subscription.id)
    success.value = t('earn.redeemed')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('earn.redeemFailed'))
  } finally {
    actionId.value = 0
  }
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain earn-page">
    <PageHeader :title="t('earn.title')"><template #actions><button class="icon-button" type="button" :aria-label="t('earn.refresh')" :disabled="loading" @click="load"><RefreshCw :size="21" :class="{ spin: loading }" /></button></template></PageHeader>
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('earn.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p><p v-if="success" class="success-message">{{ success }}</p><p v-if="loading" class="empty-state">{{ t('earn.loading') }}</p>
        <template v-else><section class="earn-banner"><Landmark :size="25" /><div><strong>{{ t('earn.bannerTitle') }}</strong><p>{{ t('earn.bannerDescription') }}</p></div></section><div class="earn-list"><button v-for="product in products" :key="product.id" class="earn-card" type="button" @click="openSubscribe(product)"><AssetMark :symbol="product.assetSymbol" :size="39" /><div><strong>{{ product.name || t('earn.defaultName', { asset: product.assetSymbol }) }}</strong><small>{{ t('earn.term', { category: product.category, days: product.termDays }) }}</small></div><span><b class="up">{{ (product.aprRate * 100).toFixed(2) }}%</b><small>{{ t('earn.estimatedApr') }}</small></span></button></div><p v-if="!products.length" class="empty-state">{{ t('earn.emptyProducts') }}</p><section class="subscriptions"><div class="section-heading"><span>{{ t('earn.myHoldings') }}</span></div><article v-for="subscription in subscriptions" :key="subscription.id" class="subscription-row"><div><strong>{{ t('earn.holdingSummary', { amount: formatAmount(subscription.amount), days: subscription.termDays }) }}</strong><small>{{ t('earn.subscribedAt', { time: formatDateTime(subscription.subscribedAt) }) }}</small></div><span><b>{{ subscription.status }}</b><button v-if="subscription.status === 'subscribed'" class="button button--secondary" type="button" :disabled="actionId === subscription.id" @click="redeem(subscription)">{{ actionId === subscription.id ? t('earn.redeeming') : t('earn.redeem') }}</button></span></article><p v-if="!subscriptions.length" class="empty-state">{{ t('earn.emptyHoldings') }}</p></section></template>
      </template>
    </div>

    <div v-if="selected" class="earn-mask" @click.self="selected = null"><form class="earn-dialog" @submit.prevent="subscribe"><header><div><strong>{{ t('earn.subscribeTitle', { name: selected.name }) }}</strong><small>{{ t('earn.subscribeSummary', { days: selected.termDays, apr: (selected.aprRate * 100).toFixed(2) }) }}</small></div><button class="icon-button" type="button" :aria-label="t('common.close')" @click="selected = null"><X :size="21" /></button></header><label><span>{{ t('earn.amount') }}</span><div class="amount-field"><input v-model="amount" inputmode="decimal" /><b>{{ selected.assetSymbol }}</b><button type="button" @click="useMaximum">{{ t('earn.all') }}</button></div></label><p>{{ t('earn.availability', { available: formatAmount(available), asset: selected.assetSymbol, minimum: formatAmount(selected.minSubscribe) }) }}</p><button class="button button--primary button--full" type="submit" :disabled="submitting">{{ submitting ? t('common.submitting') : t('earn.confirm') }}</button></form></div>
  </main>
</template>

<style scoped>
.earn-page .page-content { display: grid; gap: 18px; padding-bottom: 42px; padding-top: 16px; }.earn-banner { align-items: center; background: var(--positive-soft); border: 1px solid #ccefdc; border-radius: var(--radius); color: var(--positive); display: flex; gap: 11px; padding: 15px; }.earn-banner div { display: grid; gap: 4px; }.earn-banner strong { color: var(--ink); font-size: 17px; }.earn-banner p { color: var(--muted-strong); font-size: 12px; margin: 0; }.earn-list { display: grid; gap: 10px; }.earn-card { align-items: center; background: white; border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow-soft); display: grid; gap: 12px; grid-template-columns: 39px minmax(0, 1fr) auto; min-height: 78px; padding: 12px; text-align: left; width: 100%; }.earn-card div,.earn-card > span { display: grid; gap: 5px; }.earn-card strong { font-size: 15px; }.earn-card small { color: var(--muted); font-size: 11px; }.earn-card > span { text-align: right; }.earn-card > span b { font-size: 17px; }.subscriptions { border-top: 1px solid var(--line); }.subscriptions .section-heading { margin-top: 20px; }.subscription-row { align-items: center; border-bottom: 1px solid var(--line); display: flex; justify-content: space-between; min-height: 66px; }.subscription-row > div,.subscription-row > span { display: grid; gap: 5px; }.subscription-row strong,.subscription-row b { font-size: 13px; }.subscription-row small { color: var(--muted); font-size: 11px; }.subscription-row > span { justify-items: end; }.subscription-row .button { font-size: 11px; min-height: 30px; padding: 0 10px; }.earn-mask { align-items: flex-end; background: rgb(15 23 42 / 42%); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: 60; }.earn-dialog { background: white; border-radius: var(--radius); display: grid; gap: 15px; max-height: calc(100dvh - 32px - env(safe-area-inset-top)); max-width: 520px; overflow-y: auto; padding: 17px; width: 100%; }.earn-dialog header { align-items: center; display: flex; justify-content: space-between; }.earn-dialog header div { display: grid; gap: 4px; }.earn-dialog header strong { font-size: 18px; }.earn-dialog header small,.earn-dialog > p { color: var(--muted); font-size: 12px; margin: 0; }.earn-dialog label { display: grid; gap: 8px; }.earn-dialog label > span { color: var(--muted); font-size: 13px; }.amount-field { align-items: center; background: var(--soft); border-radius: var(--radius); display: grid; grid-template-columns: 1fr auto auto; min-height: 52px; padding: 0 12px; }.amount-field input { background: transparent; border: 0; font-size: 20px; font-weight: 720; min-width: 0; outline: 0; width: 100%; }.amount-field b { font-size: 13px; margin-right: 8px; }.amount-field button { background: transparent; color: var(--accent); font-size: 12px; font-weight: 720; }.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: 0; }.spin { animation: spin .8s linear infinite; }@keyframes spin { to { transform: rotate(360deg); } }
</style>
