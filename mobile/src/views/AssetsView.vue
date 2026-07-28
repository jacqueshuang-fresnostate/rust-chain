<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowDownUp, ArrowUpToLine, ChevronRight, Download, ReceiptText, RefreshCw, WalletCards, X } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchMarginWallets } from '@/api/trading'
import { fetchWalletAccounts, transferWalletFunds } from '@/api/wallet'
import { formatAmount, formatFiat } from '@/core/format'
import { useModalDialog } from '@/core/modalDialog'
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
const transferDialog = ref<HTMLElement | null>(null)
const { trapFocus: trapTransferFocus } = useModalDialog(transferOpen, transferDialog)

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

function closeTransfer(): void {
  if (transferring.value) return
  transferOpen.value = false
}

function handleTransferDialogKeydown(event: KeyboardEvent): void {
  trapTransferFocus(event, closeTransfer)
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
    <PageHeader :title="t('assets.title')" :eyebrow="t('assets.totalValue')" :back="false">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('assets.refresh')" :disabled="loading" @click="loadAccounts">
          <RefreshCw :size="21" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content assets-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('assets.loginDescription')" />
      <template v-else>
        <section class="assets-summary">
          <div class="assets-summary__heading">
            <span>{{ t('assets.totalValue') }}</span>
            <WalletCards :size="20" aria-hidden="true" />
          </div>
          <strong class="numeric">{{ formatFiat(totalEstimate) }}</strong>
          <p>{{ t('assets.estimateNote') }}</p>
        </section>

        <div class="asset-actions" :aria-label="t('assets.operations')">
          <button type="button" @click="openDeposit">
            <span class="asset-action__icon"><Download :size="19" /></span>
            <span>{{ t('assets.deposit') }}</span>
          </button>
          <button type="button" @click="openProtectedRoute('withdraw-asset')">
            <span class="asset-action__icon"><ArrowUpToLine :size="19" /></span>
            <span>{{ t('assets.withdraw') }}</span>
          </button>
          <button type="button" @click="openTransfer">
            <span class="asset-action__icon"><ArrowDownUp :size="19" /></span>
            <span>{{ t('assets.transfer') }}</span>
          </button>
          <button type="button" @click="openProtectedRoute('wallet-ledger')">
            <span class="asset-action__icon"><ReceiptText :size="19" /></span>
            <span>{{ t('assets.ledger') }}</span>
          </button>
        </div>

        <button class="quick-recharge-entry" type="button" @click="openProtectedRoute('quick-recharge')">
          <span>
            <b>{{ t('assets.quickBuy') }}</b>
            <small>{{ t('assets.quickBuyDescription') }}</small>
          </span>
          <span class="quick-recharge-entry__action">{{ t('assets.go') }}<ChevronRight :size="18" /></span>
        </button>

        <div class="section-heading">
          <span>{{ t('assets.list') }}</span>
          <button class="section-heading__action" type="button" @click="session.logout">{{ t('assets.logout') }}</button>
        </div>
        <p v-if="error" class="error-message" role="alert">{{ error }}</p>
        <div v-if="assetRows.length" class="asset-list" role="list">
          <div v-for="account in assetRows" :key="account.symbol" class="asset-row" role="listitem">
            <AssetMark :symbol="account.symbol" :src="account.spot?.logoUrl || account.margin?.logoUrl" />
            <span class="asset-row__symbol">
              <b>{{ account.symbol }}</b>
              <small>{{ t('assets.accountSummary', { funding: formatAmount(account.spot?.available), contract: formatAmount(account.margin?.available) }) }}</small>
            </span>
            <span class="asset-row__value">
              <b class="numeric">{{ formatAmount(walletTotal(account.spot) + walletTotal(account.margin)) }}</b>
              <small>{{ t('assets.frozen', { amount: formatAmount((account.spot?.frozen || 0) + (account.spot?.locked || 0) + (account.margin?.frozen || 0) + (account.margin?.locked || 0)) }) }}</small>
            </span>
          </div>
        </div>
        <p v-else-if="!loading" class="empty-state">{{ t('assets.empty') }}</p>
      </template>
    </div>

    <div v-if="transferOpen" class="transfer-mask" @click.self="closeTransfer">
      <form
        ref="transferDialog"
        class="transfer-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="transfer-title"
        @keydown="handleTransferDialogKeydown"
        @submit.prevent="submitTransfer"
      >
        <header>
          <strong id="transfer-title">{{ t('assets.transferTitle') }}</strong>
          <button data-dialog-initial class="icon-button" type="button" :aria-label="t('common.close')" :disabled="transferring" @click="closeTransfer">
            <X :size="21" />
          </button>
        </header>
        <label class="transfer-field">
          <span>{{ t('assets.asset') }}</span>
          <select v-model="transferAsset">
            <option v-for="account in transferAccounts" :key="account.symbol" :value="account.symbol">{{ t('assets.assetAvailable', { asset: account.symbol, amount: formatAmount(account.available) }) }}</option>
          </select>
        </label>
        <label class="transfer-field">
          <span>{{ t('assets.from') }}</span>
          <select v-model="transferFrom">
            <option value="spot">{{ t('assets.fundingAccount') }}</option>
            <option value="margin">{{ t('assets.contractAccount') }}</option>
          </select>
        </label>
        <div class="transfer-direction">
          <span>{{ transferFrom === 'spot' ? t('assets.fundingAccount') : t('assets.contractAccount') }}</span>
          <ArrowDownUp :size="19" />
          <span>{{ transferFrom === 'spot' ? t('assets.contractAccount') : t('assets.fundingAccount') }}</span>
        </div>
        <p class="transfer-available">{{ t('assets.availableBalance', { amount: formatAmount(transferAvailable), asset: transferAsset }) }}</p>
        <label class="transfer-field">
          <span>{{ t('assets.transferAmount') }}</span>
          <input v-model="transferAmount" inputmode="decimal" :placeholder="t('assets.transferPlaceholder')" />
        </label>
        <button class="button button--primary button--full" type="submit" :disabled="transferring">{{ transferring ? t('assets.transferring') : t('assets.confirmTransfer') }}</button>
        <p v-if="transferFeedback" :class="transferFeedbackTone === 'success' ? 'up' : 'down'" class="transfer-feedback" aria-live="polite">{{ transferFeedback }}</p>
      </form>
    </div>
  </main>
</template>

<style scoped>
.assets-content { padding-bottom: calc(36px + env(safe-area-inset-bottom)); }
.assets-content > :deep(.login-required) { margin: 12px -16px 0; }
.assets-summary {
  background:
    radial-gradient(circle at 86% 15%, color-mix(in srgb, var(--signal-blue) 16%, transparent), transparent 34%),
    radial-gradient(circle at 12% 86%, color-mix(in srgb, var(--signal-green) 14%, transparent), transparent 34%),
    var(--surface-elevated);
  border-bottom: 1px solid var(--line-strong);
  border-top: 3px solid var(--signal-green);
  margin: 0 -16px;
  min-height: 224px;
  overflow: hidden;
  padding: 30px 16px 26px;
  position: relative;
}
.assets-summary::before {
  background: linear-gradient(90deg, var(--line-strong) 0 42%, transparent 42% 52%, var(--signal-green) 52% 100%);
  content: '';
  height: 2px;
  position: absolute;
  right: 16px;
  top: 16px;
  width: 58px;
}
.assets-summary__heading { align-items: center; color: var(--muted); display: flex; font-size: 13px; font-weight: 700; justify-content: space-between; }
.assets-summary__heading svg { color: var(--signal-green); }
.assets-summary strong { display: block; font-family: var(--data-font); font-size: 38px; letter-spacing: 0; line-height: 1.08; margin-top: 28px; }
.assets-summary p { color: var(--muted); font-size: 12px; line-height: 1.5; margin: 9px 0 0; }
.asset-actions { border-bottom: 1px solid var(--line); display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); margin: 0 -16px; }
.asset-actions button { align-items: center; background: transparent; color: var(--ink); display: flex; flex-direction: column; font-size: 11px; font-weight: 720; gap: 7px; justify-content: center; min-height: 86px; min-width: 0; padding: 8px 2px; }
.asset-actions button + button { border-left: 1px solid var(--line); }
.asset-action__icon { align-items: center; background: var(--soft); border: 1px solid var(--line); border-radius: 50%; color: var(--signal-green); display: inline-flex; height: 40px; justify-content: center; width: 40px; }
.quick-recharge-entry { align-items: center; background: var(--surface-elevated); border-bottom: 1px solid var(--line); border-top: 1px solid var(--line); display: flex; justify-content: space-between; margin: 16px -16px 0; min-height: 72px; padding: 12px 16px; text-align: left; width: calc(100% + 32px); }
.quick-recharge-entry > span:first-child { display: grid; gap: 4px; min-width: 0; padding-right: 10px; }
.quick-recharge-entry b { font-size: 15px; }
.quick-recharge-entry small { color: var(--muted); font-size: 12px; line-height: 1.4; }
.quick-recharge-entry__action { align-items: center; color: var(--signal-green); display: inline-flex; flex: 0 0 auto; font-size: 13px; font-weight: 750; gap: 2px; }
.asset-list { border-top: 1px solid var(--line); display: grid; }
.asset-row { align-items: center; border-bottom: 1px solid var(--line); display: grid; gap: 12px; grid-template-columns: 40px minmax(0, 1fr) minmax(90px, auto); min-height: 78px; }
.asset-row__symbol,
.asset-row__value { display: grid; min-width: 0; }
.asset-row b { font-size: 15px; }
.asset-row small { color: var(--muted); font-size: 11px; line-height: 1.35; margin-top: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.asset-row__value { text-align: right; }
.asset-row__value b { font-variant-numeric: tabular-nums; }
.transfer-mask { align-items: flex-end; background: var(--overlay); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: var(--layer-overlay); }
.transfer-dialog { background: var(--surface-elevated); border: 1px solid var(--line); border-radius: 8px; box-shadow: var(--shadow-soft); display: grid; gap: 13px; max-height: calc(100dvh - 32px - env(safe-area-inset-top) - env(safe-area-inset-bottom)); max-width: 448px; overflow-y: auto; overscroll-behavior: contain; padding: 18px; width: 100%; }
.transfer-dialog header { align-items: center; display: flex; justify-content: space-between; }
.transfer-dialog header strong { font-size: 20px; }
.transfer-field { background: var(--field-surface); border: 1px solid var(--line); border-radius: var(--radius); display: grid; gap: 2px; padding: 7px 12px; }
.transfer-field:focus-within { border-color: var(--focus); box-shadow: 0 0 0 2px var(--focus-ring); }
.transfer-field > span { color: var(--muted); font-size: 11px; font-weight: 650; }
.transfer-field input,
.transfer-field select { background: transparent; border: 0; color: var(--ink); min-height: 30px; outline: 0; padding: 0; width: 100%; }
.transfer-direction { align-items: center; background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); display: grid; font-size: 12px; font-weight: 700; gap: 8px; grid-template-columns: 1fr 28px 1fr; min-height: 48px; padding: 7px 12px; text-align: center; }
.transfer-direction svg { color: var(--positive); justify-self: center; }
.transfer-available { color: var(--muted-strong); font-size: 12px; margin: -3px 0 0; text-align: right; }
.transfer-feedback { font-size: 13px; margin: 0; text-align: center; }
.spin { animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 360px) {
  .assets-content > :deep(.login-required),
  .assets-summary,
  .asset-actions { margin-left: -12px; margin-right: -12px; }
  .quick-recharge-entry { margin-left: -12px; margin-right: -12px; padding-inline: 12px; width: calc(100% + 24px); }
  .assets-summary { padding-left: 12px; padding-right: 12px; }
}
@media (max-width: 340px) {
  .assets-content { padding-left: 12px; padding-right: 12px; }
  .assets-content > :deep(.login-required),
  .assets-summary,
  .asset-actions { margin-left: -12px; margin-right: -12px; }
  .quick-recharge-entry { margin-left: -12px; margin-right: -12px; padding-inline: 12px; width: calc(100% + 24px); }
  .assets-summary { padding-left: 12px; padding-right: 12px; }
  .asset-actions button { font-size: 11px; min-height: 76px; }
  .asset-action__icon { height: 38px; width: 38px; }
  .asset-row { gap: 9px; grid-template-columns: 36px minmax(0, 1fr) minmax(78px, auto); }
  .asset-row small { font-size: 10px; }
}
</style>
