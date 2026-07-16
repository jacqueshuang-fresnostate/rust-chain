<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Banknote, RefreshCw, X } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { applyLoan, cancelLoanOrder, fetchLoanOrders, fetchLoanProducts, repayLoanOrder, type LoanOrder, type LoanProduct } from '@/api/loan'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const { t } = useI18n()
const products = ref<LoanProduct[]>([])
const orders = ref<LoanOrder[]>([])
const accounts = ref<WalletAccount[]>([])
const selected = ref<LoanProduct | null>(null)
const amount = ref('')
const collateralAssetId = ref(0)
const collateralAmount = ref('')
const loading = ref(false)
const submitting = ref(false)
const actionId = ref(0)
const error = ref('')
const success = ref('')

const amountNumber = computed(() => Number(amount.value || 0))
const collateralAmountNumber = computed(() => Number(collateralAmount.value || 0))
const selectedCollateral = computed(() => accounts.value.find((account) => account.assetId === collateralAssetId.value))
const canApply = computed(() => {
  const product = selected.value
  if (!product || !Number.isFinite(amountNumber.value) || amountNumber.value < product.minAmount) return false
  if (product.maxAmount && amountNumber.value > product.maxAmount) return false
  if (product.loanType === 'collateralized' && (!selectedCollateral.value || !Number.isFinite(collateralAmountNumber.value) || collateralAmountNumber.value <= 0 || collateralAmountNumber.value > selectedCollateral.value.available)) return false
  return true
})

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    const productsPromise = fetchLoanProducts()
    if (session.isAuthenticated) {
      const [nextProducts, nextOrders, nextAccounts] = await Promise.all([productsPromise, fetchLoanOrders(), fetchWalletAccounts()])
      products.value = nextProducts
      orders.value = nextOrders
      accounts.value = nextAccounts
    } else {
      products.value = await productsPromise
    }
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('loan.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openApply(product: LoanProduct): void {
  if (!session.isAuthenticated) return
  selected.value = product
  amount.value = String(product.minAmount)
  collateralAssetId.value = accounts.value[0]?.assetId || 0
  collateralAmount.value = ''
  error.value = ''
  success.value = ''
}

async function submitApplication(): Promise<void> {
  if (!selected.value || !canApply.value) {
    error.value = t('loan.invalidApplication')
    return
  }
  submitting.value = true
  error.value = ''
  try {
    await applyLoan({
      productId: selected.value.id,
      amount: amountNumber.value,
      collateralAssetId: selected.value.loanType === 'collateralized' ? collateralAssetId.value : undefined,
      collateralAmount: selected.value.loanType === 'collateralized' ? collateralAmountNumber.value : undefined,
    })
    selected.value = null
    success.value = t('loan.submitted')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('loan.submitFailed'))
  } finally {
    submitting.value = false
  }
}

async function actOnOrder(order: LoanOrder): Promise<void> {
  actionId.value = order.id
  error.value = ''
  try {
    if (order.status === 'pending') {
      await cancelLoanOrder(order.id)
      success.value = t('loan.canceled')
    } else {
      await repayLoanOrder(order.id)
      success.value = t('loan.repaid')
    }
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, order.status === 'pending' ? t('loan.cancelFailed') : t('loan.repayFailed'))
  } finally {
    actionId.value = 0
  }
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain loan-page">
    <PageHeader :title="t('loan.title')"><template #actions><button class="icon-button" type="button" :aria-label="t('loan.refresh')" :disabled="loading" @click="load"><RefreshCw :size="21" :class="{ spin: loading }" /></button></template></PageHeader>
    <div class="page-content">
      <p v-if="error" class="error-message">{{ error }}</p><p v-if="success" class="success-message">{{ success }}</p><p v-if="loading" class="empty-state">{{ t('loan.loading') }}</p>
      <template v-else><section class="loan-banner"><Banknote :size="25" /><div><strong>{{ t('loan.bannerTitle') }}</strong><p>{{ t('loan.bannerDescription') }}</p></div></section><div class="loan-list"><button v-for="product in products" :key="product.id" class="loan-card" type="button" @click="openApply(product)"><AssetMark :symbol="product.assetSymbol" :size="39" /><div><strong>{{ product.name }}</strong><small>{{ product.loanType === 'collateralized' ? t('loan.collateralized') : t('loan.credit') }} · {{ t('loan.termDays', { days: product.termDays }) }}</small></div><span><b>{{ (product.interestRate * 100).toFixed(2) }}%</b><small>{{ t('loan.annualRate') }}</small></span></button></div><p v-if="!products.length" class="empty-state">{{ t('loan.noProducts') }}</p><LoginRequiredState v-if="!session.isAuthenticated" :description="t('loan.loginDescription')" /><section v-else class="loan-orders"><div class="section-heading"><span>{{ t('loan.myLoans') }}</span></div><article v-for="order in orders" :key="order.id" class="loan-order"><div><strong>{{ order.productName }} · {{ formatAmount(order.amount) }} {{ order.assetSymbol }}</strong><small>{{ formatDateTime(order.createdAt) }} · {{ order.status }}</small></div><span><b>{{ order.status === 'disbursed' ? t('loan.repaymentDue', { amount: formatAmount(order.repaymentAmount) }) : order.status }}</b><button v-if="order.status === 'pending' || order.status === 'disbursed'" class="button button--secondary" type="button" :disabled="actionId === order.id" @click="actOnOrder(order)">{{ actionId === order.id ? t('loan.processing') : order.status === 'pending' ? t('loan.cancel') : t('loan.repay') }}</button></span></article><p v-if="!orders.length" class="empty-state">{{ t('loan.noOrders') }}</p></section></template>
    </div>

    <div v-if="selected" class="loan-mask" @click.self="selected = null"><form class="loan-dialog" @submit.prevent="submitApplication"><header><div><strong>{{ t('loan.applyTitle', { name: selected.name }) }}</strong><small>{{ t('loan.minimum', { type: selected.loanType === 'collateralized' ? t('loan.collateralized') : t('loan.credit'), amount: formatAmount(selected.minAmount), asset: selected.assetSymbol }) }}</small></div><button class="icon-button" type="button" :aria-label="t('common.close')" @click="selected = null"><X :size="21" /></button></header><label><span>{{ t('loan.loanAmount') }}</span><div class="loan-amount"><input v-model="amount" inputmode="decimal" /><b>{{ selected.assetSymbol }}</b></div></label><template v-if="selected.loanType === 'collateralized'"><label><span>{{ t('loan.collateralAsset') }}</span><select v-model="collateralAssetId"><option v-for="account in accounts" :key="account.assetId" :value="account.assetId">{{ t('loan.assetAvailable', { asset: account.symbol, amount: formatAmount(account.available) }) }}</option></select></label><label><span>{{ t('loan.collateralAmount') }}</span><div class="loan-amount"><input v-model="collateralAmount" inputmode="decimal" /><b>{{ selectedCollateral?.symbol || '' }}</b></div></label></template><dl><div><dt>{{ t('loan.term') }}</dt><dd>{{ t('loan.termDays', { days: selected.termDays }) }}</dd></div><div><dt>{{ t('loan.annualRate') }}</dt><dd>{{ (selected.interestRate * 100).toFixed(2) }}%</dd></div><div><dt>{{ t('loan.minimumKyc') }}</dt><dd>{{ selected.minKycLevel }}</dd></div></dl><button class="button button--primary button--full" type="submit" :disabled="submitting">{{ submitting ? t('common.submitting') : t('loan.submit') }}</button></form></div>
  </main>
</template>

<style scoped>
.loan-page .page-content { display: grid; gap: 18px; padding-bottom: 42px; padding-top: 16px; }.loan-banner { align-items: center; background: #fff0dc; border: 1px solid #f2d8b4; border-radius: var(--radius); color: #b96a12; display: flex; gap: 11px; padding: 15px; }.loan-banner div { display: grid; gap: 4px; }.loan-banner strong { color: var(--ink); font-size: 17px; }.loan-banner p { color: var(--muted-strong); font-size: 12px; margin: 0; }.loan-list { display: grid; gap: 10px; }.loan-card { align-items: center; background: white; border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow-soft); display: grid; gap: 12px; grid-template-columns: 39px minmax(0, 1fr) auto; min-height: 78px; padding: 12px; text-align: left; width: 100%; }.loan-card div,.loan-card > span { display: grid; gap: 5px; }.loan-card strong { font-size: 15px; }.loan-card small { color: var(--muted); font-size: 11px; }.loan-card > span { text-align: right; }.loan-card > span b { color: #a4590b; font-size: 17px; }.loan-orders { border-top: 1px solid var(--line); }.loan-orders .section-heading { margin-top: 20px; }.loan-order { align-items: center; border-bottom: 1px solid var(--line); display: flex; justify-content: space-between; min-height: 66px; }.loan-order > div,.loan-order > span { display: grid; gap: 5px; min-width: 0; }.loan-order strong { font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.loan-order small { color: var(--muted); font-size: 11px; }.loan-order > span { justify-items: end; }.loan-order b { font-size: 12px; }.loan-order .button { font-size: 11px; min-height: 30px; padding: 0 10px; }.loan-mask { align-items: flex-end; background: rgb(15 23 42 / 42%); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: 60; }.loan-dialog { background: white; border-radius: var(--radius); display: grid; gap: 15px; max-height: calc(100dvh - 32px - env(safe-area-inset-top)); max-width: 520px; overflow-y: auto; padding: 17px; width: 100%; }.loan-dialog header { align-items: center; display: flex; justify-content: space-between; }.loan-dialog header div { display: grid; gap: 4px; }.loan-dialog header strong { font-size: 18px; }.loan-dialog header small { color: var(--muted); font-size: 12px; }.loan-dialog label { display: grid; gap: 8px; }.loan-dialog label > span { color: var(--muted); font-size: 13px; }.loan-dialog select { background: var(--soft); border: 0; border-radius: var(--radius); color: var(--ink); font: inherit; min-height: 48px; padding: 0 12px; }.loan-amount { align-items: center; background: var(--soft); border-radius: var(--radius); display: grid; grid-template-columns: 1fr auto; min-height: 52px; padding: 0 12px; }.loan-amount input { background: transparent; border: 0; font-size: 20px; font-weight: 720; min-width: 0; outline: 0; width: 100%; }.loan-amount b { font-size: 13px; }.loan-dialog dl { border-top: 1px solid var(--line); display: grid; margin: 0; }.loan-dialog dl div { align-items: center; display: flex; justify-content: space-between; min-height: 35px; }.loan-dialog dt,.loan-dialog dd { color: var(--muted); font-size: 12px; margin: 0; }.loan-dialog dd { color: var(--ink); }.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: 0; }.spin { animation: spin .8s linear infinite; }@keyframes spin { to { transform: rotate(360deg); } }
</style>
