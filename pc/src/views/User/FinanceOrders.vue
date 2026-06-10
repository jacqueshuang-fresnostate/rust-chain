<template>
  <div class="space-y-6">
    <div class="flex items-center gap-2 mb-6 border-b border-border pb-4">
      <h2 class="text-2xl font-bold flex items-center gap-2">
        <Icon icon="mdi:robot-outline" class="text-primary" />
        {{ $t('ai_finance.my_orders') }}
      </h2>
    </div>

    <!-- Status Tabs -->
    <div class="flex gap-4 border-b border-border overflow-x-auto no-scrollbar">
        <button
          v-for="status in [0, 1]"
          :key="status"
          @click="setOrderStatus(status)"
          :class="[
              'py-3 border-b-2 font-bold transition-all px-2 whitespace-nowrap',
              orderStatus === status ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'
          ]"
        >
          {{ getStatusText(status) }}
        </button>
    </div>

    <div v-if="loadingOrders" class="py-12 flex justify-center">
         <span class="i-mdi-loading animate-spin text-4xl text-primary"></span>
    </div>

    <div v-else-if="orders.length === 0" class="py-12 text-center text-muted-foreground">
         <span class="i-mdi-robot-outline text-6xl mb-4 block mx-auto opacity-50"></span>
         {{ $t('ai_finance.no_orders', { status: getStatusText(orderStatus).toLowerCase() }) }}
    </div>

    <div v-else class="grid gap-4">
        <div v-for="order in orders" :key="order.id" class="bg-card border border-border rounded-xl p-6 shadow-sm hover:border-primary/50 transition-colors">
            <div class="flex flex-col md:flex-row justify-between md:items-center gap-4 mb-4">
                <div>
                    <div class="text-sm text-muted-foreground font-mono">{{ $t('ai_finance.order_sn') }}: {{ order.id }}</div>
                    <div class="text-xl font-bold flex items-center gap-2 mt-1">
                        {{ formatNumber(order.num) }} USDT
                        <span class="text-xs px-2 py-0.5 rounded-full bg-primary/10 text-primary border border-primary/20">
                            {{ order.cycle }} {{ $t('ai_finance.days') }}
                        </span>
                    </div>
                </div>
                <div class="flex items-center gap-3">
                    <span v-if="order.status === 0" class="text-blue-500 font-bold flex items-center gap-1">
                        <span class="i-mdi-clock-outline"></span> {{ $t('ai_finance.status_open') }}
                    </span>
                    <span v-else-if="order.status === 1" class="text-muted-foreground font-bold flex items-center gap-1">
                        <span class="i-mdi-check-circle"></span> {{ $t('ai_finance.status_close') }}
                    </span>

                    <!-- Actions -->
                    <div v-if="order.status === 0" class="flex gap-2 border-l border-border pl-3 ml-1">
                       <button @click="handleTerminate(order)" class="px-4 py-2 bg-destructive/10 text-destructive border border-destructive/20 font-bold rounded hover:bg-destructive/20 transition-all text-sm">
                           {{ $t('ai_finance.terminate') }}
                       </button>
                    </div>
                </div>
            </div>
            <div class="grid grid-cols-2 lg:grid-cols-4 gap-4 text-sm mt-4 pt-4 border-t border-border/50">
                <div>
                    <div class="text-muted-foreground text-xs mb-1">{{ $t('ai_finance.earnings') }}</div>
                    <div class="font-mono text-up font-bold">+{{ formatNumber(order.earnNum) }} {{ order.coinSymbol }}</div>
                </div>
                <div>
                    <div class="text-muted-foreground text-xs mb-1">{{ $t('ai_finance.roi') }}</div>
                    <div class="font-mono">{{ (order.minDaysProfit * 100).toFixed(2) }}% - {{ (order.maxDaysProfit * 100).toFixed(2) }}%</div>
                </div>
                <div>
                    <div class="text-muted-foreground text-xs mb-1">{{ $t('ai_finance.create_time') }}</div>
                    <div class="font-mono">{{ formatDate(order.createTime) }}</div>
                </div>
            </div>
        </div>

        <!-- Pagination -->
        <div class="flex justify-center gap-2 mt-6" v-if="totalPages > 1">
            <button
                @click="changePage(pageNo - 1)"
                :disabled="pageNo === 1"
                class="p-2 bg-card border border-border rounded text-foreground hover:bg-muted disabled:opacity-50"
            >
                <span class="i-mdi-chevron-left w-5 h-5"></span>
            </button>
            <div class="flex items-center px-4 font-mono">
                {{ pageNo }} / {{ totalPages }}
            </div>
            <button
                @click="changePage(pageNo + 1)"
                :disabled="pageNo >= totalPages"
                class="p-2 bg-card border border-border rounded text-foreground hover:bg-muted disabled:opacity-50"
            >
                <span class="i-mdi-chevron-right w-5 h-5"></span>
            </button>
        </div>
    </div>

    <!-- Early Termination Modal -->
    <div v-if="terminateModalVisible" class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
        <div class="bg-card w-full max-w-sm p-6 rounded-2xl border border-border shadow-2xl relative text-center">
            <div class="w-16 h-16 rounded-full bg-destructive/10 flex items-center justify-center mx-auto mb-4 border border-destructive/20">
                <span class="i-mdi-alert text-3xl text-destructive"></span>
            </div>
            <h3 class="text-xl font-bold mb-2 text-foreground">{{ $t('ai_finance.terminate') }}</h3>
            <p class="text-sm text-muted-foreground mb-4">
                {{ $t('ai_finance.terminate_confirm') }}
            </p>
            <div class="text-sm font-mono text-destructive bg-destructive/5 rounded p-2 mb-8" v-if="orderToTerminate">
                Breach Fee Rate: {{ (orderToTerminate.breachFee * 100).toFixed(2) }}%
            </div>

            <div class="flex gap-4">
                <button @click="closeTerminateModal" class="flex-1 py-3 bg-muted text-foreground font-bold rounded-lg hover:bg-muted/80 transition-all">{{ $t('ai_finance.cancel') }}</button>
                <button @click="executeTerminate" :disabled="terminating" class="flex-1 py-3 bg-destructive text-destructive-foreground font-bold rounded-lg hover:bg-destructive/90 transition-all disabled:opacity-50">
                    {{ terminating ? $t('ai_finance.processing') : $t('common.confirm') }}
                </button>
            </div>
        </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { Icon } from '@iconify/vue'
import { useUserStore } from '@/stores/user'
import { fetchFinanceHistory, closeFinanceOrder, type AiFinanceOrder, AiFinanceOrderStatus } from '@/api/finance'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import numeral from 'numeral'

const { t } = useI18n()
const userStore = useUserStore()
const toast = useToast()

// --- Orders State ---
const orderStatus = ref<AiFinanceOrderStatus>(AiFinanceOrderStatus.OPEN)
const orders = ref<AiFinanceOrder[]>([])
const loadingOrders = ref(false)
const pageNo = ref(1)
const pageSize = ref(10)
const totalElements = ref(0)
const totalPages = computed(() => Math.ceil(totalElements.value / pageSize.value))

// --- Early Termination Confirm State ---
const terminateModalVisible = ref(false)
const orderToTerminate = ref<AiFinanceOrder | null>(null)
const terminating = ref(false)

// --- Formatters ---
const formatNumber = (val: number | string) => numeral(val).format('0,0.[00]')
const formatDate = (ts: any) => {
    if (!ts) return '-'
    const d = typeof ts === 'number' && ts < 1e12 ? ts * 1000 : ts
    return new Date(d).toLocaleString()
}

// --- Orders Logic ---
const loadOrders = async () => {
    if (!userStore.isLoggedIn) return
    loadingOrders.value = true
    try {
        const res = await fetchFinanceHistory(orderStatus.value, pageNo.value, pageSize.value)
        if (res.data && res.data.code === 0 && res.data.data) {
            const data = res.data.data
            orders.value = data.content || []
            totalElements.value = data.totalElements ?? data.total ?? data.page?.totalElements ?? data.page?.total ?? 0
            if (data.totalPages !== undefined) {
                 totalElements.value = data.totalPages * pageSize.value
            } else if (data.page?.totalPages !== undefined) {
                 totalElements.value = data.page.totalPages * pageSize.value
            }
        } else {
             orders.value = []
        }
    } catch (e) {
        console.error("Failed to fetch finance orders", e)
        orders.value = []
    } finally {
        loadingOrders.value = false
    }
}

const setOrderStatus = (status: AiFinanceOrderStatus | number) => {
    orderStatus.value = status as AiFinanceOrderStatus
    pageNo.value = 0
    loadOrders()
}

const getStatusText = (status: number) => {
    switch(status) {
        case AiFinanceOrderStatus.OPEN: return t('ai_finance.status_open')
        case AiFinanceOrderStatus.CLOSE: return t('ai_finance.status_close')
        default: return t('loan.status_unknown')
    }
}

const changePage = (p: number) => {
    pageNo.value = p
    loadOrders()
}

// --- Early Terminate Modal Logic ---
const handleTerminate = (order: AiFinanceOrder) => {
    orderToTerminate.value = order
    terminateModalVisible.value = true
}

const closeTerminateModal = () => {
    terminateModalVisible.value = false
    orderToTerminate.value = null
    terminating.value = false
}

const executeTerminate = async () => {
    if (!orderToTerminate.value) return
    terminating.value = true
    try {
        const res = await closeFinanceOrder(orderToTerminate.value.id)
        if (res.data && res.data.code === 0) {
            toast.success(t('ai_finance.terminate_success'))
            closeTerminateModal()
            loadOrders() // Refresh current view
        } else {
            toast.error(res.data?.message || t('ai_finance.terminate_failed'))
        }
    } catch (e) {
        toast.error(t('ai_finance.terminate_failed'))
        console.error(e)
    } finally {
        terminating.value = false
    }
}

onMounted(() => {
    if (userStore.isLoggedIn) {
        loadOrders()
    }
})

</script>
