<template>
  <div class="flex h-full">
    <!-- Market List Sidebar -->
    <div class="w-64 border-r border-border flex flex-col bg-card">
      <div class="p-4 border-b border-border font-bold text-foreground">
        Markets
      </div>
      <div class="overflow-auto flex-1">
        <div
          v-for="ticker in tickers"
          :key="ticker.symbol"
          class="p-3 cursor-pointer hover:bg-muted flex justify-between items-center transition-colors"
          :class="{ 'bg-muted': activeSymbol === ticker.symbol }"
          @click="selectSymbol(ticker.symbol)"
        >
          <div>
            <div class="font-bold text-sm">{{ ticker.symbol }}</div>
            <div class="text-xs text-muted-foreground">Vol {{ formatNumber(ticker.volume, 'volume') }}</div>
          </div>
          <div class="text-right">
            <div class="font-medium text-sm" :class="(ticker.chg || 0) >= 0 ? 'text-up' : 'text-down'">
              {{ formatNumber(ticker.close, 'price') }}
            </div>
            <div class="text-xs" :class="(ticker.chg || 0) >= 0 ? 'text-up' : 'text-down'">
              {{ (ticker.chg || 0) >= 0 ? '+' : '' }}{{ formatChange(ticker.chg || 0) }}%
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Main Chart Area -->
    <div class="flex-1 flex flex-col relative">
      <div class="h-12 border-b border-border flex items-center px-4 justify-between bg-card">
        <div class="flex items-center gap-4">
          <h2 class="text-lg font-bold">{{ activeSymbol }}</h2>
          <span class="text-sm font-medium" :class="(currentTicker?.chg || 0) >= 0 ? 'text-up' : 'text-down'">
            {{ currentTicker ? formatNumber(currentTicker.close, 'price') : '---' }}
          </span>
        </div>
        <div class="flex gap-2">
            <!-- Replaced NButton with standard buttons for now since we removed Naive UI in favor of shadcn-like styles -->
            <button class="px-2 py-1 text-xs rounded hover:bg-muted transition-colors">15m</button>
            <button class="px-2 py-1 text-xs rounded hover:bg-muted transition-colors">1h</button>
            <button class="px-2 py-1 text-xs rounded hover:bg-muted transition-colors">4h</button>
            <button class="px-2 py-1 text-xs rounded hover:bg-muted transition-colors">1d</button>
        </div>
      </div>
      <div class="flex-1 bg-background p-1">
         <TVChart :dataList="chartData" :symbol="activeSymbol" period="1m" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useMarketStore } from '@/stores/market'
import { fetchMarketSnapshot, fetchHistoryKLine } from '@/api/market'
import TVChart from '@/components/chart/TVChart.vue'
import { formatNumber } from '@/utils/format'
import numeral from 'numeral'

function formatChange(val: number) {
    return numeral(val).format('0.00')
}

const marketStore = useMarketStore()
const activeSymbol = computed(() => marketStore.activeSymbol)
const tickers = computed(() => marketStore.tickers)

const currentTicker = computed(() =>
  tickers.value.find(t => t.symbol === activeSymbol.value)
)

const chartData = ref<any[]>([])

// Real implementation to fetch chart data
async function loadChartData(symbol: string) {
  try {
    const to = new Date().getTime()
    const from = to - (24 * 60 * 60 * 1000) // 24 hours ago
    const res = await fetchHistoryKLine(symbol, '15m', from, to)
    // Map data to KLine structure: { timestamp, open, high, low, close, volume }
    if (res.data && Array.isArray(res.data)) {
        chartData.value = res.data.map((item: any) => ({
            timestamp: item[0],
            open: item[1],
            high: item[2],
            low: item[3],
            close: item[4],
            volume: item[5]
        }))
    }
  } catch (e) {
    console.error("Failed to load chart data", e)
    // Fallback to mock if API fails during dev
    // generateMockData()
  }
}

function selectSymbol(symbol: string) {
  marketStore.setActiveSymbol(symbol)
  loadChartData(symbol)
}

onMounted(async () => {
  if (marketStore.tickers.length === 0) {
    try {
      const response = await fetchMarketSnapshot()
      // ... (existing logic)
      const res = response.data
      if (Array.isArray(res)) {
        marketStore.setTickers(res)
      } else if (res && Array.isArray(res.data)) {
        marketStore.setTickers(res.data)
      }
    } catch (error) {
      console.error('Failed to fetch market data:', error)
    }
  }
  loadChartData(activeSymbol.value)
})
</script>
