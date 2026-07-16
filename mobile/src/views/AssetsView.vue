<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowDownUp, ArrowUpToLine, Download, ReceiptText, RefreshCw, X } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchMarginWallets } from '@/api/trading'
import { fetchWalletAccounts, transferWalletFunds } from '@/api/wallet'
import { formatAmount, formatFiat } from '@/core/format'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const router = useRouter()
const marketStore = useMarketStore()
const session = useSessionStore()
const { t } = useI18n()
const accounts = ref<WalletAccount[]>([])
const marginAccounts = ref<WalletAccount[]>([])
const loading = ref(false)
const error = ref('')
const transferOpen = ref(false)
const transferAsset = ref('')
const transferAmount = ref('')
const transferFrom = ref<'spot' | 'margin'>('spot')
const transferFeedback = ref('')
const transferFeedbackTone = ref<'success' | 'error'>('error')
const transferring = ref(false)

const assetRows = computed(() => {
  const rows = new Map<string, { symbol: string; spot?: WalletAccount; margin?: WalletAccount }>()
  for (const account of accounts.value) rows.set(account.symbol, { ...rows.get(account.symbol), symbol: account.symbol, spot: account })
  for (const account of marginAccounts.value) rows.set(account.symbol, { ...rows.get(account.symbol), symbol: account.symbol, margin: account })
  return [...rows.values()].sort((left, right) => left.symbol.localeCompare(right.symbol))
})

const totalEstimate = computed(() => assetRows.value.reduce((total, row) => {
  const amount = walletTotal(row.spot) + walletTotal(row.margin)
  if (row.symbol === 'USDT' || row.symbol === 'USDC' || row.symbol === 'USD') return total + amount
  return total + amount * (marketStore.tickerFor(`${row.symbol}/USDT`)?.lastPrice || 0)
}, 0))

const transferAccounts = computed(() => transferFrom.value === 'spot' ? accounts.value : marginAccounts.value)
const transferAccount = computed(() => transferAccounts.value.find((account) => account.symbol === transferAsset.value))
const transferAvailable = computed(() => transferAccount.value?.available || 0)

async function loadAccounts(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [, nextAccounts, marginState] = await Promise.all([marketStore.refresh(), fetchWalletAccounts(), fetchMarginWallets()])
    accounts.value = nextAccounts
    marginAccounts.value = marginState.wallets
    if (!transferAccounts.value.some((account) => account.symbol === transferAsset.value)) transferAsset.value = transferAccounts.value[0]?.symbol || ''
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('assets.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openDeposit() {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/assets/deposit' } })
    return
  }
  void router.push({ name: 'deposit-asset' })
}

function openTransfer() {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/assets' } })
    return
  }
  transferFrom.value = 'spot'
  if (!accounts.value.some((account) => account.symbol === transferAsset.value)) transferAsset.value = accounts.value[0]?.symbol || ''
  transferFeedback.value = ''
  transferOpen.value = true
}

function openProtectedRoute(name: 'withdraw-asset' | 'wallet-ledger' | 'quick-recharge'): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/assets' } })
    return
  }
  void router.push({ name })
}

async function submitTransfer(): Promise<void> {
  const amount = Number(transferAmount.value)
  if (!transferAsset.value || !Number.isFinite(amount) || amount <= 0) {
    transferFeedback.value = t('assets.invalidTransfer')
    transferFeedbackTone.value = 'error'
    return
  }
  if (amount > transferAvailable.value) {
    transferFeedback.value = t('assets.exceedsBalance')
    transferFeedbackTone.value = 'error'
    return
  }
  transferring.value = true
  transferFeedback.value = ''
  try {
    const to = transferFrom.value === 'spot' ? 'margin' : 'spot'
    await transferWalletFunds(transferAsset.value, transferFrom.value, to, amount)
    transferFeedback.value = t('assets.transferSuccess')
    transferFeedbackTone.value = 'success'
    transferAmount.value = ''
    await loadAccounts()
  } catch (reason) {
    transferFeedback.value = apiErrorMessage(reason, t('assets.transferFailed'))
    transferFeedbackTone.value = 'error'
  } finally {
    transferring.value = false
  }
}

function walletTotal(account?: WalletAccount): number {
  return account ? account.available + account.frozen + account.locked : 0
}

function syncTransferAsset(): void {
  if (!transferAccounts.value.some((account) => account.symbol === transferAsset.value)) transferAsset.value = transferAccounts.value[0]?.symbol || ''
}

watch(transferFrom, syncTransferAsset)
watch(() => session.isAuthenticated, () => { void loadAccounts() }, { immediate: true })
</script>

<template>
  <main class="page assets-page">
    <PageHeader :title="t('assets.title')" :back="false">
      <template #actions><button class="icon-button" type="button" :aria-label="t('assets.refresh')" :disabled="loading" @click="loadAccounts"><RefreshCw :size="21" :class="{ spin: loading }" /></button></template>
    </PageHeader>
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('assets.loginDescription')" />
      <template v-else>
        <section class="assets-summary"><span>{{ t('assets.totalValue') }}</span><strong>{{ formatFiat(totalEstimate) }}</strong><p>{{ t('assets.estimateNote') }}</p></section>
        <div class="asset-actions" :aria-label="t('assets.operations')">
          <button type="button" @click="openDeposit"><Download :size="19" /><span>{{ t('assets.deposit') }}</span></button>
          <button type="button" @click="openProtectedRoute('withdraw-asset')"><ArrowUpToLine :size="19" /><span>{{ t('assets.withdraw') }}</span></button>
          <button type="button" @click="openTransfer"><ArrowDownUp :size="19" /><span>{{ t('assets.transfer') }}</span></button>
          <button type="button" @click="openProtectedRoute('wallet-ledger')"><ReceiptText :size="19" /><span>{{ t('assets.ledger') }}</span></button>
        </div>
        <button class="quick-recharge-entry surface" type="button" @click="openProtectedRoute('quick-recharge')"><span><b>{{ t('assets.quickBuy') }}</b><small>{{ t('assets.quickBuyDescription') }}</small></span><span>{{ t('assets.go') }}</span></button>
        <div class="section-heading"><span>{{ t('assets.list') }}</span><button class="section-heading__action" type="button" @click="session.logout">{{ t('assets.logout') }}</button></div>
        <p v-if="error" class="error-message">{{ error }}</p>
        <div v-if="assetRows.length" class="asset-list"><div v-for="account in assetRows" :key="account.symbol" class="asset-row"><AssetMark :symbol="account.symbol" :src="account.spot?.logoUrl || account.margin?.logoUrl" /><span class="asset-row__symbol"><b>{{ account.symbol }}</b><small>{{ t('assets.accountSummary', { funding: formatAmount(account.spot?.available), contract: formatAmount(account.margin?.available) }) }}</small></span><span class="asset-row__value"><b>{{ formatAmount(walletTotal(account.spot) + walletTotal(account.margin)) }}</b><small>{{ t('assets.frozen', { amount: formatAmount((account.spot?.frozen || 0) + (account.spot?.locked || 0) + (account.margin?.frozen || 0) + (account.margin?.locked || 0)) }) }}</small></span></div></div>
        <p v-else-if="!loading" class="empty-state">{{ t('assets.empty') }}</p>
      </template>
    </div>

    <div v-if="transferOpen" class="transfer-mask" @click.self="transferOpen = false">
      <section class="transfer-dialog" role="dialog" aria-modal="true" :aria-label="t('assets.transferTitle')">
        <header><strong>{{ t('assets.transferTitle') }}</strong><button class="icon-button" type="button" :aria-label="t('common.close')" @click="transferOpen = false"><X :size="21" /></button></header>
        <label><span>{{ t('assets.asset') }}</span><select v-model="transferAsset"><option v-for="account in transferAccounts" :key="account.symbol" :value="account.symbol">{{ t('assets.assetAvailable', { asset: account.symbol, amount: formatAmount(account.available) }) }}</option></select></label>
        <label><span>{{ t('assets.from') }}</span><select v-model="transferFrom"><option value="spot">{{ t('assets.fundingAccount') }}</option><option value="margin">{{ t('assets.contractAccount') }}</option></select></label>
        <div class="transfer-direction"><span>{{ transferFrom === 'spot' ? t('assets.fundingAccount') : t('assets.contractAccount') }}</span><ArrowDownUp :size="19" /><span>{{ transferFrom === 'spot' ? t('assets.contractAccount') : t('assets.fundingAccount') }}</span></div>
        <p class="transfer-available">{{ t('assets.availableBalance', { amount: formatAmount(transferAvailable), asset: transferAsset }) }}</p>
        <label><span>{{ t('assets.transferAmount') }}</span><input v-model="transferAmount" class="input" inputmode="decimal" :placeholder="t('assets.transferPlaceholder')" /></label>
        <button class="button button--primary button--full" type="button" :disabled="transferring" @click="submitTransfer">{{ transferring ? t('assets.transferring') : t('assets.confirmTransfer') }}</button>
        <p v-if="transferFeedback" :class="transferFeedbackTone === 'success' ? 'up' : 'down'" class="transfer-feedback">{{ transferFeedback }}</p>
      </section>
    </div>
  </main>
</template>

<style scoped>
.assets-summary { padding: 16px 0 26px; }.assets-summary span { color: var(--muted); font-size: 15px; }.assets-summary strong { display: block; font-size: 37px; line-height: 1.2; margin-top: 10px; }.assets-summary p { color: var(--muted); font-size: 12px; margin: 8px 0 0; }
.asset-actions { border-bottom: 1px solid var(--line); border-top: 1px solid var(--line); display: grid; grid-template-columns: repeat(4, 1fr); }.asset-actions button { align-items: center; background: transparent; color: var(--ink); display: flex; flex-direction: column; font-size: 12px; font-weight: 700; gap: 4px; justify-content: center; min-height: 65px; }.asset-actions button + button { border-left: 1px solid var(--line); }.asset-actions svg { color: var(--accent); }
.quick-recharge-entry { align-items: center; background: var(--surface); display: flex; justify-content: space-between; margin-top: 14px; padding: 14px; text-align: left; width: 100%; }.quick-recharge-entry > span:first-child { display: grid; gap: 4px; }.quick-recharge-entry b { font-size: 15px; }.quick-recharge-entry small { color: var(--muted); font-size: 12px; }.quick-recharge-entry > span:last-child { color: var(--accent); font-size: 13px; font-weight: 700; }
.asset-list { display: grid; }.asset-row { align-items: center; border-bottom: 1px solid var(--line); display: grid; gap: 12px; grid-template-columns: 38px 1fr auto; min-height: 74px; }.asset-row__symbol,.asset-row__value { display: grid; }.asset-row b { font-size: 16px; }.asset-row small { color: var(--muted); font-size: 12px; margin-top: 4px; }.asset-row__value { text-align: right; }
.transfer-mask { align-items: flex-end; background: rgb(15 23 42 / 42%); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: 60; }.transfer-dialog { background: white; border-radius: 8px; max-height: calc(100dvh - 32px - env(safe-area-inset-top) - env(safe-area-inset-bottom)); max-width: 520px; overflow-y: auto; overscroll-behavior: contain; padding: 16px; width: 100%; }.transfer-dialog header { align-items: center; display: flex; justify-content: space-between; margin-bottom: 15px; }.transfer-dialog header strong { font-size: 19px; }.transfer-dialog label { display: grid; gap: 7px; margin: 13px 0; }.transfer-dialog label > span { color: var(--muted); font-size: 13px; }.transfer-dialog select { background: var(--soft); border: 0; border-radius: 6px; min-height: 44px; padding: 0 11px; }.transfer-direction { align-items: center; background: var(--soft); border-radius: 6px; display: flex; font-size: 13px; font-weight: 650; justify-content: space-between; padding: 13px; }.transfer-direction svg { color: var(--positive); }.transfer-available { color: var(--muted-strong); font-size: 12px; margin: -3px 0 4px; text-align: right; }.transfer-feedback { font-size: 13px; margin: 10px 0 0; text-align: center; }.spin { animation: spin .8s linear infinite; }@keyframes spin { to { transform: rotate(360deg); } }
</style>
