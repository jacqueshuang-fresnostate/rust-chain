<template>
  <div class="h-full flex flex-col bg-card text-xs">
    <div class="p-2 border-b border-border font-bold text-muted-foreground flex justify-between">
        <span>{{ $t('trade.limit') }}</span>
        <div class="flex gap-2 text-[10px] text-muted-foreground font-normal">
            <span>{{ $t('trade.price') }}</span>
            <span>{{ $t('trade.amount') }}</span>
            <span>{{ $t('trade.total') }}</span>
        </div>
    </div>

    <!-- Headers (Optional if needed, but flex row above handles title) -->
    <!-- Let's add a proper header row for columns -->
    <div class="flex px-2 py-1 text-[10px] text-muted-foreground">
        <span class="w-1/3">{{ $t('trade.price') }}({{ quoteSymbol }})</span>
        <span class="w-1/3 text-right">{{ $t('trade.amount') }}({{ baseSymbol }})</span>
        <span class="w-1/3 text-right">{{ $t('trade.total') }}</span>
    </div>

    <!-- Asks (Sells) -->
    <div class="flex-1 overflow-hidden relative flex flex-col-reverse">
       <div v-for="(ask, i) in asks" :key="'ask-'+i" class="flex justify-between px-2 py-0.5 hover:bg-muted cursor-pointer relative group">
          <div class="absolute right-0 top-0 bottom-0 bg-down/10 transition-all" :style="{ width: (ask.total || 0) / maxVol * 100 + '%' }"></div>
          <span class="text-down z-10 font-mono w-1/3">{{ formatNumber(ask.price, 'price') }}</span>
          <span class="text-muted-foreground z-10 font-mono w-1/3 text-right group-hover:text-foreground">{{ formatNumber(ask.amount, 'amount') }}</span>
          <span class="text-muted-foreground z-10 font-mono w-1/3 text-right group-hover:text-foreground">{{ formatNumber(ask.total, 'price') }}</span>
       </div>
    </div>

    <!-- Current Price -->
    <div class="py-2 border-y border-border flex items-center justify-center gap-2 bg-muted/20">
      <span class="text-lg font-bold text-up font-mono">{{ formatNumber(currentPrice, 'price') }}</span>
      <span class="text-xs text-muted-foreground">≈ ${{ formatNumber(currentPrice, 'price') }}</span>
    </div>

    <!-- Bids (Buys) -->
    <div class="flex-1 overflow-hidden relative">
       <div v-for="(bid, i) in bids" :key="'bid-'+i" class="flex justify-between px-2 py-0.5 hover:bg-muted cursor-pointer relative group">
          <div class="absolute right-0 top-0 bottom-0 bg-up/10 transition-all" :style="{ width: (bid.total || 0) / maxVol * 100 + '%' }"></div>
          <span class="text-up z-10 font-mono w-1/3">{{ formatNumber(bid.price, 'price') }}</span>
          <span class="text-muted-foreground z-10 font-mono w-1/3 text-right group-hover:text-foreground">{{ formatNumber(bid.amount, 'amount') }}</span>
          <span class="text-muted-foreground z-10 font-mono w-1/3 text-right group-hover:text-foreground">{{ formatNumber(bid.total, 'price') }}</span>
       </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { formatNumber } from '@/utils/format'

const props = defineProps<{
  symbol?: string
  bids: Array<{ price: number, amount: number, total: number }>
  asks: Array<{ price: number, amount: number, total: number }>
  currentPrice: number
}>()

const baseSymbol = computed(() => props.symbol?.split('/')[0] || 'BTC')
const quoteSymbol = computed(() => props.symbol?.split('/')[1] || 'USDT')

const maxVol = computed(() => {
  const maxBid = Math.max(...props.bids.map(b => b.total || 0), 0)
  const maxAsk = Math.max(...props.asks.map(a => a.total || 0), 0)
  return Math.max(maxBid, maxAsk) * 1.5 || 1 // Scale factor, avoid div by 0
})
</script>
