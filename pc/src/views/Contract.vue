<template>
  <div v-if="!isLoggedIn" class="h-full bg-background p-4 text-foreground md:p-8">
    <AuthRequiredState />
  </div>
  <div v-else class="flex flex-col h-full overflow-hidden bg-background text-foreground">
    <!-- Header: Ticker Info -->
    <div class="h-16 min-h-[4rem] border-b border-border flex items-center px-4 bg-card justify-between z-10">
      <div class="flex items-center space-x-6 overflow-x-auto no-scrollbar">
          <div class="flex items-center shrink-0">
             <div id="pair-trigger-btn" class="cursor-pointer flex items-center hover:opacity-80 transition-opacity" @click.stop="toggleDropdown">
               <h1 class="text-xl font-bold font-mono tracking-tight mr-2 flex items-center gap-2">
                 <PairLogo class="h-8 w-8" :symbol="activeSymbol" :src="activeCoinInfo?.logoUrl" />
                 {{ activeSymbol }}
                 <span class="i-lucide-chevron-down w-5 h-5 ml-1 text-muted-foreground transition-transform" :class="showPairDropdown ? 'rotate-180' : ''"></span>
               </h1>
             </div>
             <span class="text-[10px] font-bold px-1.5 py-0.5 rounded bg-orange-500/10 text-orange-500 border border-orange-500/20">PERP</span>
          </div>
          <div class="h-8 w-px bg-border mx-2 shrink-0"></div>
          <div class="flex items-center space-x-3 shrink-0">
              <span :class="['text-2xl font-bold font-mono', (currentThumb?.change || 0) >= 0 ? 'text-up' : 'text-down']">
                 {{ formatPrice(currentPrice) }}
              </span>
              <span class="text-sm text-muted-foreground font-medium">≈ ${{ formatPrice(currentPrice) }}</span>
          </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.high') }}</span>
                 <span class="text-sm font-bold font-mono">{{ formatPrice(currentThumb?.high) }}</span>
             </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.low') }}</span>
                 <span class="text-sm font-bold font-mono">{{ formatPrice(currentThumb?.low) }}</span>
             </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.change') }}</span>
                 <span :class="['text-sm font-bold font-mono', (currentThumb?.change || 0) >= 0 ? 'text-up' : 'text-down']">
                    {{ (currentThumb?.change || 0) >= 0 ? '+' : '' }}{{ numeral(currentThumb?.change || 0).format('0.00') }}%
                 </span>
             </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.vol') }}</span>
                 <span class="text-sm font-bold font-mono">{{ formatNumber(currentThumb?.vol) }}</span>
             </div>
             <div class="flex flex-col shrink-0" v-if="activeCoinInfo">
                 <span class="text-xs text-muted-foreground">{{ $t('trade.max_leverage') }}</span>
                 <span class="text-sm font-bold font-mono">{{ activeCoinInfo.maxLeverage }}x</span>
             </div>
             <div class="flex flex-col shrink-0" v-if="activeCoinInfo">
                 <span class="text-xs text-muted-foreground">{{ $t('trade.margin_rate') }}</span>
                 <span class="text-sm font-bold font-mono">{{ numeral(activeCoinInfo.marginRate).format('0.[00]') }}%</span>
             </div>
             <div class="flex flex-col shrink-0" v-if="activeCoinInfo">
                 <span class="text-xs text-muted-foreground">{{ $t('trade.min_turnover') }}</span>
                 <span class="text-sm font-bold font-mono">{{ activeCoinInfo.minTurnover }} {{ activeCoinInfo.baseSymbol }}</span>
             </div>
         </div>

         <!-- Right Header Actions -->
         <div class="flex items-center space-x-4 text-sm text-muted-foreground shrink-0 ml-4">
             <button class="hover:text-primary transition-colors flex items-center gap-1"><span class="i-lucide-book-open w-4 h-4"></span> {{ $t('trade.tutorial') }}</button>
             <button class="hover:text-primary transition-colors flex items-center gap-1"><span class="i-lucide-settings w-4 h-4"></span> {{ $t('trade.settings') }}</button>
         </div>

         <!-- Click-away backdrop -->
         <div v-if="showPairDropdown" class="fixed inset-0 z-40" @click="showPairDropdown = false"></div>
    </div>

    <!-- Pair Dropdown: Teleported to body to avoid overflow/stacking issues -->
    <Teleport to="body">
       <div v-if="showPairDropdown" class="fixed z-[9999] w-[320px] rounded-lg overflow-hidden flex flex-col"
            :style="{ top: dropdownPos.top + 'px', left: dropdownPos.left + 'px', backgroundColor: '#13131B', border: '1px solid rgba(255,255,255,0.12)', boxShadow: '0 20px 60px rgba(0,0,0,0.9)', minHeight: '300px' }">
           <div class="p-3 border-b text-xs font-medium flex justify-between" style="background-color: #1A1A24; border-color: rgba(255,255,255,0.08); color: rgba(255,255,255,0.5);">
               <span>{{ $t('trade.pair') }}</span>
               <span>{{ $t('trade.price') }}</span>
           </div>
           <div class="max-h-[400px] overflow-y-auto" style="background-color: #13131B;">
               <div v-for="coin in contractStore.coins" :key="coin.symbol"
                    @click="switchPair(coin.symbol)"
                    class="flex items-center justify-between p-3 cursor-pointer transition-colors border-b last:border-0"
                    :class="activeSymbol === coin.symbol ? 'bg-[rgba(0,200,150,0.12)]' : 'hover:bg-[rgba(255,255,255,0.05)]'"
                    style="border-color: rgba(255,255,255,0.05);">
                   <div class="flex min-w-0 items-center gap-2">
                       <PairLogo class="h-7 w-7" :symbol="coin.symbol" :src="coin.logoUrl" />
                       <div class="flex min-w-0 flex-col">
                           <span class="truncate text-sm font-bold font-mono" :class="activeSymbol === coin.symbol ? 'text-primary' : 'text-white'">{{ coin.symbol }}</span>
                           <span class="text-[10px]" style="color: rgba(255,255,255,0.4);">{{ coin.maxLeverage ? coin.maxLeverage + 'x' : '' }}</span>
                       </div>
                   </div>
                   <div class="flex flex-col items-end">
                       <span class="text-sm font-mono font-medium" :class="(contractStore.getThumbBySymbol(coin.symbol)?.change || 0) >= 0 ? 'text-up' : 'text-down'">
                           {{ formatPrice(contractStore.getThumbBySymbol(coin.symbol)?.last) }}
                       </span>
                       <span class="text-[10px] font-mono" :class="(contractStore.getThumbBySymbol(coin.symbol)?.change || 0) >= 0 ? 'text-up' : 'text-down'">
                           {{ (contractStore.getThumbBySymbol(coin.symbol)?.change || 0) >= 0 ? '+' : '' }}{{ numeral(contractStore.getThumbBySymbol(coin.symbol)?.change || 0).format('0.00') }}%
                       </span>
                   </div>
               </div>
           </div>
       </div>
    </Teleport>
    <!-- Main Layout -->
    <div class="flex-1 flex overflow-hidden">
      <!-- Left: Order Book -->
      <div class="w-[320px] border-r border-border flex flex-col bg-card shrink-0">
        <OrderBook :bids="orderBookBids" :asks="orderBookAsks" :currentPrice="currentPrice" class="flex-1" :symbol="activeSymbol" />
      </div>

      <!-- Center: Chart & Order History -->
      <div class="flex-1 flex flex-col min-w-0 bg-background relative">
         <!-- Chart -->
         <div class="flex-1 border-b border-border relative flex flex-col">
             <!-- Chart Toolbar Placeholder -->
             <div class="h-10 border-b border-border bg-card flex items-center px-4 gap-4">
                <span :class="settingStore.chartProvider === 'klinecharts' ? 'text-sm font-bold text-primary border-b-2 border-primary h-full flex items-center px-2' : 'text-sm font-medium text-muted-foreground h-full flex items-center px-2'">{{ $t('trade.original') }}</span>
                <span :class="settingStore.chartProvider === 'tradingview' ? 'text-sm font-bold text-primary border-b-2 border-primary h-full flex items-center px-2' : 'text-sm font-medium text-muted-foreground h-full flex items-center px-2'">{{ $t('trade.tradingview') }}</span>
                <span class="text-sm font-medium text-muted-foreground hover:text-foreground cursor-pointer">{{ $t('trade.depth') }}</span>
                <div class="w-px h-4 bg-border mx-2"></div>
                <span class="text-sm font-medium text-foreground">1m</span>
                <span class="text-sm font-medium text-muted-foreground">15m</span>
                <span class="text-sm font-medium text-muted-foreground">1h</span>
                <span class="text-sm font-medium text-muted-foreground">4h</span>
                <span class="text-sm font-medium text-muted-foreground">1D</span>
             </div>
            <MarketChart v-if="activeSymbol" :dataList="chartData" :symbol="activeSymbol" module="margin" period="1m" class="flex-1" />
         </div>
         <!-- Order History -->
         <div class="h-[320px] bg-card border-t-4 border-background shrink-0 flex flex-col">
            <ContractOrders :symbol="activeSymbol" />
         </div>
      </div>

      <!-- Right: Trade Form & Trades -->
      <div class="w-[340px] border-l border-border flex flex-col bg-card shrink-0">
         <div class="flex-none h-[60%] border-b border-border">
             <ContractOrderForm :symbol="activeSymbol" :currentPrice="currentPrice" />
         </div>
         <!-- Market Trades -->
         <div class="flex-1 flex flex-col min-h-0">
<!--             <div class="p-2 text-xs font-bold text-muted-foreground border-b border-border">Market Trades</div>-->
             <MarketTrades :symbol="activeSymbol" module="margin" />
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
import AuthRequiredState from '@/components/common/AuthRequiredState.vue'
import PairLogo from '@/components/common/PairLogo.vue'
import OrderBook from '@/components/trade/OrderBook.vue'
import ContractOrderForm from '@/components/trade/ContractOrderForm.vue'
import MarketTrades from '@/components/trade/MarketTrades.vue'
import ContractOrders from '@/components/trade/ContractOrders.vue'
import { useMarketStore } from '@/stores/market'
import { useContractStore } from '@/stores/contract'
import { useSettingStore } from '@/stores/setting'
import type { ContractCoin } from '@/stores/contract'
import { fetchExchangePlate } from '@/api/contract'
import { stompService } from '@/api/stomp'
import { useAuthRequired } from '@/composables/useAuthRequired'

const route = useRoute()
const router = useRouter()
const marketStore = useMarketStore()
const contractStore = useContractStore()
const settingStore = useSettingStore()
const { isLoggedIn } = useAuthRequired()

const activeSymbol = computed(() => contractStore.activeCoin?.symbol || '')
const currentThumb = computed(() => contractStore.getThumbBySymbol(activeSymbol.value))
const activeCoinInfo = computed(() => contractStore.activeCoin)
const currentPrice = computed(() => currentThumb.value?.last || 0)

// Data Refs
const orderBookBids = ref<any[]>([])
const orderBookAsks = ref<any[]>([])
const chartData = ref<any[]>([])
const showPairDropdown = ref(false)
const dropdownPos = ref({ top: 0, left: 0 })
const contractProductsReady = ref(false)

const toggleDropdown = (e: MouseEvent) => {
    const btn = (e.currentTarget as HTMLElement).getBoundingClientRect()
    dropdownPos.value = { top: btn.bottom + 8, left: btn.left }
    showPairDropdown.value = !showPairDropdown.value
}

// Subscriptions
let plateSub: any = null
let thumbSub: any = null
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
    if (type && !type.startsWith('margin.') && !type.startsWith('wallet.')) return
    contractStore.triggerOrderRefresh()
    void contractStore.loadWallets()
}

const routeParamToSymbol = (symbol: unknown) => {
    if (!symbol) return null
    const symbolStr = Array.isArray(symbol) ? symbol[0] : String(symbol)
    const trimmed = symbolStr.trim()
    if (!trimmed) return null
    return trimmed.replace(/[-_]/g, '/').toUpperCase()
}

const symbolToRouteParam = (symbol: string) => symbol.replace(/\//g, '_')

const currentRouteSymbolParam = () => {
    const symbol = route.params.symbol
    return Array.isArray(symbol) ? symbol[0] : symbol
}

const clearMarketDataSubscriptions = () => {
    if (plateSub) {
        plateSub.unsubscribe()
        plateSub = null
    }
    if (thumbSub) {
        thumbSub.unsubscribe()
        thumbSub = null
    }
}

const clearContractMarketData = () => {
    clearMarketDataSubscriptions()
    orderBookBids.value = []
    orderBookAsks.value = []
    chartData.value = []
}

const syncRouteToContractCoin = async (coin: ContractCoin) => {
    const urlSymbol = symbolToRouteParam(coin.symbol)
    if (currentRouteSymbolParam() !== urlSymbol) {
        await router.replace({ name: 'Contract', params: { symbol: urlSymbol } })
    }
}

const resolveContractRouteSymbol = async (routeSymbol: string | null) => {
    const requestedCoin = routeSymbol ? contractStore.getCoinBySymbol(routeSymbol) : null
    const resolvedCoin = requestedCoin || contractStore.coins[0] || null

    if (!resolvedCoin) {
        contractStore.setActiveCoin(null)
        clearContractMarketData()
        return null
    }

    contractStore.setActiveCoin(resolvedCoin)
    if (marketStore.activeSymbol !== resolvedCoin.symbol) {
        marketStore.setActiveSymbol(resolvedCoin.symbol)
    }
    await syncRouteToContractCoin(resolvedCoin)
    return resolvedCoin
}

// URL Persistence Logic
watch(() => route.params.symbol, (newSymbol) => {
    if (!contractProductsReady.value) return
    void resolveContractRouteSymbol(routeParamToSymbol(newSymbol))
})

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

const switchPair = (symbol: string) => {
    const urlSymbol = symbolToRouteParam(symbol)
    router.push({ name: 'Contract', params: { symbol: urlSymbol } })
    showPairDropdown.value = false
}

// Fetch Order Book
const refreshOrderBook = async () => {
    if (!activeSymbol.value) return
    try {
        const res = await fetchExchangePlate(activeSymbol.value)
        if (res.data?.data) {
            const data = res.data.data
            if (data.bids) orderBookBids.value = mapItems(data.bids)
            if (data.asks) orderBookAsks.value = mapItems(data.asks)
        }
    } catch (e) {
        console.error('Failed to fetch order book', e)
    }
}

// Subscribe to Data
const subscribeToData = async () => {
    if (!activeSymbol.value) return

    clearMarketDataSubscriptions()

    const depthTopic = `margin:depth:${activeSymbol.value}`
    plateSub = await stompService.subscribe('margin', depthTopic, (msg) => {
        try {
            const data = JSON.parse(msg.body)
            if (data.bids) orderBookBids.value = mapItems(data.bids)
            if (data.asks) orderBookAsks.value = mapItems(data.asks)
        } catch (e) {
            console.error(e)
        }
    })

    const tickerTopic = `margin:ticker:${activeSymbol.value}`
    thumbSub = await stompService.subscribe('margin', tickerTopic, (msg) => {
        try {
            contractStore.updateThumb(JSON.parse(msg.body))
        } catch (e) {
            console.error('Failed to parse margin ticker', e)
        }
    })
}

watch(activeSymbol, (newSymbol) => {
    if (!contractProductsReady.value || !newSymbol) return
    refreshOrderBook()
    subscribeToData()
})

onMounted(async () => {
    if (!isLoggedIn.value) return

    stompService.connect('margin')

    // Load contract data
    await contractStore.loadCoins()
    await resolveContractRouteSymbol(routeParamToSymbol(route.params.symbol))
    contractProductsReady.value = true
    await contractStore.loadThumbs()
    if (isLoggedIn.value) {
        await contractStore.loadWallets()
    }

    if (activeSymbol.value) {
        refreshOrderBook()
        subscribeToData()
    }
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
