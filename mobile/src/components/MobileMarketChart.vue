<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChartNoAxesCombined, LoaderCircle } from 'lucide-vue-next'
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
import { currentIntlLocale } from '@/i18n'
import { normalizeMarketChartPoints } from '@/core/marketChart'

const props = withDefaults(defineProps<{
  points: KlinePoint[]
  loading?: boolean
}>(), {
  loading: false,
})

const { t } = useI18n()
const container = ref<HTMLElement | null>(null)
let chart: IChartApi | null = null
let candles: ISeriesApi<'Candlestick'> | null = null
let volume: ISeriesApi<'Histogram'> | null = null
let observer: ResizeObserver | null = null
let themeObserver: MutationObserver | null = null
const normalizedPoints = computed(() => normalizeMarketChartPoints(props.points))
const hasRenderableData = computed(() => normalizedPoints.value.length > 0)

interface ChartTheme {
  background: string
  grid: string
  muted: string
  negative: string
  positive: string
}

function chartTheme(): ChartTheme {
  const styles = getComputedStyle(document.documentElement)
  const ink = styles.getPropertyValue('--ink').trim() || styles.color
  const muted = styles.getPropertyValue('--muted').trim() || ink
  return {
    background: styles.getPropertyValue('--surface').trim() || styles.backgroundColor,
    grid: styles.getPropertyValue('--line').trim() || muted,
    muted,
    negative: styles.getPropertyValue('--negative').trim() || ink,
    positive: styles.getPropertyValue('--positive').trim() || ink,
  }
}

function withAlpha(color: string, alpha: number): string {
  const hex = /^#([\da-f]{3}|[\da-f]{6})$/i.exec(color)
  if (hex) {
    const value = hex[1].length === 3
      ? hex[1].split('').map((part) => `${part}${part}`).join('')
      : hex[1]
    return `rgba(${Number.parseInt(value.slice(0, 2), 16)}, ${Number.parseInt(value.slice(2, 4), 16)}, ${Number.parseInt(value.slice(4, 6), 16)}, ${alpha})`
  }
  const channels = color.match(/[\d.]+/g)
  if (channels && channels.length >= 3) {
    return `rgba(${channels[0]}, ${channels[1]}, ${channels[2]}, ${alpha})`
  }
  return color
}

function candleRows(): CandlestickData<UTCTimestamp>[] {
  return normalizedPoints.value.map((point) => ({
    time: point.time as UTCTimestamp,
    open: point.open,
    high: point.high,
    low: point.low,
    close: point.close,
  }))
}

function volumeRows(): HistogramData<UTCTimestamp>[] {
  const theme = chartTheme()
  return normalizedPoints.value.map((point) => ({
    time: point.time as UTCTimestamp,
    value: point.volume,
    color: withAlpha(point.close >= point.open ? theme.positive : theme.negative, .4),
  }))
}

function renderData(): void {
  candles?.setData(candleRows())
  volume?.setData(volumeRows())
  if (hasRenderableData.value) chart?.timeScale().fitContent()
}

function applyTheme(): void {
  if (!chart) return
  const theme = chartTheme()
  chart.applyOptions({
    layout: {
      background: { type: ColorType.Solid, color: theme.background },
      textColor: theme.muted,
    },
    grid: {
      vertLines: { color: withAlpha(theme.grid, .62) },
      horzLines: { color: withAlpha(theme.grid, .62) },
    },
    rightPriceScale: { borderColor: theme.grid },
    timeScale: { borderColor: theme.grid },
  })
  candles?.applyOptions({
    upColor: theme.positive,
    downColor: theme.negative,
    wickUpColor: theme.positive,
    wickDownColor: theme.negative,
  })
  renderData()
}

function resize(): void {
  if (!chart || !container.value) return
  const width = container.value.clientWidth
  const height = container.value.clientHeight
  if (width <= 0 || height <= 0) return
  chart.resize(width, height)
}

onMounted(() => {
  if (!container.value) return
  const theme = chartTheme()
  chart = createChart(container.value, {
    height: container.value.clientHeight || 300,
    layout: {
      background: { type: ColorType.Solid, color: theme.background },
      textColor: theme.muted,
      fontFamily: getComputedStyle(document.documentElement).fontFamily,
    },
    grid: {
      vertLines: { color: withAlpha(theme.grid, .62) },
      horzLines: { color: withAlpha(theme.grid, .62) },
    },
    localization: { locale: currentIntlLocale() },
    rightPriceScale: { borderColor: theme.grid },
    timeScale: { borderColor: theme.grid, timeVisible: true, secondsVisible: false },
    handleScroll: { mouseWheel: true, pressedMouseMove: true, horzTouchDrag: true, vertTouchDrag: false },
    handleScale: { axisPressedMouseMove: true, mouseWheel: true, pinch: true },
  })
  candles = chart.addSeries(CandlestickSeries, {
    upColor: theme.positive,
    downColor: theme.negative,
    borderVisible: false,
    wickUpColor: theme.positive,
    wickDownColor: theme.negative,
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
  themeObserver = new MutationObserver(applyTheme)
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-theme'],
  })
  renderData()
})

watch(() => props.points, renderData, { deep: true })

onUnmounted(() => {
  observer?.disconnect()
  themeObserver?.disconnect()
  chart?.remove()
  chart = null
  candles = null
  volume = null
})
</script>

<template>
  <div
    class="mobile-market-chart"
    :class="{ 'has-data': hasRenderableData }"
    :data-chart-state="loading ? 'loading' : hasRenderableData ? 'ready' : 'empty'"
    :aria-busy="loading"
  >
    <div
      ref="container"
      class="mobile-market-chart__canvas"
      data-kline-provider="tradingview"
      role="img"
      :aria-label="t('marketDetail.market')"
    />
    <div v-if="loading && !hasRenderableData" class="mobile-market-chart__state" role="status">
      <LoaderCircle :size="20" class="spin" />
      <span>{{ t('marketDetail.loadingChart') }}</span>
    </div>
    <div v-else-if="!hasRenderableData" class="mobile-market-chart__state">
      <ChartNoAxesCombined :size="22" />
      <span>{{ t('common.marketUnavailable') }}</span>
    </div>
  </div>
</template>

<style scoped>
.mobile-market-chart {
  background: var(--surface);
  height: 100%;
  min-height: 220px;
  min-width: 0;
  overflow: hidden;
  position: relative;
  width: 100%;
}

.mobile-market-chart__canvas {
  height: 100%;
  min-height: 220px;
  min-width: 0;
  width: 100%;
}

.mobile-market-chart__state {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 12px;
  gap: 8px;
  inset: 0;
  justify-content: center;
  pointer-events: none;
  position: absolute;
  z-index: 2;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
