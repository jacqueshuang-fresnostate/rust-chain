<template>
  <div class="flex flex-col h-full overflow-hidden bg-background text-foreground">
    <!-- Header: Ticker Info (Desktop Style) -->
    <div class="h-16 min-h-[4rem] border-b border-border flex items-center px-4 bg-card justify-between z-10 shrink-0">
      <div class="flex items-center space-x-6 overflow-x-auto no-scrollbar w-full">
          <div class="flex items-center shrink-0">
             <h1 class="text-xl font-bold font-mono tracking-tight mr-2 flex items-center gap-2">
               <PairLogo class="h-8 w-8" :symbol="activeSymbol" :src="currentTicker?.icon" />
               {{ activeSymbol }}
             </h1>
             <span class="text-[10px] font-bold px-1.5 py-0.5 rounded bg-neon-purple/10 text-neon-purple border border-neon-purple/20">{{ $t('nav.launchpad').toUpperCase() }}</span>
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
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.vol') }}({{ baseSymbol }})</span>
                 <span class="text-sm font-bold font-mono">{{ formatNumber(currentTicker?.volume) }}</span>
             </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ $t('market.turnover') }}({{ quoteSymbol }})</span>
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

      <!-- Left: Chart & Order History -->
      <div class="w-full lg:flex-1 flex flex-col min-h-[500px] lg:h-full bg-background relative">
         <!-- Chart -->
         <div class="flex-1 border-b lg:border-r lg:border-b-0 border-border relative flex flex-col min-h-[400px]">
             <!-- Chart Toolbar -->
             <div class="h-10 border-b border-border bg-card flex items-center px-4 gap-4 overflow-x-auto no-scrollbar shrink-0">
                <span :class="settingStore.chartProvider === 'klinecharts' ? 'text-sm font-bold text-primary border-b-2 border-primary h-full flex items-center px-2 whitespace-nowrap' : 'text-sm font-medium text-muted-foreground h-full flex items-center px-2 whitespace-nowrap'">{{ $t('trade.original') }}</span>
                <span :class="settingStore.chartProvider === 'tradingview' ? 'text-sm font-bold text-primary border-b-2 border-primary h-full flex items-center px-2 whitespace-nowrap' : 'text-sm font-medium text-muted-foreground h-full flex items-center px-2 whitespace-nowrap'">{{ $t('trade.tradingview') }}</span>
                <div class="w-px h-4 bg-border mx-2 shrink-0"></div>
                <span class="text-sm font-medium text-foreground cursor-pointer">1m</span>
                <span class="text-sm font-medium text-muted-foreground cursor-pointer">15m</span>
                <span class="text-sm font-medium text-muted-foreground cursor-pointer">1h</span>
                <span class="text-sm font-medium text-muted-foreground cursor-pointer">4h</span>
                <span class="text-sm font-medium text-muted-foreground cursor-pointer">1D</span>
             </div>
            <MarketChart v-if="activeSymbol" :dataList="chartData" :symbol="activeSymbol" :precision="precision" period="1m" class="flex-1" :key="`${activeSymbol}-${precision}`" />
         </div>

         <!-- Order History (Bottom Left) -->
         <div class="h-[300px] bg-card border-t border-border lg:border-r shrink-0 flex flex-col z-20 relative">
            <OrderHistory :symbol="activeSymbol" />
         </div>
      </div>

      <!-- Right: Form & Details (Sidebar) -->
      <div class="w-full lg:w-[340px] flex flex-col bg-card shrink-0">
         <!-- Order Form -->
         <div class="flex-none border-b border-border">
             <OrderForm :symbol="activeSymbol" :currentPrice="currentPrice" :buyOnly="true" />
         </div>

         <!-- Purchased Amount Indicator -->
         <div class="flex-1 p-4 bg-background/50">
             <div class="bg-card border border-border rounded-xl p-5 shadow-sm space-y-2">
                 <div class="flex items-center gap-2 text-muted-foreground">
                    <span class="i-lucide-award w-4 h-4"></span>
                    <span class="text-sm font-medium">{{ $t('trade.purchased_amount') }}</span>
                 </div>
                 <div class="flex justify-between items-baseline mt-2">
                     <span class="text-3xl font-mono font-bold">{{ purchasedAmount }}</span>
                     <span class="text-sm font-bold bg-muted px-2 py-0.5 rounded text-muted-foreground">{{ baseSymbol }}</span>
                 </div>
             </div>
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
import OrderForm from '@/components/trade/OrderForm.vue'
import OrderHistory from '@/components/trade/OrderHistory.vue'
import { useMarketStore } from '@/stores/market'
import { useSettingStore } from '@/stores/setting'
import { fetchMarketSnapshot } from '@/api/market'

const route = useRoute()
const router = useRouter()
const marketStore = useMarketStore()
const settingStore = useSettingStore()
const activeSymbol = computed(() => marketStore.activeSymbol)
const currentTicker = computed(() =>
  marketStore.tickers.find(t => t.symbol === activeSymbol.value)
)
const currentPrice = computed(() => currentTicker.value?.close || 0)

const baseSymbol = computed(() => activeSymbol.value?.split('/')[0] || 'BTC')
const quoteSymbol = computed(() => activeSymbol.value?.split('/')[1] || 'USDT')

// Data Refs
const chartData = ref<any[]>([])
const purchasedAmount = ref(0) // Can be linked to real wallet data later

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

const precision = ref()

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

onMounted(async () => {
    if (!route.params.symbol) {
        const urlSymbol = marketStore.activeSymbol.replace('/', '_')
        router.replace({ name: 'LaunchpadTrade', params: { symbol: urlSymbol } })
    }

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
})

onUnmounted(() => {
    // any cleanup
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
