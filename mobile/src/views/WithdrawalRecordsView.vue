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

const session = useSessionStore()
const { t } = useI18n()
const records = ref<WithdrawalRecord[]>([])
const loading = ref(false)
const error = ref('')

const sortedRecords = computed(() => [...records.value].sort((left, right) => right.createdAt - left.createdAt))

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
  <main class="page page--plain">
    <PageHeader :title="t('withdrawRecords.title')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('withdrawRecords.refresh')" :disabled="loading" @click="load()">
          <RefreshCw :size="21" :class="{ spin: loading }" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content records-page">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('withdrawRecords.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message" role="alert">{{ error }}</p>
        <div v-if="loading" class="records-loading" role="status">
          <LoaderCircle :size="23" class="spin" aria-hidden="true" />
          <span>{{ t('withdrawRecords.loading') }}</span>
        </div>
        <div v-else-if="sortedRecords.length" class="records-list" role="list">
          <article v-for="record in sortedRecords" :key="record.id" class="record-row" role="listitem">
            <header>
              <span class="record-row__asset"><AssetMark :symbol="record.assetSymbol" :size="34" /><strong class="numeric">{{ formatAmount(record.amount) }} {{ record.assetSymbol }}</strong></span>
              <b class="record-row__status" :class="statusTone(record.status)">{{ statusLabel(record.status) }}</b>
            </header>
            <dl>
              <div v-if="record.network"><dt>{{ t('withdrawRecords.network') }}</dt><dd>{{ record.network }}</dd></div>
              <div><dt>{{ t('withdrawRecords.address') }}</dt><dd class="numeric">{{ shortAddress(record.address) }}</dd></div>
              <div><dt>{{ t('common.fee') }}</dt><dd class="numeric">{{ formatAmount(record.fee) }} {{ record.assetSymbol }}</dd></div>
              <div><dt>{{ t('common.time') }}</dt><dd>{{ formatDateTime(record.createdAt) }}</dd></div>
              <div v-if="record.txHash"><dt>{{ t('withdrawRecords.txHash') }}</dt><dd class="numeric record-row__hash">{{ record.txHash }}</dd></div>
              <div v-if="record.failureReason || record.reviewReason"><dt>{{ t('withdrawRecords.reason') }}</dt><dd>{{ record.failureReason || record.reviewReason }}</dd></div>
            </dl>
          </article>
        </div>
        <p v-else class="empty-state">{{ t('withdrawRecords.empty') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.records-page {
  padding-bottom: calc(36px + env(safe-area-inset-bottom));
  padding-top: 8px;
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
  border-top: 1px solid var(--line);
  display: grid;
}

.record-row {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 13px;
  padding: 16px 0;
}

.record-row header {
  align-items: center;
  display: flex;
  gap: 10px;
  justify-content: space-between;
  min-width: 0;
}

.record-row__asset {
  align-items: center;
  display: flex;
  gap: 10px;
  min-width: 0;
}

.record-row__asset strong {
  font-size: 15px;
  min-width: 0;
  overflow-wrap: anywhere;
}

.record-row__status {
  border: 1px solid var(--line);
  border-radius: 999px;
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 750;
  line-height: 1;
  padding: 7px 9px;
  white-space: nowrap;
}

.record-row__status.is-positive {
  background: var(--positive-soft);
  border-color: color-mix(in srgb, var(--positive) 28%, var(--line));
  color: var(--positive);
}

.record-row__status.is-negative {
  background: var(--negative-soft);
  border-color: color-mix(in srgb, var(--negative) 28%, var(--line));
  color: var(--negative);
}

.record-row__status.is-progress {
  background: var(--accent-soft);
  border-color: color-mix(in srgb, var(--accent) 28%, var(--line));
  color: var(--accent);
}

.record-row__status.is-pending {
  background: var(--soft);
  color: var(--muted-strong);
}

.record-row dl {
  display: grid;
  gap: 8px;
  margin: 0;
}

.record-row dl div {
  display: grid;
  font-size: 12px;
  gap: 12px;
  grid-template-columns: minmax(72px, auto) minmax(0, 1fr);
}

.record-row dt {
  color: var(--muted);
}

.record-row dd {
  color: var(--muted-strong);
  margin: 0;
  min-width: 0;
  overflow-wrap: anywhere;
  text-align: right;
}

.record-row__hash {
  font-size: 11px;
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

  .record-row header {
    align-items: flex-start;
  }

  .record-row__asset strong {
    font-size: 14px;
  }

  .record-row dl div {
    gap: 8px;
    grid-template-columns: minmax(64px, auto) minmax(0, 1fr);
  }
}
</style>
