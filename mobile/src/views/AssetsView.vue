<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowDownLeft,
  ArrowLeftRight,
  ArrowUpRight,
  CandlestickChart,
  CheckCircle2,
  ChevronRight,
  CreditCard,
  Eye,
  EyeOff,
  History,
  Landmark,
  Layers3,
  SlidersHorizontal,
  WalletCards,
  X,
} from 'lucide-vue-next'
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

const visibleAssetRows = computed<Array<{
  key: string
  symbol: string
  spot?: WalletAccount
  margin?: WalletAccount
  placeholder?: boolean
}>>(() => assetRows.value.length
  ? assetRows.value.slice(0, 3).map((row) => ({ ...row, key: row.symbol }))
  : [1, 2, 3].map((slot) => ({ key: `placeholder-${slot}`, symbol: '--', placeholder: true })))
const spotEstimate = computed(() => estimateWallets(accounts.value))
const marginEstimate = computed(() => estimateWallets(marginAccounts.value))
const totalEstimate = computed(() => spotEstimate.value + marginEstimate.value)
const accountDataAvailable = computed(() => session.isAuthenticated && accountsReady.value && !error.value)
const accountStateLabel = computed(() => {
  if (!session.isAuthenticated) return t('common.loginRequiredTitle')
  if (loading.value) return t('common.loading')
  if (error.value) return t('common.serviceUnavailable')
  return t('common.liveData')
})
const allocation = computed(() => {
  const values = new Map<string, number>()
  for (const row of assetRows.value) {
    const amount = walletTotal(row.spot) + walletTotal(row.margin)
    const value = ['USDT', 'USDC', 'USD'].includes(row.symbol)
      ? amount
      : amount * (marketStore.tickerFor(`${row.symbol}/USDT`)?.lastPrice || 0)
    values.set(row.symbol, value)
  }
  const total = [...values.values()].reduce((sum, value) => sum + value, 0)
  if (total <= 0) return { btc: 0, eth: 0, usdt: 0, other: 0 }
  const ratio = (symbol: string) => total > 0 ? Math.round(((values.get(symbol) || 0) / total) * 100) : 0
  const btc = ratio('BTC')
  const eth = ratio('ETH')
  const usdt = ratio('USDT') + ratio('USDC') + ratio('USD')
  return { btc, eth, usdt, other: Math.max(0, 100 - btc - eth - usdt) }
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

function openProtectedRoute(name: 'withdraw-asset' | 'wallet-ledger' | 'quick-recharge'): void {
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
  <main class="view assets-view prototype-root-view" data-assets-workspace="live">
    <div class="page-intro compact">
      <span class="eyebrow">{{ t('rootPrototype.assetField') }}</span>
      <h1>{{ t('rootPrototype.assetHeadlineLine1') }}<br />{{ t('rootPrototype.assetHeadlineLine2') }}</h1>
    </div>

    <section class="asset-hero" :aria-busy="loading">
      <div class="asset-orbit" aria-hidden="true"><span /><i /><b /></div>
      <div class="asset-hero-copy">
        <div class="balance-label">
          <span>{{ t('home.totalAssetValue') }}</span>
          <button
            class="inline-icon"
            type="button"
            :aria-label="t('home.assetOverview')"
            :aria-pressed="!balanceVisible"
            @click="balanceVisible = !balanceVisible"
          >
            <Eye v-if="balanceVisible" :size="16" aria-hidden="true" />
            <EyeOff v-else :size="16" aria-hidden="true" />
          </button>
        </div>
        <strong class="numeric">
          {{ !balanceVisible
            ? '$••••••'
            : accountDataAvailable
              ? formatFiat(totalEstimate)
              : session.isAuthenticated
                ? '$--'
                : '$••••••' }}
        </strong>
        <span class="positive">{{ t('rootPrototype.todayReturn') }} --</span>
      </div>
      <div v-if="error" class="asset-hero-state" role="alert">
        <span>{{ error }}</span>
        <button type="button" :disabled="loading" @click="loadAccounts">{{ t('common.retry') }}</button>
      </div>
    </section>

    <div class="asset-actions">
      <button type="button" @click="openDeposit"><span><ArrowDownLeft :size="20" /></span>{{ t('assets.deposit') }}</button>
      <button type="button" @click="openProtectedRoute('withdraw-asset')"><span><ArrowUpRight :size="20" /></span>{{ t('assets.withdraw') }}</button>
      <button type="button" @click="openTransfer"><span><ArrowLeftRight :size="20" /></span>{{ t('assets.transfer') }}</button>
      <button type="button" @click="openProtectedRoute('quick-recharge')"><span><CreditCard :size="20" /></span>{{ t('assets.quickBuy') }}</button>
    </div>

    <section class="content-section allocation-section">
      <div class="section-heading">
        <div><span class="eyebrow">{{ t('rootPrototype.allocationLabel') }}</span><h2>{{ t('rootPrototype.assetAllocation') }}</h2></div>
        <button class="icon-button small" type="button" :aria-label="t('rootPrototype.assetAllocation')" @click="openProtectedRoute('wallet-ledger')">
          <SlidersHorizontal :size="15" />
        </button>
      </div>
      <div class="allocation-track" :aria-label="t('rootPrototype.assetAllocation')">
        <i class="allocation-btc" :style="{ width: `${allocation.btc}%` }" />
        <i class="allocation-eth" :style="{ width: `${allocation.eth}%` }" />
        <i class="allocation-usdt" :style="{ width: `${allocation.usdt}%` }" />
        <i class="allocation-other" :style="{ width: `${allocation.other}%` }" />
      </div>
      <div class="allocation-legend">
        <span><i class="btc-dot" /> BTC <b>{{ allocation.btc }}%</b></span>
        <span><i class="eth-dot" /> ETH <b>{{ allocation.eth }}%</b></span>
        <span><i class="usdt-dot" /> USDT <b>{{ allocation.usdt }}%</b></span>
        <span><i class="other-dot" /> {{ t('rootPrototype.otherAssets') }} <b>{{ allocation.other }}%</b></span>
      </div>
    </section>

    <section class="content-section">
      <div class="section-heading"><div><span class="eyebrow">{{ t('rootPrototype.holdingsLabel') }}</span><h2>{{ t('rootPrototype.holdings') }}</h2></div></div>
      <div class="account-list">
        <button
          v-for="row in visibleAssetRows"
          :key="row.key"
          type="button"
          class="account-row"
          :disabled="row.placeholder"
          @click="openProtectedRoute('wallet-ledger')"
        >
          <span class="account-icon"><WalletCards :size="19" /></span>
          <span>
            <strong>{{ row.symbol }}</strong>
            <small>
              {{ t('common.available') }}
              {{ accountDataAvailable && !row.placeholder ? formatAmount(row.spot?.available || 0) : '--' }}
            </small>
          </span>
          <b class="numeric">
            {{ !balanceVisible
              ? '••••'
              : accountDataAvailable && !row.placeholder
                ? formatAmount(walletTotal(row.spot) + walletTotal(row.margin))
                : '--' }}
          </b>
          <ChevronRight :size="16" />
        </button>
      </div>
    </section>

    <section class="content-section">
      <div class="section-heading">
        <div><span class="eyebrow">{{ t('rootPrototype.accountsLabel') }}</span><h2>{{ t('rootPrototype.accounts') }}</h2></div>
        <button class="text-action" type="button" @click="openProtectedRoute('wallet-ledger')">
          {{ t('assets.ledger') }} <History :size="15" />
        </button>
      </div>
      <div class="account-list">
        <button class="account-row" type="button" @click="openProtectedRoute('wallet-ledger')">
          <span class="account-icon"><CandlestickChart :size="19" /></span>
          <span><strong>{{ t('assets.fundingAccount') }}</strong><small :class="{ positive: accountDataAvailable }">{{ accountStateLabel }}</small></span>
          <b class="numeric">{{ balanceVisible && accountDataAvailable ? formatFiat(spotEstimate) : '$••••' }}</b>
          <ChevronRight :size="16" />
        </button>
        <button class="account-row" type="button" @click="openProtectedRoute('wallet-ledger')">
          <span class="account-icon"><Layers3 :size="19" /></span>
          <span><strong>{{ t('assets.marginAccount') }}</strong><small :class="{ positive: accountDataAvailable }">{{ accountStateLabel }}</small></span>
          <b class="numeric">{{ balanceVisible && accountDataAvailable ? formatFiat(marginEstimate) : '$••••' }}</b>
          <ChevronRight :size="16" />
        </button>
        <button class="account-row" type="button" @click="router.push({ name: 'earn' })">
          <span class="account-icon"><Landmark :size="19" /></span>
          <span><strong>{{ t('rootPrototype.earnAccount') }}</strong><small>{{ t('products.earn') }}</small></span>
          <b>--</b>
          <ChevronRight :size="16" />
        </button>
      </div>
    </section>

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
