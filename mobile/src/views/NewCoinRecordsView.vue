<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { CheckCircle2, CircleAlert, LoaderCircle, PackageOpen, RefreshCw, SlidersHorizontal, X } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import NewCoinRecordCard from '@/components/new-coin/NewCoinRecordCard.vue'
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
  type NewCoinProject,
  type NewCoinPurchase,
  type NewCoinSubscription,
  type NewCoinUnlock,
} from '@/api/newCoin'
import { fetchWalletAccounts } from '@/api/wallet'
import { decimalCompare, decimalTextFromBoundary, type DecimalText } from '@/core/decimal'
import { formatFinancialAmount } from '@/core/financialDisplay'
import { useModalDialog } from '@/core/modalDialog'
import {
  buildUnifiedNewCoinRecords,
  filterUnifiedNewCoinRecords,
  type NewCoinRecordStatusFilter,
  type NewCoinRecordTypeFilter,
  type UnifiedNewCoinRecord,
} from '@/core/newCoinPresentation'
import type { WalletAccount } from '@/core/types'
import { useSessionStore } from '@/stores/session'

const router = useRouter()
const { locale, t } = useI18n()
const session = useSessionStore()
const projects = ref<NewCoinProject[]>([])
const subscriptions = ref<NewCoinSubscription[]>([])
const distributions = ref<NewCoinDistribution[]>([])
const purchases = ref<NewCoinPurchase[]>([])
const unlocks = ref<NewCoinUnlock[]>([])
const accounts = ref<WalletAccount[]>([])
const statusFilter = ref<NewCoinRecordStatusFilter>('all')
const typeFilter = ref<NewCoinRecordTypeFilter>('all')
const loading = ref(false)
const error = ref('')
const success = ref('')
const saving = ref('')
const pendingUnlock = ref<NewCoinUnlock | null>(null)
const feeDialog = ref<HTMLElement | null>(null)
const typeSheetOpen = ref(false)
const typeDialog = ref<HTMLElement | null>(null)

const feeOpen = computed(() => pendingUnlock.value !== null)
const { trapFocus: trapFeeFocus, setReturnFocus: setFeeReturnFocus } = useModalDialog(
  feeOpen,
  feeDialog,
  '[data-dialog-cancel]',
)
const { trapFocus: trapTypeFocus, setReturnFocus: setTypeReturnFocus } = useModalDialog(
  typeSheetOpen,
  typeDialog,
  '[data-dialog-initial]',
)
const statusFilters: ReadonlyArray<{ key: NewCoinRecordStatusFilter; label: string }> = [
  { key: 'all', label: 'common.all' },
  { key: 'inProgress', label: 'newCoin.inProgress' },
  { key: 'pendingSettlement', label: 'newCoin.pendingSettlement' },
  { key: 'completed', label: 'newCoin.completed' },
]
const typeFilters: ReadonlyArray<{ key: NewCoinRecordTypeFilter; label: string }> = [
  { key: 'all', label: 'common.all' },
  { key: 'subscriptions', label: 'newCoin.tabSubscriptions' },
  { key: 'distributions', label: 'newCoin.tabDistributions' },
  { key: 'purchases', label: 'newCoin.tabPurchases' },
  { key: 'unlocks', label: 'newCoin.tabUnlocks' },
]
const records = computed(() => buildUnifiedNewCoinRecords({
  projects: projects.value,
  subscriptions: subscriptions.value,
  distributions: distributions.value,
  purchases: purchases.value,
  unlocks: unlocks.value,
}))
const visibleRecords = computed(() => filterUnifiedNewCoinRecords(
  records.value,
  typeFilter.value,
  statusFilter.value,
))
const hasCachedRecords = computed(() => records.value.length > 0)
const paymentAccount = computed(() => accounts.value.find(
  (account) => account.assetId === pendingUnlock.value?.unlockFeeAssetId,
))
const paymentAmountText = computed<DecimalText | null>(() => pendingUnlock.value?.unlockFeeAmountText || null)
const paymentAvailableText = computed(() => decimalTextFromBoundary(
  paymentAccount.value?.availableText,
  { allowNegative: false },
))

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = !hasCachedRecords.value
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
    projects.value = nextProjects
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

function openTypeSheet(event: MouseEvent): void {
  setTypeReturnFocus(event.currentTarget instanceof HTMLElement ? event.currentTarget : null)
  typeSheetOpen.value = true
}

function closeTypeSheet(): void {
  typeSheetOpen.value = false
}

function selectType(next: NewCoinRecordTypeFilter): void {
  typeFilter.value = next
  closeTypeSheet()
}

function openProject(record: UnifiedNewCoinRecord): void {
  if (!record.project) return
  void router.push({ name: 'new-coin-detail', params: { symbol: record.project.symbol } })
}

function openFeePayment(record: UnifiedNewCoinRecord, event?: MouseEvent): void {
  const unlock = record.unlock
  if (!unlock?.unlockFeeAmountText || !unlock.unlockFeeAssetId) {
    error.value = t('newCoin.invalidFeeConfig')
    return
  }
  setFeeReturnFocus(event?.currentTarget instanceof HTMLElement ? event.currentTarget : null)
  pendingUnlock.value = unlock
  error.value = ''
}

function closeFeePayment(): void {
  if (saving.value) return
  pendingUnlock.value = null
  error.value = ''
}

async function confirmFeePayment(): Promise<void> {
  if (!pendingUnlock.value || !paymentAmountText.value || !pendingUnlock.value.unlockFeeAssetId) {
    error.value = t('newCoin.invalidFeeConfig')
    return
  }
  if (!paymentAvailableText.value || decimalCompare(paymentAmountText.value, paymentAvailableText.value) > 0) {
    error.value = t('newCoin.insufficientFeeBalance')
    return
  }
  saving.value = `fee-${pendingUnlock.value.id}`
  error.value = ''
  try {
    await payNewCoinUnlockFee({
      idempotencyKey: pendingUnlock.value.idempotencyKey,
      paymentAssetId: pendingUnlock.value.unlockFeeAssetId,
      amount: paymentAmountText.value,
    })
    success.value = t('newCoin.feePaid')
    pendingUnlock.value = null
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('newCoin.feePaymentFailed'))
  } finally {
    saving.value = ''
  }
}

async function release(record: UnifiedNewCoinRecord): Promise<void> {
  const unlock = record.unlock
  if (!unlock) return
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

function formatMoney(value: DecimalText | null, asset?: string): string {
  return value
    ? formatFinancialAmount(value, locale.value, { assetSymbol: asset })
    : t('newCoin.unavailableValue')
}

function handleFeeKeydown(event: KeyboardEvent): void {
  trapFeeFocus(event, closeFeePayment)
}

function handleTypeKeydown(event: KeyboardEvent): void {
  trapTypeFocus(event, closeTypeSheet)
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain pencil-page new-coin-records-page" data-pencil-source="A9It6g h4gfd">
    <PageHeader :back="true" :pencil="true" back-icon="chevron" :title="t('newCoin.recordTitle')">
      <template #actions>
        <button
          class="icon-button new-coin-records-filter-button"
          type="button"
          :aria-label="t('newCoin.filterRecordType')"
          :aria-pressed="typeFilter !== 'all'"
          @click="openTypeSheet"
        >
          <span><SlidersHorizontal :size="17" /></span>
        </button>
      </template>
    </PageHeader>

    <LoginRequiredState v-if="!session.isAuthenticated" :description="t('newCoin.recordLoginDescription')" />
    <template v-else>
      <nav class="new-coin-record-status-filters" :aria-label="t('newCoin.recordStatusFilters')">
        <button
          v-for="filter in statusFilters"
          :key="filter.key"
          type="button"
          :aria-pressed="statusFilter === filter.key"
          @click="statusFilter = filter.key"
        >
          <span>{{ t(filter.label) }}</span>
        </button>
      </nav>

      <div v-if="success" class="new-coin-record-feedback" role="status"><CheckCircle2 :size="16" /><span>{{ success }}</span></div>
      <div v-if="error && hasCachedRecords" class="new-coin-record-feedback new-coin-record-feedback--error" role="alert">
        <CircleAlert :size="16" /><span>{{ error }}</span><button type="button" :aria-label="t('common.retry')" @click="load"><RefreshCw :size="15" /></button>
      </div>

      <section class="new-coin-record-list">
        <div v-if="loading" class="new-coin-record-state" aria-live="polite">
          <LoaderCircle :size="24" class="spin" /><span>{{ t('newCoin.loadingRecords') }}</span>
        </div>
        <div v-else-if="error && !hasCachedRecords" class="new-coin-record-state" role="alert">
          <CircleAlert :size="24" /><span>{{ error }}</span><button type="button" @click="load">{{ t('common.retry') }}</button>
        </div>
        <div v-else-if="visibleRecords.length" class="new-coin-record-stack">
          <NewCoinRecordCard
            v-for="record in visibleRecords"
            :key="record.key"
            :record="record"
            :saving="saving === `fee-${record.id}` || saving === `release-${record.id}`"
            @open="openProject(record)"
            @pay-fee="openFeePayment(record, $event)"
            @release="release(record)"
          />
        </div>
        <div v-else class="new-coin-record-state">
          <PackageOpen :size="24" /><span>{{ t('newCoin.noMatchingRecords') }}</span>
        </div>
      </section>
    </template>

    <Teleport to="body">
      <div v-if="typeSheetOpen" class="new-coin-record-dialog-mask new-coin-record-dialog-layer" @click.self="closeTypeSheet">
        <section
          ref="typeDialog"
          class="new-coin-record-sheet"
          role="dialog"
          aria-modal="true"
          aria-labelledby="new-coin-type-filter-title"
          @keydown="handleTypeKeydown"
        >
          <header>
            <h2 id="new-coin-type-filter-title">{{ t('newCoin.filterRecordType') }}</h2>
            <button type="button" :aria-label="t('common.close')" @click="closeTypeSheet"><X :size="20" /></button>
          </header>
          <div class="new-coin-record-type-options">
            <button
              v-for="(filter, index) in typeFilters"
              :key="filter.key"
              type="button"
              :data-dialog-initial="index === 0 ? '' : undefined"
              :aria-pressed="typeFilter === filter.key"
              @click="selectType(filter.key)"
            >
              {{ t(filter.label) }}
            </button>
          </div>
        </section>
      </div>

      <div v-if="pendingUnlock" class="new-coin-record-dialog-mask new-coin-record-dialog-layer" @click.self="closeFeePayment">
        <section
          ref="feeDialog"
          class="new-coin-record-sheet"
          role="dialog"
          aria-modal="true"
          aria-labelledby="new-coin-fee-title"
          aria-describedby="new-coin-fee-description"
          @keydown="handleFeeKeydown"
        >
          <header>
            <h2 id="new-coin-fee-title">{{ t('newCoin.payFee') }}</h2>
            <button type="button" :aria-label="t('common.close')" :disabled="Boolean(saving)" @click="closeFeePayment"><X :size="20" /></button>
          </header>
          <p id="new-coin-fee-description">{{ t('newCoin.feeDescription') }}</p>
          <dl>
            <div><dt>{{ t('newCoin.paymentAsset') }}</dt><dd>{{ paymentAccount?.symbol || t('newCoin.assetNumber', { id: pendingUnlock.unlockFeeAssetId }) }}</dd></div>
            <div><dt>{{ t('newCoin.unlockFee') }}</dt><dd>{{ formatMoney(paymentAmountText, paymentAccount?.symbol) }} {{ paymentAccount?.symbol || '' }}</dd></div>
            <div><dt>{{ t('newCoin.availableBalance') }}</dt><dd>{{ formatMoney(paymentAvailableText, paymentAccount?.symbol) }} {{ paymentAccount?.symbol || '' }}</dd></div>
          </dl>
          <p v-if="error" class="new-coin-record-sheet__error" role="alert">{{ error }}</p>
          <div class="new-coin-record-sheet__actions">
            <button type="button" data-dialog-cancel :disabled="Boolean(saving)" @click="closeFeePayment">{{ t('common.cancel') }}</button>
            <button type="button" :disabled="Boolean(saving) || !paymentAccount" :aria-busy="Boolean(saving)" @click="confirmFeePayment">
              {{ t(saving ? 'newCoin.paying' : 'newCoin.confirmPayment') }}
            </button>
          </div>
        </section>
      </div>
    </Teleport>
  </main>
</template>

<style scoped>
.new-coin-records-page {
  min-height: 100dvh;
  overflow-x: clip;
  padding-bottom: calc(20px + env(safe-area-inset-bottom));
}

.new-coin-records-page :deep(.pencil-page-header) {
  background: var(--new-coin-record-header);
  height: 58px;
  min-height: 58px;
  padding: 7px 16px;
}

.new-coin-records-page :deep(.page-header__title) {
  font-size: 22px;
  font-weight: 700;
  line-height: 32px;
}

.new-coin-records-page :deep(.new-coin-records-filter-button) {
  background: transparent !important;
  height: 44px !important;
  min-height: 44px !important;
  padding: 0 !important;
  width: 44px !important;
}

.new-coin-records-page :deep(.new-coin-records-filter-button > span) {
  align-items: center;
  background: var(--new-coin-record-filter-face);
  border-radius: 50%;
  display: flex;
  height: 34px;
  justify-content: center;
  width: 34px;
}

.new-coin-record-status-filters {
  display: grid;
  gap: 6px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  height: 56px;
  padding: 8px 16px;
}

.new-coin-record-status-filters button {
  background: transparent;
  border: 0;
  color: var(--new-coin-record-muted);
  font-size: 12px;
  height: 44px;
  margin-top: -2px;
  min-width: 0;
  padding: 2px 0;
}

.new-coin-record-status-filters button span {
  align-items: center;
  background: var(--new-coin-record-filter);
  border-radius: 20px;
  display: flex;
  height: 40px;
  justify-content: center;
  overflow: hidden;
  padding: 0 5px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-record-status-filters button[aria-pressed='true'] span {
  background: var(--new-coin-record-filter-active);
  color: var(--new-coin-record-filter-active-ink);
  font-weight: 700;
}

.new-coin-record-list {
  padding: 10px 16px 20px;
}

.new-coin-record-stack {
  display: grid;
  gap: 14px;
  margin: 0 auto;
}

.new-coin-record-feedback {
  align-items: center;
  background: var(--new-coin-record-status);
  color: var(--new-coin-record-active);
  display: flex;
  font-size: 10px;
  gap: 6px;
  min-height: 34px;
  padding: 4px 16px;
}

.new-coin-record-feedback span {
  flex: 1;
}

.new-coin-record-feedback--error {
  background: var(--negative-soft);
  color: var(--negative);
}

.new-coin-record-feedback button {
  background: transparent;
  border: 0;
  color: inherit;
  height: 44px;
  margin-block: -8px;
  width: 44px;
}

.new-coin-record-state {
  align-items: center;
  color: var(--new-coin-record-muted);
  display: flex;
  flex-direction: column;
  gap: 10px;
  justify-content: center;
  min-height: 300px;
  text-align: center;
}

.new-coin-record-state button {
  background: var(--new-coin-record-filter-active);
  border: 0;
  border-radius: 13px;
  color: var(--new-coin-record-filter-active-ink);
  min-height: 44px;
  padding: 0 18px;
}

.new-coin-record-dialog-mask {
  align-items: end;
  background: var(--overlay);
  display: grid;
  inset: 0;
  position: fixed;
  z-index: var(--layer-overlay);
}

.new-coin-record-sheet {
  background: var(--new-coin-record-card);
  border-radius: 24px 24px 0 0;
  box-sizing: border-box;
  color: var(--new-coin-record-ink);
  padding: 18px 16px calc(18px + env(safe-area-inset-bottom));
  width: 100%;
}

.new-coin-record-sheet:focus-within {
  box-shadow: 0 0 0 2px var(--focus-ring);
}

.new-coin-record-sheet > header {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.new-coin-record-sheet h2 {
  font-size: 18px;
  margin: 0;
}

.new-coin-record-sheet header button {
  align-items: center;
  background: transparent;
  border: 0;
  color: inherit;
  display: flex;
  height: 44px;
  justify-content: center;
  width: 44px;
}

.new-coin-record-sheet > p {
  color: var(--new-coin-record-muted);
  font-size: 11px;
}

.new-coin-record-type-options {
  display: grid;
  gap: 8px;
  margin-top: 14px;
}

.new-coin-record-type-options button {
  background: var(--new-coin-record-filter);
  border: 0;
  border-radius: 14px;
  color: var(--new-coin-record-ink);
  min-height: 48px;
}

.new-coin-record-type-options button[aria-pressed='true'] {
  background: var(--new-coin-record-filter-active);
  color: var(--new-coin-record-filter-active-ink);
}

.new-coin-record-sheet dl {
  display: grid;
  gap: 10px;
  margin: 16px 0;
}

.new-coin-record-sheet dl div {
  align-items: center;
  display: flex;
  font-size: 11px;
  justify-content: space-between;
}

.new-coin-record-sheet dt {
  color: var(--new-coin-record-muted);
}

.new-coin-record-sheet dd {
  font-family: var(--font-numeric);
  margin: 0;
}

.new-coin-record-sheet__error {
  color: var(--negative) !important;
}

.new-coin-record-sheet__actions {
  display: grid;
  gap: 10px;
  grid-template-columns: 1fr 1fr;
}

.new-coin-record-sheet__actions button {
  background: var(--new-coin-record-filter);
  border: 0;
  border-radius: 14px;
  color: var(--new-coin-record-ink);
  min-height: 48px;
}

.new-coin-record-sheet__actions button:last-child {
  background: var(--new-coin-record-filter-active);
  color: var(--new-coin-record-filter-active-ink);
}

@media (max-width: 340px) {
  .new-coin-record-status-filters {
    gap: 3px;
    padding-inline: 10px;
  }

  .new-coin-record-status-filters button {
    font-size: 10px;
    padding-inline: 2px;
  }

  .new-coin-record-list {
    padding-inline: 16px;
  }
}
</style>
