<template>
  <div class="flex flex-col h-full bg-card">
    <div class="p-2 border-b border-border font-bold text-sm text-muted-foreground">Markets</div>
    <div class="flex-1 overflow-auto">
      <div v-for="ticker in tickers" :key="ticker.symbol"
           class="flex justify-between items-center px-3 py-2 hover:bg-muted cursor-pointer transition-colors"
           :class="{ 'bg-muted': activeSymbol === ticker.symbol }"
           @click="selectSymbol(ticker.symbol)">
        <div>
          <div class="text-xs font-bold">{{ ticker.symbol }}</div>
          <div class="text-[10px] text-muted-foreground">Vol {{ formatNumber(ticker.volume, 'volume') }}</div>
        </div>
        <div class="text-right">
          <div class="text-xs font-medium" :class="ticker.chg >= 0 ? 'text-up' : 'text-down'">{{ formatNumber(ticker.close, 'price') }}</div>
          <div class="text-[10px]" :class="ticker.chg >= 0 ? 'text-up' : 'text-down'">{{ ticker.chg >= 0 ? '+' : '' }}{{ formatChange(ticker.chg) }}%</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useMarketStore } from '@/stores/market'
import { formatNumber } from '@/utils/format'
import numeral from 'numeral'

function formatChange(val: number) {
    return numeral(val).format('0.00')
}

const marketStore = useMarketStore()
const tickers = computed(() => marketStore.tickers)
const activeSymbol = computed(() => marketStore.activeSymbol)

function selectSymbol(symbol: string) {
  marketStore.setActiveSymbol(symbol)
}
</script>
