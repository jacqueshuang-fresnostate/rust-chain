<template>
  <div class="flex flex-col h-full bg-card">
<!--    <div class="p-2 border-b border-border font-bold text-sm text-muted-foreground">{{ $t('trade.market') }} {{ $t('nav.trade') }}</div>-->
    <div class="flex-1 overflow-auto">
       <div class="flex text-[10px] text-muted-foreground px-2 py-1 border-b border-border/50">
          <span class="w-1/3">{{ $t('trade.price') }}({{ quoteSymbol }})</span>
          <span class="w-1/3 text-right">{{ $t('trade.amount') }}({{ baseSymbol }})</span>
          <span class="w-1/3 text-right">{{ $t('trade.time') }}</span>
       </div>
       <div v-for="trade in trades" :key="marketTradeIdentity(trade)" class="flex text-[10px] px-2 py-0.5 hover:bg-muted/50 transition-colors">
          <span class="w-1/3 font-mono" :class="trade.direction === 'BUY' ? 'text-up' : 'text-down'">{{ formatNumber(trade.price, 'price') }}<MarketProvenanceLabel :provenance="trade" /></span>
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
import MarketProvenanceLabel from './MarketProvenanceLabel.vue'
import { marketTradeIdentity } from '@/api/marketProvenance'
import type { PcMarketTrade } from '@/api/backendAdapters'

const props = withDefaults(defineProps<{
  symbol?: string
  module?: 'spot' | 'margin' | 'seconds' | 'market' | 'second' | 'swap'
}>(), {
  module: 'spot'
})

const trades = ref<PcMarketTrade[]>([])
let tradeSub: any = null
let generation = 0

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
    const currentGeneration = generation
    try {
        const fetcher = getTradeFetcher()
        const res = await fetcher(props.symbol)
        if (currentGeneration === generation && res.data) {
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
            const seen = new Set<string>()
            trades.value = [...trades.value, ...list].filter((trade) => {
                const identity = marketTradeIdentity(trade)
                if (seen.has(identity)) return false
                seen.add(identity)
                return true
            }).slice(0, 50)
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
    const currentGeneration = generation
    const subscription = await stompService.subscribe(wsModule, topic, (msg) => {
        if (currentGeneration !== generation) return
        try {
            const data = JSON.parse(msg.body)
            const items = Array.isArray(data) ? data : [data]
            const seen = new Set<string>()
            trades.value = [...items, ...trades.value].filter((trade) => {
                const identity = marketTradeIdentity(trade)
                if (seen.has(identity)) return false
                seen.add(identity)
                return true
            }).slice(0, 50)
        } catch (e) {
            console.error(e)
        }
    })
    if (currentGeneration !== generation) subscription.unsubscribe()
    else tradeSub = subscription
}

watch(() => [props.symbol, props.module], () => {
    generation += 1
    trades.value = []
    fetchTrades()
    subscribeTrades()
})

onMounted(() => {
    fetchTrades()
    subscribeTrades()
})

onUnmounted(() => {
    generation += 1
    if (tradeSub) tradeSub.unsubscribe()
})
</script>
