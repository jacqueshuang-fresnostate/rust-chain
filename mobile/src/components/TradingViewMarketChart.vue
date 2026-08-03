<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import {
  CandlestickSeries,
  ColorType,
  HistogramSeries,
  LineSeries,
  createChart,
  type CandlestickData,
  type HistogramData,
  type IChartApi,
  type ISeriesApi,
  type LineData,
  type LogicalRange,
  type UTCTimestamp,
} from 'lightweight-charts'
import type { NormalizedMarketChartPoint } from '@/core/marketChart'
import {
  captureMarketChartLogicalViewport,
  classifyMarketChartDataUpdate,
  resolveMarketChartLogicalRange,
  type MarketChartLogicalViewport,
} from '@/core/marketChartEngine'
import {
  marketChartColorWithAlpha,
  observeMarketChartTheme,
  readMarketChartTheme,
  type MarketChartTheme,
} from '@/core/marketChartTheme'
import type {
  MarketIndicatorPoint,
  MarketMovingAverages,
} from '@/core/marketIndicators'

const props = withDefaults(defineProps<{
  points: NormalizedMarketChartPoint[]
  movingAverages: MarketMovingAverages
  interval?: string
  locale: string
  label: string
}>(), {
  interval: '',
})

const container = ref<HTMLElement | null>(null)
let chart: IChartApi | null = null
let candles: ISeriesApi<'Candlestick'> | null = null
let volume: ISeriesApi<'Histogram'> | null = null
let ma5Series: ISeriesApi<'Line'> | null = null
let ma10Series: ISeriesApi<'Line'> | null = null
let ma20Series: ISeriesApi<'Line'> | null = null
let resizeObserver: ResizeObserver | null = null
let stopObservingTheme: (() => void) | null = null
let currentTheme: MarketChartTheme | null = null
let renderedPoints: readonly NormalizedMarketChartPoint[] = []
let fitNextDataset = true
let viewportRestoreFrame = 0

function captureViewport(
  points: readonly NormalizedMarketChartPoint[] = renderedPoints,
): MarketChartLogicalViewport | null {
  const range = chart?.timeScale().getVisibleLogicalRange()
  return captureMarketChartLogicalViewport(points, range ?? null)
}

function restoreViewport(viewport: MarketChartLogicalViewport | null): void {
  if (!chart || !viewport) return
  const range = resolveMarketChartLogicalRange(props.points, viewport)
  if (range) chart.timeScale().setVisibleLogicalRange(range as LogicalRange)
}

function scheduleViewportRestore(viewport: MarketChartLogicalViewport | null): void {
  if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame)
  viewportRestoreFrame = 0
  if (!viewport) return
  viewportRestoreFrame = requestAnimationFrame(() => {
    viewportRestoreFrame = 0
    restoreViewport(viewport)
  })
}

function candleRow(point: NormalizedMarketChartPoint): CandlestickData<UTCTimestamp> {
  return {
    time: point.time as UTCTimestamp,
    open: point.open,
    high: point.high,
    low: point.low,
    close: point.close,
  }
}

function volumeRow(
  point: NormalizedMarketChartPoint,
  theme: MarketChartTheme,
): HistogramData<UTCTimestamp> {
  return {
    time: point.time as UTCTimestamp,
    value: point.volume,
    color: marketChartColorWithAlpha(
      point.close >= point.open ? theme.positive : theme.negative,
      .4,
    ),
  }
}

function movingAverageRows(points: MarketIndicatorPoint[]): LineData<UTCTimestamp>[] {
  return points.map((point) => ({
    time: point.time as UTCTimestamp,
    value: point.value,
  }))
}

function renderAllData(
  allowFit = true,
  viewport: MarketChartLogicalViewport | null = null,
): void {
  const theme = currentTheme
  if (!theme) return
  scheduleViewportRestore(null)
  candles?.setData(props.points.map(candleRow))
  volume?.setData(props.points.map((point) => volumeRow(point, theme)))
  ma5Series?.setData(movingAverageRows(props.movingAverages.ma5))
  ma10Series?.setData(movingAverageRows(props.movingAverages.ma10))
  ma20Series?.setData(movingAverageRows(props.movingAverages.ma20))

  if (!props.points.length) {
    fitNextDataset = true
    return
  }
  if (allowFit && fitNextDataset) {
    chart?.timeScale().fitContent()
    fitNextDataset = false
    return
  }
  scheduleViewportRestore(viewport)
}

function updateLatestData(): void {
  const point = props.points.at(-1)
  const theme = currentTheme
  if (!point || !theme) return
  candles?.update(candleRow(point))
  volume?.update(volumeRow(point, theme))
  updateLatestAverage(ma5Series, props.movingAverages.ma5)
  updateLatestAverage(ma10Series, props.movingAverages.ma10)
  updateLatestAverage(ma20Series, props.movingAverages.ma20)
}

function updateLatestAverage(
  series: ISeriesApi<'Line'> | null,
  points: MarketIndicatorPoint[],
): void {
  const point = points.at(-1)
  if (!point) return
  series?.update({ time: point.time as UTCTimestamp, value: point.value })
}

function applyTheme(): void {
  if (!chart || !container.value) return
  const viewport = captureViewport()
  const theme = readMarketChartTheme(container.value)
  currentTheme = theme
  chart.applyOptions({
    layout: {
      attributionLogo: false,
      background: { type: ColorType.Solid, color: theme.background },
      textColor: theme.muted,
    },
    grid: {
      vertLines: { color: marketChartColorWithAlpha(theme.grid, .62) },
      horzLines: { color: marketChartColorWithAlpha(theme.grid, .62) },
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
  ma5Series?.applyOptions({ color: theme.ma5 })
  ma10Series?.applyOptions({ color: theme.ma10 })
  ma20Series?.applyOptions({ color: theme.ma20 })
  renderAllData(false, viewport)
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
  const theme = readMarketChartTheme(container.value)
  currentTheme = theme
  chart = createChart(container.value, {
    height: container.value.clientHeight || 300,
    layout: {
      attributionLogo: false,
      background: { type: ColorType.Solid, color: theme.background },
      textColor: theme.muted,
      fontFamily: getComputedStyle(document.documentElement).fontFamily,
    },
    grid: {
      vertLines: { color: marketChartColorWithAlpha(theme.grid, .62) },
      horzLines: { color: marketChartColorWithAlpha(theme.grid, .62) },
    },
    localization: { locale: props.locale },
    rightPriceScale: { borderColor: theme.grid },
    timeScale: { borderColor: theme.grid, timeVisible: true, secondsVisible: false },
    handleScroll: {
      mouseWheel: true,
      pressedMouseMove: true,
      horzTouchDrag: true,
      vertTouchDrag: false,
    },
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
  ma5Series = chart.addSeries(LineSeries, {
    color: theme.ma5,
    crosshairMarkerVisible: false,
    lastValueVisible: false,
    lineWidth: 1,
    priceLineVisible: false,
  })
  ma10Series = chart.addSeries(LineSeries, {
    color: theme.ma10,
    crosshairMarkerVisible: false,
    lastValueVisible: false,
    lineWidth: 1,
    priceLineVisible: false,
  })
  ma20Series = chart.addSeries(LineSeries, {
    color: theme.ma20,
    crosshairMarkerVisible: false,
    lastValueVisible: false,
    lineWidth: 1,
    priceLineVisible: false,
  })
  volume.priceScale().applyOptions({ scaleMargins: { top: .76, bottom: 0 } })
  resizeObserver = new ResizeObserver(resize)
  resizeObserver.observe(container.value)
  stopObservingTheme = observeMarketChartTheme(
    container.value,
    document.documentElement,
    applyTheme,
  )
  renderedPoints = props.points
  renderAllData()
})

watch(
  () => ({ interval: props.interval, points: props.points }),
  (next, previous) => {
    const fitKeyChanged = next.interval !== previous.interval
    const pointsChanged = next.points !== previous.points
    if (fitKeyChanged) fitNextDataset = true
    if (fitKeyChanged && !pointsChanged) return

    const update = classifyMarketChartDataUpdate(renderedPoints, next.points)
    if (renderedPoints.length <= 1 && next.points.length > renderedPoints.length) {
      fitNextDataset = true
    }
    const viewport = !fitNextDataset && !fitKeyChanged && renderedPoints.length > 0
      ? captureViewport(renderedPoints)
      : null
    renderedPoints = next.points
    if (!fitNextDataset && !fitKeyChanged && (update === 'update-last' || update === 'append')) {
      updateLatestData()
      return
    }
    if (update !== 'none' || fitKeyChanged || (fitNextDataset && pointsChanged)) {
      renderAllData(true, viewport)
    }
  },
  { deep: true },
)

watch(() => props.locale, (locale) => {
  chart?.applyOptions({ localization: { locale } })
})

onUnmounted(() => {
  scheduleViewportRestore(null)
  resizeObserver?.disconnect()
  stopObservingTheme?.()
  stopObservingTheme = null
  chart?.remove()
  chart = null
  candles = null
  volume = null
  ma5Series = null
  ma10Series = null
  ma20Series = null
})
</script>

<template>
  <div
    ref="container"
    class="market-chart-engine"
    data-kline-provider="tradingview"
    data-chart-package="lightweight-charts@5.2.0"
    role="img"
    :aria-label="label"
  />
</template>

<style scoped>
.market-chart-engine {
  background: var(--surface);
  height: 100%;
  min-height: 0;
  min-width: 0;
  width: 100%;
}
</style>
