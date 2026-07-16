<template>
  <div ref="chartContainer" class="w-full h-full relative" data-kline-provider="klinecharts" id="kline-chart"></div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { KLineChartPro } from '@klinecharts/pro'
import { stompService } from '@/api/stomp'
import {
  chartSymbolValue,
  historyKlineBars,
  klineSubscriptionKey,
  normalizeKlineModule,
  parseRealtimeKline,
  resolveKlineTopic,
  type KlineFetcher,
  type KlineModule
} from './klineData'
import { resolveKlineHistoryFetcher } from './klineDataSource'

/**
 * Props:
 *  - module: 'spot' | 'margin' | 'seconds'  — selects which API endpoint & WS topic to use
 *  - symbol: trading pair string e.g. 'BTC/USDT'
 *  - period: initial period string e.g. '1m'
 *  - precision: optional price precision
 *  - dataList: legacy prop (unused, kept for backward compatibility)
 *  - klineTopic: optional override for the WebSocket kline topic pattern
 *  - fetchKLine: optional custom function override for fetching kline history
 */
const props = withDefaults(defineProps<{
  module?: KlineModule
  symbol: string
  period?: string
  precision?: number
  dataList?: any[]
  klineTopic?: string
  fetchKLine?: KlineFetcher
}>(), {
  module: 'market',
  period: '1m',
  precision: 8
})

const chartContainer = ref<HTMLElement | null>(null)
let chart: any = null
const subscriptions = new Map<string, any>()

onMounted(() => {
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
          const periodText = String(period?.text || props.period)
          const fetcher = resolveKlineHistoryFetcher(props.module, props.fetchKLine)
          const res = await fetcher(chartSymbolValue(symbol, props.symbol), periodText, from, to)
          return historyKlineBars(res.data)
        } catch (e) {
          console.error('Failed to fetch KLine data:', e)
        }
        return []
      },
      subscribe: (symbols: any, period: any, callback: any) => {
        const symbol = chartSymbolValue(symbols, props.symbol)
        const periodText = String(period?.text || props.period)
        const topic = resolveKlineTopic(props.module, props.klineTopic, symbol, periodText)

        const wsModule = normalizeKlineModule(props.module)
        stompService.connect(wsModule)

        stompService.subscribe(wsModule, topic, (msg) => {
          try {
            const klineData = parseRealtimeKline(JSON.parse(msg.body))
            if (klineData && callback && typeof callback === 'function') callback(klineData)
          } catch (e) {
            console.error('Failed to parse KLine message', e)
          }
        }).then(sub => {
          subscriptions.set(klineSubscriptionKey(symbol, periodText), sub)
        })
      },
      unsubscribe: (symbol: any, period: any) => {
        const key = klineSubscriptionKey(
          chartSymbolValue(symbol, props.symbol),
          String(period?.text || props.period)
        )
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
