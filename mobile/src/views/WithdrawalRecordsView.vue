<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { LoaderCircle, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchWithdrawalRecords, type WithdrawalRecord } from '@/api/wallet'
import { formatAmount, formatDateTime, shortAddress } from '@/core/format'
import { useSessionStore } from '@/stores/session'

type RecordFilter = 'all' | 'processing' | 'completed' | 'failed'

const recordFilters: RecordFilter[] = ['all', 'processing', 'completed', 'failed']
const session = useSessionStore()
const { t } = useI18n()
const records = ref<WithdrawalRecord[]>([])
const activeFilter = ref<RecordFilter>('all')
const loading = ref(false)
const error = ref('')

const sortedRecords = computed(() => [...records.value].sort((left, right) => right.createdAt - left.createdAt))
const filteredRecords = computed(() => {
  if (activeFilter.value === 'all') return sortedRecords.value
  return sortedRecords.value.filter((record) => recordFilter(record.status) === activeFilter.value)
})

const statusKeys: Record<string, string> = {
  pending_review: 'withdrawRecords.statusPendingReview',
  approved: 'withdrawRecords.statusApproved',
  broadcasting: 'withdrawRecords.statusBroadcasting',
  broadcasted: 'withdrawRecords.statusBroadcasted',
  confirmed: 'withdrawRecords.statusConfirmed',
  manual_review: 'withdrawRecords.statusManualReview',
  rejected: 'withdrawRecords.statusRejected',
  failed: 'withdrawRecords.statusFailed',
}

function statusLabel(status: string): string {
  return statusKeys[status] ? t(statusKeys[status]) : status
}

function statusTone(status: string): string {
  if (status === 'confirmed') return 'is-positive'
  if (status === 'rejected' || status === 'failed') return 'is-negative'
  if (status === 'approved' || status === 'broadcasting' || status === 'broadcasted') return 'is-progress'
  return 'is-pending'
}

function recordFilter(status: string): Exclude<RecordFilter, 'all'> {
  if (status === 'confirmed') return 'completed'
  if (status === 'rejected' || status === 'failed') return 'failed'
  return 'processing'
}

function filterLabel(filter: RecordFilter): string {
  if (filter === 'all') return t('common.all')
  if (filter === 'completed') return t('withdrawRecords.statusConfirmed')
  if (filter === 'failed') return t('withdrawRecords.statusFailed')
  return t('withdrawRecords.statusBroadcasting')
}

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    records.value = await fetchWithdrawalRecords()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('withdrawRecords.loadFailed'))
  } finally {
    loading.value = false
  }
}

onMounted(() => { void load() })
</script>

<template>
  <main
    class="page page--plain pencil-page wallet-pencil-page withdrawal-records-pencil"
    data-pencil-source="DxqMB G3HecO"
  >
    <PageHeader
      :back="true"
      :eyebrow="t('assets.withdraw')"
      :fallback="{ name: 'assets' }"
      :pencil="true"
      :subtitle="t('withdrawRecords.loginDescription')"
      :title="t('withdrawRecords.title')"
    >
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('withdrawRecords.refresh')" :disabled="loading" @click="load()">
          <RefreshCw :size="18" :class="{ spin: loading }" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content records-page">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="wallet-login-prompt"
        :description="t('withdrawRecords.loginDescription')"
      />
      <template v-else>
        <nav class="records-tabs" :aria-label="t('common.status')">
          <button
            v-for="filter in recordFilters"
            :key="filter"
            type="button"
            :aria-pressed="activeFilter === filter"
            :class="{ 'is-active': activeFilter === filter }"
            @click="activeFilter = filter"
          >
            {{ filterLabel(filter) }}
          </button>
        </nav>
        <p v-if="error" class="error-message wallet-feedback" role="alert">{{ error }}</p>
        <div v-if="loading" class="records-loading" role="status">
          <LoaderCircle :size="23" class="spin" aria-hidden="true" />
          <span>{{ t('withdrawRecords.loading') }}</span>
        </div>
        <div v-else-if="filteredRecords.length" class="records-list" role="list">
          <article v-for="record in filteredRecords" :key="record.id" class="record-row" role="listitem">
            <AssetMark :symbol="record.assetSymbol" :size="36" />
            <div class="record-row__copy">
              <strong>
                <span>{{ record.assetSymbol }}</span>
                <span class="numeric">{{ formatAmount(record.amount) }} {{ record.assetSymbol }}</span>
              </strong>
              <small>{{ record.network || t('withdraw.reviewedNetwork') }} · {{ shortAddress(record.address) }}</small>
              <small>{{ formatDateTime(record.createdAt) }} · {{ t('common.fee') }} {{ formatAmount(record.fee) }} {{ record.assetSymbol }}</small>
              <small v-if="record.txHash" class="numeric">{{ t('withdrawRecords.txHash') }} · {{ shortAddress(record.txHash) }}</small>
              <small v-if="record.failureReason || record.reviewReason" class="record-row__reason">{{ record.failureReason || record.reviewReason }}</small>
            </div>
            <b class="record-row__status" :class="statusTone(record.status)">{{ statusLabel(record.status) }}</b>
          </article>
        </div>
        <p v-else class="empty-state">{{ t('withdrawRecords.empty') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.wallet-pencil-page {
  background: var(--page);
}

.records-page {
  display: grid;
  gap: 10px;
  padding: 6px 20px calc(20px + env(safe-area-inset-bottom));
}

.records-tabs {
  align-items: flex-start;
  display: flex;
  gap: 20px;
  min-height: 34px;
  overflow-x: auto;
  scrollbar-width: none;
}

.records-tabs::-webkit-scrollbar {
  display: none;
}

.records-tabs button {
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  border-radius: 0;
  color: var(--muted);
  flex: 0 0 auto;
  font-size: 13px;
  font-weight: 500;
  height: 28px;
  min-height: 28px;
  padding: 0 0 7px;
  position: relative;
}

.records-tabs button::before {
  content: '';
  inset: -8px 0;
  position: absolute;
}

.records-tabs button.is-active {
  border-bottom-color: var(--accent);
  color: var(--ink);
  font-weight: 400;
}

.records-loading {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 180px;
}

.records-list {
  display: grid;
}

.record-row {
  align-items: center;
  border-bottom: 1px solid var(--hairline);
  display: grid;
  gap: 12px;
  grid-template-columns: 36px minmax(0, 1fr) auto;
  min-height: 64px;
  padding: 8px 0;
}

.record-row__copy {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.record-row__copy > strong {
  align-items: center;
  display: flex;
  font-size: 14px;
  gap: 8px;
  justify-content: space-between;
  line-height: 20px;
  min-width: 0;
}

.record-row__copy > strong span:last-child {
  font-size: 11px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.record-row__copy small {
  color: var(--muted);
  font-size: 10px;
  line-height: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.record-row__status {
  border: 0;
  border-radius: 0;
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 400;
  line-height: 15px;
  padding: 0;
  white-space: nowrap;
}

.record-row__status.is-positive {
  background: transparent;
  color: var(--positive);
}

.record-row__status.is-negative {
  background: transparent;
  color: var(--negative);
}

.record-row__status.is-progress {
  background: transparent;
  color: var(--signal-blue);
}

.record-row__status.is-pending {
  background: transparent;
  color: var(--muted-strong);
}

.record-row__reason {
  color: var(--negative) !important;
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
  .records-page {
    padding-left: 16px;
    padding-right: 16px;
  }

  .records-tabs {
    gap: 16px;
  }

  .record-row {
    gap: 9px;
  }

  .wallet-login-prompt {
    align-items: center;
    grid-template-columns: 34px minmax(0, 1fr);
  }

  .wallet-login-prompt :deep(.button) {
    grid-column: 2;
    justify-self: start;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
