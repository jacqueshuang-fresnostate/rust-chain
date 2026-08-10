<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowDownToLine,
  ArrowLeftRight,
  ArrowRight,
  ArrowUpFromLine,
  ChevronRight,
  Eye,
  EyeOff,
  ReceiptText,
  WalletCards,
  Zap,
  X,
} from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import PageHeader from '@/components/PageHeader.vue'
import assetsHeroDark from '@/assets/assets/assets-hero-dark.jpg'
import assetsHeroLight from '@/assets/assets/assets-hero-light.jpg'
import { apiErrorMessage } from '@/api/client'
import { fetchMarginWallets } from '@/api/trading'
import {
  createTodayReturnRequestLifecycle,
  fetchTodayReturn,
  fetchWalletAccounts,
  transferWalletFunds,
  type TodayReturn,
} from '@/api/wallet'
import { formatAmount, formatFiat } from '@/core/format'
import { useModalDialog } from '@/core/modalDialog'
import { createSessionRequestLifecycle } from '@/core/sessionRequest'
import {
  resolveTodayReturnPresentation,
  type TodayReturnViewState,
} from '@/core/todayReturnPresentation'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'
import { useThemeStore } from '@/stores/theme'
import type { WalletAccount } from '@/core/types'

type AssetHoldingRow = {
  symbol: string
  spot?: WalletAccount
  margin?: WalletAccount
  logoUrl?: string
  amount: number
  available: number
  frozen: number
  estimatedValue: number | null
}

const QUOTE_ASSET_SYMBOL = 'USDT'

const router = useRouter()
const marketStore = useMarketStore()
const session = useSessionStore()
const theme = useThemeStore()
const { locale, t } = useI18n()
const accounts = ref<WalletAccount[]>([])
const marginAccounts = ref<WalletAccount[]>([])
const balanceVisible = ref(true)
const loading = ref(false)
const error = ref('')
const accountsReady = ref(false)
const todayReturn = ref<TodayReturn | null>(null)
const todayReturnState = ref<TodayReturnViewState>('idle')
const accountRequestLifecycle = createSessionRequestLifecycle({
  sessionKey: () => session.token,
  request: async () => {
    const [, nextAccounts, marginState] = await Promise.all([
      marketStore.refresh(),
      fetchWalletAccounts(),
      fetchMarginWallets(),
    ])
    return { accounts: nextAccounts, marginAccounts: marginState.wallets }
  },
})
const todayReturnRequestLifecycle = createTodayReturnRequestLifecycle({
  sessionKey: () => session.token,
  fetchTodayReturn,
})
const transferOpen = ref(false)
const transferAsset = ref('')
const transferAmount = ref('')
const transferFrom = ref<'spot' | 'margin'>('spot')
const transferFeedback = ref('')
const transferFeedbackTone = ref<'success' | 'error'>('error')
const transferring = ref(false)
let transferRequestVersion = 0
const transferDialog = ref<HTMLElement | null>(null)
const { trapFocus: trapTransferFocus } = useModalDialog(transferOpen, transferDialog)

const assetRows = computed(() => {
  const rows = new Map<string, { symbol: string; spot?: WalletAccount; margin?: WalletAccount }>()
  for (const account of accounts.value) rows.set(account.symbol, { ...rows.get(account.symbol), symbol: account.symbol, spot: account })
  for (const account of marginAccounts.value) rows.set(account.symbol, { ...rows.get(account.symbol), symbol: account.symbol, margin: account })
  return [...rows.values()]
})

const accountDataAvailable = computed(() => session.isAuthenticated && accountsReady.value && !error.value)
const holdingRows = computed<AssetHoldingRow[]>(() => assetRows.value
  .map((row) => {
    const amount = walletTotal(row.spot) + walletTotal(row.margin)
    const available = walletAvailable(row.spot) + walletAvailable(row.margin)
    const frozen = walletFrozen(row.spot) + walletFrozen(row.margin)
    return {
      ...row,
      logoUrl: row.spot?.logoUrl || row.margin?.logoUrl,
      amount,
      available,
      frozen,
      estimatedValue: estimateAssetValue(row.symbol, amount),
    }
  })
  .filter((row) => row.amount > 0)
  .sort((left, right) => {
    if (left.estimatedValue === null && right.estimatedValue !== null) return 1
    if (left.estimatedValue !== null && right.estimatedValue === null) return -1
    if (left.estimatedValue !== null && right.estimatedValue !== null && left.estimatedValue !== right.estimatedValue) {
      return right.estimatedValue - left.estimatedValue
    }
    return left.symbol.localeCompare(right.symbol)
  }))
const hasHoldings = computed(() => accountDataAvailable.value && holdingRows.value.length > 0)
const valuedHoldingCount = computed(() => holdingRows.value.filter((row) => row.estimatedValue !== null).length)
const totalEstimate = computed(() => holdingRows.value.reduce((total, row) => total + (row.estimatedValue ?? 0), 0))
const memberState = computed<'loading' | 'error' | 'empty' | 'holdings'>(() => {
  if (error.value) return 'error'
  if (loading.value || !accountsReady.value) return 'loading'
  return hasHoldings.value ? 'holdings' : 'empty'
})
const estimateCoverage = computed<'full' | 'partial' | 'unavailable' | 'empty'>(() => {
  if (!hasHoldings.value) return 'empty'
  if (valuedHoldingCount.value === 0) return 'unavailable'
  return valuedHoldingCount.value === holdingRows.value.length ? 'full' : 'partial'
})
const totalEstimateLabel = computed(() => {
  if (!balanceVisible.value) return '••••••'
  if (!accountDataAvailable.value || estimateCoverage.value === 'unavailable') return '--'
  return new Intl.NumberFormat(locale.value === 'en' ? 'en-US' : 'zh-CN', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(totalEstimate.value)
})
const todayReturnPresentation = computed(() => resolveTodayReturnPresentation({
  visible: balanceVisible.value,
  state: todayReturnState.value,
  value: todayReturn.value,
  amountMask: '••••••',
  detailMask: '••••',
  messages: {
    loading: t('home.todayReturnLoading'),
    partial: (assets) => t('home.todayReturnPartial', { assets }),
    partialUnknown: t('home.todayReturnPartialUnknown'),
    error: t('home.todayReturnUnavailable'),
  },
}))

const transferAccounts = computed(() => transferFrom.value === 'spot' ? accounts.value : marginAccounts.value)
const transferAccount = computed(() => transferAccounts.value.find((account) => account.symbol === transferAsset.value))
const transferAvailable = computed<number | null>(() => transferAccount.value?.available ?? null)
const transferAvailableLabel = computed(() => transferAvailable.value === null ? '--' : formatAmount(transferAvailable.value))
const transferTarget = computed<'spot' | 'margin'>(() => transferFrom.value === 'spot' ? 'margin' : 'spot')
const canSubmitTransfer = computed(() => {
  const value = Number(transferAmount.value)
  return Boolean(
    transferAsset.value
    && transferAvailable.value !== null
    && Number.isFinite(value)
    && value > 0
    && value <= transferAvailable.value
    && !transferring.value,
  )
})

async function loadAccounts(): Promise<void> {
  if (!session.token) {
    resetSessionAccountState()
    loading.value = false
    error.value = ''
    return
  }
  loading.value = true
  accountsReady.value = false
  error.value = ''
  const result = await accountRequestLifecycle.load()
  if (result.state === 'stale') return
  if (result.state === 'guest') {
    accounts.value = []
    marginAccounts.value = []
    loading.value = false
    return
  }
  if (result.state === 'loaded') {
    accounts.value = result.value.accounts
    marginAccounts.value = result.value.marginAccounts
    accountsReady.value = true
    if (!transferAccounts.value.some((account) => account.symbol === transferAsset.value)) transferAsset.value = transferAccounts.value[0]?.symbol || ''
  } else {
    accounts.value = []
    marginAccounts.value = []
    error.value = apiErrorMessage(result.error, t('assets.loadFailed'))
  }
  loading.value = false
}

async function loadTodayReturn(): Promise<void> {
  if (!session.token) {
    todayReturn.value = null
    todayReturnState.value = 'idle'
    return
  }

  todayReturn.value = null
  todayReturnState.value = 'loading'
  const result = await todayReturnRequestLifecycle.load()
  if (result.state === 'stale') return
  if (result.state === 'guest') {
    todayReturn.value = null
    todayReturnState.value = 'idle'
    return
  }
  if (result.state === 'loaded') {
    todayReturn.value = result.value
    todayReturnState.value = result.value.status
    return
  }
  todayReturn.value = null
  todayReturnState.value = 'error'
}

function resetSessionAccountState(): void {
  accounts.value = []
  marginAccounts.value = []
  accountsReady.value = false
  transferOpen.value = false
  transferAsset.value = ''
  transferAmount.value = ''
  transferFeedback.value = ''
  transferring.value = false
  transferRequestVersion += 1
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

function swapTransferRoute(): void {
  if (transferring.value) return
  const nextFrom = transferTarget.value
  transferFrom.value = nextFrom
  const nextAccounts = nextFrom === 'spot' ? accounts.value : marginAccounts.value
  if (!nextAccounts.some((account) => account.symbol === transferAsset.value)) {
    transferAsset.value = nextAccounts[0]?.symbol || ''
  }
  transferAmount.value = ''
  transferFeedback.value = ''
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
  if (!transferAsset.value || transferAvailable.value === null || !Number.isFinite(transferValue) || transferValue <= 0) {
    transferFeedback.value = t('assets.invalidTransfer')
    transferFeedbackTone.value = 'error'
    return
  }
  if (transferValue > transferAvailable.value) {
    transferFeedback.value = t('assets.exceedsBalance')
    transferFeedbackTone.value = 'error'
    return
  }
  const sessionKey = session.token
  if (!sessionKey) return
  const requestVersion = ++transferRequestVersion
  transferring.value = true
  transferFeedback.value = ''
  try {
    const to = transferFrom.value === 'spot' ? 'margin' : 'spot'
    const sourceLogo = transferAccount.value?.logoUrl
      || accounts.value.find((account) => account.symbol === transferAsset.value)?.logoUrl
      || marginAccounts.value.find((account) => account.symbol === transferAsset.value)?.logoUrl
    const result = await transferWalletFunds(transferAsset.value, transferFrom.value, to, transferValue)
    if (requestVersion !== transferRequestVersion || session.token !== sessionKey) return
    accounts.value = upsertWalletAccount(accounts.value, { ...result.spotWallet, logoUrl: sourceLogo })
    marginAccounts.value = upsertWalletAccount(marginAccounts.value, { ...result.marginWallet, logoUrl: sourceLogo })
    transferFeedback.value = t('assets.transferSuccess')
    transferFeedbackTone.value = 'success'
    transferAmount.value = ''
  } catch (reason) {
    if (requestVersion !== transferRequestVersion || session.token !== sessionKey) return
    transferFeedback.value = apiErrorMessage(reason, t('assets.transferFailed'))
    transferFeedbackTone.value = 'error'
  } finally {
    if (requestVersion === transferRequestVersion && session.token === sessionKey) transferring.value = false
  }
}

function walletTotal(account?: WalletAccount): number {
  return account ? account.available + account.frozen + account.locked : 0
}

function walletAvailable(account?: WalletAccount): number {
  return account?.available || 0
}

function walletFrozen(account?: WalletAccount): number {
  return account ? account.frozen + account.locked : 0
}

function upsertWalletAccount(wallets: WalletAccount[], next: WalletAccount): WalletAccount[] {
  const current = wallets.find((account) => account.assetId === next.assetId || account.symbol === next.symbol)
  const merged = { ...current, ...next, logoUrl: next.logoUrl || current?.logoUrl }
  return current
    ? wallets.map((account) => account === current ? merged : account)
    : [...wallets, merged]
}

function estimateAssetValue(symbol: string, amount: number): number | null {
  if (symbol === QUOTE_ASSET_SYMBOL) return amount
  const lastPrice = marketStore.tickerFor(`${symbol}/USDT`)?.lastPrice
  return Number.isFinite(lastPrice) && Number(lastPrice) > 0 ? amount * Number(lastPrice) : null
}

function syncTransferAsset(): void {
  if (!transferAccounts.value.some((account) => account.symbol === transferAsset.value)) transferAsset.value = transferAccounts.value[0]?.symbol || ''
}

watch(transferFrom, syncTransferAsset)
watch(() => session.token, () => {
  accountRequestLifecycle.invalidate()
  resetSessionAccountState()
  void loadAccounts()
}, { immediate: true })
watch(() => session.token, () => {
  todayReturnRequestLifecycle.invalidate()
  void loadTodayReturn()
}, { immediate: true })
onUnmounted(() => {
  accountRequestLifecycle.stop()
  todayReturnRequestLifecycle.stop()
  transferRequestVersion += 1
})
</script>

<template>
  <main
    class="page pencil-page pencil-root-page assets-pencil"
    data-assets-workspace="live"
    :data-assets-branch="session.isAuthenticated ? 'member' : 'guest'"
    data-pencil-source="CUK3y i6YDBr p61z2Q Q4JYj v6phV TuWXq"
  >
    <PageHeader :back="false" :pencil="true" :title="t('assets.title')" />

    <div v-if="!session.isAuthenticated" class="pencil-content assets-pencil__guest-content" data-assets-state="guest">
      <section class="pencil-hero assets-hero assets-hero--guest">
        <img v-show="!theme.isDark" class="assets-hero__image" :src="assetsHeroLight" alt="">
        <img v-show="theme.isDark" class="assets-hero__image" :src="assetsHeroDark" alt="">
        <span class="assets-hero__overlay" :class="{ 'assets-hero__overlay--dark': theme.isDark }" aria-hidden="true" />
        <span class="assets-hero__bloom" aria-hidden="true" />
        <div class="assets-guest-copy">
          <span class="assets-hero__eyebrow">{{ t('assets.guestKicker') }}</span>
          <h1>{{ t('assets.guestTitle') }}</h1>
          <p>{{ t('assets.guestDescription') }}</p>
        </div>
        <button class="assets-guest-login" type="button" @click="openAssetsLogin">
          {{ t('assets.loginViewAssets') }}
          <ArrowRight :size="17" aria-hidden="true" />
        </button>
      </section>
    </div>

    <div
      v-else
      class="pencil-content assets-pencil__member-content"
      :aria-busy="loading"
      :data-assets-state="memberState"
    >
      <section
        class="pencil-hero assets-hero assets-hero--member"
        :data-account-state="memberState"
        :data-estimate-coverage="estimateCoverage"
      >
        <img v-show="!theme.isDark" class="assets-hero__image" :src="assetsHeroLight" alt="">
        <img v-show="theme.isDark" class="assets-hero__image" :src="assetsHeroDark" alt="">
        <span class="assets-hero__overlay" :class="{ 'assets-hero__overlay--dark': theme.isDark }" aria-hidden="true" />
        <span class="assets-hero__bloom" aria-hidden="true" />

        <div class="assets-member-summary">
          <div class="assets-member-summary__total">
            <div class="assets-member-summary__label">
              <span>{{ t('assets.totalValue') }}</span>
              <button
                class="assets-balance-toggle"
                type="button"
                :aria-label="balanceVisible ? t('assets.hideBalance') : t('assets.showBalance')"
                :aria-pressed="!balanceVisible"
                @click="balanceVisible = !balanceVisible"
              >
                <Eye v-if="balanceVisible" :size="14" aria-hidden="true" />
                <EyeOff v-else :size="14" aria-hidden="true" />
              </button>
            </div>
            <div class="assets-member-summary__value">
              <strong class="pencil-numeric">{{ totalEstimateLabel }}</strong>
              <small>{{ QUOTE_ASSET_SYMBOL }}</small>
            </div>
            <small v-if="estimateCoverage === 'partial'">{{ t('assets.partialEstimateNote') }}</small>
          </div>
          <div
            class="assets-member-summary__return"
            :data-today-return-status="balanceVisible ? todayReturnState : 'hidden'"
            :aria-busy="balanceVisible && todayReturnState === 'loading'"
            aria-live="polite"
          >
            <span>{{ t('rootPrototype.todayReturn') }}</span>
            <strong class="pencil-numeric" :class="todayReturnPresentation.tone">{{ todayReturnPresentation.amount }}</strong>
            <small class="pencil-numeric" :class="todayReturnPresentation.tone">{{ todayReturnPresentation.detail }}</small>
          </div>
        </div>

        <nav class="assets-hero-actions" :aria-label="t('assets.operations')">
          <button type="button" @click="openDeposit"><ArrowDownToLine :size="19" aria-hidden="true" />{{ t('assets.deposit') }}</button>
          <button type="button" @click="openProtectedRoute('withdraw-asset')"><ArrowUpFromLine :size="19" aria-hidden="true" />{{ t('assets.withdraw') }}</button>
          <button type="button" @click="openTransfer"><ArrowLeftRight :size="19" aria-hidden="true" />{{ t('assets.transfer') }}</button>
          <button type="button" @click="openProtectedRoute('wallet-ledger')"><ReceiptText :size="20" aria-hidden="true" />{{ t('assets.quickLedger') }}</button>
        </nav>
      </section>

      <section class="pencil-section assets-holdings" :aria-busy="loading">
        <div class="pencil-section__heading">
          <h2>{{ t('assets.holdings') }}</h2>
          <span v-if="memberState === 'holdings'" class="assets-holdings__count">{{ t('assets.sortedByEstimate') }}</span>
          <span v-else-if="memberState === 'empty'" class="assets-holdings__count">{{ t('assets.holdingCount', { count: 0 }) }}</span>
        </div>

        <div v-if="memberState === 'loading'" class="assets-holdings__state" role="status">
          <span>{{ t('common.loading') }}</span>
        </div>
        <div v-else-if="memberState === 'error'" class="assets-holdings__state assets-holdings__state--error" role="alert">
          <strong>{{ t('common.serviceUnavailable') }}</strong>
          <span>{{ error }}</span>
          <button class="pencil-secondary" type="button" :disabled="loading" @click="loadAccounts">{{ t('common.retry') }}</button>
        </div>
        <div v-else-if="hasHoldings" class="assets-holdings__list">
          <button
            v-for="row in holdingRows"
            :key="row.symbol"
            class="assets-holding-row"
            type="button"
            @click="openProtectedRoute('wallet-ledger')"
          >
            <AssetMark :symbol="row.symbol" :src="row.logoUrl" :size="32" />
            <span class="assets-holding-row__copy">
              <strong>{{ row.symbol }}</strong>
              <small>{{ t('assets.availableFrozenSummary', { available: formatAmount(row.available, 8), frozen: formatAmount(row.frozen, 8) }) }}</small>
            </span>
            <span class="assets-holding-row__amount">
              <strong class="pencil-numeric">{{ formatAmount(row.amount, 8) }}</strong>
              <small v-if="row.estimatedValue !== null">{{ t('assets.estimatedValue', { value: formatFiat(row.estimatedValue) }) }}</small>
              <small v-else>{{ t('assets.estimateUnavailable') }}</small>
            </span>
          </button>
        </div>
        <div v-else class="assets-holdings__state assets-holdings__state--empty">
          <span class="assets-holdings__empty-icon"><WalletCards :size="26" aria-hidden="true" /></span>
          <div class="assets-holdings__empty-copy" role="status">
            <strong>{{ t('assets.emptyHoldings') }}</strong>
            <span>{{ t('assets.emptyHoldingsDescription') }}</span>
          </div>
          <button class="pencil-primary" type="button" @click="openDeposit">
            <ArrowDownToLine :size="17" aria-hidden="true" />{{ t('assets.depositNow') }}
          </button>
        </div>
      </section>

      <section class="pencil-section assets-tools">
        <div class="pencil-section__heading"><h2>{{ t('assets.fundTools') }}</h2></div>
        <div class="pencil-list">
          <button class="pencil-row" type="button" @click="openProtectedRoute('wallet-ledger')">
            <span class="pencil-row__icon"><ReceiptText :size="18" aria-hidden="true" /></span>
            <span class="pencil-row__copy"><strong>{{ t('assets.fundLedger') }}</strong></span>
            <span class="pencil-row__value"><small>{{ t('assets.fundLedgerDescription') }}</small><ChevronRight :size="17" aria-hidden="true" /></span>
          </button>
          <button class="pencil-row" type="button" @click="openProtectedRoute('withdrawal-records')">
            <span class="pencil-row__icon"><ArrowUpFromLine :size="19" aria-hidden="true" /></span>
            <span class="pencil-row__copy"><strong>{{ t('withdrawRecords.title') }}</strong></span>
            <span class="pencil-row__value"><small>{{ t('assets.withdrawalRecordsDescription') }}</small><ChevronRight :size="17" aria-hidden="true" /></span>
          </button>
          <button class="pencil-row" type="button" @click="openProtectedRoute('quick-recharge')">
            <span class="pencil-row__icon"><Zap :size="18" aria-hidden="true" /></span>
            <span class="pencil-row__copy"><strong>{{ t('assets.quickRecharge') }}</strong></span>
            <span class="pencil-row__value"><small>{{ t('assets.quickRechargeDescription') }}</small><ChevronRight :size="17" aria-hidden="true" /></span>
          </button>
        </div>
      </section>
    </div>

    <Teleport to="body">
      <div v-if="session.isAuthenticated && transferOpen" class="confirmation-layer assets-transfer-layer">
        <button class="confirmation-overlay-dismiss" type="button" :aria-label="t('common.close')" :disabled="transferring" tabindex="-1" @click="closeTransfer" />
        <section
          ref="transferDialog"
          class="confirmation-sheet assets-transfer-sheet"
          role="dialog"
          aria-modal="true"
          :aria-busy="transferring"
          :aria-label="t('assets.transfer')"
          tabindex="-1"
          @keydown="handleTransferDialogKeydown"
        >
        <span class="assets-transfer-sheet__grab" aria-hidden="true" />
        <header class="assets-transfer-sheet__header">
          <h2>{{ t('assets.transferTitle') }}</h2>
          <button
            class="assets-transfer-sheet__close"
            type="button"
            :aria-label="t('common.close')"
            :disabled="transferring"
            data-dialog-cancel
            @click="closeTransfer"
          >
            <X :size="16" aria-hidden="true" />
          </button>
        </header>

        <div class="assets-transfer-route">
          <div class="assets-transfer-account">
            <span>{{ t('assets.from') }}</span>
            <strong>{{ transferFrom === 'spot' ? t('assets.fundingAccount') : t('assets.marginAccount') }}</strong>
          </div>
          <button type="button" :aria-label="t('assets.swapTransferDirection')" :disabled="transferring" @click="swapTransferRoute">
            <ArrowLeftRight :size="18" aria-hidden="true" />
          </button>
          <div class="assets-transfer-account">
            <span>{{ t('assets.to') }}</span>
            <strong>{{ transferTarget === 'spot' ? t('assets.fundingAccount') : t('assets.marginAccount') }}</strong>
          </div>
        </div>

        <label class="assets-transfer-field">
          <span class="assets-transfer-field__heading">
            <span>{{ t('assets.asset') }}</span>
            <small>{{ t('assets.availableBalance', { amount: transferAvailableLabel, asset: transferAsset || '--' }) }}</small>
          </span>
          <select v-model="transferAsset">
            <option v-for="account in transferAccounts" :key="account.assetId" :value="account.symbol">{{ account.symbol }}</option>
          </select>
        </label>

        <label class="assets-transfer-field">
          <span class="assets-transfer-field__heading">
            <span>{{ t('assets.transferAmount') }}</span>
            <small>{{ transferAsset || '--' }}</small>
          </span>
          <input v-model="transferAmount" inputmode="decimal" :placeholder="t('assets.transferPlaceholder')" />
        </label>

        <p class="assets-transfer-hint">{{ t('assets.transferHint') }}</p>
        <p v-if="transferFeedback" :class="transferFeedbackTone === 'success' ? 'positive' : 'field-error'" aria-live="polite">{{ transferFeedback }}</p>
        <button class="assets-transfer-submit" type="button" :disabled="!canSubmitTransfer" :aria-busy="transferring" @click="submitTransfer">
          <ArrowLeftRight :size="17" aria-hidden="true" />
          {{ transferring ? t('assets.transferring') : t('assets.confirmTransfer') }}
        </button>
        </section>
      </div>
    </Teleport>
  </main>
</template>

<style scoped>
.assets-pencil {
  overflow-x: clip;
}

.assets-pencil__guest-content,
.assets-pencil__member-content {
  min-width: 0;
  padding-inline: 16px;
  padding-top: 8px;
}

.assets-pencil__guest-content {
  min-height: calc(100dvh - 144px);
}

.assets-pencil__member-content {
  display: grid;
  gap: 0;
  padding-bottom: calc(20px + env(safe-area-inset-bottom));
}

.assets-hero {
  border: 1px solid var(--line);
  border-radius: 24px;
  box-sizing: border-box;
  height: 236px;
  min-height: 236px;
  overflow: hidden;
  padding: 18px 20px 16px;
}

.assets-hero__image,
.assets-hero__overlay,
.assets-hero__bloom {
  inset: 0;
  pointer-events: none;
  position: absolute;
}

.assets-hero__image {
  height: 100%;
  object-fit: cover;
  width: 100%;
}

.assets-hero__overlay {
  background: transparent;
}

.assets-hero__overlay--dark {
  background: color-mix(in srgb, var(--page) 25%, transparent);
}

.assets-hero__bloom {
  background: radial-gradient(circle at 88% 0%, rgb(67 239 169 / 18%), transparent 58%);
}

.assets-hero--guest {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

.assets-guest-copy,
.assets-guest-login,
.assets-member-summary,
.assets-hero-actions {
  position: relative;
  z-index: 1;
}

.assets-guest-copy {
  display: grid;
  gap: 8px;
}

.assets-hero__eyebrow {
  color: var(--muted-strong);
  font-family: var(--font-geist-mono), var(--data-font);
  font-size: 11px;
  font-weight: 600;
  line-height: 16px;
}

.assets-guest-copy h1 {
  color: var(--ink);
  font-size: 26px;
  font-weight: 750;
  line-height: 34px;
  margin: 0;
}

.assets-guest-copy p {
  color: var(--muted-strong);
  font-size: 12px;
  line-height: 18px;
  margin: 0;
}

.assets-guest-login {
  align-items: center;
  backdrop-filter: blur(18px);
  background: color-mix(in srgb, var(--surface-elevated) 72%, transparent);
  border: 1px solid color-mix(in srgb, var(--surface-elevated) 86%, var(--line));
  border-radius: 12px;
  color: var(--ink);
  display: flex;
  font-size: 14px;
  font-weight: 650;
  gap: 8px;
  height: 50px;
  justify-content: center;
  min-height: 50px;
  width: 100%;
}

.assets-hero--member {
  display: grid;
  gap: 14px;
  align-content: center;
  grid-template-rows: auto 66px;
}

.assets-member-summary {
  align-items: end;
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) minmax(84px, auto);
  min-width: 0;
}

.assets-member-summary__total,
.assets-member-summary__return {
  display: grid;
  gap: 6px;
  min-width: 0;
}

.assets-member-summary__label > span,
.assets-member-summary__return > span {
  color: var(--muted-strong);
  font-size: 12px;
  font-weight: 500;
  line-height: 16px;
}

.assets-member-summary__return > span {
  font-size: 11px;
}

.assets-member-summary__label {
  align-items: center;
  display: flex;
  gap: 6px;
  min-height: 16px;
}

.assets-balance-toggle {
  background: transparent;
  border: 0;
  color: var(--muted-strong);
  display: grid;
  flex: 0 0 44px;
  height: 44px;
  margin: -14px;
  min-height: 44px;
  padding: 0;
  place-items: center;
  width: 44px;
}

.assets-member-summary__value {
  align-items: end;
  display: flex;
  gap: 6px;
  min-width: 0;
}

.assets-member-summary__value strong {
  color: var(--ink);
  font-size: 34px;
  font-weight: 700;
  line-height: 38px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.assets-member-summary__value small,
.assets-member-summary__total > small,
.assets-member-summary__return small {
  color: var(--muted-strong);
  font-size: 10px;
  line-height: 14px;
}

.assets-member-summary__return {
  justify-items: end;
  text-align: right;
}

.assets-member-summary__return strong {
  color: var(--muted-strong);
  font-size: 18px;
  font-weight: 650;
  line-height: 23px;
}

.assets-member-summary__return strong,
.assets-member-summary__return small {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.assets-hero-actions {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  min-width: 0;
}

.assets-hero-actions button {
  align-items: center;
  backdrop-filter: blur(18px);
  background: color-mix(in srgb, var(--surface-elevated) 70%, transparent);
  border: 1px solid color-mix(in srgb, var(--surface-elevated) 82%, var(--line));
  border-radius: 12px;
  color: var(--ink);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  font-weight: 500;
  gap: 6px;
  height: 66px;
  justify-content: center;
  min-height: 66px;
  min-width: 0;
  padding: 0;
}

.assets-holdings {
  padding: 14px 4px 4px;
}

.assets-holdings__count {
  color: var(--muted);
  font-size: 11px;
  line-height: 16px;
}

.assets-holdings__list {
  display: grid;
  margin-top: 8px;
}

.assets-holding-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 1px solid var(--hairline);
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-columns: 32px minmax(0, 1fr) minmax(84px, auto);
  min-height: 52px;
  min-width: 0;
  padding: 7px 0;
  text-align: left;
  width: 100%;
}

.assets-holding-row:last-child {
  border-bottom: 0;
}

.assets-holding-row__copy,
.assets-holding-row__amount {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.assets-holding-row__copy strong,
.assets-holding-row__amount strong {
  font-size: 13px;
  font-weight: 650;
  line-height: 18px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.assets-holding-row__copy small,
.assets-holding-row__amount small {
  color: var(--muted);
  font-size: 10px;
  line-height: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.assets-holding-row__amount {
  justify-items: end;
  max-width: 150px;
  text-align: right;
}

.assets-holdings__state {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 10px;
  justify-content: center;
  min-height: 156px;
  padding: 20px 0 8px;
  text-align: center;
}

.assets-holdings__empty-copy > strong,
.assets-holdings__state > strong {
  color: var(--ink);
  font-size: 15px;
  line-height: 21px;
}

.assets-holdings__empty-copy {
  display: grid;
  gap: 10px;
  justify-items: center;
}

.assets-holdings__empty-copy > span,
.assets-holdings__state > span:not(.assets-holdings__empty-icon) {
  line-height: 16px;
  max-width: 286px;
}

.assets-holdings__state .pencil-primary,
.assets-holdings__state .pencil-secondary {
  align-items: center;
  display: flex;
  gap: 8px;
  justify-content: center;
  min-height: 48px;
  width: min(100%, 318px);
}

.assets-holdings__state--error {
  min-height: 148px;
}

.assets-holdings__state--error .pencil-secondary {
  min-height: 44px;
}

.assets-holdings__empty-icon {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--muted);
  display: flex;
  height: 56px;
  justify-content: center;
  width: 56px;
}

.assets-tools {
  display: grid;
  gap: 6px;
  grid-template-rows: 23px 156px;
  padding: 12px 4px 0;
}

.assets-tools .pencil-list {
  grid-template-rows: repeat(3, 52px);
}

.assets-tools .pencil-row {
  height: 52px;
  min-height: 52px;
}

.assets-tools .pencil-row__value {
  max-width: min(48vw, 176px);
}

.assets-tools .pencil-row__value small {
  min-width: 0;
}

.assets-pencil button:focus-visible,
.assets-transfer-layer button:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.assets-transfer-layer {
  inset: 0;
  justify-items: center;
  padding: 0;
  position: fixed;
  width: 100%;
}

.assets-transfer-sheet {
  background: var(--surface);
  border: 1px solid var(--line);
  border-bottom: 0;
  border-radius: 20px 20px 0 0;
  box-shadow: 0 -18px 48px var(--shadow);
  box-sizing: border-box;
  color: var(--ink);
  gap: 12px;
  height: min(460px, calc(100dvh - max(16px, env(safe-area-inset-top))));
  max-height: none;
  max-width: 448px;
  padding: 8px 16px calc(14px + env(safe-area-inset-bottom));
  width: 100%;
}

.assets-transfer-sheet__grab {
  background: var(--line-strong);
  border-radius: 2px;
  display: block;
  height: 4px;
  justify-self: center;
  width: 40px;
}

.assets-transfer-sheet > .assets-transfer-sheet__header {
  align-items: center;
  display: flex;
  gap: 12px;
  grid-template-columns: none;
  justify-content: space-between;
  min-height: 32px;
}

.assets-transfer-sheet__header h2 {
  font-size: 18px;
  font-weight: 700;
  margin: 0;
}

.assets-transfer-sheet__close,
.assets-transfer-route > button {
  align-items: center;
  background: var(--surface-2);
  border: 0;
  border-radius: 50%;
  color: var(--muted);
  display: flex;
  flex: 0 0 44px;
  height: 44px;
  justify-content: center;
  padding: 0;
  width: 44px;
}

.assets-transfer-route {
  align-items: center;
  display: grid;
  gap: 6px;
  grid-template-columns: minmax(0, 1fr) 44px minmax(0, 1fr);
  min-width: 0;
}

.assets-transfer-route > button {
  background: transparent;
  color: var(--positive);
}

.assets-transfer-account {
  background: var(--surface-2);
  border: 1px solid var(--line);
  border-radius: 4px;
  display: grid;
  gap: 4px;
  min-width: 0;
  padding: 10px 12px;
}

.assets-transfer-account span,
.assets-transfer-field__heading,
.assets-transfer-hint {
  color: var(--muted);
  font-size: 10px;
  line-height: 14px;
}

.assets-transfer-account strong {
  font-size: 14px;
  font-weight: 650;
  line-height: 20px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.assets-transfer-field {
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 4px;
  display: grid;
  gap: 0;
  min-width: 0;
  padding: 2px 12px;
}

.assets-transfer-field:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 2px var(--focus-ring);
}

.assets-transfer-field__heading {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.assets-transfer-field__heading small {
  color: inherit;
  font-size: inherit;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.assets-transfer-field input,
.assets-transfer-field select {
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 16px;
  font-weight: 600;
  height: 44px;
  min-height: 44px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.assets-transfer-sheet > .assets-transfer-hint,
.assets-transfer-sheet > .positive,
.assets-transfer-sheet > .field-error {
  border: 0;
  font-size: 10px;
  line-height: 14px;
  margin: -4px 0;
  padding: 0;
}

.assets-transfer-submit {
  align-items: center;
  background: var(--accent);
  border: 0;
  border-radius: 4px;
  color: var(--on-accent);
  display: flex;
  font-size: 13px;
  font-weight: 650;
  gap: 8px;
  height: 50px;
  justify-content: center;
  margin-top: auto;
  min-height: 50px;
  width: 100%;
}

.assets-transfer-submit:disabled {
  background: var(--surface-3);
  color: var(--muted);
}

@media (max-width: 340px) {
  .assets-pencil__guest-content,
  .assets-pencil__member-content {
    padding-inline: 16px;
  }

  .assets-hero {
    padding-inline: 16px;
  }

  .assets-member-summary {
    gap: 8px;
    grid-template-columns: minmax(0, 1fr) 78px;
  }

  .assets-member-summary__value strong {
    font-size: 27px;
  }

  .assets-hero-actions {
    gap: 6px;
  }

  .assets-holding-row {
    grid-template-columns: 30px minmax(0, 1fr) minmax(74px, auto);
  }

  .assets-holding-row :deep(.asset-mark) {
    height: 30px !important;
    width: 30px !important;
  }

  .assets-holding-row__amount {
    max-width: 112px;
  }
}
</style>
