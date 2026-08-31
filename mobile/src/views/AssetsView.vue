<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowDownToLine,
  ArrowLeftRight,
  ArrowRight,
  ArrowUpFromLine,
  Check,
  ChevronRight,
  Eye,
  EyeOff,
  ReceiptText,
  Search,
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
import { decimalCompare, decimalTextFromBoundary, tryNormalizeDecimalText } from '@/core/decimal'
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

type AssetAccountScope = 'all' | 'spot' | 'margin'
type AssetValueSizeTier = 'full' | 'medium' | 'small' | 'minimum'

const QUOTE_ASSET_SYMBOL = 'USDT'

const router = useRouter()
const marketStore = useMarketStore()
const session = useSessionStore()
const theme = useThemeStore()
const { locale, t } = useI18n()
const accounts = ref<WalletAccount[]>([])
const marginAccounts = ref<WalletAccount[]>([])
const assetAccountScope = ref<AssetAccountScope>('all')
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
const transferAssetPickerOpen = ref(false)
const transferAssetSearch = ref('')
let transferRequestVersion = 0
const transferDialog = ref<HTMLElement | null>(null)
const transferAssetTrigger = ref<HTMLButtonElement | null>(null)
const transferAssetSearchInput = ref<HTMLInputElement | null>(null)
const { trapFocus: trapTransferFocus } = useModalDialog(transferOpen, transferDialog)

const assetRows = computed(() => {
  const rows = new Map<string, { symbol: string; spot?: WalletAccount; margin?: WalletAccount }>()
  for (const account of accounts.value) rows.set(account.symbol, { ...rows.get(account.symbol), symbol: account.symbol, spot: account })
  for (const account of marginAccounts.value) rows.set(account.symbol, { ...rows.get(account.symbol), symbol: account.symbol, margin: account })
  return [...rows.values()]
})

const accountDataAvailable = computed(() => session.isAuthenticated && accountsReady.value && !error.value)
const allHoldingRows = computed<AssetHoldingRow[]>(() => buildHoldingRows('all'))
const spotHoldingRows = computed<AssetHoldingRow[]>(() => buildHoldingRows('spot'))
const marginHoldingRows = computed<AssetHoldingRow[]>(() => buildHoldingRows('margin'))
const holdingRows = computed<AssetHoldingRow[]>(() => {
  if (assetAccountScope.value === 'spot') return spotHoldingRows.value
  if (assetAccountScope.value === 'margin') return marginHoldingRows.value
  return allHoldingRows.value
})
const hasHoldings = computed(() => accountDataAvailable.value && allHoldingRows.value.length > 0)
const selectedHasHoldings = computed(() => accountDataAvailable.value && holdingRows.value.length > 0)
const valuedHoldingCount = computed(() => allHoldingRows.value.filter((row) => row.estimatedValue !== null).length)
const totalEstimate = computed(() => holdingEstimate(allHoldingRows.value))
const accountCards = computed(() => [
  {
    scope: 'spot' as const,
    label: t('assets.spotAccount'),
    balance: accountEstimateLabel(spotHoldingRows.value),
    count: spotHoldingRows.value.length,
  },
  {
    scope: 'margin' as const,
    label: t('assets.marginAccount'),
    balance: accountEstimateLabel(marginHoldingRows.value),
    count: marginHoldingRows.value.length,
  },
])
const selectedHoldingsTitle = computed(() => {
  if (assetAccountScope.value === 'spot') return t('assets.spotHoldings')
  if (assetAccountScope.value === 'margin') return t('assets.marginHoldings')
  return t('assets.holdings')
})
const memberState = computed<'loading' | 'error' | 'empty' | 'holdings'>(() => {
  if (error.value) return 'error'
  if (loading.value || !accountsReady.value) return 'loading'
  return hasHoldings.value ? 'holdings' : 'empty'
})
const estimateCoverage = computed<'full' | 'partial' | 'unavailable' | 'empty'>(() => {
  if (!hasHoldings.value) return 'empty'
  if (valuedHoldingCount.value === 0) return 'unavailable'
  return valuedHoldingCount.value === allHoldingRows.value.length ? 'full' : 'partial'
})
const totalEstimateLabel = computed(() => {
  if (!balanceVisible.value) return '••••••'
  if (!accountDataAvailable.value || estimateCoverage.value === 'unavailable') return '--'
  return new Intl.NumberFormat(locale.value === 'en' ? 'en-US' : 'zh-CN', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(totalEstimate.value)
})
const assetsHeroImage = computed(() => theme.isDark ? assetsHeroDark : assetsHeroLight)
const todayReturnPresentation = computed(() => resolveTodayReturnPresentation({
  visible: balanceVisible.value,
  state: todayReturnState.value,
  value: todayReturn.value,
  locale: locale.value === 'en' ? 'en-US' : 'zh-CN',
  amountMask: '••••••',
  detailMask: '••••',
  messages: {
    loading: t('home.todayReturnLoading'),
    partial: (assets) => t('home.todayReturnPartial', { assets }),
    partialUnknown: t('home.todayReturnPartialUnknown'),
    error: t('home.todayReturnUnavailable'),
  },
}))
const totalValueSizeTier = computed(() => assetValueSizeTier(totalEstimateLabel.value))
const returnValueSizeTier = computed(() => assetValueSizeTier(
  `${todayReturnPresentation.value.amount} ${todayReturnPresentation.value.detail}`,
))
const summaryValueSizeTier = computed<AssetValueSizeTier>(() => (
  totalValueSizeTier.value === 'minimum' || returnValueSizeTier.value === 'minimum'
    ? 'minimum'
    : totalValueSizeTier.value
))

function assetValueSizeTier(value: string): AssetValueSizeTier {
  const length = Array.from(value).length
  if (length <= 8) return 'full'
  if (length <= 11) return 'medium'
  if (length <= 15) return 'small'
  return 'minimum'
}

const marginTransferAssetIds = computed(() => new Set(
  marginAccounts.value
    .filter((account) => account.marginTransferEnabled !== false)
    .map((account) => account.assetId),
))
const spotTransferAccounts = computed(() => accounts.value.filter((account) => marginTransferAssetIds.value.has(account.assetId)))
const transferAccounts = computed(() => transferFrom.value === 'spot' ? spotTransferAccounts.value : marginAccounts.value)
const transferAccount = computed(() => transferAccounts.value.find((account) => account.symbol === transferAsset.value))
const transferAvailable = computed<number | null>(() => transferAccount.value?.available ?? null)
const transferAvailableText = computed(() => decimalTextFromBoundary(transferAccount.value?.availableText ?? transferAvailable.value, { allowNegative: false }))
const transferAvailableLabel = computed(() => transferAvailable.value === null ? '--' : formatAmount(transferAvailable.value))
const transferAssetLogo = computed(() => transferAccount.value?.logoUrl
  || accounts.value.find((account) => account.symbol === transferAsset.value)?.logoUrl
  || marginAccounts.value.find((account) => account.symbol === transferAsset.value)?.logoUrl)
const filteredTransferAccounts = computed(() => {
  const query = transferAssetSearch.value.trim().toUpperCase()
  return transferAccounts.value
    .filter((account) => !query || account.symbol.toUpperCase().includes(query))
    .sort((left, right) => {
      if (left.symbol === QUOTE_ASSET_SYMBOL) return -1
      if (right.symbol === QUOTE_ASSET_SYMBOL) return 1
      return left.symbol.localeCompare(right.symbol)
    })
})
const transferTarget = computed<'spot' | 'margin'>(() => transferFrom.value === 'spot' ? 'margin' : 'spot')
const transferValueText = computed(() => tryNormalizeDecimalText(transferAmount.value, { allowNegative: false, allowZero: false, maxIntegerDigits: 20, maxScale: transferAccount.value?.precisionScale ?? 18 }))
const canSubmitTransfer = computed(() => Boolean(transferAsset.value
  && transferValueText.value && transferAvailableText.value
  && decimalCompare(transferValueText.value, transferAvailableText.value) <= 0
  && !transferring.value))

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
    if (!transferAccounts.value.some((account) => account.symbol === transferAsset.value)) {
      transferAsset.value = preferredTransferAsset(transferAccounts.value)
    }
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
  assetAccountScope.value = 'all'
  accountsReady.value = false
  transferOpen.value = false
  transferAsset.value = ''
  transferAmount.value = ''
  transferFeedback.value = ''
  transferAssetPickerOpen.value = false
  transferAssetSearch.value = ''
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
  if (!transferAccounts.value.some((account) => account.symbol === transferAsset.value)) {
    transferAsset.value = preferredTransferAsset(transferAccounts.value)
  }
  transferAmount.value = ''
  transferFeedback.value = ''
  transferAssetPickerOpen.value = false
  transferAssetSearch.value = ''
  transferOpen.value = true
}

function closeTransfer(): void {
  if (transferring.value) return
  transferAssetPickerOpen.value = false
  transferAssetSearch.value = ''
  transferOpen.value = false
}

function openTransferAssetPicker(): void {
  if (transferring.value || transferAccounts.value.length === 0) return
  transferAssetSearch.value = ''
  transferAssetPickerOpen.value = true
  void nextTick(() => transferAssetSearchInput.value?.focus())
}

function closeTransferAssetPicker(): void {
  transferAssetPickerOpen.value = false
  transferAssetSearch.value = ''
  void nextTick(() => transferAssetTrigger.value?.focus())
}

function selectTransferAsset(account: WalletAccount): void {
  if (transferring.value) return
  transferAsset.value = account.symbol
  transferAmount.value = ''
  transferFeedback.value = ''
  closeTransferAssetPicker()
}

function fillTransferAvailable(): void {
  if (transferring.value || !transferAvailableText.value || transferAvailableText.value === '0') return
  transferAmount.value = transferAvailableText.value
  transferFeedback.value = ''
}

function swapTransferRoute(): void {
  if (transferring.value) return
  const nextFrom = transferTarget.value
  transferFrom.value = nextFrom
  const nextAccounts = transferAccounts.value
  if (!nextAccounts.some((account) => account.symbol === transferAsset.value)) {
    transferAsset.value = preferredTransferAsset(nextAccounts)
  }
  transferAmount.value = ''
  transferFeedback.value = ''
  transferAssetPickerOpen.value = false
  transferAssetSearch.value = ''
}

function handleTransferDialogKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && transferAssetPickerOpen.value) {
    event.preventDefault()
    closeTransferAssetPicker()
    return
  }
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
  const requestAmount = transferValueText.value
  if (!transferAsset.value || !transferAvailableText.value || !requestAmount) {
    transferFeedback.value = t('assets.invalidTransfer')
    transferFeedbackTone.value = 'error'
    return
  }
  if (decimalCompare(requestAmount, transferAvailableText.value) > 0) {
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
    const result = await transferWalletFunds(transferAsset.value, transferFrom.value, to, requestAmount)
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

function selectAssetAccountScope(scope: AssetAccountScope): void {
  assetAccountScope.value = scope
}

function buildHoldingRows(scope: AssetAccountScope): AssetHoldingRow[] {
  return assetRows.value
    .map((row) => {
      const spot = scope === 'margin' ? undefined : row.spot
      const margin = scope === 'spot' ? undefined : row.margin
      const amount = walletTotal(spot) + walletTotal(margin)
      const available = walletAvailable(spot) + walletAvailable(margin)
      const frozen = walletFrozen(spot) + walletFrozen(margin)
      return {
        ...row,
        spot,
        margin,
        logoUrl: spot?.logoUrl || margin?.logoUrl,
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
    })
}

function holdingEstimate(rows: AssetHoldingRow[]): number {
  return rows.reduce((total, row) => total + (row.estimatedValue ?? 0), 0)
}

function accountEstimateLabel(rows: AssetHoldingRow[]): string {
  if (!balanceVisible.value) return '••••••'
  if (!accountDataAvailable.value) return '--'
  if (rows.length === 0) return '0.00'
  if (!rows.some((row) => row.estimatedValue !== null)) return '--'
  return new Intl.NumberFormat(locale.value === 'en' ? 'en-US' : 'zh-CN', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(holdingEstimate(rows))
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

function preferredTransferAsset(wallets: WalletAccount[]): string {
  return wallets.find((account) => account.symbol === QUOTE_ASSET_SYMBOL)?.symbol
    || wallets[0]?.symbol
    || ''
}

function syncTransferAsset(): void {
  if (!transferAccounts.value.some((account) => account.symbol === transferAsset.value)) {
    transferAsset.value = preferredTransferAsset(transferAccounts.value)
  }
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
    data-pencil-source="CUK3y i6YDBr p61z2Q Q4JYj v6phV TuWXq tPkL1 tPkD1"
  >
    <PageHeader :back="false" :pencil="true" :title="t('assets.title')" />

    <div v-if="!session.isAuthenticated" class="pencil-content assets-pencil__guest-content" data-assets-state="guest">
      <section class="pencil-hero assets-hero assets-hero--guest">
        <img class="assets-hero__image" :src="assetsHeroImage" alt="">
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
        <img class="assets-hero__image" :src="assetsHeroImage" alt="">
        <span class="assets-hero__overlay" :class="{ 'assets-hero__overlay--dark': theme.isDark }" aria-hidden="true" />
        <span class="assets-hero__bloom" aria-hidden="true" />

        <div class="assets-member-summary" :data-value-size="summaryValueSizeTier">
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
            <div class="assets-member-summary__value" :data-value-size="totalValueSizeTier">
              <strong class="pencil-numeric">{{ totalEstimateLabel }}</strong>
              <small>{{ QUOTE_ASSET_SYMBOL }}</small>
            </div>
            <small v-if="estimateCoverage === 'partial'">{{ t('assets.partialEstimateNote') }}</small>
          </div>
          <div
            class="assets-member-summary__return"
            :data-value-size="returnValueSizeTier"
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

      <section class="pencil-section assets-account-overview" :aria-busy="loading">
        <div class="pencil-section__heading">
          <h2>{{ t('assets.accountBalances') }}</h2>
          <button
            class="assets-account-overview__all"
            :class="{ 'is-active': assetAccountScope === 'all' }"
            type="button"
            :aria-pressed="assetAccountScope === 'all'"
            :disabled="loading || Boolean(error)"
            @click="selectAssetAccountScope('all')"
          >
            {{ t('assets.allAccounts') }}
          </button>
        </div>
        <nav class="assets-account-cards" :aria-label="t('assets.accountBalances')">
          <button
            v-for="card in accountCards"
            :key="card.scope"
            class="assets-account-card"
            :class="{ 'is-active': assetAccountScope === card.scope }"
            type="button"
            :aria-pressed="assetAccountScope === card.scope"
            :disabled="loading || Boolean(error)"
            @click="selectAssetAccountScope(card.scope)"
          >
            <span class="assets-account-card__top">
              <span class="assets-account-card__icon"><WalletCards :size="16" aria-hidden="true" /></span>
              <strong>{{ card.label }}</strong>
              <ChevronRight :size="15" aria-hidden="true" />
            </span>
            <span class="assets-account-card__balance">
              <strong class="pencil-numeric">{{ card.balance }}</strong>
              <small>{{ QUOTE_ASSET_SYMBOL }}</small>
            </span>
            <small>{{ t('assets.accountAssetCount', { count: card.count }) }}</small>
          </button>
        </nav>
      </section>

      <section class="pencil-section assets-holdings" :aria-busy="loading">
        <div class="pencil-section__heading">
          <h2>{{ selectedHoldingsTitle }}</h2>
          <span v-if="memberState === 'holdings'" class="assets-holdings__count">{{ t('assets.holdingCount', { count: holdingRows.length }) }}</span>
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
        <div v-else-if="selectedHasHoldings" class="assets-holdings__list">
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
            <strong>{{ assetAccountScope === 'margin' ? t('assets.emptyMarginHoldings') : t('assets.emptyHoldings') }}</strong>
            <span>{{ assetAccountScope === 'margin' ? t('assets.emptyMarginHoldingsDescription') : t('assets.emptyHoldingsDescription') }}</span>
          </div>
          <button class="pencil-primary" type="button" @click="assetAccountScope === 'margin' ? openTransfer() : openDeposit()">
            <ArrowLeftRight v-if="assetAccountScope === 'margin'" :size="17" aria-hidden="true" />
            <ArrowDownToLine v-else :size="17" aria-hidden="true" />
            {{ assetAccountScope === 'margin' ? t('assets.transferToMargin') : t('assets.depositNow') }}
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
          :class="{ 'assets-transfer-sheet--picker': transferAssetPickerOpen }"
          role="dialog"
          aria-modal="true"
          :aria-busy="transferring"
          :aria-label="transferAssetPickerOpen ? t('assets.selectTransferAsset') : t('assets.transfer')"
          tabindex="-1"
          @keydown="handleTransferDialogKeydown"
        >
          <div class="assets-transfer-sheet__top">
            <span class="assets-transfer-sheet__grab" aria-hidden="true" />
            <header class="assets-transfer-sheet__header">
              <h2>{{ transferAssetPickerOpen ? t('assets.selectTransferAsset') : t('assets.transferTitle') }}</h2>
              <button
                class="assets-transfer-sheet__close"
                type="button"
                :aria-label="t('common.close')"
                :disabled="transferring"
                data-dialog-cancel
                @click="transferAssetPickerOpen ? closeTransferAssetPicker() : closeTransfer()"
              >
                <X :size="16" aria-hidden="true" />
              </button>
            </header>
          </div>

          <div v-if="transferAssetPickerOpen" class="assets-transfer-picker" data-transfer-surface="asset-picker">
            <label class="assets-transfer-search">
              <Search :size="17" aria-hidden="true" />
              <input
                ref="transferAssetSearchInput"
                v-model="transferAssetSearch"
                type="search"
                autocomplete="off"
                :placeholder="t('assets.searchTransferAsset')"
                :aria-label="t('assets.searchTransferAsset')"
              />
            </label>

            <div v-if="filteredTransferAccounts.length" class="assets-transfer-picker__list" role="list">
              <button
                v-for="account in filteredTransferAccounts"
                :key="account.assetId"
                class="assets-transfer-picker__row"
                :class="{ 'is-selected': account.symbol === transferAsset }"
                type="button"
                :aria-pressed="account.symbol === transferAsset"
                :disabled="transferring"
                @click="selectTransferAsset(account)"
              >
                <AssetMark :symbol="account.symbol" :src="account.logoUrl" :size="32" />
                <span class="assets-transfer-picker__copy">
                  <strong>{{ account.symbol }}</strong>
                  <small>{{ t('assets.transferAssetSource', { account: transferFrom === 'spot' ? t('assets.spotAccount') : t('assets.marginAccount') }) }}</small>
                </span>
                <span class="assets-transfer-picker__value">
                  <strong class="pencil-numeric">{{ formatAmount(account.available) }}</strong>
                  <small>{{ t('assets.transferAvailable') }}</small>
                </span>
                <Check v-if="account.symbol === transferAsset" :size="17" aria-hidden="true" />
              </button>
            </div>
            <p v-else class="assets-transfer-picker__empty" role="status">{{ t('assets.noTransferAssets') }}</p>
          </div>

          <p v-if="transferAssetPickerOpen" class="assets-transfer-picker__hint">{{ t('assets.transferPickerHint') }}</p>

          <div v-else class="assets-transfer-sheet__body" data-transfer-surface="main">
            <label class="assets-transfer-amount">
              <span class="assets-transfer-amount__bloom" aria-hidden="true" />
              <span class="assets-transfer-amount__label">{{ t('assets.transferQuantityWithAsset', { asset: transferAsset || '--' }) }}</span>
              <input
                v-model="transferAmount"
                inputmode="decimal"
                autocomplete="off"
                placeholder="0.00"
                :aria-label="t('assets.transferAmount')"
                @input="transferFeedback = ''"
              />
              <span class="assets-transfer-amount__meta">
                <span>{{ t('assets.transferAvailableAmount', { amount: transferAvailableLabel }) }}</span>
                <button
                  type="button"
                  :disabled="transferring || transferAvailable === null || transferAvailable <= 0"
                  @click.prevent="fillTransferAvailable"
                >
                  <span>{{ t('common.all') }}</span>
                </button>
              </span>
            </label>

            <div class="assets-transfer-route">
              <div class="assets-transfer-account">
                <span>{{ t('assets.from') }}</span>
                <strong>{{ transferFrom === 'spot' ? t('assets.spotAccount') : t('assets.marginAccount') }}</strong>
              </div>
              <button type="button" :aria-label="t('assets.swapTransferDirection')" :disabled="transferring" @click="swapTransferRoute">
                <ArrowLeftRight :size="16" aria-hidden="true" />
              </button>
              <div class="assets-transfer-account assets-transfer-account--target">
                <span>{{ t('assets.to') }}</span>
                <strong>{{ transferTarget === 'spot' ? t('assets.spotAccount') : t('assets.marginAccount') }}</strong>
              </div>
            </div>

            <button
              ref="transferAssetTrigger"
              class="assets-transfer-asset"
              type="button"
              :disabled="transferring || transferAccounts.length === 0"
              @click="openTransferAssetPicker"
            >
              <AssetMark :symbol="transferAsset || '--'" :src="transferAssetLogo" :size="32" />
              <span class="assets-transfer-asset__copy">
                <strong>{{ transferAsset || '--' }}</strong>
                <small>{{ t('assets.selectTransferAsset') }}</small>
              </span>
              <span class="assets-transfer-asset__value">
                <strong class="pencil-numeric">{{ transferAvailableLabel }}</strong>
                <small>{{ t('assets.transferAvailable') }}</small>
              </span>
              <ChevronRight :size="17" aria-hidden="true" />
            </button>

            <p class="assets-transfer-hint">{{ t('assets.transferHint') }}</p>
            <p v-if="transferFeedback" :class="transferFeedbackTone === 'success' ? 'positive' : 'field-error'" aria-live="polite">{{ transferFeedback }}</p>
          </div>

          <button
            v-if="!transferAssetPickerOpen"
            class="assets-transfer-submit"
            type="button"
            :disabled="!canSubmitTransfer"
            :aria-busy="transferring"
            @click="submitTransfer"
          >
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
  flex-wrap: wrap;
  gap: 6px;
  min-width: 0;
}

.assets-member-summary__value strong {
  color: var(--ink);
  font-size: clamp(30px, 8vw, 34px);
  font-weight: 700;
  line-height: 38px;
  min-width: 0;
  max-width: 100%;
  font-variant-numeric: tabular-nums;
  overflow-wrap: anywhere;
}

.assets-member-summary__value[data-value-size='medium'] strong { font-size: 30px; line-height: 34px; }
.assets-member-summary__value[data-value-size='small'] strong { font-size: 25px; line-height: 30px; }
.assets-member-summary__value[data-value-size='minimum'] strong { font-size: 20px; line-height: 24px; overflow-wrap: anywhere; }

.assets-member-summary[data-value-size='minimum'] {
  align-items: start;
  grid-template-columns: minmax(0, 1fr);
  gap: 4px;
}

.assets-member-summary[data-value-size='minimum'] .assets-member-summary__return {
  grid-template-columns: auto minmax(0, 1fr) auto;
  justify-items: start;
  text-align: left;
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
  overflow-wrap: anywhere;
}

.assets-member-summary__return[data-value-size='small'] strong,
.assets-member-summary__return[data-value-size='minimum'] strong {
  font-size: 15px;
  line-height: 20px;
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

.assets-account-overview {
  display: grid;
  gap: 10px;
  padding: 16px 4px 2px;
}

.assets-account-overview__all {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted);
  display: inline-flex;
  font-size: 11px;
  font-weight: 600;
  justify-content: center;
  margin: -12px;
  min-height: 44px;
  min-width: 56px;
  padding: 12px;
}

.assets-account-overview__all.is-active {
  color: var(--positive);
}

.assets-account-cards {
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(0, 1fr);
  min-width: 0;
}

.assets-account-card {
  backdrop-filter: blur(18px);
  background:
    radial-gradient(circle at 90% 0%, color-mix(in srgb, var(--accent) 11%, transparent), transparent 52%),
    color-mix(in srgb, var(--surface-elevated) 76%, transparent);
  border: 1px solid color-mix(in srgb, var(--line) 88%, transparent);
  border-radius: 16px;
  color: var(--ink);
  display: grid;
  gap: 4px 12px;
  grid-template-areas:
    "top balance"
    "meta balance";
  grid-template-columns: minmax(0, 1fr) minmax(116px, auto);
  min-height: 82px;
  min-width: 0;
  padding: 13px 14px;
  text-align: left;
  transition: background-color 160ms ease, border-color 160ms ease, transform 160ms ease;
}

.assets-account-card.is-active {
  background:
    radial-gradient(circle at 90% 0%, color-mix(in srgb, var(--accent) 19%, transparent), transparent 58%),
    color-mix(in srgb, var(--surface-elevated) 86%, transparent);
  border-color: color-mix(in srgb, var(--accent) 58%, var(--line));
}

.assets-account-card:not(:disabled):active {
  transform: translateY(1px);
}

.assets-account-card__top {
  align-items: center;
  display: grid;
  gap: 7px;
  grid-area: top;
  grid-template-columns: 28px minmax(0, 1fr) 15px;
  min-width: 0;
}

.assets-account-card__top > strong {
  font-size: 12px;
  font-weight: 650;
  line-height: 17px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.assets-account-card__top > svg {
  color: var(--muted);
}

.assets-account-card__icon {
  align-items: center;
  background: color-mix(in srgb, var(--accent) 12%, var(--surface-elevated));
  border: 1px solid color-mix(in srgb, var(--accent) 24%, var(--line));
  border-radius: 9px;
  color: var(--positive);
  display: flex;
  height: 28px;
  justify-content: center;
  width: 28px;
}

.assets-account-card__balance {
  align-items: baseline;
  display: flex;
  gap: 5px;
  grid-area: balance;
  justify-self: end;
  min-width: 0;
}

.assets-account-card__balance strong {
  font-size: 18px;
  font-weight: 700;
  line-height: 22px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.assets-account-card__balance small,
.assets-account-card > small {
  color: var(--muted);
  font-size: 9px;
  line-height: 13px;
}

.assets-account-card > small {
  grid-area: meta;
  padding-left: 35px;
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

.assets-pencil button:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.assets-transfer-layer button:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.assets-transfer-layer {
  bottom: 0;
  justify-items: center;
  left: auto;
  padding: 0;
  position: fixed;
  right: 5.5vw;
  top: 0;
  width: min(100%, 448px);
}

.assets-transfer-sheet {
  --surface: rgb(247 249 248);
  --surface-elevated: rgb(255 255 255);
  --surface-2: rgb(238 242 240);
  --surface-3: rgb(228 234 231);
  --line: rgb(204 213 208);
  --line-strong: rgb(174 187 180);
  --hairline: rgb(221 228 224);
  --ink: rgb(17 23 20);
  --muted: rgb(104 115 109);
  --accent: rgb(67 239 169);
  --positive: rgb(8 123 82);
  --on-accent: rgb(7 17 13);
  --shadow: rgb(18 32 24 / 14%);
  --font-data: var(--data-font, ui-monospace, SFMono-Regular, Menlo, monospace);
  --transfer-glass: rgb(255 255 255 / 60%);
  --transfer-glass-border: rgb(255 255 255 / 80%);
  --transfer-glint-soft: rgb(255 255 255 / 30%);
  --transfer-glint-strong: rgb(255 255 255 / 42%);
  --transfer-control-shadow: rgb(67 239 169 / 26%);
  --transfer-focus-ring: rgb(67 239 169 / 20%);
  --transfer-close-shadow: rgb(18 32 24 / 14%);
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--surface) 94%, var(--surface-elevated)) 0%, var(--surface) 38%),
    var(--surface);
  border: 1px solid var(--line);
  border-bottom: 0;
  border-radius: 20px 20px 0 0;
  box-shadow: 0 -18px 48px var(--shadow);
  box-sizing: border-box;
  color: var(--ink);
  gap: 10px;
  grid-template-rows: auto minmax(0, 1fr) auto;
  height: min(520px, calc(100dvh - max(16px, env(safe-area-inset-top))));
  max-height: none;
  max-width: 448px;
  overflow: hidden;
  overscroll-behavior: contain;
  padding: 8px 16px calc(22px + env(safe-area-inset-bottom));
  width: 100%;
}

html[data-theme='dark'] .assets-transfer-sheet {
  --surface: rgb(0 0 0);
  --surface-elevated: rgb(12 16 14);
  --surface-2: rgb(18 23 20);
  --surface-3: rgb(25 33 29);
  --line: rgb(41 52 46);
  --line-strong: rgb(58 74 66);
  --hairline: rgb(32 41 35);
  --ink: rgb(242 247 244);
  --muted: rgb(149 161 154);
  --accent: rgb(67 239 169);
  --positive: rgb(97 241 182);
  --on-accent: rgb(7 17 13);
  --shadow: rgb(0 0 0 / 48%);
  --transfer-glass: rgb(255 255 255 / 8%);
  --transfer-glass-border: rgb(255 255 255 / 15%);
  --transfer-focus-ring: rgb(67 239 169 / 28%);
  --transfer-close-shadow: rgb(0 0 0 / 48%);
}

.assets-transfer-sheet--picker {
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--surface) 97%, var(--surface-elevated)) 0%, var(--surface) 100%),
    var(--surface);
}

.assets-transfer-sheet__top {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.assets-transfer-sheet__grab {
  background: var(--line-strong);
  border-radius: 2px;
  display: block;
  height: 4px;
  justify-self: center;
  width: 40px;
}

.assets-transfer-sheet__header {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 40px;
}

.assets-transfer-sheet__header h2 {
  font-size: 18px;
  font-weight: 700;
  line-height: 24px;
  margin: 0;
}

.assets-transfer-sheet__close,
.assets-transfer-route > button {
  align-items: center;
  border: 0;
  border-radius: 50%;
  display: flex;
  flex: 0 0 44px;
  height: 44px;
  justify-content: center;
  min-height: 44px;
  padding: 0;
  position: relative;
  width: 44px;
}

.assets-transfer-sheet__close {
  background: transparent;
  color: var(--muted);
}

.assets-transfer-sheet__close::before,
.assets-transfer-route > button::before {
  border-radius: 50%;
  content: '';
  height: 32px;
  inset: 6px;
  pointer-events: none;
  position: absolute;
  width: 32px;
}

.assets-transfer-sheet__close::before {
  background:
    radial-gradient(circle at 36% 28%, color-mix(in srgb, rgb(255 255 255) 32%, transparent) 0 12%, transparent 42%),
    var(--surface-2);
  box-shadow:
    inset 0 1px 0 var(--transfer-glint-soft),
    0 5px 14px var(--transfer-close-shadow);
}

.assets-transfer-sheet__close > svg,
.assets-transfer-route > button > svg {
  position: relative;
  z-index: 1;
}

.assets-transfer-sheet__body,
.assets-transfer-picker {
  min-height: 0;
  min-width: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: none;
}

.assets-transfer-sheet__body::-webkit-scrollbar,
.assets-transfer-picker::-webkit-scrollbar,
.assets-transfer-picker__list::-webkit-scrollbar {
  display: none;
}

.assets-transfer-sheet__body {
  align-content: start;
  display: grid;
  gap: 10px;
}

.assets-transfer-amount {
  background:
    linear-gradient(145deg, color-mix(in srgb, var(--surface-elevated) 92%, transparent), color-mix(in srgb, var(--surface-2) 86%, transparent));
  border: 1px solid var(--line);
  border-radius: var(--radius-l, 16px);
  box-sizing: border-box;
  display: grid;
  gap: 7px;
  min-height: 140px;
  overflow: hidden;
  padding: 16px 18px;
  position: relative;
}

.assets-transfer-amount::after {
  background: linear-gradient(100deg, transparent 0 35%, color-mix(in srgb, rgb(255 255 255) 12%, transparent) 48%, transparent 62%);
  content: '';
  inset: 0;
  pointer-events: none;
  position: absolute;
}

.assets-transfer-amount__bloom {
  background: radial-gradient(circle, color-mix(in srgb, var(--accent) 24%, transparent) 0, transparent 68%);
  height: 220px;
  pointer-events: none;
  position: absolute;
  right: -46px;
  top: -82px;
  width: 220px;
}

.assets-transfer-amount__label,
.assets-transfer-amount__meta {
  color: var(--muted);
  font-size: 11px;
  font-weight: 500;
  line-height: 15px;
  position: relative;
  z-index: 1;
}

.assets-transfer-amount input {
  appearance: textfield;
  background: transparent;
  border: 0;
  color: var(--ink);
  font-family: var(--font-data, ui-monospace, SFMono-Regular, Menlo, monospace);
  font-size: 30px;
  font-weight: 700;
  height: 42px;
  letter-spacing: -.04em;
  line-height: 42px;
  min-width: 0;
  outline: 0;
  padding: 0;
  position: relative;
  width: 100%;
  z-index: 1;
}

.assets-transfer-amount input::placeholder {
  color: color-mix(in srgb, var(--ink) 36%, transparent);
  opacity: 1;
}

.assets-transfer-amount:focus-within {
  border-color: color-mix(in srgb, var(--accent) 72%, var(--line));
  box-shadow: 0 0 0 2px var(--transfer-focus-ring);
}

.assets-transfer-amount__meta {
  align-items: center;
  display: flex;
  justify-content: space-between;
  margin-top: auto;
}

.assets-transfer-amount__meta > span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.assets-transfer-amount__meta button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--ink);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 10px;
  font-weight: 650;
  height: 44px;
  justify-content: center;
  margin-block: -8px;
  min-height: 44px;
  min-width: 52px;
  padding: 0 10px;
  position: relative;
}

.assets-transfer-amount__meta button::before {
  backdrop-filter: blur(18px);
  background: var(--transfer-glass);
  border: 1px solid var(--transfer-glass-border);
  border-radius: 14px;
  content: '';
  height: 28px;
  inset: 8px 2px;
  pointer-events: none;
  position: absolute;
}

.assets-transfer-amount__meta button > span {
  position: relative;
  z-index: 1;
}

.assets-transfer-route {
  align-items: center;
  backdrop-filter: blur(18px);
  background: var(--transfer-glass);
  border: 1px solid var(--transfer-glass-border);
  border-radius: var(--radius-m, 12px);
  box-sizing: border-box;
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) 44px minmax(0, 1fr);
  height: 52px;
  min-width: 0;
  padding: 4px 8px 4px 12px;
}

.assets-transfer-route > button {
  background: transparent;
  box-shadow: none;
  color: var(--on-accent);
}

.assets-transfer-route > button::before {
  background:
    radial-gradient(circle at 34% 26%, color-mix(in srgb, rgb(255 255 255) 52%, transparent) 0 10%, transparent 40%),
    var(--accent);
  box-shadow:
    inset 0 1px 0 var(--transfer-glint-strong),
    0 5px 14px var(--transfer-control-shadow);
}

.assets-transfer-account {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.assets-transfer-account--target {
  justify-items: end;
  text-align: right;
}

.assets-transfer-account span,
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

.assets-transfer-asset,
.assets-transfer-picker__row {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--ink);
  display: grid;
  gap: 10px;
  min-height: 52px;
  min-width: 0;
  padding: 0;
  text-align: left;
  width: 100%;
}

.assets-transfer-asset {
  border-bottom: 1px solid var(--hairline);
  grid-template-columns: 32px minmax(0, 1fr) minmax(64px, auto) 18px;
}

.assets-transfer-asset__copy,
.assets-transfer-asset__value,
.assets-transfer-picker__copy,
.assets-transfer-picker__value {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.assets-transfer-asset__copy strong,
.assets-transfer-picker__copy strong {
  font-family: var(--font-data, ui-monospace, SFMono-Regular, Menlo, monospace);
  font-size: 14px;
  font-weight: 700;
  line-height: 19px;
}

.assets-transfer-asset__copy small,
.assets-transfer-asset__value small,
.assets-transfer-picker__copy small,
.assets-transfer-picker__value small {
  color: var(--muted);
  font-size: 10px;
  line-height: 14px;
}

.assets-transfer-asset__value,
.assets-transfer-picker__value {
  justify-items: end;
  text-align: right;
}

.assets-transfer-asset__value strong,
.assets-transfer-picker__value strong {
  font-size: 14px;
  font-weight: 650;
  line-height: 19px;
  max-width: 118px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.assets-transfer-asset > svg {
  color: var(--muted);
}

.assets-transfer-hint,
.assets-transfer-sheet__body > .positive,
.assets-transfer-sheet__body > .field-error,
.assets-transfer-picker__hint {
  border: 0;
  font-size: 10px;
  line-height: 14px;
  margin: 0;
  padding: 0;
}

.assets-transfer-hint,
.assets-transfer-picker__hint {
  color: var(--muted);
}

.assets-transfer-search {
  align-items: center;
  backdrop-filter: blur(18px);
  background: var(--transfer-glass);
  border: 1px solid var(--transfer-glass-border);
  border-radius: var(--radius-m, 12px);
  box-sizing: border-box;
  color: var(--muted);
  display: grid;
  gap: 9px;
  grid-template-columns: 20px minmax(0, 1fr);
  height: 46px;
  padding: 0 13px;
  position: sticky;
  top: 0;
  z-index: 2;
}

.assets-transfer-search:focus-within {
  border-color: color-mix(in srgb, var(--accent) 72%, var(--line));
  box-shadow: 0 0 0 2px var(--transfer-focus-ring);
}

.assets-transfer-search input {
  appearance: none;
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 13px;
  height: 44px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.assets-transfer-search input::-webkit-search-cancel-button {
  appearance: none;
}

.assets-transfer-picker {
  align-content: start;
  display: grid;
  gap: 8px;
}

.assets-transfer-picker__list {
  display: grid;
  min-height: 0;
}

.assets-transfer-picker__row {
  border-bottom: 1px solid var(--hairline);
  grid-template-columns: 32px minmax(0, 1fr) minmax(64px, auto) 18px;
  padding: 6px 8px;
}

.assets-transfer-picker__row.is-selected {
  background: color-mix(in srgb, var(--accent) 11%, transparent);
  border-color: color-mix(in srgb, var(--accent) 28%, transparent);
  border-radius: var(--radius-m, 12px);
}

.assets-transfer-picker__row > svg {
  color: var(--accent);
}

.assets-transfer-picker__empty {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 11px;
  justify-content: center;
  margin: 0;
  min-height: 180px;
  text-align: center;
}

.assets-transfer-picker__hint {
  align-self: center;
  text-align: center;
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
  min-height: 50px;
  transition: filter 160ms ease, opacity 160ms ease, transform 160ms ease;
  width: 100%;
}

.assets-transfer-submit:not(:disabled):active {
  transform: translateY(1px) scale(.995);
}

.assets-transfer-submit:disabled {
  background: var(--accent);
  color: var(--on-accent);
  filter: saturate(.72);
  opacity: .56;
}

@media (prefers-reduced-motion: reduce) {
  .assets-account-card,
  .assets-transfer-sheet,
  .assets-transfer-submit {
    animation: none !important;
    transition: none !important;
  }
}

@media (max-width: 340px) {
  .assets-transfer-sheet {
    padding-inline: 12px;
  }

  .assets-transfer-amount {
    min-height: 136px;
    padding-inline: 15px;
  }

  .assets-transfer-amount input {
    font-size: 28px;
  }

  .assets-transfer-route {
    gap: 3px;
    padding-inline: 9px 5px;
  }

  .assets-transfer-account strong {
    font-size: 13px;
  }

  .assets-transfer-asset,
  .assets-transfer-picker__row {
    gap: 8px;
    grid-template-columns: 30px minmax(0, 1fr) minmax(58px, auto) 17px;
  }

  .assets-transfer-asset__value strong,
  .assets-transfer-picker__value strong {
    max-width: 88px;
  }

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

  .assets-account-cards {
    gap: 8px;
  }

  .assets-account-card {
    grid-template-columns: minmax(0, 1fr) minmax(104px, auto);
    padding-inline: 12px;
  }

  .assets-account-card__balance strong {
    font-size: 16px;
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

@media (max-width: 820px) {
  .assets-transfer-layer {
    right: 0;
    width: 100%;
  }
}
</style>
