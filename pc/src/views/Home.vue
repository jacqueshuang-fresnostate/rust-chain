<template>
  <div class="min-h-full flex flex-col items-center relative overflow-y-auto bg-background/50">
    <!-- Background Elements -->
    <div class="absolute inset-0 z-0 overflow-hidden pointer-events-none">
       <div class="absolute top-[-20%] left-[-10%] w-[50%] h-[50%] bg-primary/10 rounded-full blur-[120px] animate-pulse"></div>
       <div class="absolute bottom-[-10%] right-[-10%] w-[50%] h-[50%] bg-neon-pink/5 rounded-full blur-[120px] animate-pulse" style="animation-delay: 2s"></div>
    </div>

    <!-- Hero Section with Glass Card -->
    <div class="relative z-10 w-full max-w-7xl px-4 pt-10 pb-8 grid grid-cols-1 lg:grid-cols-12 gap-8">
      <!-- Main CTA (Left) -->
      <div class="lg:col-span-8 flex flex-col justify-center text-left">
        <h1 class="text-5xl md:text-7xl font-black mb-6 leading-tight tracking-tighter">
           {{ $t('home.hero_title') }} <br />
           <span class="text-transparent bg-clip-text bg-gradient-to-r from-neon-blue to-neon-green text-glow">DECENTRALIZED</span>
           TRADING
        </h1>
        <p class="text-lg text-muted-foreground mb-8 max-w-2xl">
           {{ $t('home.hero_subtitle') }}
        </p>
        <div class="flex flex-wrap gap-4">
           <button @click="$router.push('/trade')" class="px-8 py-3 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-all box-glow flex items-center gap-2">
             {{ $t('home.cta') }}
             <Icon icon="mdi:arrow-right" />
           </button>
           <button class="px-8 py-3 border border-border bg-card/30 backdrop-blur font-bold rounded-lg hover:border-primary hover:text-primary transition-all">
             {{ $t('home.view_docs') || 'View Documentation' }}
           </button>
        </div>
      </div>

      <!-- Hero Stats / News (Right) -->
      <div class="lg:col-span-4 flex flex-col gap-6">
        <!-- 24h Volume Card -->
        <div class="bg-card/40 backdrop-blur border border-border rounded-xl p-6 shadow-neon relative overflow-hidden group">
           <div class="absolute right-0 top-0 p-4 opacity-10 group-hover:opacity-20 transition-opacity">
             <Icon icon="mdi:chart-bar-stacked" class="w-24 h-24 text-primary" />
           </div>
           <div class="text-sm text-muted-foreground mb-1">24h {{ $t('market.vol') }}</div>
           <div class="text-3xl font-mono font-bold text-glow">$1,245,678,901</div>
           <div class="text-xs text-up mt-2 flex items-center gap-1">
             <Icon icon="mdi:trending-up" />
             +12.5% vs yesterday
           </div>
        </div>

        <!-- News Ticker Component -->
        <NewsTicker />
      </div>
    </div>

    <!-- Market Ticker Board (Table Style) -->
    <div class="relative z-10 w-full max-w-7xl px-4 pb-16">
      <div class="flex items-center justify-between mb-6">
        <h2 class="text-2xl font-bold flex items-center gap-2">
           <Icon icon="mdi:fire" class="text-neon-pink" />
           Trending Markets
        </h2>
      </div>

      <div class="bg-card/40 backdrop-blur border border-border rounded-xl overflow-hidden shadow-2xl">
        <table class="w-full text-left border-collapse">
          <thead>
            <tr class="border-b border-border text-muted-foreground text-sm">
              <th class="p-4 font-medium">{{ $t('trade.symbol') }}</th>
              <th class="p-4 font-medium text-right">{{ $t('trade.price') }}</th>
              <th class="p-4 font-medium text-right">{{ $t('market.change') }}</th>
              <th class="p-4 font-medium text-right hidden md:table-cell">24h {{ $t('market.vol') }}</th>
              <th class="p-4 font-medium text-right hidden lg:table-cell">{{ $t('home.chart') || 'Chart' }}</th>
              <th class="p-4 font-medium text-center">{{ $t('trade.action') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="ticker in tickers" :key="ticker.symbol"
                class="hover:bg-muted/30 transition-colors group border-b border-border/50 last:border-0"
            >
              <td class="p-4">
                <div class="flex items-center gap-3">
                  <div class="w-8 h-8 rounded-lg bg-background/50 border border-border flex items-center justify-center text-lg">
                     <!-- Dynamic Icon Handling based on symbol name -->
<!--                     <Icon v-if="ticker.symbol.includes('BTC')" icon="mdi:bitcoin" class="text-[#F7931A]" />-->
<!--                     <Icon v-else-if="ticker.symbol.includes('ETH')" icon="mdi:ethereum" class="text-[#627EEA]" />-->
<!--                     <Icon v-else-if="ticker.symbol.includes('SOL')" icon="simple-icons:solana" class="text-[#14F195]" />-->
<!--                     <Icon v-else-if="ticker.symbol.includes('BNB')" icon="simple-icons:binance" class="text-[#F3BA2F]" />-->
<!--                     <Icon v-else icon="mdi:currency-usd-circle-outline" class="text-muted-foreground" />-->
                    <img :src="ticker.icon" alt="" />
                  </div>
                  <div>
                    <div class="font-bold flex items-center gap-2">
                      {{ ticker.symbol }}
                      <span class="px-1.5 py-0.5 rounded text-[10px] bg-muted text-muted-foreground font-normal">SPOT</span>
                    </div>
                    <!-- Zone or other meta info -->
                    <div class="text-xs text-muted-foreground">Zone {{ ticker.zone }}</div>
                  </div>
                </div>
              </td>
              <td class="p-4 text-right font-mono font-bold text-lg">
                {{ formatNumber(ticker.close, 'price') }}
              </td>
              <td class="p-4 text-right">
                <div class="inline-flex items-center px-2 py-1 rounded text-xs font-bold" :class="ticker.chg >= 0 ? 'bg-up/10 text-up' : 'bg-down/10 text-down'">
                  {{ ticker.chg >= 0 ? '+' : '' }}{{ formatChange(ticker.chg) }}%
                </div>
              </td>
              <td class="p-4 text-right text-muted-foreground font-mono text-sm hidden md:table-cell">
                {{ formatNumber(ticker.volume, 'volume') }}
              </td>
              <td class="p-4 text-right hidden lg:table-cell w-32">
                 <div class="h-8 w-24 ml-auto opacity-50 group-hover:opacity-100 transition-opacity">
                   <svg viewBox="0 0 100 30" class="w-full h-full stroke-current" :class="ticker.chg >= 0 ? 'text-up' : 'text-down'" fill="none" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                      <!-- Simple trend line simulation based on OHLC if available or just chg direction -->
                      <path :d="generateChartPath(ticker)" />
                   </svg>
                 </div>
              </td>
              <td class="p-4 text-center">
                <button @click="goToTrade(ticker.symbol)" class="px-4 py-1.5 bg-primary/10 text-primary border border-primary/20 rounded hover:bg-primary hover:text-primary-foreground transition-all text-sm font-bold">
                  {{ $t('nav.trade') }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, computed } from 'vue'
import { useRouter } from 'vue-router'
import { Icon } from '@iconify/vue'
import NewsTicker from '@/components/home/NewsTicker.vue'
import { useMarketStore } from '@/stores/market'
import { fetchMarketSnapshot } from '@/api/market'
import { stompService } from '@/api/stomp'
import { formatNumber } from '@/utils/format'

import numeral from 'numeral'
function formatChange(val: number) {
    return numeral(val).format('0.00')
}
const router = useRouter()
const marketStore = useMarketStore()
const tickers = computed(() => marketStore.tickers)

function goToTrade(symbol: string) {
    marketStore.setActiveSymbol(symbol)
    const urlSymbol = symbol.replace('/', '_')
    router.push({ name: 'Trade', params: { symbol: urlSymbol } })
}

function generateChartPath(ticker: any) {
  // Simple visualization logic: Open at left, Close at right
  // Ideally this needs historical data, but for snapshot we simulate based on Open/Close
  // M0 Y1 L100 Y2
  const min = Math.min(ticker.low, ticker.open, ticker.close)
  const max = Math.max(ticker.high, ticker.open, ticker.close)
  const range = max - min || 1

  const normalize = (val: number) => 30 - ((val - min) / range) * 30

  const y1 = normalize(ticker.open)
  const y2 = normalize(ticker.close)
  const yHigh = normalize(ticker.high)
  const yLow = normalize(ticker.low)

  // Draw a simple path: Start -> High -> Low -> End
  return `M0 ${y1} L33 ${yHigh} L66 ${yLow} L100 ${y2}`
}

onMounted(async () => {
  // 1. Fetch initial snapshot
  try {
    const res = await fetchMarketSnapshot()
    // Depending on API response structure, adjust accordingly
    // Assuming res.data contains the list directly or in a property
    const data = Array.isArray(res.data) ? res.data : (res.data.data || [])
     marketStore.setTickers(data)
  } catch (e) {
    console.error('Failed to fetch market snapshot', e)
  }
  // 2. Connect WebSocket
  stompService.connect()


})

onUnmounted(() => {
  stompService.disconnect()
})
</script>
