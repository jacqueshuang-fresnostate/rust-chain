<template>
  <div class="flex flex-col h-full overflow-hidden bg-background text-foreground">
    <!-- Header: Ticker Info -->
    <div class="h-16 min-h-[4rem] border-b border-border flex items-center px-4 bg-card justify-between z-10 shrink-0">
      <div class="flex items-center space-x-6 overflow-x-auto no-scrollbar w-full">
          <div class="flex items-center shrink-0">
             <h1 class="text-xl font-bold font-mono tracking-tight mr-2 flex items-center gap-2">
               <PairLogo class="h-8 w-8" :symbol="activeSymbol" :src="currentTicker?.icon" />
               {{ activeSymbol }}
             </h1>
             <span class="text-[10px] font-bold px-1.5 py-0.5 rounded bg-primary/10 text-primary border border-primary/20">SPOT</span>
          </div>
          <div class="h-8 w-px bg-border mx-2 shrink-0"></div>
          <div class="flex items-center space-x-3 shrink-0">
              <span :class="['text-2xl font-bold font-mono', (currentTicker?.chg || 0) >= 0 ? 'text-up' : 'text-down']">
                 {{ formatPrice(currentPrice) }}
              </span>
              <span class="text-sm text-muted-foreground font-medium">≈ ${{ formatPrice(currentPrice) }}</span>
          </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.change') }}</span>
                 <span :class="['text-sm font-bold font-mono', (currentTicker?.chg || 0) >= 0 ? 'text-up' : 'text-down']">
                    {{ (currentTicker?.chg || 0) >= 0 ? '+' : '' }}{{ numeral(currentTicker?.chg || 0).format('0.00') }}%
                 </span>
             </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.high') }}</span>
                 <span class="text-sm font-bold font-mono">{{ formatPrice(currentTicker?.high) }}</span>
             </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.low') }}</span>
                 <span class="text-sm font-bold font-mono">{{ formatPrice(currentTicker?.low) }}</span>
             </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.vol') }}(BTC)</span>
                 <span class="text-sm font-bold font-mono">{{ formatNumber(currentTicker?.volume) }}</span>
             </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.turnover') }}(USDT)</span>
                 <span class="text-sm font-bold font-mono">{{ formatNumber(currentTicker?.turnover) }}</span>
             </div>
         </div>

         <!-- Right Header Actions -->
         <div class="hidden md:flex items-center space-x-4 text-sm text-muted-foreground shrink-0 ml-4">
             <button class="hover:text-primary transition-colors flex items-center gap-1"><span class="i-lucide-book-open w-4 h-4"></span> {{ $t('trade.tutorial') }}</button>
             <button class="hover:text-primary transition-colors flex items-center gap-1"><span class="i-lucide-settings w-4 h-4"></span> {{ $t('trade.settings') }}</button>
         </div>
    </div>

    <!-- Main Layout -->
    <div class="flex-1 flex flex-col lg:flex-row overflow-y-auto lg:overflow-hidden">
      <!-- Left: Order Book -->
      <div class="w-full lg:w-[320px] h-[500px] lg:h-full border-b lg:border-b-0 lg:border-r border-border flex flex-col bg-card shrink-0 order-3 lg:order-1">
        <OrderBook :bids="orderBookBids" :asks="orderBookAsks" :currentPrice="currentPrice" class="flex-1" :symbol="activeSymbol" :visible-rows="20" />
      </div>

      <!-- Center: Chart & Order History -->
      <div class="w-full lg:flex-1 flex flex-col min-h-[500px] lg:h-full bg-background relative order-1 lg:order-2">
         <!-- Chart -->
         <div class="flex-1 border-b border-border relative flex flex-col min-h-[350px]">
             <!-- Chart Toolbar -->
             <div class="h-10 border-b border-border bg-card flex items-center px-4 gap-4 overflow-x-auto no-scrollbar shrink-0">
                <span :class="settingStore.chartProvider === 'klinecharts' ? 'text-sm font-bold text-primary border-b-2 border-primary h-full flex items-center px-2 whitespace-nowrap' : 'text-sm font-medium text-muted-foreground h-full flex items-center px-2 whitespace-nowrap'">{{ $t('trade.original') }}</span>
                <span :class="settingStore.chartProvider === 'tradingview' ? 'text-sm font-bold text-primary border-b-2 border-primary h-full flex items-center px-2 whitespace-nowrap' : 'text-sm font-medium text-muted-foreground h-full flex items-center px-2 whitespace-nowrap'">{{ $t('trade.tradingview') }}</span>
                <span class="text-sm font-medium text-muted-foreground hover:text-foreground cursor-pointer whitespace-nowrap">{{ $t('trade.depth') }}</span>
                <div class="w-px h-4 bg-border mx-2 shrink-0"></div>
                <span class="text-sm font-medium text-foreground cursor-pointer">1m</span>
                <span class="text-sm font-medium text-muted-foreground cursor-pointer">15m</span>
                <span class="text-sm font-medium text-muted-foreground cursor-pointer">1h</span>
                <span class="text-sm font-medium text-muted-foreground cursor-pointer">4h</span>
                <span class="text-sm font-medium text-muted-foreground cursor-pointer">1D</span>
             </div>
            <MarketChart v-if="activeSymbol" :dataList="chartData" :symbol="activeSymbol" :precision="precision" period="1m" class="flex-1" :key="`${activeSymbol}-${precision}`" />
         </div>
         <!-- Order History -->
         <div class="h-[260px] bg-card border-t-4 border-background shrink-0 flex flex-col z-20 relative">
            <OrderHistory :symbol="activeSymbol" />
         </div>
      </div>

      <!-- Right: Trade Form & Trades -->
      <div class="w-full lg:w-[340px] border-t lg:border-t-0 lg:border-l border-border flex flex-col bg-card shrink-0 order-2 lg:order-3">
         <div class="flex-none">
             <OrderForm :symbol="activeSymbol" :currentPrice="currentPrice" />
         </div>
         <!-- Market Trades -->
         <div class="h-[300px] lg:flex-1 border-t border-border flex flex-col min-h-0">
             <MarketTrades :symbol="activeSymbol" />
         </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import numeral from 'numeral'
import MarketChart from '@/components/chart/MarketChart.vue'
import PairLogo from '@/components/common/PairLogo.vue'
import OrderBook from '@/components/trade/OrderBook.vue'
import OrderForm from '@/components/trade/OrderForm.vue'
import MarketTrades from '@/components/trade/MarketTrades.vue'
import OrderHistory from '@/components/trade/OrderHistory.vue'
import { useMarketStore } from '@/stores/market'
import { useSettingStore } from '@/stores/setting'
import { fetchMarketSnapshot, fetchTradePlate } from '@/api/market'
import { stompService } from '@/api/stomp'
import { useAuthRequired } from '@/composables/useAuthRequired'

const route = useRoute()
const router = useRouter()
const marketStore = useMarketStore()
const settingStore = useSettingStore()
const { isLoggedIn } = useAuthRequired()
const activeSymbol = computed(() => marketStore.activeSymbol)
const currentTicker = computed(() =>
  marketStore.tickers.find(t => t.symbol === activeSymbol.value)
)
const currentPrice = computed(() => currentTicker.value?.close || 0)

// Data Refs
const orderBookBids = ref<any[]>([])
const orderBookAsks = ref<any[]>([])
const chartData = ref<any[]>([])

// Subscriptions
let plateSub: any = null
let tickerSub: any = null
let privateSub: any = null

const privateEventType = (msg: { body: string }) => {
    try {
        return String(JSON.parse(msg.body)?.type || '')
    } catch {
        return ''
    }
}

const handlePrivateEvent = (msg: { body: string }) => {
    const type = privateEventType(msg)
    if (type && !type.startsWith('spot.') && !type.startsWith('wallet.')) return
    marketStore.triggerOrderRefresh()
}

// URL Persistence Logic
watch(() => route.params.symbol, (newSymbol) => {
    if (newSymbol) {
        const symbolStr = Array.isArray(newSymbol) ? newSymbol[0] : newSymbol
        const formattedSymbol = symbolStr.replace('_', '/')
        if (marketStore.activeSymbol !== formattedSymbol) {
            marketStore.setActiveSymbol(formattedSymbol)
        }
    }
}, { immediate: true })

// Formatting helpers
const formatPrice = (val: number | undefined) => {
    if (val === undefined) return '0.00'
    return numeral(val).format(val < 1 ? '0.000000' : '0,0.00')
}

const formatNumber = (val: number | undefined) => {
    if (val === undefined) return '0.00a'
    return numeral(val).format('0.00a').toUpperCase()
}

const mapItems = (items: any[]) => {
    if (!Array.isArray(items)) return []
    return items.map((i: any) => ({
        price: Number(i.price),
        amount: Number(i.amount),
        total: Number(i.price) * Number(i.amount)
    }))
}

// Fetch Order Book
const refreshOrderBook = async () => {
    if (!activeSymbol.value) return
    try {
        const res = await fetchTradePlate(activeSymbol.value)
        if (res.data) {

            const data = res.data
            if (data.bids) orderBookBids.value = mapItems(data.bids)
            if (data.asks) orderBookAsks.value = mapItems(data.asks)
        }
    } catch (e) {
        console.error('Failed to fetch order book', e)
    }
}

// Subscribe to Data
const clearMarketDataSubscriptions = () => {
    if (plateSub) {
        plateSub.unsubscribe()
        plateSub = null
    }
    if (tickerSub) {
        tickerSub.unsubscribe()
        tickerSub = null
    }
}

const subscribeToData = async () => {
    if (!activeSymbol.value) return

    clearMarketDataSubscriptions()

    const topic = `spot:depth:${activeSymbol.value}`
    plateSub = await stompService.subscribe('spot', topic, (msg) => {
        try {
            const data = JSON.parse(msg.body)
            if (data.bids) orderBookBids.value = mapItems(data.bids)
            if (data.asks) orderBookAsks.value = mapItems(data.asks)
        } catch (e) {
            console.error(e)
        }
    })

    const tickerTopic = `spot:ticker:${activeSymbol.value}`
    tickerSub = await stompService.subscribe('spot', tickerTopic, (msg) => {
        try {
            marketStore.updateTicker(JSON.parse(msg.body))
        } catch (e) {
            console.error('Failed to parse spot ticker', e)
        }
    })
}
const precision = ref()

// const precision = computed(() => {
//   const val = currentPrice.value
//   // console.log("currentPrice", currentPrice.value)
//   if (val === 0) return 2
//   if (val < 0.1) return 6
//   if (val < 1) return 4
//
//   return 2
// })
watch(currentPrice, () => {
  if (!currentPrice.value) return
  let p = 2
  if (currentPrice.value < 0.1) p = 6
  else if (currentPrice.value < 1) p = 4
  if (currentTicker.value?.zone===1) p = 8
  if (precision.value !== p) {
    precision.value = p
  }

}, { immediate: true })

watch(activeSymbol, () => {
    refreshOrderBook()
    subscribeToData()
    // KLine and Trades are handled in their own components
})

onMounted(async () => {
    if (!route.params.symbol) {
        const urlSymbol = marketStore.activeSymbol.replace('/', '_')
        router.replace({ name: 'Trade', params: { symbol: urlSymbol } })
    }

    stompService.connect('spot')

    if (marketStore.tickers.length === 0) {
        try {
            const response = await fetchMarketSnapshot()
            const res = response.data
            // Adapt to API structure
            const data = Array.isArray(res) ? res : (res.data || [])
            marketStore.setTickers(data)
        } catch (error) {
            console.error('Failed to fetch market data:', error)
        }
    }

    refreshOrderBook()
    subscribeToData()
    if (isLoggedIn.value) {
        privateSub = await stompService.subscribePrivate(handlePrivateEvent)
    }
})

onUnmounted(() => {
    clearMarketDataSubscriptions()
    if (privateSub) privateSub.unsubscribe()
})
</script>

<style scoped>
.no-scrollbar::-webkit-scrollbar {
    display: none;
}
.no-scrollbar {
    -ms-overflow-style: none;
    scrollbar-width: none;
}
</style>
