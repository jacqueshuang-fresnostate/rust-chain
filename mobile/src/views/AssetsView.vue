<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowDownToLine,
  ArrowLeftRight,
  ArrowRight,
  ArrowUpFromLine,
  CheckCircle2,
  ChevronRight,
  Eye,
  EyeOff,
  PieChart,
  ReceiptText,
  Zap,
  X,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
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
const balanceVisible = ref(true)
const loading = ref(false)
const error = ref('')
const accountsReady = ref(false)
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

const totalEstimate = computed(() => estimateWallets(accounts.value) + estimateWallets(marginAccounts.value))
const accountDataAvailable = computed(() => session.isAuthenticated && accountsReady.value && !error.value)
const hasHoldings = computed(() => accountDataAvailable.value && assetRows.value.some((row) => walletTotal(row.spot) + walletTotal(row.margin) > 0))
const hasAllocation = computed(() => hasHoldings.value && totalEstimate.value > 0)
const accountStateLabel = computed(() => {
  if (!session.isAuthenticated) return t('common.loginRequiredTitle')
  if (loading.value) return t('common.loading')
  if (error.value) return t('common.serviceUnavailable')
  if (!hasHoldings.value) return t('assets.empty')
  return t('common.liveData')
})
const allocationRows = computed(() => {
  const rows = assetRows.value.map((row) => {
    const amount = walletTotal(row.spot) + walletTotal(row.margin)
    const value = ['USDT', 'USDC', 'USD'].includes(row.symbol)
      ? amount
      : amount * (marketStore.tickerFor(`${row.symbol}/USDT`)?.lastPrice || 0)
    return { ...row, amount, value }
  }).filter((row) => row.amount > 0 && row.value > 0)
  const total = rows.reduce((sum, row) => sum + row.value, 0)
  return rows
    .map((row) => ({ ...row, percent: total > 0 ? Math.max(1, Math.round((row.value / total) * 100)) : 0 }))
    .sort((left, right) => right.value - left.value)
    .slice(0, 4)
})

const transferAccounts = computed(() => transferFrom.value === 'spot' ? accounts.value : marginAccounts.value)
const transferAccount = computed(() => transferAccounts.value.find((account) => account.symbol === transferAsset.value))
const transferAvailable = computed(() => transferAccount.value?.available || 0)

async function loadAccounts(): Promise<void> {
  if (!session.isAuthenticated) {
    accounts.value = []
    marginAccounts.value = []
    accountsReady.value = false
    loading.value = false
    error.value = ''
    return
  }
  loading.value = true
  accountsReady.value = false
  error.value = ''
  try {
    const [, nextAccounts, marginState] = await Promise.all([marketStore.refresh(), fetchWalletAccounts(), fetchMarginWallets()])
    accounts.value = nextAccounts
    marginAccounts.value = marginState.wallets
    accountsReady.value = true
    if (!transferAccounts.value.some((account) => account.symbol === transferAsset.value)) transferAsset.value = transferAccounts.value[0]?.symbol || ''
  } catch (reason) {
    accounts.value = []
    marginAccounts.value = []
    error.value = apiErrorMessage(reason, t('assets.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openDeposit(): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/assets/deposit' } })
    return
  }
  void router.push({ name: 'deposit-asset' })
}

function openAssetsLogin(): void {
  void router.push({ name: 'login', query: { redirect: '/assets' } })
}

function openTransfer(): void {
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

function openProtectedRoute(name: 'withdraw-asset' | 'wallet-ledger' | 'withdrawal-records' | 'quick-recharge'): void {
  if (!session.isAuthenticated) {
    void router.push({ name: 'login', query: { redirect: '/assets' } })
    return
  }
  void router.push({ name })
}

async function submitTransfer(): Promise<void> {
  const transferValue = Number(transferAmount.value)
  if (!transferAsset.value || !Number.isFinite(transferValue) || transferValue <= 0) {
    transferFeedback.value = t('assets.invalidTransfer')
    transferFeedbackTone.value = 'error'
    return
  }
  if (transferValue > transferAvailable.value) {
    transferFeedback.value = t('assets.exceedsBalance')
    transferFeedbackTone.value = 'error'
    return
  }
  transferring.value = true
  transferFeedback.value = ''
  try {
    const to = transferFrom.value === 'spot' ? 'margin' : 'spot'
    await transferWalletFunds(transferAsset.value, transferFrom.value, to, transferValue)
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

function estimateWallets(wallets: WalletAccount[]): number {
  return wallets.reduce((total, account) => {
    const accountAmount = walletTotal(account)
    if (account.symbol === 'USDT' || account.symbol === 'USDC' || account.symbol === 'USD') return total + accountAmount
    return total + accountAmount * (marketStore.tickerFor(`${account.symbol}/USDT`)?.lastPrice || 0)
  }, 0)
}

function syncTransferAsset(): void {
  if (!transferAccounts.value.some((account) => account.symbol === transferAsset.value)) transferAsset.value = transferAccounts.value[0]?.symbol || ''
}

watch(transferFrom, syncTransferAsset)
watch(() => session.isAuthenticated, () => { void loadAccounts() }, { immediate: true })
</script>

<template>
  <main
    class="page pencil-page pencil-root-page assets-pencil"
    data-assets-workspace="live"
    data-pencil-source="CUK3y i6YDBr"
  >
    <PageHeader :back="false" :pencil="true" :title="t('assets.title')">
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('home.assetOverview')"
          :aria-pressed="!balanceVisible"
          @click="balanceVisible = !balanceVisible"
        >
          <Eye v-if="balanceVisible" :size="20" aria-hidden="true" />
          <EyeOff v-else :size="20" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>

    <div class="pencil-content assets-pencil__content">
      <section
        class="pencil-hero assets-summary"
        :aria-busy="loading"
        :data-account-state="accountDataAvailable ? 'ready' : loading ? 'loading' : error ? 'error' : 'guest'"
      >
        <span class="pencil-eyebrow">{{ t('assets.totalValue') }}</span>
        <strong class="pencil-numeric assets-summary__value">
          <span>$</span>
          <b>{{ !balanceVisible
            ? '••••••'
            : accountDataAvailable
              ? formatFiat(totalEstimate).replace(/^\$/, '')
              : session.isAuthenticated && loading
                ? t('common.loading')
                : session.isAuthenticated && error
                  ? t('common.serviceUnavailable')
                  : '••••••' }}</b>
        </strong>
        <p>
          {{ !session.isAuthenticated
            ? t('assets.syncEstimateHint')
            : accountDataAvailable && !hasHoldings
              ? t('assets.empty')
              : t('assets.estimateNote') }}
        </p>
        <button v-if="!session.isAuthenticated" class="pencil-primary" type="button" @click="openAssetsLogin">
          {{ t('assets.loginViewAssets') }}
          <ArrowRight :size="17" aria-hidden="true" />
        </button>
        <button v-else-if="error" class="pencil-secondary" type="button" :disabled="loading" @click="loadAccounts">
          {{ t('common.retry') }}
        </button>
      </section>

      <nav class="pencil-action-grid assets-actions" :aria-label="t('assets.operations')">
        <button type="button" @click="openDeposit"><span><ArrowDownToLine :size="19" /></span>{{ t('assets.deposit') }}</button>
        <button type="button" @click="openProtectedRoute('withdraw-asset')"><span><ArrowUpFromLine :size="19" /></span>{{ t('assets.withdraw') }}</button>
        <button type="button" @click="openTransfer"><span><ArrowLeftRight :size="19" /></span>{{ t('assets.transfer') }}</button>
        <button type="button" @click="openProtectedRoute('wallet-ledger')"><span><ReceiptText :size="20" /></span>{{ t('assets.quickLedger') }}</button>
      </nav>

      <section class="pencil-section assets-distribution" :aria-busy="loading">
        <div class="pencil-section__heading">
          <h2>{{ t('rootPrototype.assetAllocation') }}</h2>
          <span v-if="hasAllocation" class="pencil-pill">{{ t('common.liveData') }}</span>
        </div>
        <div v-if="accountDataAvailable && hasAllocation" class="assets-allocation">
          <button
            v-for="row in allocationRows"
            :key="row.symbol"
            class="assets-allocation__row"
            type="button"
            @click="openProtectedRoute('wallet-ledger')"
          >
            <AssetMark :symbol="row.symbol" :size="32" />
            <span><strong>{{ row.symbol }}</strong><i><b :style="{ width: `${row.percent}%` }" /></i></span>
            <em class="pencil-numeric">{{ row.percent }}%</em>
          </button>
        </div>
        <div v-else class="pencil-state assets-distribution__state" role="status">
          <PieChart :size="27" aria-hidden="true" />
          <span>{{ loading ? t('common.loading') : session.isAuthenticated ? accountStateLabel : t('assets.distributionLoginHint') }}</span>
        </div>
      </section>

      <section class="pencil-section assets-tools">
        <div class="pencil-section__heading"><h2>{{ t('assets.fundTools') }}</h2></div>
        <div class="pencil-list">
          <button class="pencil-row" type="button" @click="openProtectedRoute('wallet-ledger')">
            <span class="pencil-row__icon"><ReceiptText :size="18" /></span>
            <span class="pencil-row__copy"><strong>{{ t('assets.fundLedger') }}</strong></span>
            <ChevronRight :size="17" />
          </button>
          <button class="pencil-row" type="button" @click="openProtectedRoute('withdrawal-records')">
            <span class="pencil-row__icon"><ArrowUpFromLine :size="19" /></span>
            <span class="pencil-row__copy"><strong>{{ t('withdrawRecords.title') }}</strong></span>
            <ChevronRight :size="17" />
          </button>
          <button class="pencil-row" type="button" @click="openProtectedRoute('quick-recharge')">
            <span class="pencil-row__icon"><Zap :size="18" /></span>
            <span class="pencil-row__copy"><strong>{{ t('assets.quickRecharge') }}</strong></span>
            <ChevronRight :size="17" />
          </button>
        </div>
      </section>
    </div>

    <div v-if="transferOpen" class="confirmation-layer">
      <button class="confirmation-overlay-dismiss" type="button" :aria-label="t('common.close')" :disabled="transferring" tabindex="-1" @click="closeTransfer" />
      <section
        ref="transferDialog"
        class="confirmation-sheet"
        role="dialog"
        aria-modal="true"
        :aria-busy="transferring"
        :aria-label="t('assets.transfer')"
        tabindex="-1"
        @keydown="handleTransferDialogKeydown"
      >
        <header>
          <span class="confirmation-icon"><CheckCircle2 :size="20" /></span>
          <div><span>{{ t('common.confirm') }}</span><h2>{{ t('assets.transfer') }}</h2></div>
        </header>
        <label class="field">
          <span>{{ t('assets.transferFrom') }}</span>
          <select v-model="transferFrom">
            <option value="spot">{{ t('assets.fundingAccount') }}</option>
            <option value="margin">{{ t('assets.marginAccount') }}</option>
          </select>
        </label>
        <label class="field">
          <span>{{ t('common.amount') }}</span>
          <div>
            <input v-model="transferAmount" inputmode="decimal" />
            <select v-model="transferAsset">
              <option v-for="account in transferAccounts" :key="account.assetId" :value="account.symbol">{{ account.symbol }}</option>
            </select>
          </div>
        </label>
        <p class="field-hint">{{ t('common.available') }} {{ formatAmount(transferAvailable) }} {{ transferAsset }}</p>
        <p v-if="transferFeedback" :class="transferFeedbackTone === 'success' ? 'positive' : 'field-error'" aria-live="polite">{{ transferFeedback }}</p>
        <div class="confirmation-actions">
          <button data-dialog-cancel type="button" :disabled="transferring" @click="closeTransfer"><X :size="16" />{{ t('common.cancel') }}</button>
          <button class="confirmation-primary" type="button" :disabled="transferring" @click="submitTransfer">
            {{ transferring ? t('common.submitting') : t('common.confirm') }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.assets-pencil__content {
  display: grid;
  gap: 10px;
  grid-template-rows: 157px 80px 159px 207px;
  padding-bottom: 0;
  padding-top: 10px;
}

.assets-summary {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 8px;
  height: 157px;
  justify-content: flex-start;
  margin-inline: -20px;
  min-height: 157px;
  padding: 10px 20px 8px;
}

.assets-summary > .pencil-eyebrow {
  line-height: 16px;
}

.assets-summary__value {
  align-items: center;
  display: flex;
  gap: 8px;
  letter-spacing: 0;
  height: 39px;
  line-height: 39px;
  min-height: 39px;
  overflow: hidden;
  white-space: nowrap;
}

.assets-summary__value > span {
  color: var(--accent);
  font-size: 20px;
  font-weight: 650;
  line-height: 26px;
  transform: translateY(.5px);
}

.assets-summary__value > b {
  color: var(--ink);
  font-size: 30px;
  font-weight: 700;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.assets-summary p {
  color: var(--muted);
  font-size: 10px;
  line-height: 14px;
  margin: 0;
  min-height: 14px;
}

.assets-summary .pencil-primary,
.assets-summary .pencil-secondary {
  align-self: stretch;
  flex: 0 0 46px;
  height: 46px;
  margin: 0;
  min-height: 46px;
  width: 100%;
}

.assets-actions {
  align-items: start;
  box-sizing: border-box;
  height: 80px;
  min-height: 80px;
  padding: 6px 0 0;
}

.assets-actions button {
  min-height: 68px;
}

.assets-distribution {
  box-sizing: border-box;
  display: grid;
  gap: 10px;
  grid-template-rows: 23px 110px;
  height: 159px;
  min-height: 159px;
  padding: 12px 0 4px;
}

.assets-allocation {
  align-content: center;
  display: grid;
  gap: 1px;
  height: 110px;
  overflow: hidden;
}

.assets-allocation__row { align-items: center; background: transparent; display: grid; gap: 10px; grid-template-columns: 28px minmax(0, 1fr) 40px; height: 27px; min-height: 27px; padding: 0; text-align: left; width: 100%; }
.assets-allocation__row :deep(.asset-mark) { height: 26px !important; width: 26px !important; }
.assets-allocation__row > span { display: grid; gap: 6px; min-width: 0; }
.assets-allocation__row strong { font-size: 12px; }
.assets-allocation__row i { background: var(--surface-3); border-radius: 999px; display: block; height: 3px; overflow: hidden; }
.assets-allocation__row i b { background: var(--accent); border-radius: inherit; display: block; height: 100%; }
.assets-allocation__row em { color: var(--muted-strong); font-size: 11px; font-style: normal; text-align: right; }
.assets-distribution__state { height: 110px; min-height: 110px; padding: 0; }
.assets-distribution__state strong { color: var(--ink); font-size: 13px; }
.assets-distribution__state span { font-size: 11px; }
.assets-tools {
  box-sizing: border-box;
  display: grid;
  gap: 6px;
  grid-template-rows: 23px 156px;
  height: 207px;
  min-height: 207px;
  padding: 10px 0 12px;
}

.assets-tools .pencil-list {
  grid-template-rows: repeat(3, 52px);
}

.assets-tools .pencil-row {
  height: 52px;
  min-height: 52px;
}

@media (max-width: 340px) {
  .assets-summary__value > b { font-size: 28px; }
}
</style>
