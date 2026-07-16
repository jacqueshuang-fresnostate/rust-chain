<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { LockKeyhole, ReceiptText, UnlockKeyhole, WalletCards, X } from 'lucide-vue-next'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  fetchNewCoinDistributions,
  fetchNewCoinProjects,
  fetchNewCoinPurchases,
  fetchNewCoinSubscriptions,
  fetchNewCoinUnlocks,
  payNewCoinUnlockFee,
  releaseNewCoinUnlock,
  type NewCoinDistribution,
  type NewCoinPurchase,
  type NewCoinSubscription,
  type NewCoinUnlock,
} from '@/api/newCoin'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime, formatPrice } from '@/core/format'
import type { WalletAccount } from '@/core/types'
import { useSessionStore } from '@/stores/session'

type RecordTab = 'subscriptions' | 'distributions' | 'purchases' | 'unlocks'

const session = useSessionStore()
const { t } = useI18n()
const activeTab = ref<RecordTab>('subscriptions')
const subscriptions = ref<NewCoinSubscription[]>([])
const distributions = ref<NewCoinDistribution[]>([])
const purchases = ref<NewCoinPurchase[]>([])
const unlocks = ref<NewCoinUnlock[]>([])
const accounts = ref<WalletAccount[]>([])
const projectSymbols = ref<Record<number, string>>({})
const pendingUnlock = ref<NewCoinUnlock | null>(null)
const paymentAssetId = ref(0)
const loading = ref(false)
const saving = ref('')
const error = ref('')
const success = ref('')

const paymentAccount = computed(() => accounts.value.find((account) => account.assetId === paymentAssetId.value))
const paymentOptions = computed(() => pendingUnlock.value?.unlockFeeAssetId ? accounts.value.filter((account) => account.assetId === pendingUnlock.value?.unlockFeeAssetId) : accounts.value)
const paymentAmount = computed(() => pendingUnlock.value?.unlockFeeAmount || 0)
const tabs = computed<Array<{ key: RecordTab; label: string }>>(() => [
  { key: 'subscriptions', label: t('newCoin.tabSubscriptions') },
  { key: 'distributions', label: t('newCoin.tabDistributions') },
  { key: 'purchases', label: t('newCoin.tabPurchases') },
  { key: 'unlocks', label: t('newCoin.tabUnlocks') },
])

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextProjects, nextSubscriptions, nextDistributions, nextPurchases, nextUnlocks, nextAccounts] = await Promise.all([
      fetchNewCoinProjects(),
      fetchNewCoinSubscriptions(),
      fetchNewCoinDistributions(),
      fetchNewCoinPurchases(),
      fetchNewCoinUnlocks(),
      fetchWalletAccounts(),
    ])
    projectSymbols.value = Object.fromEntries(nextProjects.map((project) => [project.id, project.symbol]))
    subscriptions.value = nextSubscriptions
    distributions.value = nextDistributions
    purchases.value = nextPurchases
    unlocks.value = nextUnlocks
    accounts.value = nextAccounts
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.recordLoadFailed'))
  } finally {
    loading.value = false
  }
}

function projectLabel(projectId: number): string {
  return projectSymbols.value[projectId] || t('newCoin.projectNumber', { id: projectId })
}

function assetLabel(assetId: number): string {
  return accounts.value.find((account) => account.assetId === assetId)?.symbol || t('newCoin.assetNumber', { id: assetId })
}

function statusLabel(status: string): string {
  const keys: Record<string, string> = {
    pending: 'newCoin.statusPending',
    processing: 'newCoin.statusProcessing',
    completed: 'newCoin.statusCompleted',
    allocated: 'newCoin.statusAllocated',
    distributed: 'newCoin.statusDistributed',
    locked: 'newCoin.statusLocked',
    paid: 'newCoin.statusPaid',
    unpaid: 'newCoin.statusUnpaid',
    released: 'newCoin.statusReleased',
    cancelled: 'newCoin.statusCancelled',
    canceled: 'newCoin.statusCancelled',
  }
  const key = keys[status.toLowerCase()]
  return key ? t(key) : status
}

function openFeePayment(unlock: NewCoinUnlock): void {
  pendingUnlock.value = unlock
  paymentAssetId.value = unlock.unlockFeeAssetId || accounts.value[0]?.assetId || 0
  error.value = ''
}

async function payFee(): Promise<void> {
  if (!pendingUnlock.value || !paymentAssetId.value || paymentAmount.value <= 0) {
    error.value = t('newCoin.invalidFeeConfig')
    return
  }
  if ((paymentAccount.value?.available || 0) < paymentAmount.value) {
    error.value = t('newCoin.insufficientFeeBalance')
    return
  }
  saving.value = `fee-${pendingUnlock.value.id}`
  error.value = ''
  try {
    await payNewCoinUnlockFee({
      idempotencyKey: pendingUnlock.value.idempotencyKey,
      paymentAssetId: paymentAssetId.value,
      amount: paymentAmount.value,
    })
    pendingUnlock.value = null
    success.value = t('newCoin.feePaid')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.feePaymentFailed'))
  } finally {
    saving.value = ''
  }
}

async function release(unlock: NewCoinUnlock): Promise<void> {
  if (!unlock.idempotencyKey) return
  saving.value = `release-${unlock.id}`
  error.value = ''
  try {
    await releaseNewCoinUnlock(unlock.idempotencyKey)
    success.value = t('newCoin.assetReleased')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.releaseUnavailable'))
  } finally {
    saving.value = ''
  }
}

function feePaid(unlock: NewCoinUnlock): boolean {
  return unlock.feePaidStatus.toLowerCase() === 'paid'
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain new-coin-records-page">
    <PageHeader :title="t('newCoin.recordTitle')" />
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('newCoin.recordLoginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p>
        <p v-if="success" class="success-message">{{ success }}</p>
        <nav class="record-tabs" :aria-label="t('newCoin.recordCategory')"><button v-for="tab in tabs" :key="tab.key" type="button" :class="{ active: activeTab === tab.key }" @click="activeTab = tab.key">{{ tab.label }}</button></nav>
        <p v-if="loading" class="empty-state">{{ t('newCoin.loadingRecords') }}</p>
        <template v-else>
          <section v-if="activeTab === 'subscriptions'" class="record-list"><article v-for="record in subscriptions" :key="record.id"><div class="record-icon record-icon--green"><ReceiptText :size="19" /></div><div class="record-main"><strong>{{ t('newCoin.subscriptionRecord', { project: projectLabel(record.projectId) }) }}</strong><small>{{ formatDateTime(record.createdAt) }}</small></div><div class="record-value"><b>{{ formatAmount(record.requestedQuantity) }}</b><small>{{ t('newCoin.distributed', { amount: formatAmount(record.allocatedQuantity) }) }}</small><em>{{ statusLabel(record.status) }}</em></div></article><p v-if="!subscriptions.length" class="empty-state">{{ t('newCoin.noSubscriptions') }}</p></section>
          <section v-else-if="activeTab === 'distributions'" class="record-list"><article v-for="record in distributions" :key="record.id"><div class="record-icon record-icon--blue"><WalletCards :size="19" /></div><div class="record-main"><strong>{{ t('newCoin.distributionRecord', { project: projectLabel(record.projectId) }) }}</strong><small>{{ formatDateTime(record.createdAt) }}</small></div><div class="record-value"><b>{{ formatAmount(record.quantity) }} {{ assetLabel(record.assetId) }}</b><small>{{ t(record.lockPositionId ? 'newCoin.locked' : 'newCoin.credited') }}</small><em>{{ statusLabel(record.status) }}</em></div></article><p v-if="!distributions.length" class="empty-state">{{ t('newCoin.noDistributions') }}</p></section>
          <section v-else-if="activeTab === 'purchases'" class="record-list"><article v-for="record in purchases" :key="record.id"><div class="record-icon record-icon--orange"><LockKeyhole :size="19" /></div><div class="record-main"><strong>{{ t('newCoin.purchaseRecord', { project: projectLabel(record.projectId) }) }}</strong><small>{{ formatDateTime(record.createdAt) }}</small></div><div class="record-value"><b>{{ formatAmount(record.quantity) }} {{ assetLabel(record.baseAssetId) }}</b><small>{{ t('newCoin.paidAmount', { price: formatPrice(record.price), amount: formatAmount(record.quoteAmount) }) }}</small><em>{{ statusLabel(record.status) }}</em></div></article><p v-if="!purchases.length" class="empty-state">{{ t('newCoin.noPurchases') }}</p></section>
          <section v-else class="record-list unlock-list"><article v-for="unlock in unlocks" :key="unlock.id"><div class="record-icon record-icon--purple"><UnlockKeyhole :size="19" /></div><div class="record-main"><strong>{{ t('newCoin.pendingUnlock', { asset: assetLabel(unlock.assetId) }) }}</strong><small>{{ formatDateTime(unlock.createdAt) }} · {{ statusLabel(unlock.status) }}</small></div><div class="record-value"><b>{{ formatAmount(unlock.unlockQuantity) }} {{ assetLabel(unlock.assetId) }}</b><small v-if="unlock.unlockFeeEnabled">{{ t('newCoin.feeAmount', { amount: formatAmount(unlock.unlockFeeAmount), asset: assetLabel(unlock.unlockFeeAssetId || 0) }) }}</small><small v-else>{{ t('newCoin.noUnlockFee') }}</small><em :class="{ paid: feePaid(unlock) }">{{ unlock.unlockFeeEnabled ? t('newCoin.feeStatus', { status: statusLabel(unlock.feePaidStatus) }) : t('newCoin.directlyReleasable') }}</em></div><div class="unlock-actions"><button v-if="unlock.unlockFeeEnabled && !feePaid(unlock)" class="button button--secondary" type="button" :disabled="saving === `fee-${unlock.id}`" @click="openFeePayment(unlock)">{{ t('newCoin.payFee') }}</button><button v-else class="button button--primary" type="button" :disabled="saving === `release-${unlock.id}`" @click="release(unlock)">{{ t(saving === `release-${unlock.id}` ? 'newCoin.releasing' : 'newCoin.release') }}</button></div></article><p v-if="!unlocks.length" class="empty-state">{{ t('newCoin.noUnlocks') }}</p></section>
        </template>
      </template>
    </div>

    <div v-if="pendingUnlock" class="fee-mask" @click.self="pendingUnlock = null"><form class="fee-dialog" role="dialog" aria-modal="true" @submit.prevent="payFee"><header><div><span>{{ t('newCoin.payFee') }}</span><h2>{{ t('newCoin.releaseAsset', { asset: assetLabel(pendingUnlock.assetId) }) }}</h2></div><button class="icon-button" type="button" :aria-label="t('common.close')" @click="pendingUnlock = null"><X :size="21" /></button></header><p>{{ t('newCoin.feeDescription') }}</p><label><span>{{ t('newCoin.paymentAsset') }}</span><select v-model="paymentAssetId"><option v-for="account in paymentOptions" :key="account.assetId" :value="account.assetId">{{ t('newCoin.assetAvailable', { asset: account.symbol, amount: formatAmount(account.available) }) }}</option></select></label><dl><div><dt>{{ t('newCoin.unlockFee') }}</dt><dd>{{ formatAmount(paymentAmount) }} {{ paymentAccount?.symbol || assetLabel(pendingUnlock.unlockFeeAssetId || 0) }}</dd></div><div><dt>{{ t('newCoin.availableBalance') }}</dt><dd>{{ formatAmount(paymentAccount?.available) }} {{ paymentAccount?.symbol }}</dd></div></dl><button class="button button--primary button--full" type="submit" :disabled="saving.startsWith('fee-')">{{ t(saving.startsWith('fee-') ? 'newCoin.paying' : 'newCoin.confirmPayment') }}</button></form></div>
  </main>
</template>

<style scoped>
.new-coin-records-page .page-content { display: grid; gap: 16px; padding-bottom: 42px; padding-top: 16px; }
.record-tabs { background: var(--soft); border-radius: var(--radius); display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); padding: 3px; }
.record-tabs button { background: transparent; border-radius: calc(var(--radius) - 2px); color: var(--muted-strong); font-size: 13px; min-height: 37px; padding: 0 3px; }
.record-tabs button.active { background: white; box-shadow: var(--shadow-soft); color: var(--ink); font-weight: 700; }
.record-list { border-top: 1px solid var(--line); display: grid; }
.record-list article { align-items: center; border-bottom: 1px solid var(--line); display: grid; gap: 10px; grid-template-columns: 38px minmax(0, 1fr) minmax(88px, auto); min-height: 77px; padding: 10px 0; }
.record-icon { align-items: center; border-radius: var(--radius); display: inline-flex; height: 36px; justify-content: center; width: 36px; }
.record-icon--green { background: var(--positive-soft); color: var(--positive); }.record-icon--blue { background: #eaf1ff; color: #3975ca; }.record-icon--orange { background: #fff0dc; color: #bb6b12; }.record-icon--purple { background: #f1ebff; color: #7759c9; }
.record-main,.record-value { display: grid; gap: 5px; min-width: 0; }
.record-main strong,.record-value b { font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.record-main small,.record-value small { color: var(--muted); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.record-value { text-align: right; }.record-value em { color: var(--muted-strong); font-size: 11px; font-style: normal; }.record-value em.paid { color: var(--positive); }
.unlock-list article { grid-template-columns: 38px minmax(0, 1fr) minmax(88px, auto); }.unlock-actions { grid-column: 2 / -1; justify-self: end; margin-top: -3px; }.unlock-actions .button { font-size: 12px; min-height: 34px; padding: 0 11px; }
.fee-mask { align-items: flex-end; background: rgb(15 23 42 / 42%); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: 60; }.fee-dialog { background: white; border-radius: var(--radius); display: grid; gap: 14px; max-height: calc(100dvh - 32px - env(safe-area-inset-top) - env(safe-area-inset-bottom)); max-width: 520px; overflow-y: auto; overscroll-behavior: contain; padding: 17px; width: 100%; }.fee-dialog header { align-items: center; display: flex; justify-content: space-between; }.fee-dialog header div { display: grid; gap: 4px; }.fee-dialog header span { color: var(--positive); font-size: 12px; font-weight: 700; }.fee-dialog h2 { font-size: 19px; margin: 0; }.fee-dialog > p { color: var(--muted); font-size: 12px; line-height: 1.45; margin: 0; }.fee-dialog label { display: grid; gap: 7px; }.fee-dialog label > span { color: var(--muted); font-size: 13px; }.fee-dialog select { background: var(--soft); border: 0; border-radius: var(--radius); color: var(--ink); font: inherit; min-height: 48px; padding: 0 12px; }.fee-dialog dl { border-top: 1px solid var(--line); display: grid; margin: 0; }.fee-dialog dl div { align-items: center; display: flex; justify-content: space-between; min-height: 34px; }.fee-dialog dt,.fee-dialog dd { color: var(--muted); font-size: 12px; margin: 0; }.fee-dialog dd { color: var(--ink); font-weight: 650; text-align: right; }.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: 0; }
</style>
