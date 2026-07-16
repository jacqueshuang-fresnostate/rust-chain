<template>
  <div class="space-y-6">
    <h2 class="text-2xl font-bold flex items-center gap-2">
      <span class="i-mdi-history text-primary"></span>
      {{ t('nav.transaction') }}
    </h2>

    <div class="bg-card border border-border rounded-xl shadow-sm overflow-hidden">
        <div class="transaction-tabs flex gap-6 border-b border-border px-6 overflow-x-auto">
            <button
                type="button"
                class="py-4 border-b-2 text-sm font-bold transition-colors whitespace-nowrap"
                :class="activeTab === 'transactions' ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'"
                @click="activeTab = 'transactions'"
            >
                {{ t('nav.transaction') }}
            </button>
            <button
                type="button"
                class="py-4 border-b-2 text-sm font-bold transition-colors whitespace-nowrap"
                :class="activeTab === 'swapOrders' ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'"
                @click="activeTab = 'swapOrders'"
            >
                {{ t('swap.recent_orders') }}
            </button>
        </div>

        <div v-if="activeTab === 'transactions'" class="p-6">
        <!-- Filters -->
            <div class="flex flex-col md:flex-row gap-4 mb-6">
                <div class="flex-1">
                    <label class="block text-xs text-muted-foreground mb-1">{{ t('transaction.type') }}</label>
                    <select v-model="filterType" class="w-full bg-background border border-border rounded px-3 py-2 text-sm focus:border-primary outline-none">
                        <option :value="undefined">{{ t('transaction.all_types') }}</option>
                        <option v-for="option in transactionTypeOptions" :key="option.value" :value="option.value">
                            {{ t(option.labelKey) }}
                        </option>
                    </select>
                </div>
                <div ref="dateRangePickerRef" class="flex-1 relative">
                    <label class="block text-xs text-muted-foreground mb-1">{{ t('transaction.date_range') }}</label>
                    <button
                        type="button"
                        class="w-full bg-background border border-border rounded px-3 py-2 text-sm focus:border-primary outline-none transition-colors flex items-center justify-between gap-3 text-left"
                        :aria-expanded="dateRangePickerOpen"
                        aria-haspopup="dialog"
                        @click="toggleDateRangePicker"
                    >
                        <span class="min-w-0 truncate" :class="hasDateRange ? 'text-foreground' : 'text-muted-foreground'">{{ dateRangeDisplay }}</span>
                        <Icon icon="mdi:calendar-clock" class="h-4 w-4 shrink-0 text-muted-foreground" />
                    </button>

                    <div
                        v-if="dateRangePickerOpen"
                        class="absolute left-0 top-full z-40 mt-2 w-full md:w-[34rem] overflow-hidden rounded-xl border border-border bg-popover p-4 shadow-2xl"
                        role="dialog"
                        :aria-label="t('transaction.date_range')"
                        @keydown.escape.prevent="cancelDateRange"
                    >
                        <div class="grid gap-3 md:grid-cols-2">
                            <label class="space-y-1">
                                <span class="block text-xs font-medium text-muted-foreground">{{ t('transaction.start_time') }}</span>
                                <input
                                    v-model="draftStartTime"
                                    type="datetime-local"
                                    class="w-full bg-background border border-border rounded px-3 py-2 text-sm focus:border-primary outline-none"
                                />
                            </label>
                            <label class="space-y-1">
                                <span class="block text-xs font-medium text-muted-foreground">{{ t('transaction.end_time') }}</span>
                                <input
                                    v-model="draftEndTime"
                                    type="datetime-local"
                                    class="w-full bg-background border border-border rounded px-3 py-2 text-sm focus:border-primary outline-none"
                                />
                            </label>
                        </div>

                        <p v-if="dateRangeError" class="mt-3 text-xs text-red-500">{{ dateRangeError }}</p>

                        <div class="mt-4 flex flex-wrap items-center justify-end gap-2">
                            <button type="button" class="px-3 py-2 rounded bg-muted text-muted-foreground hover:bg-muted/80 text-sm transition-colors" @click="clearDateRange">
                                {{ t('transaction.clear_date_range') }}
                            </button>
                            <button type="button" class="px-3 py-2 rounded bg-muted text-muted-foreground hover:bg-muted/80 text-sm transition-colors" @click="cancelDateRange">
                                {{ t('transaction.cancel') }}
                            </button>
                            <button type="button" class="px-4 py-2 rounded bg-primary text-primary-foreground hover:bg-primary/90 text-sm font-bold transition-colors" @click="applyDateRange">
                                {{ t('transaction.apply') }}
                            </button>
                        </div>
                    </div>
                </div>
                <div class="flex items-end">
                    <button @click="resetFilters" class="px-4 py-2 bg-muted text-muted-foreground hover:bg-muted/80 rounded text-sm transition-colors">
                        {{ t('transaction.reset') }}
                    </button>
                </div>
            </div>

            <!-- Table -->
            <div class="overflow-x-auto">
                <table class="w-full text-sm text-left">
                    <thead class="text-xs text-muted-foreground uppercase bg-muted/20">
                        <tr>
                            <th class="px-4 py-3">{{ t('transaction.time') }}</th>
                            <th class="px-4 py-3">{{ t('transaction.type') }}</th>
                            <th class="px-4 py-3">{{ t('transaction.symbol') }}</th>
                            <th class="px-4 py-3 text-right">{{ t('transaction.amount') }}</th>
                            <th class="px-4 py-3 text-right">{{ t('transaction.fee') }}</th>
                            <th class="px-4 py-3 text-center">{{ t('transaction.status') }}</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-border/50">
                        <tr v-for="item in records" :key="item.id" class="hover:bg-muted/10 transition-colors">
                            <td class="px-4 py-3 font-mono text-muted-foreground">{{ item.createTime }}</td>
                            <td class="px-4 py-3 font-medium">{{ getTypeName(item.type) }}</td>
                            <td class="px-4 py-3 font-bold">{{ item.symbol }}</td>
                            <td class="px-4 py-3 text-right font-mono" :class="getAmountColor(item.amount)">
                                {{ item.amount > 0 ? '+' : '' }}{{ formatNumber(item.amount) }}
                            </td>
                            <td class="px-4 py-3 text-right font-mono text-muted-foreground">{{ formatNumber(item.fee) }}</td>
                            <td class="px-4 py-3 text-center">
                                 <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-green-500/10 text-green-500" v-if="item.status === 1">{{ t('transaction.status_success') }}</span>
                                 <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-yellow-500/10 text-yellow-500" v-else-if="item.status === 0">{{ t('transaction.status_pending') }}</span>
                                 <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-red-500/10 text-red-500" v-else>{{ t('transaction.status_failed') }}</span>
                            </td>
                        </tr>
                        <tr v-if="records.length === 0">
                            <td colspan="6" class="px-4 py-12 text-center text-muted-foreground">
                                {{ t('transaction.no_records') }}
                            </td>
                        </tr>
                    </tbody>
                </table>
            </div>

            <!-- Pagination -->
            <div class="flex justify-between items-center mt-6 text-xs text-muted-foreground">
                 <div>{{ t('transaction.page_of', { page: pageNo, total: totalPages }) }}</div>
                 <div class="flex gap-2">
                     <button @click="prevPage" :disabled="pageNo <= 1" class="px-3 py-1 bg-muted rounded disabled:opacity-50 hover:bg-muted/80 transition-colors">{{ t('transaction.prev') }}</button>
                     <button @click="nextPage" :disabled="pageNo >= totalPages" class="px-3 py-1 bg-muted rounded disabled:opacity-50 hover:bg-muted/80 transition-colors">{{ t('transaction.next') }}</button>
                 </div>
            </div>
        </div>

        <div v-else class="p-6">
            <div class="flex flex-wrap items-center justify-end gap-3 mb-4">
                <button
                    type="button"
                    class="inline-flex items-center gap-1 text-sm font-medium text-primary hover:opacity-80 disabled:opacity-50"
                    :disabled="swapOrdersLoading"
                    @click="loadSwapOrders"
                >
                    <Icon icon="mdi:refresh" class="h-4 w-4" />
                    {{ t('swap.refresh') }}
                </button>
            </div>

            <div v-if="swapOrdersLoading" class="rounded-lg border border-dashed border-border py-12 text-center text-sm text-muted-foreground">
                {{ t('common.loading') }}
            </div>

            <div v-else-if="swapOrders.length === 0" class="rounded-lg border border-dashed border-border py-12 text-center text-sm text-muted-foreground">
                {{ t('swap.no_orders') }}
            </div>

            <div v-else class="overflow-x-auto">
                <table class="w-full text-sm">
                    <thead class="border-b border-border text-xs text-muted-foreground">
                        <tr>
                            <th class="py-2 text-right">{{ t('swap.from_amount') }}</th>
                            <th class="py-2 text-right">{{ t('swap.to_amount') }}</th>
                            <th class="py-2 text-right">{{ t('swap.status') }}</th>
                            <th class="py-2 text-right">{{ t('swap.time') }}</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr v-for="order in swapOrders" :key="order.id" class="border-b border-border/50 last:border-0">
                            <td class="py-3 text-right">{{ formatNumber(order.fromAmount) }} {{ order.fromUnit }}</td>
                            <td class="py-3 text-right">{{ formatNumber(order.toAmount) }} {{ order.toUnit }}</td>
                            <td class="py-3 text-right font-medium" :class="swapStatusClass(order.status)">{{ swapStatusText(order.status) }}</td>
                            <td class="py-3 text-right text-muted-foreground">{{ formatSwapTime(order.createdAt) }}</td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { fetchTransactionHistory, type TransactionRecord, WALLET_LEDGER_TRANSACTION_TYPES } from '@/api/transaction'
import { fetchSwapOrders } from '@/api/swap'
import type { PcSwapOrderRow } from '@/api/backendAdapters'
import { Icon } from '@iconify/vue'
import { useI18n } from 'vue-i18n'
import numeral from 'numeral'

type TransactionTab = 'transactions' | 'swapOrders'

const records = ref<TransactionRecord[]>([])
const swapOrders = ref<PcSwapOrderRow[]>([])
const activeTab = ref<TransactionTab>('transactions')
const pageNo = ref(1)
const pageSize = ref(10)
const totalPages = ref(1)
const filterType = ref<string | undefined>(undefined)
const startTime = ref('')
const endTime = ref('')
const draftStartTime = ref('')
const draftEndTime = ref('')
const dateRangePickerOpen = ref(false)
const dateRangePickerRef = ref<HTMLElement | null>(null)
const dateRangeError = ref('')
const swapOrdersLoading = ref(false)
const { t, te } = useI18n()

const transactionTypeLabelKey = (type: string) => `transaction.type_${type}`

const transactionTypeOptions = WALLET_LEDGER_TRANSACTION_TYPES.map((type) => ({
    value: type,
    labelKey: transactionTypeLabelKey(type),
}))

const formatNumber = (val: number) => numeral(val).format('0,0.0000')

const hasDateRange = computed(() => Boolean(startTime.value || endTime.value))

const dateRangeDisplay = computed(() => {
    if (!hasDateRange.value) return t('transaction.select_date_time_range')
    return `${formatDateTimeLabel(startTime.value)} - ${formatDateTimeLabel(endTime.value)}`
})

const formatDateTimeLabel = (value: string) => {
    return value ? value.replace('T', ' ') : t('transaction.not_set')
}

const getTypeName = (type: string) => {
    const key = transactionTypeLabelKey(type)
    if (te(key)) return t(key)
    return type || t('transaction.type_unknown')
}

const getAmountColor = (amount: number) => {
    if (amount > 0) return 'text-green-500'
    if (amount < 0) return 'text-red-500'
    return ''
}

const loadData = async () => {
    try {
        const res = await fetchTransactionHistory({
            pageNo: pageNo.value,
            pageSize: pageSize.value,
            type: filterType.value,
            startTime: startTime.value,
            endTime: endTime.value
        })
        if (res.data.code === 0) {
            records.value = res.data.data.content
            totalPages.value = res.data.data.page.totalPages
        }
    } catch (e) {
        console.error(e)
    }
}

const loadSwapOrders = async () => {
    swapOrdersLoading.value = true
    try {
        const res = await fetchSwapOrders()
        swapOrders.value = res.data.data
    } catch (e) {
        console.error(e)
        swapOrders.value = []
    } finally {
        swapOrdersLoading.value = false
    }
}

const swapStatusText = (status: string) => {
    const key = `swap.status_${status.toLowerCase()}`
    return te(key) ? t(key) : status
}

const swapStatusClass = (status: string) => {
    const normalized = status.toLowerCase()
    if (normalized === 'completed') return 'text-up'
    if (normalized === 'failed' || normalized === 'cancelled') return 'text-down'
    return 'text-primary'
}

const formatSwapTime = (value: number) => {
    if (!value) return '--'
    return new Date(value).toLocaleString()
}

const syncDraftDateRange = () => {
    draftStartTime.value = startTime.value
    draftEndTime.value = endTime.value
    dateRangeError.value = ''
}

const openDateRangePicker = () => {
    syncDraftDateRange()
    dateRangePickerOpen.value = true
}

const closeDateRangePicker = () => {
    dateRangePickerOpen.value = false
    dateRangeError.value = ''
}

const toggleDateRangePicker = () => {
    if (dateRangePickerOpen.value) {
        cancelDateRange()
    } else {
        openDateRangePicker()
    }
}

const applyDateRange = () => {
    const nextStartTime = draftStartTime.value.trim()
    const nextEndTime = draftEndTime.value.trim()
    if (nextStartTime && nextEndTime && nextStartTime > nextEndTime) {
        dateRangeError.value = t('transaction.date_range_invalid')
        return
    }
    startTime.value = nextStartTime
    endTime.value = nextEndTime
    closeDateRangePicker()
}

const clearDateRange = () => {
    draftStartTime.value = ''
    draftEndTime.value = ''
    startTime.value = ''
    endTime.value = ''
    closeDateRangePicker()
}

const cancelDateRange = () => {
    syncDraftDateRange()
    closeDateRangePicker()
}

const handleDateRangeOutsideClick = (event: MouseEvent) => {
    if (!dateRangePickerRef.value?.contains(event.target as Node)) {
        cancelDateRange()
    }
}

const resetFilters = () => {
    filterType.value = undefined
    startTime.value = ''
    endTime.value = ''
    draftStartTime.value = ''
    draftEndTime.value = ''
    closeDateRangePicker()
    pageNo.value = 1
    loadData()
}

const prevPage = () => {
    if (pageNo.value > 1) {
        pageNo.value--
        loadData()
    }
}

const nextPage = () => {
    if (pageNo.value < totalPages.value) {
        pageNo.value++
        loadData()
    }
}

watch([filterType, startTime, endTime], () => {
    pageNo.value = 1
    loadData()
})

onMounted(() => {
    loadData()
    loadSwapOrders()
    window.addEventListener('click', handleDateRangeOutsideClick)
})

onBeforeUnmount(() => {
    window.removeEventListener('click', handleDateRangeOutsideClick)
})

</script>
