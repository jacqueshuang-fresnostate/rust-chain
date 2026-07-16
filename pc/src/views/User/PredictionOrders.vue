<template>
  <div class="flex flex-col gap-5">
    <div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
      <div>
        <h2 class="text-2xl font-bold text-foreground">{{ t('prediction.orders_title') }}</h2>
        <p class="text-sm text-muted-foreground">{{ t('prediction.orders_desc') }}</p>
      </div>
      <button class="rounded-lg bg-muted px-4 py-2 text-sm font-semibold text-foreground hover:bg-muted/80" :disabled="loading" @click="loadOrders">
        {{ t('prediction.refresh') }}
      </button>
    </div>

    <div class="rounded-xl border border-border bg-card">
      <div class="flex gap-6 border-b border-border px-5">
        <button
          v-for="tab in statusTabs"
          :key="tab.value || 'all'"
          type="button"
          class="border-b-2 px-1 py-4 text-sm font-bold transition-colors"
          :class="activeStatus === tab.value ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'"
          @click="setStatus(tab.value)"
        >
          {{ t(tab.labelKey) }}
        </button>
      </div>

      <div class="overflow-x-auto">
        <table class="w-full table-fixed text-sm">
          <thead>
            <tr class="border-b border-border text-left text-muted-foreground">
              <th class="w-[22%] px-4 py-3">{{ t('prediction.order_no') }}</th>
              <th class="w-[12%] px-4 py-3">{{ t('prediction.outcome') }}</th>
              <th class="w-[18%] px-4 py-3">{{ t('prediction.stake') }}</th>
              <th class="w-[14%] px-4 py-3">{{ t('prediction.price') }}</th>
              <th class="w-[14%] px-4 py-3">{{ t('prediction.status') }}</th>
              <th class="w-[20%] px-4 py-3">{{ t('prediction.time') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="loading">
              <td colspan="6" class="px-4 py-10 text-center text-muted-foreground">{{ t('common.loading') }}</td>
            </tr>
            <tr v-else-if="orders.length === 0">
              <td colspan="6" class="px-4 py-10 text-center text-muted-foreground">{{ t('prediction.no_orders') }}</td>
            </tr>
            <template v-for="order in orders" :key="order.id">
              <tr class="border-b border-border">
                <td class="px-4 py-4">
                  <button class="inline-flex max-w-full items-center gap-2 text-left font-mono text-foreground hover:text-primary" @click="toggleExpanded(order.id)">
                    <span class="text-muted-foreground">{{ expandedIds.has(order.id) ? 'v' : '>' }}</span>
                    <span class="truncate">{{ order.orderNo }}</span>
                  </button>
                </td>
                <td class="px-4 py-4">
                  <span :class="order.outcome === 'yes' ? 'text-emerald-400' : 'text-rose-400'" class="font-bold">{{ outcomeText(order.outcome) }}</span>
                </td>
                <td class="px-4 py-4 font-mono">{{ formatAmount(order.stake_amount) }} {{ order.asset_symbol }}</td>
                <td class="px-4 py-4 font-mono">{{ percentText(order.accepted_price) }}</td>
                <td class="px-4 py-4">
                  <span class="rounded-full px-3 py-1 text-xs font-bold" :class="statusClass(order.status)">{{ statusText(order.status) }}</span>
                </td>
                <td class="px-4 py-4 text-muted-foreground">{{ formatTime(order.created_at) }}</td>
              </tr>
              <tr v-if="expandedIds.has(order.id)" class="border-b border-border bg-muted/20">
                <td colspan="6" class="px-4 py-4">
                  <div class="grid gap-3 text-sm md:grid-cols-4">
                    <InfoItem :label="t('prediction.market')" :value="order.market_title" />
                    <InfoItem :label="t('prediction.fee')" :value="`${formatAmount(order.fee_amount)} ${order.asset_symbol}`" />
                    <InfoItem :label="t('prediction.shares')" :value="formatAmount(order.shares)" />
                    <InfoItem :label="t('prediction.max_payout')" :value="`${formatAmount(order.theoretical_payout)} ${order.asset_symbol}`" />
                    <InfoItem :label="t('prediction.payout_cap')" :value="formatPayoutCap(order.effective_payout_cap, order.asset_symbol)" />
                    <InfoItem :label="t('prediction.result')" :value="outcomeText(order.result || '')" />
                    <InfoItem :label="t('prediction.payout')" :value="`${formatAmount(order.payout_amount)} ${order.asset_symbol}`" />
                    <InfoItem :label="t('prediction.refund')" :value="`${formatAmount(Number(order.refund_amount) + Number(order.fee_refund_amount))} ${order.asset_symbol}`" />
                    <InfoItem :label="t('prediction.settled_time')" :value="formatTime(order.settled_at)" />
                  </div>
                </td>
              </tr>
            </template>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { defineComponent, h, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useToast } from 'vue-toastification'
import { fetchPredictionOrders, type PredictionOrder, type PredictionOrderStatus } from '@/api/prediction'
import { formatNumber } from '@/utils/format'

const InfoItem = defineComponent({
  props: {
    label: { type: String, required: true },
    value: { type: String, required: true },
  },
  setup(props) {
    return () => h('div', { class: 'rounded-lg border border-border bg-background/60 p-3' }, [
      h('div', { class: 'text-xs text-muted-foreground' }, props.label),
      h('div', { class: 'mt-1 break-words font-mono text-foreground' }, props.value),
    ])
  },
})

const { t } = useI18n()
const toast = useToast()

type StatusTab = { value: PredictionOrderStatus | ''; labelKey: string }

const statusTabs: StatusTab[] = [
  { value: '', labelKey: 'prediction.status_all' },
  { value: 'open', labelKey: 'prediction.status_open' },
  { value: 'settled', labelKey: 'prediction.status_settled' },
  { value: 'refunded', labelKey: 'prediction.status_refunded' },
]

const orders = ref<PredictionOrder[]>([])
const activeStatus = ref<PredictionOrderStatus | ''>('')
const expandedIds = ref<Set<number>>(new Set())
const loading = ref(false)

onMounted(loadOrders)

function setStatus(status: PredictionOrderStatus | '') {
  activeStatus.value = status
  expandedIds.value = new Set()
  loadOrders()
}

async function loadOrders() {
  loading.value = true
  try {
    const response = await fetchPredictionOrders(activeStatus.value)
    orders.value = response.data
  } catch (error) {
    toast.error(errorMessage(error, t('prediction.load_orders_failed')))
  } finally {
    loading.value = false
  }
}

function toggleExpanded(orderId: number) {
  const next = new Set(expandedIds.value)
  if (next.has(orderId)) next.delete(orderId)
  else next.add(orderId)
  expandedIds.value = next
}

function statusText(status: string) {
  return t(`prediction.status_${status}`)
}

function statusClass(status: string) {
  if (status === 'settled') return 'bg-green-500/10 text-green-400'
  if (status === 'refunded') return 'bg-yellow-500/10 text-yellow-400'
  return 'bg-blue-500/10 text-blue-400'
}

function outcomeText(value: string) {
  if (value === 'yes') return 'YES'
  if (value === 'no') return 'NO'
  if (value === 'invalid') return t('prediction.outcome_invalid')
  return '--'
}

function percentText(value: string | number) {
  const number = Number(value)
  if (!Number.isFinite(number)) return '--'
  return `${formatNumber(number * 100, 'amount')}%`
}

function formatAmount(value: string | number | null | undefined) {
  return formatNumber(Number(value ?? 0), 'amount')
}

function formatPayoutCap(value: string | number | null | undefined, symbol: string) {
  const cap = Number(value ?? 0)
  if (!Number.isFinite(cap) || cap <= 0) return t('prediction.unlimited')
  return `${formatAmount(cap)} ${symbol}`
}

function formatTime(value?: number | null) {
  if (!value) return '--'
  return new Date(value).toLocaleString()
}

function errorMessage(error: unknown, fallback: string) {
  const responseMessage = (error as { response?: { data?: { message?: unknown } } })?.response?.data?.message
  if (typeof responseMessage === 'string' && responseMessage.trim()) return responseMessage
  return error instanceof Error ? error.message : fallback
}
</script>
