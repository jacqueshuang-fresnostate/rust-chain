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
  type UTCTimestamp,
} from 'lightweight-charts'
import type { KlinePoint } from '@/core/types'

const props = defineProps<{ points: KlinePoint[] }>()

const container = ref<HTMLElement | null>(null)
let chart: IChartApi | null = null
let candles: ISeriesApi<'Candlestick'> | null = null
let volume: ISeriesApi<'Histogram'> | null = null
let observer: ResizeObserver | null = null

function candleRows(): CandlestickData<UTCTimestamp>[] {
  return props.points.map((point) => ({
    time: Math.floor(point.time / 1000) as UTCTimestamp,
    open: point.open,
    high: point.high,
    low: point.low,
    close: point.close,
  }))
}

function volumeRows(): HistogramData<UTCTimestamp>[] {
  return props.points.map((point) => ({
    time: Math.floor(point.time / 1000) as UTCTimestamp,
    value: point.volume,
    color: point.close >= point.open ? 'rgba(0, 184, 107, 0.44)' : 'rgba(237, 70, 109, 0.42)',
  }))
}

function renderData(): void {
  candles?.setData(candleRows())
  volume?.setData(volumeRows())
  chart?.timeScale().fitContent()
}

function resize(): void {
  if (chart && container.value) chart.resize(container.value.clientWidth, container.value.clientHeight)
}

onMounted(() => {
  if (!container.value) return
  chart = createChart(container.value, {
    autoSize: true,
    height: container.value.clientHeight || 300,
    layout: { background: { type: ColorType.Solid, color: '#101213' }, textColor: '#9ca3af' },
    grid: { vertLines: { color: '#24282b' }, horzLines: { color: '#24282b' } },
    rightPriceScale: { borderColor: '#24282b' },
    timeScale: { borderColor: '#24282b', timeVisible: true, secondsVisible: false },
    handleScroll: { mouseWheel: true, pressedMouseMove: true, horzTouchDrag: true, vertTouchDrag: false },
    handleScale: { axisPressedMouseMove: true, mouseWheel: true, pinch: true },
  })
  candles = chart.addSeries(CandlestickSeries, {
    upColor: '#00b86b',
    downColor: '#ed466d',
    borderVisible: false,
    wickUpColor: '#00b86b',
    wickDownColor: '#ed466d',
  })
  volume = chart.addSeries(HistogramSeries, {
    priceFormat: { type: 'volume' },
    priceScaleId: 'volume',
    lastValueVisible: false,
    priceLineVisible: false,
  })
  volume.priceScale().applyOptions({ scaleMargins: { top: 0.76, bottom: 0 } })
  observer = new ResizeObserver(resize)
  observer.observe(container.value)
  renderData()
})

watch(() => props.points, renderData, { deep: true })

onUnmounted(() => {
  observer?.disconnect()
  chart?.remove()
  chart = null
  candles = null
  volume = null
})
</script>

<template>
  <div ref="container" class="mobile-market-chart" data-kline-provider="tradingview">
    <a href="https://www.tradingview.com/" target="_blank" rel="noreferrer">TradingView</a>
  </div>
</template>

<style scoped>
.mobile-market-chart { height: 100%; min-height: 260px; position: relative; width: 100%; }
.mobile-market-chart a { bottom: 5px; color: #6b7280; font-size: 10px; left: 10px; position: absolute; text-decoration: none; z-index: 1; }
</style>

