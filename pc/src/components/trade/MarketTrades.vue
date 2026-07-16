<template>
  <div class="flex flex-col h-full bg-card">
<!--    <div class="p-2 border-b border-border font-bold text-sm text-muted-foreground">{{ $t('trade.market') }} {{ $t('nav.trade') }}</div>-->
    <div class="flex-1 overflow-auto">
       <div class="flex text-[10px] text-muted-foreground px-2 py-1 border-b border-border/50">
          <span class="w-1/3">{{ $t('trade.price') }}({{ quoteSymbol }})</span>
          <span class="w-1/3 text-right">{{ $t('trade.amount') }}({{ baseSymbol }})</span>
          <span class="w-1/3 text-right">{{ $t('trade.time') }}</span>
       </div>
       <div v-for="(trade, i) in trades" :key="i" class="flex text-[10px] px-2 py-0.5 hover:bg-muted/50 transition-colors">
          <span class="w-1/3 font-mono" :class="trade.direction === 'BUY' ? 'text-up' : 'text-down'">{{ formatNumber(trade.price, 'price') }}</span>
          <span class="w-1/3 text-right text-muted-foreground font-mono">{{ formatNumber(trade.amount, 'amount') }}</span>
          <span class="w-1/3 text-right text-muted-foreground">{{ formatTime(trade.time) }}</span>
       </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from 'vue'
import { formatNumber } from '@/utils/format'
import { fetchLatestTrade as fetchMarketTrade } from '@/api/market'
import { fetchLatestTrade as fetchSecondTrade } from '@/api/second'
import { fetchLatestTrade as fetchSwapTrade } from '@/api/contract'
import { stompService } from '@/api/stomp'

const props = withDefaults(defineProps<{
  symbol?: string
  module?: 'spot' | 'margin' | 'seconds' | 'market' | 'second' | 'swap'
}>(), {
  module: 'spot'
})

const trades = ref<any[]>([])
let tradeSub: any = null

const baseSymbol = computed(() => props.symbol?.split('/')[0] || 'BTC')
const quoteSymbol = computed(() => props.symbol?.split('/')[1] || 'USDT')

/**
 * Select the correct API function based on module
 */
function getTradeFetcher() {
  switch (normalizeWsModule(props.module)) {
    case 'margin':
      return fetchSwapTrade
    case 'seconds':
      return fetchSecondTrade
    case 'spot':
    default:
      return fetchMarketTrade
  }
}

function normalizeWsModule(module: typeof props.module): 'spot' | 'margin' | 'seconds' {
  switch (module) {
    case 'swap':
    case 'margin':
      return 'margin'
    case 'second':
    case 'seconds':
      return 'seconds'
    case 'market':
    case 'spot':
    default:
      return 'spot'
  }
}

/**
 * Get the WebSocket topic based on module
 */
function getTradeTopic(symbol: string) {
  return `${normalizeWsModule(props.module)}:trade:${symbol}`
}

const formatTime = (ts: number | string) => {
    if (!ts) return ''
    const date = new Date(Number(ts))
    return date.toTimeString().slice(0, 8)
}

const fetchTrades = async () => {
    if (!props.symbol) return
    try {
        const fetcher = getTradeFetcher()
        const res = await fetcher(props.symbol)
        if (res.data) {
            const data = res.data
            // Handle both direct arrays and wrapped { code, data } responses
            let list: any[]
            if (Array.isArray(data)) {
                list = data
            } else if (data.data && Array.isArray(data.data)) {
                list = data.data
            } else if (data.trades && Array.isArray(data.trades)) {
                list = data.trades
            } else {
                list = []
            }
            trades.value = list
        }
    } catch (e) {
        console.error('Failed to fetch trades', e)
    }
}

const subscribeTrades = async () => {
    if (!props.symbol) return
    if (tradeSub) {
        tradeSub.unsubscribe()
        tradeSub = null
    }

    const topic = getTradeTopic(props.symbol)
    console.log(`[MarketTrades][${props.module}] Subscribing to:`, topic)
    const wsModule = normalizeWsModule(props.module)
    tradeSub = await stompService.subscribe(wsModule, topic, (msg) => {
        try {
            const data = JSON.parse(msg.body)
            const items = Array.isArray(data) ? data : [data]
            trades.value.unshift(...items)
            if (trades.value.length > 50) {
                trades.value = trades.value.slice(0, 50)
            }
        } catch (e) {
            console.error(e)
        }
    })
}

watch(() => props.symbol, () => {
    fetchTrades()
    subscribeTrades()
})

onMounted(() => {
    fetchTrades()
    subscribeTrades()
})

onUnmounted(() => {
    if (tradeSub) tradeSub.unsubscribe()
})
</script>
