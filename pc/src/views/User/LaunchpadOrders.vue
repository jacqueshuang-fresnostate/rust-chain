<template>
  <div class="flex flex-col gap-5">
    <div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
      <div>
        <h2 class="text-2xl font-bold text-foreground">{{ t('launchpad.records_title') }}</h2>
        <p class="text-sm text-muted-foreground">{{ t('launchpad.records_desc') }}</p>
      </div>
      <button class="rounded-lg bg-muted px-4 py-2 text-sm font-semibold text-foreground hover:bg-muted/80" :disabled="loading" @click="loadRecords">
        {{ t('launchpad.refresh') }}
      </button>
    </div>

    <div class="rounded-xl border border-border bg-card">
      <div class="flex gap-6 border-b border-border px-5">
        <button
          v-for="tab in tabs"
          :key="tab.value"
          type="button"
          class="border-b-2 px-1 py-4 text-sm font-bold transition-colors"
          :class="activeTab === tab.value ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'"
          @click="activeTab = tab.value"
        >
          {{ t(tab.labelKey) }}
        </button>
      </div>

      <div class="overflow-x-auto">
        <div v-if="loading" class="px-4 py-10 text-center text-muted-foreground">{{ t('common.loading') }}</div>
        <div v-else-if="loadError" class="px-4 py-10 text-center text-rose-400">{{ loadError }}</div>

        <table v-else-if="activeTab === 'subscriptions'" class="w-full text-sm">
          <thead>
            <tr class="border-b border-border text-left text-muted-foreground">
              <th class="px-4 py-3">{{ t('launchpad.record_id') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_paid') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_requested') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_allocated') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_status') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_time') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="subscriptions.length === 0"><td colspan="6" class="px-4 py-10 text-center text-muted-foreground">{{ t('launchpad.records_empty') }}</td></tr>
            <tr v-for="record in subscriptions" :key="record.id" class="border-b border-border">
              <td class="px-4 py-4 font-mono">#{{ record.id }}</td>
              <td class="px-4 py-4 font-mono"><template v-if="record.settlementMode === 'manual_distribution'">
                  <span v-if="record.status === 'pending'">{{ t('launchpad.record_frozen') }}: {{ trim(record.frozenQuoteAmount || '0') }}</span>
                  <span v-else>{{ t('launchpad.record_paid') }}: {{ trim(record.settledQuoteAmount || '0') }} / {{ t('launchpad.record_refund') }}: {{ trim(record.refundedQuoteAmount || '0') }}</span>
                  {{ assetSymbol(record.quoteAsset) }}
                </template>
                <template v-else>{{ trim(record.quoteAmount) }} {{ assetSymbol(record.quoteAsset) }}</template></td>
              <td class="px-4 py-4 font-mono">{{ trim(record.requestedQuantity) }}</td>
              <td class="px-4 py-4 font-mono">{{ trim(record.allocatedQuantity) }}</td>
              <td class="px-4 py-4"><span class="rounded-full px-3 py-1 text-xs font-bold" :class="statusClass(record.status)">{{ statusText(record.status) }}</span></td>
              <td class="px-4 py-4 text-muted-foreground">{{ formatTime(record.createdAt) }}</td>
            </tr>
          </tbody>
        </table>

        <table v-else-if="activeTab === 'distributions'" class="w-full text-sm">
          <thead>
            <tr class="border-b border-border text-left text-muted-foreground">
              <th class="px-4 py-3">{{ t('launchpad.record_id') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_quantity') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_locked') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_status') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_time') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="distributions.length === 0"><td colspan="5" class="px-4 py-10 text-center text-muted-foreground">{{ t('launchpad.records_empty') }}</td></tr>
            <tr v-for="record in distributions" :key="record.id" class="border-b border-border">
              <td class="px-4 py-4 font-mono">#{{ record.id }}</td>
              <td class="px-4 py-4 font-mono">{{ trim(record.quantity) }} {{ assetSymbol(record.assetId) }}</td>
              <td class="px-4 py-4 font-mono text-muted-foreground">{{ record.lockPositionId ? `#${record.lockPositionId}` : '--' }}</td>
              <td class="px-4 py-4"><span class="rounded-full px-3 py-1 text-xs font-bold" :class="statusClass(record.status)">{{ statusText(record.status) }}</span></td>
              <td class="px-4 py-4 text-muted-foreground">{{ formatTime(record.createdAt) }}</td>
            </tr>
          </tbody>
        </table>

        <table v-else-if="activeTab === 'purchases'" class="w-full text-sm">
          <thead>
            <tr class="border-b border-border text-left text-muted-foreground">
              <th class="px-4 py-3">{{ t('launchpad.record_id') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_price') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_quantity') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_paid') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_status') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_time') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="purchases.length === 0"><td colspan="6" class="px-4 py-10 text-center text-muted-foreground">{{ t('launchpad.records_empty') }}</td></tr>
            <tr v-for="record in purchases" :key="record.id" class="border-b border-border">
              <td class="px-4 py-4 font-mono">#{{ record.id }}</td>
              <td class="px-4 py-4 font-mono">{{ trim(record.price) }} {{ assetSymbol(record.quoteAsset) }}</td>
              <td class="px-4 py-4 font-mono">{{ trim(record.quantity) }} {{ assetSymbol(record.baseAsset) }}</td>
              <td class="px-4 py-4 font-mono">{{ trim(record.quoteAmount) }} {{ assetSymbol(record.quoteAsset) }}</td>
              <td class="px-4 py-4"><span class="rounded-full px-3 py-1 text-xs font-bold" :class="statusClass(record.status)">{{ statusText(record.status) }}</span></td>
              <td class="px-4 py-4 text-muted-foreground">{{ formatTime(record.createdAt) }}</td>
            </tr>
          </tbody>
        </table>

        <table v-else class="w-full text-sm">
          <thead>
            <tr class="border-b border-border text-left text-muted-foreground">
              <th class="px-4 py-3">{{ t('launchpad.record_id') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_quantity') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_fee') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_status') }}</th>
              <th class="px-4 py-3">{{ t('launchpad.record_time') }}</th>
              <th class="px-4 py-3 text-right">{{ t('launchpad.record_action') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="unlocks.length === 0"><td colspan="6" class="px-4 py-10 text-center text-muted-foreground">{{ t('launchpad.records_empty') }}</td></tr>
            <tr v-for="record in unlocks" :key="record.id" class="border-b border-border">
              <td class="px-4 py-4 font-mono">#{{ record.id }}</td>
              <td class="px-4 py-4 font-mono">{{ trim(record.unlockQuantity) }} {{ assetSymbol(record.assetId) }}</td>
              <td class="px-4 py-4 font-mono">{{ feeText(record) }}</td>
              <td class="px-4 py-4">
                <span class="rounded-full px-3 py-1 text-xs font-bold" :class="statusClass(record.status)">{{ statusText(record.status) }}</span>
              </td>
              <td class="px-4 py-4 text-muted-foreground">{{ formatTime(record.createdAt) }}</td>
              <td class="px-4 py-4 text-right">
                <button
                  v-if="needsFeePayment(record)"
                  class="rounded-lg bg-primary px-3 py-1.5 text-xs font-bold text-primary-foreground disabled:opacity-60"
                  :disabled="pendingKey === record.idempotencyKey"
                  @click="payFee(record)"
                >
                  {{ t('launchpad.record_pay_fee') }}
                </button>
                <button
                  v-else-if="canRelease(record)"
                  class="rounded-lg bg-primary px-3 py-1.5 text-xs font-bold text-primary-foreground disabled:opacity-60"
                  :disabled="pendingKey === record.idempotencyKey"
                  @click="release(record)"
                >
                  {{ t('launchpad.record_release') }}
                </button>
                <span v-else class="text-xs text-muted-foreground">--</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useToast } from 'vue-toastification'
import {
  fetchAssetSymbolMap,
  fetchNewCoinDistributions,
  fetchNewCoinPurchases,
  fetchNewCoinSubscriptions,
  fetchNewCoinUnlocks,
  payNewCoinUnlockFee,
  releaseNewCoinUnlock,
  type NewCoinDistributionRecord,
  type NewCoinPurchaseRecord,
  type NewCoinSubscriptionRecord,
  type NewCoinUnlockRecord,
} from '@/api/activity'

type TabValue = 'subscriptions' | 'distributions' | 'purchases' | 'unlocks'

const { t } = useI18n()
const toast = useToast()

const tabs: Array<{ value: TabValue; labelKey: string }> = [
  { value: 'subscriptions', labelKey: 'launchpad.tab_subscriptions' },
  { value: 'distributions', labelKey: 'launchpad.tab_distributions' },
  { value: 'purchases', labelKey: 'launchpad.tab_purchases' },
  { value: 'unlocks', labelKey: 'launchpad.tab_unlocks' },
]

const activeTab = ref<TabValue>('subscriptions')
const subscriptions = ref<NewCoinSubscriptionRecord[]>([])
const distributions = ref<NewCoinDistributionRecord[]>([])
const purchases = ref<NewCoinPurchaseRecord[]>([])
const unlocks = ref<NewCoinUnlockRecord[]>([])
const assetSymbols = ref<Record<number, string>>({})
const loading = ref(false)
const loadError = ref('')
const pendingKey = ref('')

onMounted(loadRecords)

async function loadRecords() {
  loading.value = true
  loadError.value = ''
  try {
    const [symbols, subscriptionRows, distributionRows, purchaseRows, unlockRows] = await Promise.all([
      fetchAssetSymbolMap(),
      fetchNewCoinSubscriptions(),
      fetchNewCoinDistributions(),
      fetchNewCoinPurchases(),
      fetchNewCoinUnlocks(),
    ])
    assetSymbols.value = symbols
    subscriptions.value = subscriptionRows
    distributions.value = distributionRows
    purchases.value = purchaseRows
    unlocks.value = unlockRows
  } catch (error) {
    loadError.value = errorMessage(error, t('launchpad.records_failed'))
  } finally {
    loading.value = false
  }
}

function needsFeePayment(record: NewCoinUnlockRecord) {
  return record.status === 'pending' && record.unlockFeeEnabled && record.feePaidStatus !== 'paid'
}

function canRelease(record: NewCoinUnlockRecord) {
  return record.status === 'pending' && (!record.unlockFeeEnabled || record.feePaidStatus === 'paid')
}

async function payFee(record: NewCoinUnlockRecord) {
  if (!record.unlockFeeAsset || !record.unlockFeeAmount) {
    toast.error(t('launchpad.record_fee_unavailable'))
    return
  }
  pendingKey.value = record.idempotencyKey
  try {
    await payNewCoinUnlockFee(record.idempotencyKey, record.unlockFeeAsset, record.unlockFeeAmount)
    toast.success(t('launchpad.record_fee_paid'))
    await loadRecords()
  } catch (error) {
    toast.error(errorMessage(error, t('launchpad.record_fee_failed')))
  } finally {
    pendingKey.value = ''
  }
}

async function release(record: NewCoinUnlockRecord) {
  pendingKey.value = record.idempotencyKey
  try {
    await releaseNewCoinUnlock(record.idempotencyKey)
    toast.success(t('launchpad.record_released'))
    await loadRecords()
  } catch (error) {
    toast.error(errorMessage(error, t('launchpad.record_release_failed')))
  } finally {
    pendingKey.value = ''
  }
}

function assetSymbol(assetId: number | null) {
  if (assetId == null) return ''
  return assetSymbols.value[assetId] ?? `#${assetId}`
}

function feeText(record: NewCoinUnlockRecord) {
  if (!record.unlockFeeEnabled || !record.unlockFeeAmount) return '--'
  return `${trim(record.unlockFeeAmount)} ${assetSymbol(record.unlockFeeAsset)}`
}

// 后端 DECIMAL(36,18) 序列化为定长字符串，去掉尾随零但保留全部有效位。
function trim(value: string) {
  if (!value.includes('.')) return value
  const trimmed = value.replace(/0+$/, '').replace(/\.$/, '')
  return trimmed === '' || trimmed === '-' ? '0' : trimmed
}

function statusText(status: string) {
  const key = `launchpad.record_status_${status}`
  const label = t(key)
  return label === key ? status : label
}

function statusClass(status: string) {
  if (status === 'completed' || status === 'released' || status === 'allocated') return 'bg-green-500/10 text-green-400'
  if (status === 'cancelled' || status === 'rejected' || status === 'failed') return 'bg-rose-500/10 text-rose-400'
  return 'bg-blue-500/10 text-blue-400'
}

function formatTime(value: number) {
  if (!value) return '--'
  return new Date(value).toLocaleString()
}

function errorMessage(error: unknown, fallback: string) {
  const responseMessage = (error as { response?: { data?: { message?: unknown } } })?.response?.data?.message
  if (typeof responseMessage === 'string' && responseMessage.trim()) return responseMessage
  return error instanceof Error ? error.message : fallback
}
</script>
