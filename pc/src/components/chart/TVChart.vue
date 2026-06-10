<template>
  <div ref="chartContainer" class="w-full h-full relative" id="kline-chart"></div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { KLineChartPro } from '@klinecharts/pro'
import { stompService } from '@/api/stomp'
import { fetchHistoryKLine as fetchMarketKLine } from '@/api/market'
import { fetchHistoryKLine as fetchSecondKLine } from '@/api/second'
import { fetchKlineHistory as fetchSwapKLine } from '@/api/contract'

/**
 * Props:
 *  - module: 'market' | 'second' | 'swap'  — selects which API endpoint & WS topic to use
 *  - symbol: trading pair string e.g. 'BTC/USDT'
 *  - period: initial period string e.g. '1m'
 *  - precision: optional price precision
 *  - dataList: legacy prop (unused, kept for backward compatibility)
 *  - klineTopic: optional override for the WebSocket kline topic pattern
 *  - fetchKLine: optional custom function override for fetching kline history
 */
const props = withDefaults(defineProps<{
  module?: 'market' | 'second' | 'swap'
  symbol: string
  period?: string
  precision?: number
  dataList?: any[]
  klineTopic?: string
  fetchKLine?: (symbol: string, resolution: string, from: number, to: number) => Promise<any>
}>(), {
  module: 'market',
  period: '1m',
  precision: 8
})

const chartContainer = ref<HTMLElement | null>(null)
let chart: any = null
const subscriptions = new Map<string, any>()

/**
 * Resolve the K-Line fetch function based on module or custom override
 */
function getKLineFetcher() {
  if (props.fetchKLine) return props.fetchKLine
  switch (props.module) {
    case 'swap':
      return (symbol: string, resolution: string, from: number, to: number) =>
        fetchSwapKLine(symbol, from, to, resolution)
    case 'second':
      return fetchSecondKLine
    case 'market':
    default:
      return fetchMarketKLine
  }
}

/**
 * Resolve the WebSocket topic for kline data
 */
function getKLineTopic(symbol: string, interval: string) {
  if (props.klineTopic) return props.klineTopic.replace('{symbol}', symbol)
  return `${props.module}:kline:${symbol}:${interval}`
}

onMounted(() => {
  console.log("props.precision",props.precision)
  if (!chartContainer.value) return
  chart = new KLineChartPro({
    container: chartContainer.value,
    symbol: { ticker: props.symbol, shortName: props.symbol, symbol: props.symbol, pricePrecision: props.precision ?? 4 },
    period: { multiplier: 1, timespan: 'minute', text: '1min' },
    periods: [
      { multiplier: 1, timespan: 'minute', text: '1min' },
      { multiplier: 5, timespan: 'minute', text: '5min' },
      { multiplier: 15, timespan: 'minute', text: '15min' },
      { multiplier: 1, timespan: 'hour', text: '1h' },
      { multiplier: 4, timespan: 'hour', text: '4h' },
      { multiplier: 1, timespan: 'day', text: '1d' },
    ],
    mainIndicators: ['MA'],
    subIndicators: ['VOL'],
    theme: 'dark',
    locale: 'en-US',
    timezone: 'Asia/Shanghai',
    datafeed: {
      searchSymbols: () => {
        return Promise.resolve([
          { symbol: props.symbol, ticker: props.symbol, shortName: props.symbol, pricePrecision: props.precision ?? 4, volumePrecision: 4 }
        ])
      },
      getHistoryKLineData: async (symbol: any, period: any, from: number, to: number) => {
        try {
          const fetcher = getKLineFetcher()
          const res = await fetcher(symbol?.ticker || props.symbol, period.text, from, to)
          if (res.data) {
            const list = Array.isArray(res.data) ? res.data : []
            return (list || []).map((item: any) => ({
              close: item[4],
              high: item[2],
              low: item[3],
              open: item[1],
              timestamp: item[0],
              volume: item[5]
            }))
          }
        } catch (e) {
          console.error('Failed to fetch KLine data:', e)
        }
        return []
      },
      subscribe: (symbols: any, period: any, callback: any) => {
        const  symbol=symbols.symbol
        console.log('subscribe---->',symbol)
        const topic = getKLineTopic(symbol, period.text)
        console.log(`[TVChart][${props.module}] Subscribing to KLine:`, topic)

        // Ensure the connection is initiated before subscribing
        stompService.connect(props.module as any)

        stompService.subscribe(props.module as any, topic, (msg) => {
          try {
            const resp = JSON.parse(msg.body)
            if (callback && typeof callback === 'function') {
              // console.log(`[TVChart][${props.module}] KLine data received:`, resp)
              // Force all values to Number - KLineChartPro requires strict number types
              let ts = Number(resp.time || resp.timestamp || 0)
              // Auto-detect seconds vs milliseconds: if < 1e12, it's seconds, convert to ms
              if (ts > 0 && ts < 1e12) ts *= 1000
              const klineData = {
                timestamp: ts,
                open: Number(resp.openPrice ?? resp.open ?? 0),
                high: Number(resp.highestPrice ?? resp.high ?? 0),
                low: Number(resp.lowestPrice ?? resp.low ?? 0),
                close: Number(resp.closePrice ?? resp.close ?? 0),
                volume: Number(resp.volume ?? resp.vol ?? 0),
                turnover: Number(resp.turnover ?? resp.amount ?? 0)
              }
              // Only push valid data
              if (klineData.timestamp > 0 && klineData.close > 0) {
                callback(klineData)
              }
            }
          } catch (e) {
            console.error('Failed to parse KLine message', e)
          }
        }).then(sub => {
          subscriptions.set(`${symbol}_${period.text}`, sub)
        })
      },
      unsubscribe: (symbol: any, period: any) => {
        const key = `${symbol}_${period.text}`
        const sub = subscriptions.get(key)
        if (sub) {
          sub.unsubscribe()
          subscriptions.delete(key)
        }
      }
    }
  })

  // Dark theme styling
  chart.setStyles({
    candle: {
      bar: {
        upColor: '#0ecb81',
        downColor: '#f6465d',
        noChangeColor: '#888888',
        upBorderColor: '#0ecb81',
        downBorderColor: '#f6465d',
        noChangeBorderColor: '#888888',
        upWickColor: '#0ecb81',
        downWickColor: '#f6465d',
        noChangeWickColor: '#888888'
      }
    },
    grid: {
      horizontal: { color: '#2B2B43' },
      vertical: { color: '#2B2B43' }
    }
  })
})

watch(() => props.symbol, () => {
  if (chart) {
    chart.setSymbol(props.symbol)
  }
})

onUnmounted(() => {
  subscriptions.forEach(sub => sub.unsubscribe())
  subscriptions.clear()
})
</script>

<style scoped>
#kline-chart {
  width: 100%;
  height: 100%;
}
</style>
