<template>
  <div class="space-y-6">
    <h2 class="text-2xl font-bold flex items-center gap-2">
      <span class="i-mdi-history text-primary"></span>
      Transaction History
    </h2>

    <div class="bg-card border border-border rounded-xl p-6 shadow-sm">
        <!-- Filters -->
        <div class="flex flex-col md:flex-row gap-4 mb-6">
            <div class="flex-1">
                <label class="block text-xs text-muted-foreground mb-1">Type</label>
                <select v-model="filterType" class="w-full bg-background border border-border rounded px-3 py-2 text-sm focus:border-primary outline-none">
                    <option :value="undefined">All Types</option>
                    <option :value="0">Deposit</option>
                    <option :value="1">Withdraw</option>
                    <option :value="2">Transfer</option>
                    <option :value="3">Exchange</option>
                    <option :value="4">OTC Buy</option>
                    <option :value="5">OTC Sell</option>
                </select>
            </div>
            <div class="flex-1">
                <label class="block text-xs text-muted-foreground mb-1">Date Range</label>
                <div class="flex items-center gap-2">
                     <input type="date" v-model="startTime" class="w-full bg-background border border-border rounded px-3 py-2 text-sm focus:border-primary outline-none" />
                     <span class="text-muted-foreground">-</span>
                     <input type="date" v-model="endTime" class="w-full bg-background border border-border rounded px-3 py-2 text-sm focus:border-primary outline-none" />
                </div>
            </div>
            <div class="flex items-end">
                <button @click="resetFilters" class="px-4 py-2 bg-muted text-muted-foreground hover:bg-muted/80 rounded text-sm transition-colors">
                    Reset
                </button>
            </div>
        </div>

        <!-- Table -->
        <div class="overflow-x-auto">
            <table class="w-full text-sm text-left">
                <thead class="text-xs text-muted-foreground uppercase bg-muted/20">
                    <tr>
                        <th class="px-4 py-3">Time</th>
                        <th class="px-4 py-3">Type</th>
                        <th class="px-4 py-3">Symbol</th>
                        <th class="px-4 py-3 text-right">Amount</th>
                        <th class="px-4 py-3 text-right">Fee</th>
                        <th class="px-4 py-3 text-center">Status</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-border/50">
                    <tr v-for="item in records" :key="item.id" class="hover:bg-muted/10 transition-colors">
                        <td class="px-4 py-3 font-mono text-muted-foreground">{{ item.createTime }}</td>
                        <td class="px-4 py-3 font-medium">{{ getTypeName(item.type) }}</td>
                        <td class="px-4 py-3 font-bold">{{ item.symbol }}</td>
                        <td class="px-4 py-3 text-right font-mono" :class="getAmountColor(item.type)">
                            {{ item.amount > 0 ? '+' : '' }}{{ formatNumber(item.amount) }}
                        </td>
                        <td class="px-4 py-3 text-right font-mono text-muted-foreground">{{ formatNumber(item.fee) }}</td>
                        <td class="px-4 py-3 text-center">
                             <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-green-500/10 text-green-500" v-if="item.status === 1">Success</span>
                             <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-yellow-500/10 text-yellow-500" v-else-if="item.status === 0">Pending</span>
                             <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-red-500/10 text-red-500" v-else>Failed</span>
                        </td>
                    </tr>
                    <tr v-if="records.length === 0">
                        <td colspan="6" class="px-4 py-12 text-center text-muted-foreground">
                            No transaction records found
                        </td>
                    </tr>
                </tbody>
            </table>
        </div>

        <!-- Pagination -->
        <div class="flex justify-between items-center mt-6 text-xs text-muted-foreground">
             <div>Page {{ pageNo }} of {{ totalPages }}</div>
             <div class="flex gap-2">
                 <button @click="prevPage" :disabled="pageNo <= 1" class="px-3 py-1 bg-muted rounded disabled:opacity-50 hover:bg-muted/80 transition-colors">Prev</button>
                 <button @click="nextPage" :disabled="pageNo >= totalPages" class="px-3 py-1 bg-muted rounded disabled:opacity-50 hover:bg-muted/80 transition-colors">Next</button>
             </div>
        </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { fetchTransactionHistory, type TransactionRecord, TransactionType } from '@/api/transaction'
import numeral from 'numeral'

const records = ref<TransactionRecord[]>([])
const pageNo = ref(1)
const pageSize = ref(10)
const totalPages = ref(1)
const filterType = ref<number | undefined>(undefined)
const startTime = ref('')
const endTime = ref('')

const formatNumber = (val: number) => numeral(val).format('0,0.0000')

const getTypeName = (type: number) => {
    switch (type) {
        case TransactionType.RECHARGE: return 'Deposit'
        case TransactionType.WITHDRAW: return 'Withdraw'
        case TransactionType.TRANSFER: return 'Transfer'
        case TransactionType.EXCHANGE: return 'Exchange'
        case TransactionType.OTC_BUY: return 'OTC Buy'
        case TransactionType.OTC_SELL: return 'OTC Sell'
        case TransactionType.ACTIVITY_AWARD: return 'Activity Reward'
        case TransactionType.PROMOTION_AWARD: return 'Promotion Reward'
        case TransactionType.DIVIDEND: return 'Dividend'
        case TransactionType.VOTE: return 'Vote'
        case TransactionType.ADMIN_RECHARGE: return 'Admin Recharge'
        case TransactionType.MATCH: return 'Match'
        default: return 'Unknown'
    }
}

const getAmountColor = (type: number) => {
    // Deposit, Award, Sell -> Green
    if ([0, 4, 6, 7, 8, 10].includes(type)) return 'text-green-500'
    // Withdraw, Buy -> Red
    if ([1, 3, 5].includes(type)) return 'text-red-500'
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

const resetFilters = () => {
    filterType.value = undefined
    startTime.value = ''
    endTime.value = ''
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
})

</script>
