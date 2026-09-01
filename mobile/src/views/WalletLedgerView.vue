<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { Check, ChevronDown, CircleAlert, FileSearch, LoaderCircle, X } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  createWalletLedgerPaginationController,
  fetchWalletAccounts,
  fetchWalletLedger,
  formatWalletLedgerDecimal,
  formatWalletLedgerTime,
  isWalletLedgerContractError,
  WALLET_LEDGER_DATE_PRESETS,
  WALLET_LEDGER_DIRECTIONS,
  walletLedgerAccountTranslationKey,
  walletLedgerAmountSign,
  walletLedgerDatePresetTranslationKey,
  walletLedgerDateRange,
  walletLedgerDirectionTranslationKey,
  walletLedgerEntryIdentity,
  walletLedgerTypePresentation,
  type WalletLedgerDatePreset,
  type WalletLedgerDateRange,
  type WalletLedgerDirection,
  type WalletLedgerEntry,
} from '@/api/wallet'
import { currentIntlLocale } from '@/i18n'
import { decimalSign, type DecimalText } from '@/core/decimal'
import { useModalDialog } from '@/core/modalDialog'
import { useSessionStore } from '@/stores/session'

const PAGE_SIZE = 30
type FilterSheet = 'asset' | 'direction' | 'date'

const session = useSessionStore()
const { locale, t } = useI18n()
const entries = ref<WalletLedgerEntry[]>([])
const activeAssetSymbol = ref<string>()
const activeDirection = ref<WalletLedgerDirection>('all')
const activeDatePreset = ref<WalletLedgerDatePreset>('all')
const activeDateRange = ref<WalletLedgerDateRange>(walletLedgerDateRange('all'))
const walletAssetSymbols = ref<string[]>([])
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
const directionTrigger = ref<HTMLElement | null>(null)
const dateTrigger = ref<HTMLElement | null>(null)
let assetDirectoryVersion = 0

const paginationController = createWalletLedgerPaginationController({
  sessionKey: () => session.token,
  sessionGeneration: () => session.generation,
  selectedAssetSymbol: () => activeAssetSymbol.value,
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
const { trapFocus: trapFilterFocus, setReturnFocus } = useModalDialog(filterSheetOpen, filterDialog,
  '[data-dialog-initial]',
)

const filterSheetTitle = computed(() => {
  if (openSheet.value === 'asset') return t('ledger.assetPickerTitle')
  if (openSheet.value === 'direction') return t('ledger.directionPickerTitle')
  return t('ledger.datePickerTitle')
})

const currentFilterLabel = computed(() => {
  if (openSheet.value === 'asset') return assetLabel(activeAssetSymbol.value)
  if (openSheet.value === 'direction') return directionLabel(activeDirection.value)
  return dateLabel(activeDatePreset.value)
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
  const version = ++assetDirectoryVersion
  const token = session.token
  const generation = session.generation
  assetDirectoryLoading.value = true
  assetDirectoryError.value = ''
  try {
    const accounts = await fetchWalletAccounts()
    if (version !== assetDirectoryVersion
      || token !== session.token
      || generation !== session.generation) return
    walletAssetSymbols.value = [...new Set(accounts.map((account) => account.symbol))].sort()
  } catch (reason) {
    if (version !== assetDirectoryVersion
      || token !== session.token
      || generation !== session.generation) return
    assetDirectoryError.value = apiErrorMessage(reason, t('ledger.assetLoadFailed'))
  } finally {
    if (version === assetDirectoryVersion
      && token === session.token
      && generation === session.generation) {
      assetDirectoryLoading.value = false
    }
  }
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
  const trigger = kind === 'asset'
    ? assetTrigger.value
    : kind === 'direction' ? directionTrigger.value : dateTrigger.value
  setReturnFocus(trigger)
  openSheet.value = kind
}

function closeFilterSheet(): void {
  openSheet.value = null
}

function handleFilterKeydown(event: KeyboardEvent): void {
  trapFilterFocus(event, closeFilterSheet)
}

function assetLabel(symbol?: string): string {
  return symbol || t('ledger.assetFilterTrigger')
}

function directionLabel(direction: WalletLedgerDirection): string {
  return t(walletLedgerDirectionTranslationKey(direction))
}

function dateLabel(preset: WalletLedgerDatePreset): string {
  return preset === 'all'
    ? t('ledger.dateFilterTrigger')
    : t(walletLedgerDatePresetTranslationKey(preset))
}

function entryLabel(entry: WalletLedgerEntry): string {
  return t(walletLedgerTypePresentation(entry.changeType).translationKey)
}

function entryMeta(entry: WalletLedgerEntry): string {
  const source = walletLedgerTypePresentation(entry.changeType).source
  const account = t(walletLedgerAccountTranslationKey(entry.accountType))
  return `${entry.symbol} · ${source || account}`
}

function entryTime(entry: WalletLedgerEntry): string {
  void locale.value
  return formatWalletLedgerTime(entry.createdAt, currentIntlLocale())
}

function ledgerDecimal(value: DecimalText, precisionScale: number): string {
  void locale.value
  return formatWalletLedgerDecimal(value, currentIntlLocale(), precisionScale)
}

function signedAmount(entry: WalletLedgerEntry): string {
  return `${walletLedgerAmountSign(entry.amount)}${ledgerDecimal(entry.amount, entry.precisionScale)} ${entry.symbol}`
}

function amountTone(entry: WalletLedgerEntry): 'is-positive' | 'is-default' {
  return decimalSign(entry.amount) > 0 ? 'is-positive' : 'is-default'
}

function exactAmountTitle(entry: WalletLedgerEntry): string {
  return t('ledger.amountExact', { amount: entry.amount, symbol: entry.symbol })
}

function entryAccessibleDetails(entry: WalletLedgerEntry): string {
  return t('ledger.entryDetails', {
    type: entryLabel(entry),
    asset: entry.symbol,
    amount: `${entry.amount} ${entry.symbol}`,
    balance: `${entry.balanceAfter} ${entry.symbol}`,
    fee: `${entry.fee} ${entry.symbol}`,
    account: t(walletLedgerAccountTranslationKey(entry.accountType)),
    time: entryTime(entry),
  })
}

function resetSessionState(): void {
  paginationController.reset()
  assetDirectoryVersion += 1
  closeFilterSheet()
  activeAssetSymbol.value = undefined
  activeDirection.value = 'all'
  activeDatePreset.value = 'all'
  activeDateRange.value = walletLedgerDateRange('all')
  walletAssetSymbols.value = []
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
  assetDirectoryVersion += 1
  paginationController.stop()
})
</script>

<template>
  <main
    class="page page--plain pencil-page wallet-pencil-page wallet-ledger-pencil"
    data-pencil-source="y6Y7TW m25xr0"
  >
    <PageHeader
      :back="true"
      :fallback="{ name: 'assets' }"
      :pencil="true"
      :title="t('assets.fundLedger')"
    />

    <div class="page-content ledger-page">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="wallet-login-prompt"
        :description="t('ledger.loginDescription')"
      />
      <template v-else>
        <nav class="ledger-filter-bar" :aria-label="t('ledger.filterBarLabel')">
          <button
            ref="assetTrigger"
            class="ledger-filter-trigger is-active"
            type="button"
            :aria-label="t('ledger.assetFilterLabel')"
            aria-haspopup="dialog"
            :aria-expanded="openSheet === 'asset'"
            @click="openFilterSheet('asset')"
          >
            <span>{{ assetLabel(activeAssetSymbol) }}</span>
            <ChevronDown :size="11" aria-hidden="true" />
          </button>
          <button
            ref="directionTrigger"
            class="ledger-filter-trigger"
            :class="{ 'is-active': activeDirection !== 'all' }"
            type="button"
            :aria-label="t('ledger.directionFilterLabel')"
            aria-haspopup="dialog"
            :aria-expanded="openSheet === 'direction'"
            @click="openFilterSheet('direction')"
          >
            <span>{{ directionLabel(activeDirection) }}</span>
            <ChevronDown :size="11" aria-hidden="true" />
          </button>
          <button
            ref="dateTrigger"
            class="ledger-filter-trigger"
            :class="{ 'is-active': activeDatePreset !== 'all' }"
            type="button"
            :aria-label="t('ledger.dateFilterLabel')"
            aria-haspopup="dialog"
            :aria-expanded="openSheet === 'date'"
            @click="openFilterSheet('date')"
          >
            <span>{{ dateLabel(activeDatePreset) }}</span>
            <ChevronDown :size="11" aria-hidden="true" />
          </button>
        </nav>

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
            <div class="ledger-row__copy">
              <strong>{{ entryLabel(entry) }}</strong>
              <small>{{ entryMeta(entry) }} · {{ entryTime(entry) }}</small>
            </div>
            <div class="ledger-row__amount">
              <strong
                class="numeric"
                :class="amountTone(entry)"
                :title="exactAmountTitle(entry)"
              >{{ signedAmount(entry) }}</strong>
              <small class="numeric" :title="`${entry.balanceAfter} ${entry.symbol}`">
                {{ ledgerDecimal(entry.balanceAfter, entry.precisionScale) }} {{ entry.symbol }}
              </small>
            </div>
          </article>
        </div>
        <div v-else class="ledger-state ledger-state--empty" role="status">
          <span class="ledger-state__plate"><FileSearch :size="24" aria-hidden="true" /></span>
          <strong>{{ t('ledger.empty') }}</strong>
          <span>{{ t('ledger.emptyDescription') }}</span>
        </div>

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

          <div v-else-if="openSheet === 'direction'" class="ledger-filter-options" role="list" :aria-label="t('ledger.filterOptionsLabel', { filter: t('ledger.directionPickerTitle') })">
            <button v-for="direction in WALLET_LEDGER_DIRECTIONS" :key="direction" type="button" :class="{ 'is-selected': activeDirection === direction }" :aria-pressed="activeDirection === direction" :data-dialog-initial="activeDirection === direction ? '' : undefined" @click="selectDirection(direction)">
              <span>{{ directionLabel(direction) }}</span><Check v-if="activeDirection === direction" :size="18" aria-hidden="true" />
            </button>
          </div>

          <div v-else class="ledger-filter-options" role="list" :aria-label="t('ledger.filterOptionsLabel', { filter: t('ledger.datePickerTitle') })">
            <button v-for="preset in WALLET_LEDGER_DATE_PRESETS" :key="preset" type="button" :class="{ 'is-selected': activeDatePreset === preset }" :aria-pressed="activeDatePreset === preset" :data-dialog-initial="activeDatePreset === preset ? '' : undefined" @click="selectDate(preset)">
              <span>{{ preset === 'all' ? t('ledger.dateAll') : dateLabel(preset) }}</span><Check v-if="activeDatePreset === preset" :size="18" aria-hidden="true" />
            </button>
          </div>
        </section>
      </div>
    </Teleport>
  </main>
</template>

<style scoped>
.wallet-pencil-page {
  --muted: var(--wallet-ledger-muted);
  --pencil-header-inline: 20px;
  --pencil-header-inline-compact: 20px;
  background: var(--page);
  min-width: 0;
  overflow-x: hidden;
}

.wallet-ledger-pencil :deep(.pencil-page-header) {
  grid-template-columns: 40px minmax(0, 1fr) 40px;
  padding: 10px 20px;
}

.wallet-ledger-pencil :deep(.pencil-page-header .page-header__back) {
  height: 40px !important;
  max-width: 40px !important;
  min-height: 40px !important;
  min-width: 40px !important;
  overflow: visible;
  position: relative;
  width: 40px !important;
}

.wallet-ledger-pencil :deep(.pencil-page-header .page-header__back::before) {
  background: transparent;
  content: '';
  height: 44px;
  left: 50%;
  pointer-events: auto;
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 44px;
}

.wallet-ledger-pencil :deep(.pencil-page-header .page-header__actions) {
  height: 40px;
  min-width: 40px;
  width: 40px;
}

.ledger-page {
  display: grid;
  gap: 10px;
  min-width: 0;
  overflow-x: hidden;
  padding: 6px 20px calc(20px + env(safe-area-inset-bottom));
}

.ledger-filter-bar {
  align-items: center;
  display: flex;
  gap: 8px;
  height: 28px;
  min-width: 0;
}

.ledger-filter-trigger {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 14px;
  color: var(--muted);
  display: inline-flex;
  flex: 0 1 auto;
  font-size: 11px;
  font-weight: 500;
  gap: 4px;
  height: 28px;
  justify-content: center;
  line-height: 15px;
  max-height: 28px;
  min-height: 28px;
  min-width: 0;
  overflow: visible;
  padding: 0 12px;
  position: relative;
  white-space: nowrap;
}

.ledger-filter-trigger::before {
  background: transparent;
  content: '';
  inset: -8px 0;
  pointer-events: auto;
  position: absolute;
}

.ledger-filter-trigger span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ledger-filter-trigger.is-active {
  background: var(--accent-soft);
  color: var(--positive);
  font-weight: 650;
}

.ledger-filter-trigger:focus-visible,
.ledger-state button:focus-visible,
.ledger-inline-error button:focus-visible,
.ledger-load-more:focus-visible,
.ledger-filter-sheet button:focus-visible {
  box-shadow: 0 0 0 2px var(--focus-ring);
  outline: 0;
}

.ledger-list {
  display: grid;
  gap: 0;
  min-width: 0;
}

.ledger-row {
  align-items: center;
  border: 0;
  display: flex;
  gap: 12px;
  height: 56px;
  max-height: 56px;
  min-height: 56px;
  min-width: 0;
  overflow: hidden;
  width: 100%;
}

.ledger-row__copy,
.ledger-row__amount {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.ledger-row__copy {
  flex: 1 1 0;
}

.ledger-row__copy strong,
.ledger-row__amount strong,
.ledger-row small {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__copy strong {
  color: var(--ink);
  font-size: 13px;
  font-weight: 700;
  line-height: 18px;
}

.ledger-row small {
  color: var(--muted);
  font-size: 11px;
  font-weight: 450;
  line-height: 16px;
}

.ledger-row__amount {
  flex: 0 1 auto;
  max-width: 48%;
  text-align: right;
}

.ledger-row__amount strong {
  color: var(--ink);
  font-size: clamp(11px, 3.33vw, 13px);
  font-weight: 650;
  line-height: 18px;
}

.ledger-row__amount .is-positive {
  color: var(--positive);
}

.ledger-row__amount small {
  font-size: 10px;
  font-weight: 500;
  line-height: 15px;
}

.numeric {
  font-family: var(--font-geist-mono), var(--data-font);
  font-variant-numeric: tabular-nums;
}

.ledger-loading {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 180px;
}

.ledger-state {
  align-items: center;
  color: var(--muted);
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
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--muted);
  display: flex;
  height: 56px;
  justify-content: center;
  width: 56px;
}

.ledger-state strong {
  color: var(--ink);
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
  color: var(--negative);
}

.ledger-state button,
.ledger-load-more {
  background: transparent;
  border: 1px solid var(--line);
  border-radius: 999px;
  color: var(--positive);
  font-size: 11px;
  min-height: 44px;
  padding: 0 18px;
}

.ledger-inline-error {
  align-items: center;
  background: var(--negative-soft);
  border-radius: 12px;
  color: var(--negative);
  display: grid;
  font-size: 11px;
  gap: 8px;
  grid-template-columns: 18px minmax(0, 1fr) auto;
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
  width: 100%;
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
  border-top: 1px solid var(--hairline);
  gap: 10px;
  grid-template-columns: 34px minmax(0, 1fr) auto;
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
  color: var(--muted);
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
