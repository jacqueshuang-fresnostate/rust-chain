<template>
  <div class="flex flex-col h-full">
    <!-- Tabs -->
    <div class="flex border-b border-border">
      <button
          v-for="tab in tabs"
          :key="tab"
          :class="['px-4 py-2 text-sm font-medium hover:text-primary transition-colors', activeTab === tab ? 'text-primary border-b-2 border-primary' : 'text-muted-foreground']"
          @click="activeTab = tab"
      >
        {{ $t('trade.'+tab) }}
      </button>
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-auto p-4">
      <div v-if="orders.length === 0" class="h-full flex flex-col items-center justify-center text-muted-foreground opacity-50">
        <span class="text-4xl mb-2">📄</span>
        <span>{{ $t('trade.no_orders') }}</span>
      </div>
      <table v-else class="w-full text-xs text-left">
        <thead>
          <tr class="text-muted-foreground border-b border-border">
            <th class="pb-2">{{ $t('trade.time') }}</th>
            <th class="pb-2">{{ $t('trade.symbol') }}</th>
            <th class="pb-2">{{ $t('trade.type') }}</th>
            <th class="pb-2">{{ $t('trade.side') }}</th>
            <th class="pb-2">{{ $t('trade.price') }}</th>
            <th class="pb-2">{{ $t('trade.amount') }}</th>
            <th class="pb-2">{{ $t('trade.status') }}</th>
            <th class="pb-2 text-right">{{ $t('trade.action') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="order in orders" :key="order.orderId" class="border-b border-border/50 hover:bg-muted/50">
            <td class="py-2">{{ formatTime(order.time) }}</td>
            <td class="py-2">{{ order.symbol }}</td>
            <td class="py-2">{{ order.type }}</td>
            <td class="py-2" :class="order.direction === 'BUY' ? 'text-up' : 'text-down'">{{ order.direction }}</td>
            <td class="py-2">{{ order.price }}</td>
            <td class="py-2">{{ order.amount }}</td>
            <td class="py-2">{{ order.status }}</td>
            <td class="py-2 text-right">
              <button class="text-destructive hover:underline" v-if="order.status === 'TRADING' || order.status === 'SUBMITTED'" @click="openCancelModal(order)">{{ $t('trade.cancel') }}</button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Cancel Order Confirm Modal -->
    <div v-if="showCancelModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" @click.self="showCancelModal = false">
      <div class="bg-card border border-border rounded-lg p-6 w-80 shadow-xl">
        <div class="flex items-center justify-between mb-4">
          <div class="text-base font-bold">撤销委托</div>
          <button @click="showCancelModal = false" class="text-muted-foreground hover:text-foreground transition-colors text-lg leading-none">&times;</button>
        </div>
        <div v-if="cancelingOrder" class="text-sm text-muted-foreground mb-4 space-y-1">
          <div class="flex justify-between"><span>交易对</span><span class="font-mono font-medium text-foreground">{{ cancelingOrder.symbol }}</span></div>
          <div class="flex justify-between"><span>方向</span><span :class="cancelingOrder.direction === 'BUY' ? 'text-up' : 'text-down'" class="font-medium">{{ cancelingOrder.direction }}</span></div>
          <div class="flex justify-between"><span>价格</span><span class="font-mono text-foreground">{{ cancelingOrder.price }}</span></div>
          <div class="flex justify-between"><span>数量</span><span class="font-mono text-foreground">{{ cancelingOrder.amount }}</span></div>
        </div>
        <div class="text-xs text-muted-foreground mb-5">确认撤销该委托订单？</div>
        <div class="flex gap-3">
          <button @click="showCancelModal = false"
            class="flex-1 py-2.5 text-sm border border-border rounded-lg hover:bg-muted transition-colors font-medium">
            取消
          </button>
          <button @click="confirmCancel"
            :disabled="canceling"
            class="flex-1 py-2.5 text-sm rounded-lg font-bold text-white bg-destructive hover:bg-destructive/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center">
            <span v-if="canceling" class="animate-spin mr-1">⏳</span>
            确认撤单
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { fetchCurrentOrders, fetchHistoryOrders, cancelOrder } from '@/api/exchange'
import { useToast } from 'vue-toastification'
import { useMarketStore } from '@/stores/market'

const props = defineProps<{
  symbol?: string
}>()

const toast = useToast()
const marketStore = useMarketStore()
const tabs = ['open_orders', 'order_history'] // Removed asset_details for now as it's different structure
const activeTab = ref('open_orders')
const orders = ref<any[]>([])
const showCancelModal = ref(false)
const cancelingOrder = ref<any>(null)
const canceling = ref(false)

const formatTime = (ts: number) => {
    if (!ts) return ''
    return new Date(ts).toLocaleString()
}

const loadOrders = async () => {
    if (!props.symbol) return
    try {
        let res
        if (activeTab.value === 'open_orders') {
            res = await fetchCurrentOrders(props.symbol)
        } else {
            res = await fetchHistoryOrders(props.symbol)
        }

        if (res.data) {
            // Assuming res.data.content or res.data is the list
            const list = Array.isArray(res.data) ? res.data : (res.data.content || [])
            orders.value = list
        }
    } catch (e) {
        console.error('Failed to load orders', e)
    }
}

const openCancelModal = (order: any) => {
    cancelingOrder.value = order
    canceling.value = false
    showCancelModal.value = true
}

const confirmCancel = async () => {
    if (!cancelingOrder.value) return
    canceling.value = true
    try {
        await cancelOrder(cancelingOrder.value.orderId)
        toast.success('Order Cancelled')
        showCancelModal.value = false
        loadOrders()
    } catch (e: any) {
        toast.error(e.message || 'Failed to cancel order')
    } finally {
        canceling.value = false
    }
}

watch([() => props.symbol, activeTab, () => marketStore.orderRefreshKey], () => {
    loadOrders()
})

onMounted(() => {
    loadOrders()
})
</script>
