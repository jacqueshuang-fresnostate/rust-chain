<template>
  <div ref="chartContainer" class="tradingview-chart" data-kline-provider="tradingview">
    <a
      class="tradingview-attribution"
      href="https://www.tradingview.com/"
      rel="noreferrer"
      target="_blank"
    >TradingView</a>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import {
  CandlestickSeries,
  ColorType,
  HistogramSeries,
  createChart,
  type CandlestickData,
  type HistogramData,
  type IChartApi,
  type ISeriesApi,
  type UTCTimestamp
} from 'lightweight-charts'
import { stompService } from '@/api/stomp'
import {
  historyKlineBars,
  klineLookbackMs,
  normalizeKlineModule,
  parseRealtimeKline,
  resolveKlineTopic,
  type KlineBar,
  type KlineFetcher,
  type KlineModule
} from './klineData'
import { resolveKlineHistoryFetcher } from './klineDataSource'

type Subscription = {
  unsubscribe: () => void
}

const props = withDefaults(defineProps<{
  module?: KlineModule
  symbol: string
  period?: string
  precision?: number
  dataList?: unknown[]
  klineTopic?: string
  fetchKLine?: KlineFetcher
}>(), {
  module: 'market',
  period: '1m',
  precision: 8
})

const chartContainer = ref<HTMLElement | null>(null)
let chart: IChartApi | null = null
let candleSeries: ISeriesApi<'Candlestick'> | null = null
let volumeSeries: ISeriesApi<'Histogram'> | null = null
let resizeObserver: ResizeObserver | null = null
let subscription: Subscription | null = null
let historyRequestVersion = 0
let subscriptionVersion = 0
let mounted = false

function priceFormat() {
  const precision = Math.max(0, Math.min(10, Math.trunc(props.precision ?? 4)))
  return {
    type: 'price' as const,
    precision,
    minMove: 1 / 10 ** precision
  }
}

function chartTime(timestamp: number): UTCTimestamp {
  return Math.floor(timestamp / 1000) as UTCTimestamp
}

function candleData(bar: KlineBar): CandlestickData<UTCTimestamp> {
  return {
    time: chartTime(bar.timestamp),
    open: bar.open,
    high: bar.high,
    low: bar.low,
    close: bar.close
  }
}

function volumeData(bar: KlineBar): HistogramData<UTCTimestamp> {
  return {
    time: chartTime(bar.timestamp),
    value: bar.volume,
    color: bar.close >= bar.open ? 'rgba(14, 203, 129, 0.48)' : 'rgba(246, 70, 93, 0.48)'
  }
}

function resizeChart() {
  if (!chart || !chartContainer.value) return
  chart.resize(chartContainer.value.clientWidth, chartContainer.value.clientHeight)
}

function applyPricePrecision() {
  candleSeries?.applyOptions({ priceFormat: priceFormat() })
}

async function loadKlineHistory() {
  if (!candleSeries || !props.symbol) return
  const requestVersion = ++historyRequestVersion
  const end = Date.now()

  try {
    const fetcher = resolveKlineHistoryFetcher(props.module, props.fetchKLine)
    const response = await fetcher(
      props.symbol,
      props.period,
      end - klineLookbackMs(props.period),
      end
    )
    if (!mounted || requestVersion !== historyRequestVersion || !candleSeries) return

    const bars = historyKlineBars(response.data)
    candleSeries.setData(bars.map(candleData))
    volumeSeries?.setData(bars.map(volumeData))
    if (bars.length > 0) chart?.timeScale().fitContent()
  } catch (error) {
    console.error('Failed to fetch TradingView KLine data:', error)
  }
}

async function replaceRealtimeSubscription() {
  const currentVersion = ++subscriptionVersion
  subscription?.unsubscribe()
  subscription = null
  if (!props.symbol) return

  const wsModule = normalizeKlineModule(props.module)
  const topic = resolveKlineTopic(props.module, props.klineTopic, props.symbol, props.period)
  stompService.connect(wsModule)

  const nextSubscription = await stompService.subscribe(wsModule, topic, (message) => {
    try {
      const bar = parseRealtimeKline(JSON.parse(message.body))
      if (!bar || !mounted) return
      candleSeries?.update(candleData(bar))
      volumeSeries?.update(volumeData(bar))
    } catch (error) {
      console.error('Failed to parse TradingView KLine message:', error)
    }
  })

  if (!mounted || currentVersion !== subscriptionVersion) {
    nextSubscription.unsubscribe()
    return
  }
  subscription = nextSubscription
}

function refreshChart() {
  if (!mounted) return
  applyPricePrecision()
  void loadKlineHistory()
  void replaceRealtimeSubscription()
}

onMounted(() => {
  if (!chartContainer.value) return

  chart = createChart(chartContainer.value, {
    layout: {
      background: { type: ColorType.Solid, color: 'transparent' },
      textColor: '#a2a2b2'
    },
    grid: {
      vertLines: { color: 'rgba(148, 163, 184, 0.12)' },
      horzLines: { color: 'rgba(148, 163, 184, 0.12)' }
    },
    rightPriceScale: { borderColor: 'rgba(148, 163, 184, 0.28)' },
    timeScale: {
      borderColor: 'rgba(148, 163, 184, 0.28)',
      timeVisible: true,
      secondsVisible: false
    }
  })
  candleSeries = chart.addSeries(CandlestickSeries, {
    ...priceFormat(),
    upColor: '#0ecb81',
    downColor: '#f6465d',
    borderVisible: false,
    wickUpColor: '#0ecb81',
    wickDownColor: '#f6465d'
  })
  volumeSeries = chart.addSeries(HistogramSeries, {
    priceFormat: { type: 'volume' },
    priceScaleId: 'volume',
    lastValueVisible: false,
    priceLineVisible: false
  })
  volumeSeries.priceScale().applyOptions({
    scaleMargins: { top: 0.72, bottom: 0 }
  })

  resizeObserver = new ResizeObserver(resizeChart)
  resizeObserver.observe(chartContainer.value)
  resizeChart()
  mounted = true
  refreshChart()
})

watch(
  () => [props.symbol, props.period, props.precision, props.module, props.klineTopic],
  () => refreshChart()
)

onUnmounted(() => {
  mounted = false
  historyRequestVersion += 1
  subscriptionVersion += 1
  subscription?.unsubscribe()
  subscription = null
  resizeObserver?.disconnect()
  resizeObserver = null
  chart?.remove()
  chart = null
  candleSeries = null
  volumeSeries = null
})
</script>

<style scoped>
.tradingview-chart {
  height: 100%;
  min-height: 0;
  position: relative;
  width: 100%;
}

.tradingview-attribution {
  bottom: 4px;
  color: rgba(148, 163, 184, 0.76);
  font-size: 10px;
  left: 8px;
  line-height: 1;
  position: absolute;
  text-decoration: none;
  z-index: 1;
}

.tradingview-attribution:hover {
  color: rgb(203, 213, 225);
}
</style>
