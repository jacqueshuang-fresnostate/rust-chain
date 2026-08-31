<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { CircleAlert, FileSearch, LoaderCircle, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  advanceWalletLedgerPagination,
  createWalletLedgerRequestLifecycle,
  fetchWalletLedger,
  formatWalletLedgerDecimal,
  formatWalletLedgerGroupHeading,
  formatWalletLedgerTime,
  groupWalletLedgerEntries,
  isWalletLedgerContractError,
  mergeWalletLedgerEntries,
  WALLET_LEDGER_ACCOUNT_FILTERS,
  WALLET_LEDGER_FILTERS,
  walletLedgerAmountSign,
  walletLedgerAccountTranslationKey,
  walletLedgerCategoryTranslationKey,
  walletLedgerEntryIdentity,
  walletLedgerTypePresentation,
  type WalletLedgerAccountFilter,
  type WalletLedgerDateGroup,
  type WalletLedgerEntry,
  type WalletLedgerFilter,
} from '@/api/wallet'
import { currentIntlLocale } from '@/i18n'
import { decimalSign, type DecimalText } from '@/core/decimal'
import { useSessionStore } from '@/stores/session'

const PAGE_SIZE = 30

const session = useSessionStore()
const { locale, t } = useI18n()
const entries = ref<WalletLedgerEntry[]>([])
const activeFilter = ref<WalletLedgerFilter>('all')
const activeAccountType = ref<WalletLedgerAccountFilter>('all')
const loading = ref(false)
const loadingMore = ref(false)
const exhausted = ref(false)
const nextOffset = ref(0)
const error = ref('')
const requestLifecycle = createWalletLedgerRequestLifecycle({
  sessionKey: () => session.token,
  selectedFilter: () => activeFilter.value,
  selectedAccountType: () => activeAccountType.value,
  fetchPage: fetchWalletLedger,
})

const groupedEntries = computed(() => groupWalletLedgerEntries(entries.value))

async function load(reset = true): Promise<void> {
  if (!session.isAuthenticated) return
  if (!reset && (loading.value || loadingMore.value || exhausted.value)) return
  if (reset) loading.value = true
  else loadingMore.value = true
  error.value = ''

  const offset = reset ? 0 : nextOffset.value
  const result = await requestLifecycle.load(offset, PAGE_SIZE)
  if (result.state === 'stale') return
  if (result.state === 'guest') {
    loading.value = false
    loadingMore.value = false
    return
  }
  if (result.state === 'error') {
    const fallback = t('ledger.loadFailed')
    error.value = isWalletLedgerContractError(result.error)
      ? fallback
      : apiErrorMessage(result.error, fallback)
    loading.value = false
    loadingMore.value = false
    return
  }

  const nextEntries = reset
    ? result.value.entries
    : mergeWalletLedgerEntries(entries.value, result.value.entries)
  const pagination = advanceWalletLedgerPagination(offset, result.value)
  entries.value = nextEntries
  nextOffset.value = pagination.nextOffset
  exhausted.value = pagination.exhausted
  loading.value = false
  loadingMore.value = false
}

function changeFilter(filter: WalletLedgerFilter): void {
  if (filter === activeFilter.value) return
  requestLifecycle.invalidate()
  activeFilter.value = filter
  entries.value = []
  nextOffset.value = 0
  exhausted.value = false
  error.value = ''
  loadingMore.value = false
  void load()
}

function changeAccountType(accountType: WalletLedgerAccountFilter): void {
  if (accountType === activeAccountType.value) return
  requestLifecycle.invalidate()
  activeAccountType.value = accountType
  entries.value = []
  nextOffset.value = 0
  exhausted.value = false
  error.value = ''
  loadingMore.value = false
  void load()
}

function filterLabel(filter: WalletLedgerFilter): string {
  return t(walletLedgerCategoryTranslationKey(filter))
}

function accountLabel(accountType: WalletLedgerAccountFilter): string {
  return t(walletLedgerAccountTranslationKey(accountType))
}

function categoryLabel(category: WalletLedgerEntry['category']): string {
  return t(walletLedgerCategoryTranslationKey(category))
}

function entryLabel(entry: WalletLedgerEntry): string {
  return t(walletLedgerTypePresentation(entry.changeType).translationKey)
}

function entrySource(entry: WalletLedgerEntry): string | undefined {
  return walletLedgerTypePresentation(entry.changeType).source
}

function groupHeading(group: WalletLedgerDateGroup): string {
  void locale.value
  return formatWalletLedgerGroupHeading(group, currentIntlLocale(), {
    today: t('ledger.today'),
    yesterday: t('ledger.yesterday'),
  })
}

function entryTime(entry: WalletLedgerEntry): string {
  void locale.value
  return formatWalletLedgerTime(entry.createdAt, currentIntlLocale())
}

function amountTone(entry: WalletLedgerEntry): 'is-positive' | 'is-negative' | 'is-neutral' {
  const sign = decimalSign(entry.amount)
  if (sign > 0) return 'is-positive'
  if (sign < 0) return 'is-negative'
  return 'is-neutral'
}

function signedAmount(entry: WalletLedgerEntry): string {
  return `${walletLedgerAmountSign(entry.amount)}${ledgerDecimal(entry.amount, entry.precisionScale)} ${entry.symbol}`
}

function ledgerDecimal(value: DecimalText, precisionScale: number): string {
  void locale.value
  return formatWalletLedgerDecimal(value, currentIntlLocale(), precisionScale)
}

function resetSessionState(): void {
  entries.value = []
  nextOffset.value = 0
  exhausted.value = false
  error.value = ''
  loading.value = false
  loadingMore.value = false
}

watch(() => session.token, (token) => {
  requestLifecycle.invalidate()
  resetSessionState()
  if (token) void load()
}, { immediate: true })

onBeforeUnmount(() => requestLifecycle.stop())
</script>

<template>
  <main
    class="page page--plain pencil-page wallet-pencil-page wallet-ledger-pencil"
    data-pencil-source="y6Y7TW m25xr0 Bcug6 IVMAO"
  >
    <PageHeader
      :back="true"
      :eyebrow="t('assets.title')"
      :fallback="{ name: 'assets' }"
      :pencil="true"
      :subtitle="session.isAuthenticated ? t('ledger.description') : t('ledger.loginDescription')"
      :title="t('assets.fundLedger')"
    >
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('ledger.refresh')"
          :disabled="loading || loadingMore"
          @click="load()"
        >
          <RefreshCw :size="18" :class="{ spin: loading }" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content ledger-page">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="wallet-login-prompt"
        :description="t('ledger.loginDescription')"
      />
      <template v-else>
        <nav class="ledger-account-filter" :aria-label="t('ledger.accountFilterLabel')">
          <button
            v-for="accountType in WALLET_LEDGER_ACCOUNT_FILTERS"
            :key="accountType"
            type="button"
            :aria-pressed="activeAccountType === accountType"
            :class="{ 'is-active': activeAccountType === accountType }"
            @click="changeAccountType(accountType)"
          >
            {{ accountLabel(accountType) }}
          </button>
        </nav>
        <nav class="ledger-filter" :aria-label="t('ledger.filterLabel')">
          <button
            v-for="filter in WALLET_LEDGER_FILTERS"
            :key="filter"
            type="button"
            :aria-pressed="activeFilter === filter"
            :class="{ 'is-active': activeFilter === filter }"
            @click="changeFilter(filter)"
          >
            {{ filterLabel(filter) }}
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
        <div v-else-if="groupedEntries.length" class="ledger-groups">
          <section
            v-for="group in groupedEntries"
            :key="group.key"
            class="ledger-group"
            :aria-labelledby="`ledger-group-${group.key}`"
          >
            <header class="ledger-group__header">
              <h2 :id="`ledger-group-${group.key}`">{{ groupHeading(group) }}</h2>
              <span>{{ t('ledger.groupCount', group.entries.length) }}</span>
            </header>
            <div class="ledger-list" role="list">
              <article
                v-for="entry in group.entries"
                :key="walletLedgerEntryIdentity(entry)"
                class="ledger-row"
                role="listitem"
              >
                <div class="ledger-row__title">
                  <div class="ledger-row__heading">
                    <strong>{{ entryLabel(entry) }}</strong>
                    <span
                      class="ledger-row__account"
                      :data-account-type="entry.accountType"
                    >{{ accountLabel(entry.accountType) }}</span>
                    <span class="ledger-row__category">{{ categoryLabel(entry.category) }}</span>
                  </div>
                  <small class="ledger-row__meta">{{ entry.symbol }} · {{ entryTime(entry) }}</small>
                  <small v-if="entrySource(entry)" class="ledger-row__source">
                    {{ t('ledger.sourceType', { type: entrySource(entry) }) }}
                  </small>
                </div>
                <div class="ledger-row__amount">
                  <strong class="numeric" :class="amountTone(entry)">{{ signedAmount(entry) }}</strong>
                  <small class="numeric">
                    {{ t('ledger.balance', { amount: `${ledgerDecimal(entry.balanceAfter, entry.precisionScale)} ${entry.symbol}` }) }}
                  </small>
                  <small v-if="decimalSign(entry.fee) > 0" class="numeric ledger-row__fee">
                    {{ t('ledger.fee', { amount: ledgerDecimal(entry.fee, entry.precisionScale), symbol: entry.symbol }) }}
                  </small>
                </div>
              </article>
            </div>
          </section>
        </div>
        <div v-else class="ledger-state ledger-state--empty" role="status">
          <span class="ledger-state__plate"><FileSearch :size="24" aria-hidden="true" /></span>
          <strong>{{ t('ledger.empty') }}</strong>
          <span>{{ t('ledger.emptyDescription') }}</span>
        </div>

        <div v-if="error && entries.length" class="ledger-inline-error" role="alert">
          <CircleAlert :size="16" aria-hidden="true" />
          <span>{{ error }}</span>
          <button type="button" :aria-label="t('common.retry')" :disabled="loading || loadingMore" @click="load()">
            <RefreshCw :size="16" aria-hidden="true" />
          </button>
        </div>
        <button
          v-if="!loading && !exhausted && entries.length"
          class="button button--secondary button--full ledger-load-more"
          type="button"
          :aria-busy="loadingMore"
          :disabled="loadingMore"
          @click="load(false)"
        >
          {{ loadingMore ? t('common.loading') : t('common.loadMore') }}
        </button>
      </template>
    </div>
  </main>
</template>

<style scoped>
.wallet-pencil-page {
  background: var(--page);
}

.ledger-page {
  display: grid;
  gap: 12px;
  min-width: 0;
  overflow-x: hidden;
  padding: 6px 20px calc(20px + env(safe-area-inset-bottom));
}

.ledger-filter {
  align-items: center;
  display: flex;
  gap: 8px;
  margin-inline: -20px;
  min-height: 44px;
  min-width: 0;
  overflow-x: auto;
  overscroll-behavior-inline: contain;
  padding-inline: 20px;
  scrollbar-width: none;
  scroll-snap-type: inline proximity;
}

.ledger-account-filter {
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: var(--wallet-pill-radius, 999px);
  display: grid;
  gap: 4px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  min-height: 44px;
  padding: 3px;
}

.ledger-account-filter button {
  background: transparent;
  border: 0;
  border-radius: var(--wallet-pill-radius, 999px);
  color: var(--muted);
  font-size: 12px;
  font-weight: 580;
  min-height: 44px;
  min-width: 0;
  padding: 0 8px;
}

.ledger-account-filter .is-active {
  background: var(--accent-soft);
  color: var(--positive);
  font-weight: 680;
}

.ledger-filter::-webkit-scrollbar {
  display: none;
}

.ledger-filter button {
  align-items: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--wallet-pill-radius, 999px);
  color: var(--muted);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 12px;
  font-weight: 560;
  height: 44px;
  justify-content: center;
  min-height: 44px;
  padding: 0 14px;
  scroll-snap-align: start;
  white-space: nowrap;
}

.ledger-account-filter button:focus-visible,
.ledger-filter button:focus-visible,
.ledger-state--error button:focus-visible,
.ledger-inline-error button:focus-visible,
.ledger-load-more:focus-visible {
  box-shadow: 0 0 0 2px var(--focus-ring);
  outline: 0;
}

.ledger-filter .is-active {
  background: var(--accent-soft);
  border-color: var(--positive);
  color: var(--positive);
  font-weight: 650;
}

.ledger-filter button:disabled:not(.is-active) {
  opacity: .56;
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

.ledger-state--error button {
  background: transparent;
  border: 1px solid var(--line);
  border-radius: var(--wallet-pill-radius, 999px);
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
  grid-template-columns: 18px minmax(0, 1fr) 44px;
  min-height: 44px;
  padding-left: 10px;
}

.ledger-inline-error span {
  min-width: 0;
  overflow-wrap: anywhere;
}

.ledger-inline-error button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  padding: 0;
  place-items: center;
}

.ledger-groups,
.ledger-group,
.ledger-list {
  display: grid;
  min-width: 0;
}

.ledger-groups {
  gap: 18px;
}

.ledger-group__header {
  align-items: baseline;
  display: flex;
  gap: 10px;
  justify-content: space-between;
  min-width: 0;
  padding: 2px 0 8px;
}

.ledger-group__header h2 {
  color: var(--ink);
  font-size: 14px;
  font-weight: 680;
  line-height: 20px;
  margin: 0;
  min-width: 0;
  overflow-wrap: anywhere;
}

.ledger-group__header span {
  color: var(--muted);
  flex: 0 0 auto;
  font-size: 11px;
  line-height: 16px;
}

.ledger-list {
  border-top: 1px solid var(--hairline);
}

.ledger-row {
  align-items: center;
  border-bottom: 1px solid var(--hairline);
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) minmax(104px, 42%);
  min-height: 84px;
  padding: 10px 0;
  width: 100%;
}

.ledger-row__title,
.ledger-row__amount {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.ledger-row__heading {
  align-items: center;
  display: flex;
  gap: 6px;
  min-width: 0;
}

.ledger-row strong {
  font-size: 13px;
  line-height: 18px;
  min-width: 0;
  overflow-wrap: anywhere;
}

.ledger-row__heading strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__account,
.ledger-row__category {
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: var(--wallet-pill-radius, 999px);
  color: var(--muted-strong);
  flex: 0 0 auto;
  font-size: 9px;
  line-height: 18px;
  max-width: 72px;
  overflow: hidden;
  padding: 0 6px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__account[data-account-type='margin'] {
  background: var(--accent-soft);
  border-color: var(--positive);
  color: var(--positive);
}

.ledger-row small {
  color: var(--muted);
  font-size: 11px;
  line-height: 15px;
  min-width: 0;
}

.ledger-row__meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__source {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  overflow-wrap: anywhere;
}

.ledger-row__amount {
  text-align: right;
}

.ledger-row__amount strong,
.ledger-row__amount small {
  overflow-wrap: anywhere;
}

.ledger-row__amount .is-positive {
  color: var(--positive);
}

.ledger-row__amount .is-negative {
  color: var(--negative);
}

.ledger-row__amount .is-neutral {
  color: var(--ink);
}

.ledger-row__fee {
  color: var(--muted-strong);
}

.ledger-load-more {
  border-radius: var(--wallet-pill-radius, 999px);
  min-height: 44px;
  width: 100%;
}

.wallet-feedback {
  margin: 0;
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
  border-radius: var(--wallet-pill-radius, 999px);
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
  .ledger-page {
    padding-left: 16px;
    padding-right: 16px;
  }

  .ledger-filter {
    margin-inline: -16px;
    padding-inline: 16px;
  }

  .ledger-filter button {
    font-size: 11px;
    padding-inline: 12px;
  }

  .ledger-row {
    gap: 8px;
    grid-template-columns: minmax(0, 1fr) minmax(92px, 40%);
  }

  .ledger-row__heading {
    align-items: flex-start;
    flex-direction: column;
    gap: 2px;
  }

  .ledger-row__account,
  .ledger-row__category {
    max-width: 100%;
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
