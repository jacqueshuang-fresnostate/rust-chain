<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import {
  Check,
  ChevronDown,
  CircleAlert,
  ListFilter,
  LoaderCircle,
  X,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import TransactionRecordEmptyState from '@/components/TransactionRecordEmptyState.vue'
import TransactionRecordsLayout from '@/components/TransactionRecordsLayout.vue'
import { apiErrorMessage } from '@/api/client'
import {
  createWalletLedgerAssetDirectoryRequestLifecycle,
  createWalletLedgerPaginationController,
  fetchWalletAccounts,
  fetchWalletLedger,
  formatWalletLedgerDecimal,
  isWalletLedgerContractError,
  WALLET_LEDGER_DATE_PRESETS,
  WALLET_LEDGER_DIRECTIONS,
  WALLET_LEDGER_FILTERS,
  walletLedgerAccountTranslationKey,
  walletLedgerAmountSign,
  walletLedgerCategoryTranslationKey,
  walletLedgerDatePresetTranslationKey,
  walletLedgerDateRange,
  walletLedgerDirectionForAmount,
  walletLedgerDirectionTranslationKey,
  walletLedgerEntryIdentity,
  walletLedgerFeeDebitAmount,
  walletLedgerTypePresentation,
  type WalletLedgerDatePreset,
  type WalletLedgerDateRange,
  type WalletLedgerDirection,
  type WalletLedgerEntry,
  type WalletLedgerFilter,
} from '@/api/wallet'
import { currentIntlLocale } from '@/i18n'
import { decimalSign, type DecimalText } from '@/core/decimal'
import { useModalDialog } from '@/core/modalDialog'
import { useSessionStore } from '@/stores/session'

const PAGE_SIZE = 30
type FilterSheet = 'asset' | 'category' | 'more'

const session = useSessionStore()
const { locale, t } = useI18n()
const entries = ref<WalletLedgerEntry[]>([])
const activeAssetSymbol = ref<string>()
const activeCategory = ref<WalletLedgerFilter>('all')
const activeDirection = ref<WalletLedgerDirection>('all')
const activeDatePreset = ref<WalletLedgerDatePreset>('all')
const activeDateRange = ref<WalletLedgerDateRange>(walletLedgerDateRange('all'))
const walletAssetSymbols = ref<string[]>([])
const walletAssetLogoUrls = ref<Record<string, string>>({})
const assetDirectoryLoading = ref(false)
const assetDirectoryError = ref('')
const loading = ref(false)
const loadingMore = ref(false)
const exhausted = ref(false)
const initialError = ref<unknown | null>(null)
const appendError = ref<unknown | null>(null)
const openSheet = ref<FilterSheet | null>(null)
const filterSheetOpen = computed(() => openSheet.value !== null)
const filterDialog = ref<HTMLElement | null>(null)
const assetTrigger = ref<HTMLElement | null>(null)
const categoryTrigger = ref<HTMLElement | null>(null)
const moreTrigger = ref<HTMLElement | null>(null)

const paginationController = createWalletLedgerPaginationController({
  sessionKey: () => session.token,
  sessionGeneration: () => session.generation,
  selectedAssetSymbol: () => activeAssetSymbol.value,
  selectedCategory: () => activeCategory.value,
  selectedDirection: () => activeDirection.value,
  selectedDatePreset: () => activeDatePreset.value,
  selectedDateRange: () => activeDateRange.value,
  fetchPage: fetchWalletLedger,
  pageSize: PAGE_SIZE,
  onChange: (state) => {
    entries.value = state.entries
    loading.value = state.loading
    loadingMore.value = state.loadingMore
    exhausted.value = state.exhausted
    initialError.value = state.initialError
    appendError.value = state.appendError
  },
})
const assetDirectoryController = createWalletLedgerAssetDirectoryRequestLifecycle({
  sessionKey: () => session.token,
  sessionGeneration: () => session.generation,
  fetchDirectory: () => fetchWalletAccounts(),
})
const { trapFocus: trapFilterFocus, setReturnFocus } = useModalDialog(
  filterSheetOpen,
  filterDialog,
  '[data-dialog-initial]',
)

const filterSheetTitle = computed(() => {
  if (openSheet.value === 'asset') return t('ledger.assetPickerTitle')
  if (openSheet.value === 'category') return t('ledger.categoryPickerTitle')
  return t('ledger.morePickerTitle')
})

const currentFilterLabel = computed(() => {
  if (openSheet.value === 'asset') return assetSheetLabel(activeAssetSymbol.value)
  if (openSheet.value === 'category') return categoryLabel(activeCategory.value)
  return t('ledger.moreFilterSummary', {
    direction: directionLabel(activeDirection.value),
    date: dateSheetLabel(activeDatePreset.value),
  })
})

const error = computed(() => ledgerErrorMessage(
  entries.value.length ? appendError.value : initialError.value,
))

function ledgerErrorMessage(reason: unknown | null): string {
  if (reason === null) return ''
  const fallback = t('ledger.loadFailed')
  return isWalletLedgerContractError(reason) ? fallback : apiErrorMessage(reason, fallback)
}

async function load(reset = true): Promise<void> {
  if (reset) {
    await paginationController.loadInitial()
    return
  }
  if (appendError.value) await paginationController.retryLoadMore()
  else await paginationController.loadMore()
}

async function loadWalletAssetSymbols(): Promise<void> {
  assetDirectoryLoading.value = true
  assetDirectoryError.value = ''
  const result = await assetDirectoryController.load()
  if (result.state === 'stale') return

  assetDirectoryLoading.value = false
  if (result.state === 'guest') {
    walletAssetSymbols.value = []
    walletAssetLogoUrls.value = {}
    return
  }
  if (result.state === 'error') {
    assetDirectoryError.value = apiErrorMessage(result.error, t('ledger.assetLoadFailed'))
    return
  }
  walletAssetSymbols.value = result.value.symbols
  walletAssetLogoUrls.value = result.value.logoUrls
}

function reloadForFilterChange(): void {
  paginationController.reset()
  closeFilterSheet()
  void load()
}

function selectAsset(symbol?: string): void {
  if (symbol === activeAssetSymbol.value) {
    closeFilterSheet()
    return
  }
  activeAssetSymbol.value = symbol
  reloadForFilterChange()
}

function selectDirection(direction: WalletLedgerDirection): void {
  if (direction === activeDirection.value) {
    closeFilterSheet()
    return
  }
  activeDirection.value = direction
  reloadForFilterChange()
}

function selectCategory(category: WalletLedgerFilter): void {
  if (category === activeCategory.value) {
    closeFilterSheet()
    return
  }
  activeCategory.value = category
  reloadForFilterChange()
}

function selectDate(preset: WalletLedgerDatePreset): void {
  if (preset === activeDatePreset.value) {
    closeFilterSheet()
    return
  }
  activeDatePreset.value = preset
  activeDateRange.value = walletLedgerDateRange(preset)
  reloadForFilterChange()
}

function openFilterSheet(kind: FilterSheet): void {
  if (!session.isAuthenticated) return
  const trigger = kind === 'asset'
    ? assetTrigger.value
    : kind === 'category' ? categoryTrigger.value : moreTrigger.value
  setReturnFocus(trigger)
  openSheet.value = kind
}

function closeFilterSheet(): void {
  openSheet.value = null
}

function handleFilterKeydown(event: KeyboardEvent): void {
  trapFilterFocus(event, closeFilterSheet)
}

function assetTriggerLabel(symbol?: string): string {
  return symbol || t('ledger.currencyFilterTrigger')
}

function assetSheetLabel(symbol?: string): string {
  return symbol || t('ledger.assetAll')
}

function directionLabel(direction: WalletLedgerDirection): string {
  return t(walletLedgerDirectionTranslationKey(direction))
}

function categoryLabel(category: WalletLedgerFilter): string {
  return t(walletLedgerCategoryTranslationKey(category))
}

function categoryTriggerLabel(category: WalletLedgerFilter): string {
  return category === 'all' ? t('ledger.transactionTypeFilterTrigger') : categoryLabel(category)
}

function dateLabel(preset: WalletLedgerDatePreset): string {
  return preset === 'all'
    ? t('ledger.dateFilterTrigger')
    : t(walletLedgerDatePresetTranslationKey(preset))
}

function dateSheetLabel(preset: WalletLedgerDatePreset): string {
  return preset === 'all' ? t('ledger.dateAll') : dateLabel(preset)
}

function filterSelectionLabel(filter: string, value: string): string {
  return t('ledger.filterSelectionLabel', { filter, value })
}

function entryLabel(entry: WalletLedgerEntry): string {
  return t(walletLedgerTypePresentation(entry.changeType).translationKey)
}

function entryExecutionMeta(entry: WalletLedgerEntry): string {
  const source = walletLedgerTypePresentation(entry.changeType).source
  return source || entry.changeType
}

function entryPair(entry: WalletLedgerEntry): string {
  return entryLabel(entry)
}

function entryLogoUrl(entry: WalletLedgerEntry): string | undefined {
  return walletAssetLogoUrls.value[entry.symbol]
}

function entryTime(entry: WalletLedgerEntry): string {
  void locale.value
  const date = new Date(entry.createdAt)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}/${pad(date.getMonth() + 1)}/${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

function ledgerDecimal(value: DecimalText, precisionScale: number, assetSymbol: string): string {
  void locale.value
  return formatWalletLedgerDecimal(value, currentIntlLocale(), precisionScale, assetSymbol)
}

function signedAmount(entry: WalletLedgerEntry): string {
  const amount = ledgerDecimal(entry.amount, entry.precisionScale, entry.symbol)
  return `${amount.startsWith('<') ? '' : walletLedgerAmountSign(entry.amount)}${amount}`
}

function quantity(entry: WalletLedgerEntry): string {
  // The current ledger contract exposes the net account delta, not an authoritative gross fill.
  // Keep the Pencil field present without relabeling the net delta as transaction quantity.
  void entry
  return '--'
}

function entryDirectionLabel(entry: WalletLedgerEntry): string {
  const direction = walletLedgerDirectionForAmount(entry.amount)
  return direction ? directionLabel(direction) : '--'
}

function directionTone(entry: WalletLedgerEntry): 'is-buy' | 'is-sell' | 'is-ink' {
  const direction = walletLedgerDirectionForAmount(entry.amount)
  return direction === 'credit' ? 'is-buy' : direction === 'debit' ? 'is-sell' : 'is-ink'
}

function feeAmount(entry: WalletLedgerEntry): string {
  return feeIsKnown(entry)
    ? ledgerDecimal(walletLedgerFeeDebitAmount(entry.fee), entry.precisionScale, entry.symbol)
    : '--'
}

function exactFeeAmount(entry: WalletLedgerEntry): string {
  return feeIsKnown(entry) ? `${walletLedgerFeeDebitAmount(entry.fee)} ${entry.symbol}` : '--'
}

function feeIsKnown(entry: WalletLedgerEntry): boolean {
  return decimalSign(entry.fee) !== 0
}

function feeTone(entry: WalletLedgerEntry): 'is-sell' | 'is-ink' {
  return decimalSign(entry.fee) === 0 ? 'is-ink' : 'is-sell'
}

function exactAmountTitle(entry: WalletLedgerEntry): string {
  return t('ledger.amountExact', { amount: entry.amount, symbol: entry.symbol })
}

function exactQuantityTitle(entry: WalletLedgerEntry): string {
  void entry
  return '--'
}

function entryAccessibleDetails(entry: WalletLedgerEntry): string {
  return t('ledger.entryDetails', {
    type: entryLabel(entry),
    asset: entry.symbol,
    amount: `${entry.amount} ${entry.symbol}`,
    balance: `${entry.balanceAfter} ${entry.symbol}`,
    fee: exactFeeAmount(entry),
    account: t(walletLedgerAccountTranslationKey(entry.accountType)),
    time: entryTime(entry),
  })
}

function resetSessionState(): void {
  paginationController.reset()
  assetDirectoryController.invalidate()
  closeFilterSheet()
  activeAssetSymbol.value = undefined
  activeCategory.value = 'all'
  activeDirection.value = 'all'
  activeDatePreset.value = 'all'
  activeDateRange.value = walletLedgerDateRange('all')
  walletAssetSymbols.value = []
  walletAssetLogoUrls.value = {}
  assetDirectoryLoading.value = false
  assetDirectoryError.value = ''
}

watch(() => [session.token, session.generation] as const, ([token]) => {
  resetSessionState()
  if (token) {
    void loadWalletAssetSymbols()
    void load()
  }
}, { immediate: true })

onBeforeUnmount(() => {
  assetDirectoryController.stop()
  paginationController.stop()
})
</script>

<template>
  <TransactionRecordsLayout
    class="wallet-pencil-page wallet-ledger-pencil"
    active-tab="ledger"
    :back-fallback="{ name: 'assets' }"
    data-pencil-source="kcP5D A85if"
  >
    <nav class="ledger-filter-bar" :aria-label="t('ledger.filterBarLabel')">
      <button
        ref="assetTrigger"
        class="ledger-filter-trigger"
        :class="{ 'is-active': Boolean(activeAssetSymbol) }"
        type="button"
        :disabled="!session.isAuthenticated"
        :aria-label="filterSelectionLabel(t('ledger.currencyFilterTrigger'), assetSheetLabel(activeAssetSymbol))"
        aria-haspopup="dialog"
        :aria-expanded="openSheet === 'asset'"
        @click="openFilterSheet('asset')"
      >
        <span>{{ assetTriggerLabel(activeAssetSymbol) }}</span>
        <ChevronDown :size="16" aria-hidden="true" />
      </button>
      <button
        ref="categoryTrigger"
        class="ledger-filter-trigger"
        :class="{ 'is-active': activeCategory !== 'all' }"
        type="button"
        :disabled="!session.isAuthenticated"
        :aria-label="filterSelectionLabel(t('ledger.transactionTypeFilterTrigger'), categoryLabel(activeCategory))"
        aria-haspopup="dialog"
        :aria-expanded="openSheet === 'category'"
        @click="openFilterSheet('category')"
      >
        <span>{{ categoryTriggerLabel(activeCategory) }}</span>
        <ChevronDown :size="16" aria-hidden="true" />
      </button>
      <span class="ledger-filter-bar__spacer" aria-hidden="true" />
      <button
        ref="moreTrigger"
        class="ledger-filter-more"
        :class="{ 'is-active': activeDirection !== 'all' || activeDatePreset !== 'all' }"
        type="button"
        :disabled="!session.isAuthenticated"
        :aria-label="t('ledger.morePickerTitle')"
        aria-haspopup="dialog"
        :aria-expanded="openSheet === 'more'"
        @click="openFilterSheet('more')"
      >
        <ListFilter :size="24" aria-hidden="true" />
      </button>
    </nav>

    <div class="ledger-content">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="wallet-login-prompt"
        :description="t('ledger.loginDescription')"
      />
      <template v-else>
        <div v-if="error && !entries.length" class="ledger-state ledger-state--error" role="alert">
          <span class="ledger-state__plate"><CircleAlert :size="24" aria-hidden="true" /></span>
          <strong>{{ t('common.serviceUnavailable') }}</strong>
          <span>{{ error }}</span>
          <button type="button" :disabled="loading" @click="load()">{{ t('common.retry') }}</button>
        </div>
        <div v-else-if="loading && !entries.length" class="ledger-loading" role="status">
          <LoaderCircle :size="23" class="spin" aria-hidden="true" />
          <span>{{ t('ledger.loading') }}</span>
        </div>
        <div v-else-if="entries.length" class="ledger-list" role="list">
          <article
            v-for="entry in entries"
            :key="walletLedgerEntryIdentity(entry)"
            class="ledger-row"
            role="listitem"
            :aria-label="entryAccessibleDetails(entry)"
          >
            <header class="ledger-row__header">
              <div class="ledger-row__asset">
                <AssetMark :symbol="entry.symbol" :src="entryLogoUrl(entry)" :size="30" />
                <strong>{{ entry.symbol }}</strong>
              </div>
              <strong class="ledger-row__total numeric" :class="directionTone(entry)" :title="exactAmountTitle(entry)">
                {{ signedAmount(entry) }}
              </strong>
            </header>

            <div class="ledger-row__details">
              <strong class="ledger-row__pair" :title="entryPair(entry)">{{ entryPair(entry) }}</strong>
              <div class="ledger-row__quantity">
                <span>{{ t('ledger.quantity') }}</span>
                <strong class="numeric" :title="exactQuantityTitle(entry)">{{ quantity(entry) }}</strong>
              </div>

              <div class="ledger-row__execution">
                <span>{{ t(walletLedgerAccountTranslationKey(entry.accountType)) }} ·</span>
                <strong :class="directionTone(entry)">{{ entryDirectionLabel(entry) }}</strong>
                <small :title="entryExecutionMeta(entry)">{{ entryExecutionMeta(entry) }}</small>
              </div>
              <div class="ledger-row__fee">
                <span>{{ t('ledger.feeLabel') }}</span>
                <strong class="numeric" :class="feeTone(entry)" :title="exactFeeAmount(entry)">
                  {{ feeAmount(entry) }}
                </strong>
              </div>
            </div>

            <footer class="ledger-row__footer">
              <time class="numeric" :datetime="new Date(entry.createdAt).toISOString()" :title="entryTime(entry)">{{ entryTime(entry) }}</time>
              <div class="ledger-row__balance">
                <span>{{ t('ledger.accountBalance') }}</span>
                <strong class="numeric" :title="`${entry.balanceAfter} ${entry.symbol}`">
                  {{ ledgerDecimal(entry.balanceAfter, entry.precisionScale, entry.symbol) }}
                </strong>
              </div>
            </footer>
          </article>
        </div>
        <TransactionRecordEmptyState v-else :title="t('ledger.empty')" :description="t('ledger.emptyDescription')" />

        <div v-if="error && entries.length" class="ledger-inline-error" role="alert">
          <CircleAlert :size="16" aria-hidden="true" />
          <span>{{ error }}</span>
          <button type="button" :disabled="loadingMore" @click="load(false)">{{ t('common.retry') }}</button>
        </div>
        <button
          v-if="!loading && !exhausted && entries.length"
          class="ledger-load-more"
          type="button"
          :aria-busy="loadingMore"
          :disabled="loadingMore"
          @click="load(false)"
        >
          {{ loadingMore ? t('common.loading') : t('common.loadMore') }}
        </button>
      </template>
    </div>

    <Teleport to="body">
      <div v-if="filterSheetOpen" class="pencil-sheet-mask ledger-filter-mask" @click.self="closeFilterSheet">
        <section
          ref="filterDialog"
          class="pencil-sheet ledger-filter-sheet"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="`ledger-${openSheet}-filter-title`"
          tabindex="-1"
          @keydown="handleFilterKeydown"
        >
          <div class="pencil-sheet__handle" aria-hidden="true" />
          <header>
            <div class="ledger-filter-sheet__heading">
              <h2 :id="`ledger-${openSheet}-filter-title`">{{ filterSheetTitle }}</h2>
              <p>{{ t('ledger.filterCurrent', { value: currentFilterLabel }) }}</p>
            </div>
            <button class="ledger-filter-sheet__close" type="button" :aria-label="t('ledger.filterClose')" @click="closeFilterSheet">
              <X :size="20" aria-hidden="true" />
            </button>
          </header>

          <div v-if="openSheet === 'asset'" class="ledger-filter-options" role="list" :aria-label="t('ledger.filterOptionsLabel', { filter: t('ledger.assetPickerTitle') })">
            <button type="button" :class="{ 'is-selected': !activeAssetSymbol }" :aria-pressed="!activeAssetSymbol" :data-dialog-initial="!activeAssetSymbol ? '' : undefined" @click="selectAsset()">
              <span>{{ t('ledger.assetAll') }}</span><Check v-if="!activeAssetSymbol" :size="18" aria-hidden="true" />
            </button>
            <button v-for="symbol in walletAssetSymbols" :key="symbol" type="button" :class="{ 'is-selected': activeAssetSymbol === symbol }" :aria-pressed="activeAssetSymbol === symbol" :data-dialog-initial="activeAssetSymbol === symbol ? '' : undefined" @click="selectAsset(symbol)">
              <span>{{ symbol }}</span><Check v-if="activeAssetSymbol === symbol" :size="18" aria-hidden="true" />
            </button>
            <div v-if="assetDirectoryLoading" class="ledger-filter-sheet__state" role="status">
              <LoaderCircle :size="18" class="spin" aria-hidden="true" /><span>{{ t('ledger.assetLoading') }}</span>
            </div>
            <div v-else-if="assetDirectoryError" class="ledger-filter-sheet__state" role="alert">
              <span>{{ assetDirectoryError }}</span><button type="button" @click="loadWalletAssetSymbols">{{ t('common.retry') }}</button>
            </div>
            <p v-else-if="!walletAssetSymbols.length" class="ledger-filter-sheet__state" role="status">{{ t('ledger.assetEmpty') }}</p>
          </div>

          <div v-else-if="openSheet === 'category'" class="ledger-filter-options" role="list" :aria-label="t('ledger.filterOptionsLabel', { filter: t('ledger.categoryPickerTitle') })">
            <button v-for="category in WALLET_LEDGER_FILTERS" :key="category" type="button" :class="{ 'is-selected': activeCategory === category }" :aria-pressed="activeCategory === category" :data-dialog-initial="activeCategory === category ? '' : undefined" @click="selectCategory(category)">
              <span>{{ categoryLabel(category) }}</span><Check v-if="activeCategory === category" :size="18" aria-hidden="true" />
            </button>
          </div>

          <div v-else class="ledger-more-filters">
            <section>
              <h3>{{ t('ledger.directionPickerTitle') }}</h3>
              <div class="ledger-filter-options" role="list" :aria-label="t('ledger.filterOptionsLabel', { filter: t('ledger.directionPickerTitle') })">
                <button v-for="direction in WALLET_LEDGER_DIRECTIONS" :key="direction" type="button" :class="{ 'is-selected': activeDirection === direction }" :aria-pressed="activeDirection === direction" :data-dialog-initial="activeDirection === direction ? '' : undefined" @click="selectDirection(direction)">
                  <span>{{ directionLabel(direction) }}</span><Check v-if="activeDirection === direction" :size="18" aria-hidden="true" />
                </button>
              </div>
            </section>
            <section>
              <h3>{{ t('ledger.datePickerTitle') }}</h3>
              <div class="ledger-filter-options" role="list" :aria-label="t('ledger.filterOptionsLabel', { filter: t('ledger.datePickerTitle') })">
                <button v-for="preset in WALLET_LEDGER_DATE_PRESETS" :key="preset" type="button" :class="{ 'is-selected': activeDatePreset === preset }" :aria-pressed="activeDatePreset === preset" @click="selectDate(preset)">
                  <span>{{ preset === 'all' ? t('ledger.dateAll') : dateLabel(preset) }}</span><Check v-if="activeDatePreset === preset" :size="18" aria-hidden="true" />
                </button>
              </div>
            </section>
          </div>
        </section>
      </div>
    </Teleport>
  </TransactionRecordsLayout>
</template>

<style scoped>
.wallet-ledger-pencil {
  --ink: var(--wallet-record-ink);
  --muted: var(--wallet-record-row-muted);
  --positive: var(--wallet-record-buy);
  --wallet-record-active: #18d38d;
  --wallet-record-buy: #0dbe7b;
  --wallet-record-canvas: #ffffff;
  --wallet-record-card: #ffffff;
  --wallet-record-chrome: #ffffff;
  --wallet-record-ink: #111714;
  --wallet-record-row-line: #edf1ef;
  --wallet-record-row-muted: #8a948f;
  --wallet-record-sell: #ff5878;
  --wallet-record-tab-line: #eef1ef;
  --wallet-record-tab-muted: #7b8680;
  background: var(--wallet-record-canvas);
  color: var(--wallet-record-ink);
  min-width: 0;
  overflow-x: clip;
}

:global(html[data-theme='dark'] .wallet-ledger-pencil) {
  --wallet-record-buy: #45efae;
  --wallet-record-canvas: #000000;
  --wallet-record-card: #000000;
  --wallet-record-chrome: #000000;
  --wallet-record-ink: #f3f7f5;
  --wallet-record-row-line: #17221c;
  --wallet-record-row-muted: #8f9b94;
  --wallet-record-tab-line: #18231d;
  --wallet-record-tab-muted: #8f9b94;
}

.ledger-filter-bar {
  align-items: center;
  background: var(--wallet-record-chrome);
  box-sizing: border-box;
  display: flex;
  gap: 24px;
  height: 58px;
  min-height: 58px;
  min-width: 0;
  padding: 0 16px;
}

.ledger-filter-trigger,
.ledger-filter-more {
  background: transparent;
  border: 0;
  color: var(--wallet-record-ink);
  height: 44px;
  min-height: 44px;
  padding: 0;
}

.ledger-filter-trigger {
  align-items: center;
  display: inline-flex;
  flex: 0 1 auto;
  font-size: 16px;
  font-weight: 600;
  gap: 8px;
  line-height: 22px;
  min-width: 0;
  white-space: nowrap;
}

.ledger-filter-trigger span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ledger-filter-trigger svg {
  flex: 0 0 auto;
}

.ledger-filter-bar__spacer {
  flex: 1 1 auto;
  height: 1px;
  min-width: 0;
}

.ledger-filter-more {
  display: grid;
  flex: 0 0 44px;
  place-items: center;
  width: 44px;
}

.ledger-filter-trigger.is-active,
.ledger-filter-more.is-active {
  color: var(--wallet-record-active);
}

.ledger-filter-trigger:disabled,
.ledger-filter-more:disabled {
  cursor: default;
  opacity: 1;
}

.ledger-filter-trigger:focus-visible,
.ledger-filter-more:focus-visible,
.ledger-state button:focus-visible,
.ledger-inline-error button:focus-visible,
.ledger-load-more:focus-visible,
.ledger-filter-sheet button:focus-visible {
  box-shadow: 0 0 0 2px var(--focus-ring);
  outline: 0;
}

.ledger-content {
  background: var(--wallet-record-canvas);
  min-width: 0;
  overflow-x: clip;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
}

.ledger-list {
  box-sizing: border-box;
  display: block;
  min-width: 0;
  padding: 0;
}

.ledger-row {
  align-items: stretch;
  background: var(--wallet-record-card);
  border: 0;
  border-bottom: 1px solid var(--wallet-record-row-line);
  border-radius: 0;
  box-sizing: border-box;
  box-shadow: none;
  display: grid;
  gap: 9px;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: auto auto auto;
  justify-content: stretch;
  min-height: 190px;
  min-width: 0;
  overflow: hidden;
  padding: 12px 18px;
  width: 100%;
}

.ledger-row__header {
  align-items: center;
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 0.8fr) minmax(0, 1.2fr);
  min-height: 30px;
  min-width: 0;
}

.ledger-row__header > *,
.ledger-row__details > *,
.ledger-row__footer > * {
  min-width: 0;
}

.ledger-row__details {
  display: grid;
  gap: 8px 16px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin-top: 0;
  min-width: 0;
  padding-top: 0;
}

.ledger-row__asset {
  align-items: center;
  display: flex;
  flex: 1 1 auto;
  gap: 9px;
  overflow: hidden;
}

.ledger-row__asset strong {
  color: var(--wallet-record-ink);
  font-size: 20px;
  font-weight: 650;
  line-height: 28px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__total {
  color: var(--wallet-record-ink);
  font-size: 18px;
  font-weight: 500;
  line-height: 24px;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__pair {
  align-self: center;
  color: var(--wallet-record-ink);
  display: block;
  font-size: 15px;
  font-weight: 600;
  line-height: 22px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__quantity,
.ledger-row__fee,
.ledger-row__balance {
  align-items: center;
  display: grid;
  min-width: 0;
}

.ledger-row__quantity {
  gap: 6px;
  grid-template-columns: auto minmax(0, 1fr);
}

.ledger-row__quantity > span {
  color: var(--wallet-record-row-muted);
  font-size: 13px;
  font-weight: 500;
  line-height: 19px;
}

.ledger-row__quantity strong {
  color: var(--wallet-record-ink);
  font-size: 15px;
  font-weight: 500;
  line-height: 22px;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__execution {
  align-items: center;
  column-gap: 4px;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  grid-template-rows: 20px 18px;
  min-width: 0;
  overflow: hidden;
}

.ledger-row__execution > span {
  color: var(--wallet-record-ink);
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
  white-space: nowrap;
}

.ledger-row__execution > strong {
  font-size: 15px;
  font-weight: 650;
  line-height: 22px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__execution > small {
  color: var(--wallet-record-row-muted);
  font-size: 12px;
  font-weight: 500;
  grid-column: 1 / -1;
  line-height: 18px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__fee {
  align-content: start;
  gap: 2px;
  grid-template-rows: 16px 18px;
  justify-items: end;
}

.ledger-row__fee span {
  color: var(--wallet-record-row-muted);
  font-size: 12px;
  font-weight: 500;
  line-height: 18px;
}

.ledger-row__fee strong {
  font-size: 12px;
  font-weight: 500;
  line-height: 18px;
  max-width: 100%;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__footer {
  align-items: end;
  display: grid;
  gap: 12px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin-top: 0;
  min-width: 0;
  padding-top: 0;
}

.ledger-row__footer time {
  color: var(--wallet-record-row-muted);
  font-size: 13px;
  font-weight: 400;
  line-height: 19px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__balance {
  gap: 6px;
  grid-template-columns: auto minmax(0, 1fr);
}

.ledger-row__balance span {
  color: var(--wallet-record-row-muted);
  font-size: 13px;
  font-weight: 500;
  line-height: 19px;
}

.ledger-row__balance strong {
  color: var(--wallet-record-ink);
  font-size: 14px;
  font-weight: 500;
  line-height: 19px;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.is-buy {
  color: var(--wallet-record-buy);
}

.is-sell {
  color: var(--wallet-record-sell);
}

.is-ink {
  color: var(--wallet-record-ink);
}

.numeric {
  font-family: var(--font-geist-mono), var(--data-font);
  font-variant-numeric: tabular-nums;
}

.ledger-loading {
  align-items: center;
  color: var(--wallet-record-row-muted);
  display: flex;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 180px;
}

.ledger-state {
  align-items: center;
  color: var(--wallet-record-row-muted);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 12px;
  justify-content: center;
  min-height: 225px;
  padding: 48px 20px;
  text-align: center;
}

.ledger-state__plate {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--wallet-record-row-line);
  border-radius: 50%;
  color: var(--wallet-record-row-muted);
  display: flex;
  height: 56px;
  justify-content: center;
  width: 56px;
}

.ledger-state strong {
  color: var(--wallet-record-ink);
  font-size: 15px;
  font-weight: 650;
  line-height: 20px;
}

.ledger-state > span:last-child {
  line-height: 17px;
  max-width: 300px;
}

.ledger-state--error .ledger-state__plate,
.ledger-state--error strong {
  color: var(--wallet-record-sell);
}

.ledger-state button,
.ledger-load-more {
  background: transparent;
  border: 1px solid var(--wallet-record-row-line);
  border-radius: 999px;
  color: var(--wallet-record-active);
  font-size: 11px;
  min-height: 44px;
  padding: 0 18px;
}

.ledger-inline-error {
  align-items: center;
  background: var(--negative-soft);
  border-radius: 12px;
  color: var(--wallet-record-sell);
  display: grid;
  font-size: 11px;
  gap: 8px;
  grid-template-columns: 18px minmax(0, 1fr) auto;
  margin: 10px 16px 0;
  min-height: 44px;
  padding: 0 8px 0 10px;
}

.ledger-inline-error span {
  min-width: 0;
  overflow-wrap: anywhere;
}

.ledger-inline-error button {
  background: transparent;
  color: inherit;
  min-height: 44px;
  padding: 0 8px;
}

.ledger-load-more {
  margin: 10px 16px 0;
  width: calc(100% - 32px);
}

.ledger-filter-mask {
  justify-items: center;
}

.ledger-filter-sheet {
  --muted: var(--wallet-ledger-muted);
  color: var(--ink);
  max-width: 448px;
}

.ledger-filter-sheet > header {
  gap: 12px;
}

.ledger-filter-sheet__heading {
  display: grid;
  gap: 1px;
  min-width: 0;
}

.ledger-filter-sheet__heading h2 {
  color: var(--ink);
  font-size: 18px;
  font-weight: 650;
  line-height: 24px;
}

.ledger-filter-sheet__heading p {
  color: var(--muted);
  font-size: 11px;
  line-height: 16px;
  margin: 0;
}

.ledger-filter-sheet__close {
  align-items: center;
  background: transparent;
  color: var(--ink);
  display: inline-flex;
  flex: 0 0 44px;
  height: 44px;
  justify-content: center;
  padding: 0;
  width: 44px;
}

.ledger-filter-options {
  display: grid;
  min-width: 0;
}

.ledger-filter-options > button {
  align-items: center;
  background: transparent;
  border-bottom: 1px solid var(--hairline);
  color: var(--ink);
  display: grid;
  font-size: 13px;
  font-weight: 600;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) 20px;
  min-height: 56px;
  padding: 0 4px;
  text-align: left;
}

.ledger-filter-options > button.is-selected {
  color: var(--positive);
}

.ledger-more-filters {
  display: grid;
  gap: 18px;
  max-height: min(520px, calc(100dvh - 190px - env(safe-area-inset-bottom)));
  min-width: 0;
  overflow-y: auto;
}

.ledger-more-filters section {
  min-width: 0;
}

.ledger-more-filters h3 {
  color: var(--muted);
  font-size: 12px;
  font-weight: 500;
  line-height: 18px;
  margin: 0;
  padding: 0 4px 4px;
}

.ledger-filter-sheet__state {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 12px;
  gap: 8px;
  justify-content: center;
  margin: 0;
  min-height: 72px;
  text-align: center;
}

.ledger-filter-sheet__state button {
  background: transparent;
  color: var(--positive);
  min-height: 44px;
  padding: 0 8px;
}

.wallet-login-prompt {
  background: transparent;
  background-image: none;
  border: 0;
  border-top: 1px solid var(--wallet-record-row-line);
  gap: 10px;
  grid-template-columns: 34px minmax(0, 1fr) auto;
  margin: 0 18px;
  min-height: 72px;
  padding: 10px 0;
}

.wallet-login-prompt :deep(.login-required__icon) {
  background: var(--accent-soft);
  border: 0;
  color: var(--positive);
  height: 34px;
  width: 34px;
}

.wallet-login-prompt :deep(.login-required__copy) {
  gap: 2px;
}

.wallet-login-prompt :deep(.login-required__copy strong) {
  font-size: 13px;
}

.wallet-login-prompt :deep(.login-required__copy p) {
  color: var(--wallet-record-row-muted);
  font-size: 11px;
  line-height: 1.4;
}

.wallet-login-prompt :deep(.button) {
  border-radius: 999px;
  min-height: 44px;
  padding-inline: 14px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .ledger-list {
    padding-inline: 0;
  }

  .ledger-row {
    padding: 12px 18px;
  }

  .ledger-row__header {
    gap: 10px;
    grid-template-columns: minmax(0, 0.78fr) minmax(0, 1.22fr);
  }

  .ledger-row__details {
    gap: 8px 12px;
  }

  .ledger-row__footer {
    gap: 6px;
    grid-template-columns: minmax(0, 1fr);
  }

  .ledger-row__footer time {
    font-size: 12px;
  }

  .ledger-row__balance {
    width: 100%;
  }

  .wallet-login-prompt {
    align-items: center;
    grid-template-columns: 34px minmax(0, 1fr);
  }

  .wallet-login-prompt :deep(.button) {
    grid-column: 1 / -1;
    width: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
