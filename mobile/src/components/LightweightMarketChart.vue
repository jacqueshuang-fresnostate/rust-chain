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
import { resolveMarketChartPriceFormat, type NormalizedMarketChartPoint } from '@/core/marketChart'
import {
  captureMarketChartLogicalViewport,
  classifyMarketChartDataUpdate,
  resolveMarketChartLogicalRange,
  type MarketChartLogicalViewport,
} from '@/core/marketChartRuntime'
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
  symbol: string
  interval?: string
  historyLoading?: boolean
  locale: string
  label: string
}>(), {
  interval: '',
  historyLoading: false,
})
const emit = defineEmits<{ 'load-history': [] }>()

const container = ref<HTMLElement | null>(null)
let chart: IChartApi | null = null
let candles: ISeriesApi<'Candlestick'> | null = null
let volume: ISeriesApi<'Histogram'> | null = null
let ma5Series: ISeriesApi<'Line'> | null = null
let ma10Series: ISeriesApi<'Line'> | null = null
let ma20Series: ISeriesApi<'Line'> | null = null
let resizeObserver: ResizeObserver | null = null
let stopObservingTheme: (() => void) | null = null
let stopObservingViewportIntent: (() => void) | null = null
let currentTheme: MarketChartTheme | null = null
let renderedPoints: readonly NormalizedMarketChartPoint[] = []
let fitNextDataset = true
let initialHistoryPending = props.historyLoading || !props.points.length
let viewportInteracted = false
let awaitingDatasetPoints = false
let viewportRestoreFrame = 0
let pendingViewport: MarketChartLogicalViewport | null = null
let pendingFollowAdvancingTail = false
let renderedPricePrecision: number | null = null
let historyDemandFrame = 0
let historyGestureRange: LogicalRange | null = null
const viewportIntentEvents = ['wheel', 'pointermove', 'touchmove', 'dblclick'] as const

function cancelHistoryDemand(): void {
  if (historyDemandFrame) cancelAnimationFrame(historyDemandFrame)
  historyDemandFrame = 0
  historyGestureRange = null
}

function onVisibleLogicalRangeChange(range: LogicalRange | null): void {
  const before = historyGestureRange
  if (!before || !range || props.historyLoading || awaitingDatasetPoints || !renderedPoints.length) return
  if (!Number.isFinite(range.from) || !Number.isFinite(range.to) || range.to <= range.from) return
  // A viewport notification alone is never demand: native fits, live writes,
  // prepend restoration and touch inertia can all produce the same event.
  if (range.from <= 20 && range.from < before.from - .01) {
    cancelHistoryDemand()
    emit('load-history')
  }
}

function recordViewportIntent(event: Event): void {
  if (event.type === 'pointermove' && (event as PointerEvent).buttons === 0) return
  viewportInteracted = true
  // A gesture wins over delayed history fitting and queued renderer work.
  // Keep an initial/new-dataset fit armed while there are no new data yet.
  if (renderedPoints.length && !awaitingDatasetPoints) fitNextDataset = false
  scheduleViewportRestore(null)
  if (event.type === 'dblclick') { cancelHistoryDemand(); return }
  if (!chart || props.historyLoading || awaitingDatasetPoints || !renderedPoints.length) return
  if (historyDemandFrame) return
  historyGestureRange = chart.timeScale().getVisibleLogicalRange()
  // Capture runs before the library's mouse/touch handler. Read on RAF too:
  // logical range notifications may wait for its next native draw.
  historyDemandFrame = requestAnimationFrame(() => {
    historyDemandFrame = 0
    onVisibleLogicalRangeChange(chart?.timeScale().getVisibleLogicalRange() ?? null)
    historyGestureRange = null
  })
}

function applyPriceFormat(): void {
  const priceFormat = resolveMarketChartPriceFormat(props.points)
  if (renderedPricePrecision === priceFormat.precision) return
  candles?.applyOptions({ priceFormat })
  ma5Series?.applyOptions({ priceFormat })
  ma10Series?.applyOptions({ priceFormat })
  ma20Series?.applyOptions({ priceFormat })
  renderedPricePrecision = priceFormat.precision
}

function captureViewport(
  points: readonly NormalizedMarketChartPoint[] = renderedPoints,
): MarketChartLogicalViewport | null {
  const range = pendingViewport
    ? resolveMarketChartLogicalRange(points, pendingViewport, pendingFollowAdvancingTail)
    : chart?.timeScale().getVisibleLogicalRange()
  return captureMarketChartLogicalViewport(points, range ?? null)
}

function restoreViewport(viewport: MarketChartLogicalViewport | null, followAdvancingTail = false): void {
  if (!chart || !viewport) return
  const range = resolveMarketChartLogicalRange(props.points, viewport, followAdvancingTail)
  if (range) chart.timeScale().setVisibleLogicalRange(range as LogicalRange)
}

function scheduleViewportRestore(viewport: MarketChartLogicalViewport | null, followAdvancingTail = false): void {
  if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame)
  viewportRestoreFrame = 0
  pendingViewport = viewport
  pendingFollowAdvancingTail = followAdvancingTail
  if (!viewport) return
  viewportRestoreFrame = requestAnimationFrame(() => {
    viewportRestoreFrame = 0
    pendingViewport = null
    restoreViewport(viewport, followAdvancingTail)
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
  applyPriceFormat()
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
  scheduleViewportRestore(viewport, allowFit)
}

function updateLatestData(): void {
  const point = props.points.at(-1)
  const theme = currentTheme
  if (!point || !theme) return
  applyPriceFormat()
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
  cancelHistoryDemand()
  const viewport = captureViewport()
  const theme = readMarketChartTheme(container.value)
  currentTheme = theme
  chart.applyOptions({
    layout: {
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
  cancelHistoryDemand()
  const width = container.value.clientWidth
  const height = container.value.clientHeight
  if (width <= 0 || height <= 0) return
  chart.resize(width, height)
}

function datasetKey(symbol: string, interval: string): string {
  return `${symbol.trim().toUpperCase()}::${interval}`
}

onMounted(() => {
  if (!container.value) return
  const element = container.value
  for (const event of viewportIntentEvents) {
    element.addEventListener(event, recordViewportIntent, { capture: true, passive: true })
  }
  stopObservingViewportIntent = () => {
    for (const event of viewportIntentEvents) element.removeEventListener(event, recordViewportIntent, true)
  }
  const theme = readMarketChartTheme(container.value)
  currentTheme = theme
  chart = createChart(container.value, {
    height: container.value.clientHeight || 300,
    layout: {
      attributionLogo: true,
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
    kineticScroll: { mouse: false, touch: true },
  })
  candles = chart.addSeries(CandlestickSeries, {
    upColor: theme.positive,
    downColor: theme.negative,
    borderVisible: false,
    wickVisible: false,
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
  chart.timeScale().subscribeVisibleLogicalRangeChange(onVisibleLogicalRangeChange)
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
  () => ({ key: datasetKey(props.symbol, props.interval), points: props.points, historyLoading: props.historyLoading }),
  (next, previous) => {
    cancelHistoryDemand()
    const fitKeyChanged = next.key !== previous.key
    const pointsChanged = next.points !== previous.points
    if (fitKeyChanged) {
      scheduleViewportRestore(null)
      fitNextDataset = true
      initialHistoryPending = true
      viewportInteracted = false
      awaitingDatasetPoints = !pointsChanged
    }
    if (fitKeyChanged && !pointsChanged) return
    if (awaitingDatasetPoints && !pointsChanged) return
    awaitingDatasetPoints = false

    const update = classifyMarketChartDataUpdate(renderedPoints, next.points)
    const historySettled = initialHistoryPending && previous.historyLoading && !next.historyLoading
    if (historySettled) {
      initialHistoryPending = false
      if (!viewportInteracted) fitNextDataset = true
    }
    if (!next.historyLoading && next.points.length) initialHistoryPending = false
    const viewport = !fitNextDataset && !fitKeyChanged && renderedPoints.length > 0
      ? captureViewport(renderedPoints)
      : null
    renderedPoints = next.points
    if (!fitNextDataset && !fitKeyChanged && (update === 'update-last' || update === 'append')) {
      updateLatestData()
      if (pendingViewport) scheduleViewportRestore(viewport, update === 'append')
      return
    }
    if (update !== 'none' || fitKeyChanged || (fitNextDataset && (pointsChanged || historySettled))) {
      renderAllData(true, viewport)
    }
  },
)

watch(() => props.locale, (locale) => {
  chart?.applyOptions({ localization: { locale } })
})

onUnmounted(() => {
  cancelHistoryDemand()
  scheduleViewportRestore(null)
  stopObservingViewportIntent?.()
  stopObservingViewportIntent = null
  resizeObserver?.disconnect()
  stopObservingTheme?.()
  stopObservingTheme = null
  chart?.timeScale().unsubscribeVisibleLogicalRangeChange(onVisibleLogicalRangeChange)
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
    data-kline-provider="lightweight-charts"
    data-chart-package="lightweight-charts@5.2.0"
    :data-chart-dataset="datasetKey(symbol, interval)"
    role="region"
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
