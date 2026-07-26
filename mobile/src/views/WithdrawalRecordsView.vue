<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RefreshCw } from 'lucide-vue-next'
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
  if (status === 'confirmed') return 'up'
  if (status === 'rejected' || status === 'failed') return 'down'
  return ''
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
    <PageHeader :title="t('withdrawRecords.title')"><template #actions><button class="icon-button" type="button" :aria-label="t('withdrawRecords.refresh')" :disabled="loading" @click="load()"><RefreshCw :size="21" :class="{ spin: loading }" /></button></template></PageHeader>
    <div class="page-content records-page">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('withdrawRecords.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p>
        <p v-if="loading" class="empty-state">{{ t('withdrawRecords.loading') }}</p>
        <div v-else-if="sortedRecords.length" class="records-list">
          <article v-for="record in sortedRecords" :key="record.id" class="record-row">
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
.records-page { padding-bottom: 36px; }.records-list { display: grid; gap: 12px; padding-top: 8px; }.record-row { background: var(--soft); border-radius: var(--radius); display: grid; gap: 10px; padding: 14px; }.record-row header { align-items: center; display: flex; gap: 10px; justify-content: space-between; }.record-row__asset { align-items: center; display: flex; gap: 10px; min-width: 0; }.record-row__asset strong { font-size: 15px; }.record-row__status { font-size: 13px; font-weight: 750; white-space: nowrap; }.record-row dl { display: grid; gap: 6px; margin: 0; }.record-row dl div { display: grid; font-size: 12px; gap: 12px; grid-template-columns: auto 1fr; }.record-row dt { color: var(--muted); }.record-row dd { margin: 0; min-width: 0; overflow-wrap: anywhere; text-align: right; }.record-row__hash { font-size: 11px; }.spin { animation: spin .8s linear infinite; }@keyframes spin { to { transform: rotate(360deg); } }
</style>
