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
      <AuthRequiredState v-if="!isLoggedIn" compact />
      <div v-else-if="orders.length === 0" class="h-full flex flex-col items-center justify-center text-muted-foreground opacity-50">
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
            <th class="pb-2">{{ $t('trade.trigger_price') }}</th>
            <th class="pb-2">{{ $t('trade.price') }}</th>
            <th v-if="activeTab === 'order_history'" class="pb-2">{{ $t('trade.filled_price') }}</th>
            <th class="pb-2">{{ $t('trade.amount') }}</th>
            <th class="pb-2">{{ $t('trade.status') }}</th>
            <th class="pb-2 text-right">{{ $t('trade.action') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="order in orders" :key="order.orderId" class="border-b border-border/50 hover:bg-muted/50">
            <td class="py-2">{{ formatTime(order.time) }}</td>
            <td class="py-2">{{ order.symbol }}</td>
            <td class="py-2">{{ formatOrderType(order.type) }}</td>
            <td class="py-2" :class="getOrderSideClass(order.direction)">{{ formatOrderSide(order.direction) }}</td>
            <td class="py-2">{{ formatTriggerPrice(order) }}</td>
            <td class="py-2">{{ formatOrderPrice(order) }}</td>
            <td v-if="activeTab === 'order_history'" class="py-2">{{ formatOptionalNumber(order.filledPrice) }}</td>
            <td class="py-2">{{ order.amount }}</td>
            <td class="py-2">{{ formatOrderStatus(order.status) }}</td>
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
          <div class="text-base font-bold">{{ t('trade.cancel_order_title') }}</div>
          <button @click="showCancelModal = false" class="text-muted-foreground hover:text-foreground transition-colors text-lg leading-none">&times;</button>
        </div>
        <div v-if="cancelingOrder" class="text-sm text-muted-foreground mb-4 space-y-1">
          <div class="flex justify-between"><span>{{ t('trade.symbol') }}</span><span class="font-mono font-medium text-foreground">{{ cancelingOrder.symbol }}</span></div>
          <div class="flex justify-between"><span>{{ t('trade.side') }}</span><span :class="getOrderSideClass(cancelingOrder.direction)" class="font-medium">{{ formatOrderSide(cancelingOrder.direction) }}</span></div>
          <div class="flex justify-between"><span>{{ t('trade.trigger_price') }}</span><span class="font-mono text-foreground">{{ formatTriggerPrice(cancelingOrder) }}</span></div>
          <div class="flex justify-between"><span>{{ t('trade.price') }}</span><span class="font-mono text-foreground">{{ formatOrderPrice(cancelingOrder) }}</span></div>
          <div class="flex justify-between"><span>{{ t('trade.amount') }}</span><span class="font-mono text-foreground">{{ cancelingOrder.amount }}</span></div>
        </div>
        <div class="text-xs text-muted-foreground mb-5">{{ t('trade.cancel_order_confirm') }}</div>
        <div class="flex gap-3">
          <button @click="showCancelModal = false"
            class="flex-1 py-2.5 text-sm border border-border rounded-lg hover:bg-muted transition-colors font-medium">
            {{ t('common.cancel') }}
          </button>
          <button @click="confirmCancel"
            :disabled="canceling"
            class="flex-1 py-2.5 text-sm rounded-lg font-bold text-white bg-destructive hover:bg-destructive/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center">
            <span v-if="canceling" class="animate-spin mr-1">⏳</span>
            {{ t('trade.confirm_cancel') }}
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
import { useI18n } from 'vue-i18n'
import AuthRequiredState from '@/components/common/AuthRequiredState.vue'
import { useAuthRequired } from '@/composables/useAuthRequired'

const props = defineProps<{
  symbol?: string
}>()

const toast = useToast()
const marketStore = useMarketStore()
const { t } = useI18n()
const { isLoggedIn, goToLogin } = useAuthRequired()
const tabs = ['open_orders', 'order_history'] // Removed asset_details for now as it's different structure
const activeTab = ref('open_orders')
const orders = ref<any[]>([])
const showCancelModal = ref(false)
const cancelingOrder = ref<any>(null)
const canceling = ref(false)

const orderTypeI18nKeys: Record<string, string> = {
    LIMIT: 'trade.order_type_limit_price',
    LIMIT_PRICE: 'trade.order_type_limit_price',
    MARKET: 'trade.order_type_market_price',
    MARKET_PRICE: 'trade.order_type_market_price',
    STOP_LIMIT: 'trade.order_type_stop_limit',
}

const orderSideI18nKeys: Record<string, string> = {
    BUY: 'trade.order_side_buy',
    SELL: 'trade.order_side_sell',
}

const orderStatusI18nKeys: Record<string, string> = {
    CANCELED: 'trade.order_status_canceled',
    CANCELLED: 'trade.order_status_canceled',
    COMPLETED: 'trade.order_status_completed',
    EXPIRED: 'trade.order_status_expired',
    FAILED: 'trade.order_status_failed',
    FILLED: 'trade.order_status_completed',
    OPEN: 'trade.order_status_trading',
    PARTIAL: 'trade.order_status_partially_filled',
    PARTIALLY_FILLED: 'trade.order_status_partially_filled',
    PENDING: 'trade.order_status_pending',
    REJECTED: 'trade.order_status_rejected',
    SUBMITTED: 'trade.order_status_submitted',
    TRADING: 'trade.order_status_trading',
}

const normalizeOrderEnum = (value: unknown) => String(value ?? '').trim().toUpperCase()

const formatOrderType = (value: unknown) => {
    const raw = String(value ?? '')
    const key = orderTypeI18nKeys[normalizeOrderEnum(value)]
    return key ? t(key) : raw
}

const formatOrderSide = (value: unknown) => {
    const raw = String(value ?? '')
    const key = orderSideI18nKeys[normalizeOrderEnum(value)]
    return key ? t(key) : raw
}

const formatOrderStatus = (value: unknown) => {
    const raw = String(value ?? '')
    const key = orderStatusI18nKeys[normalizeOrderEnum(value)]
    return key ? t(key) : raw
}

const getOrderSideClass = (value: unknown) => normalizeOrderEnum(value) === 'BUY' ? 'text-up' : 'text-down'

const isMarketOrder = (value: unknown) => ['MARKET', 'MARKET_PRICE'].includes(normalizeOrderEnum(value))
const isStopLimitOrder = (value: unknown) => normalizeOrderEnum(value) === 'STOP_LIMIT'

const formatOrderPrice = (order: { type?: unknown; price?: unknown } | null | undefined) => {
    if (!order) return '--'
    if (isMarketOrder(order.type)) return '--'
    const number = Number(order.price)
    return Number.isFinite(number) ? String(order.price) : '--'
}

const formatTriggerPrice = (order: { type?: unknown; triggerPrice?: unknown } | null | undefined) => {
    if (!order || !isStopLimitOrder(order.type)) return '--'
    const number = Number(order.triggerPrice)
    return Number.isFinite(number) && number > 0 ? String(order.triggerPrice) : '--'
}

const formatOptionalNumber = (value: unknown) => {
    const number = Number(value)
    return Number.isFinite(number) && number > 0 ? String(value) : '--'
}

const formatTime = (ts: number) => {
    if (!ts) return ''
    return new Date(ts).toLocaleString()
}

const loadOrders = async () => {
    if (!isLoggedIn.value) {
        orders.value = []
        return
    }
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
        console.error(t('trade.load_orders_failed'), e)
    }
}

const openCancelModal = (order: any) => {
    cancelingOrder.value = order
    canceling.value = false
    showCancelModal.value = true
}

const confirmCancel = async () => {
    if (!cancelingOrder.value) return
    if (!isLoggedIn.value) {
        goToLogin()
        return
    }
    canceling.value = true
    try {
        await cancelOrder(cancelingOrder.value.orderId)
        toast.success(t('trade.cancel_success'))
        showCancelModal.value = false
        loadOrders()
    } catch (e: any) {
        toast.error(e.message || t('trade.cancel_failed'))
    } finally {
        canceling.value = false
    }
}

watch([() => props.symbol, activeTab, () => marketStore.orderRefreshKey, isLoggedIn], () => {
    loadOrders()
})

onMounted(() => {
    loadOrders()
})
</script>
