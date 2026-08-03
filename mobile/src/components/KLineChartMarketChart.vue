<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import {
  dispose,
  init,
  type Chart,
  type DataLoader,
  type DeepPartial,
  type KLineData,
  type Period,
  type Styles,
} from 'klinecharts'
import type { NormalizedMarketChartPoint } from '@/core/marketChart'
import {
  captureMarketChartViewport,
  classifyMarketChartDataUpdate,
  marketChartPeriod,
  resolveMarketChartSymbolInfo,
  resolveMarketChartViewportRealTo,
  type MarketChartSymbolInfo,
  type MarketChartViewport,
} from '@/core/marketChartEngine'
import {
  marketChartColorWithAlpha,
  observeMarketChartTheme,
  readMarketChartTheme,
  type MarketChartTheme,
} from '@/core/marketChartTheme'

const props = withDefaults(defineProps<{
  points: NormalizedMarketChartPoint[]
  interval?: string
  locale: string
  label: string
  symbol: string
}>(), {
  interval: '',
})

const CANDLE_PANE_ID = 'candle_pane'
const VOLUME_PANE_ID = 'hippo_volume_pane'
const container = ref<HTMLElement | null>(null)
let chart: Chart | null = null
let resizeObserver: ResizeObserver | null = null
let stopObservingTheme: (() => void) | null = null
let updateBar: ((data: KLineData) => void) | null = null
let maIndicatorId: string | null = null
let volumeIndicatorId: string | null = null
let renderedPoints: readonly NormalizedMarketChartPoint[] = []
let currentRows: KLineData[] = []
let pendingPeriod: Period | null = null
let fitNextDataset = true
let appliedSymbol: MarketChartSymbolInfo | null = null

function klineRows(points: readonly NormalizedMarketChartPoint[]): KLineData[] {
  return points.map((point) => ({
    timestamp: point.time * 1000,
    open: point.open,
    high: point.high,
    low: point.low,
    close: point.close,
    volume: point.volume,
  }))
}

function indicatorLineStyles(theme: MarketChartTheme): Array<{ color: string }> {
  return [theme.ma5, theme.ma10, theme.ma20].map((color) => ({ color }))
}

function klineStyles(theme: MarketChartTheme): DeepPartial<Styles> {
  const grid = marketChartColorWithAlpha(theme.grid, .62)
  return {
    grid: {
      show: true,
      horizontal: { show: true, color: grid },
      vertical: { show: true, color: grid },
    },
    candle: {
      bar: {
        upColor: theme.positive,
        downColor: theme.negative,
        noChangeColor: theme.muted,
        upBorderColor: theme.positive,
        downBorderColor: theme.negative,
        noChangeBorderColor: theme.muted,
        upWickColor: theme.positive,
        downWickColor: theme.negative,
        noChangeWickColor: theme.muted,
      },
      priceMark: {
        high: { color: theme.muted },
        low: { color: theme.muted },
      },
      tooltip: {
        showRule: 'none',
        features: [],
        title: { show: false },
      },
    },
    indicator: {
      ohlc: {
        upColor: theme.positive,
        downColor: theme.negative,
        noChangeColor: theme.muted,
      },
      bars: [{
        upColor: marketChartColorWithAlpha(theme.positive, .5),
        downColor: marketChartColorWithAlpha(theme.negative, .5),
        noChangeColor: marketChartColorWithAlpha(theme.muted, .42),
      }],
      lines: indicatorLineStyles(theme),
      tooltip: {
        showRule: 'none',
        features: [],
        title: { show: false, showName: false, showParams: false },
      },
    },
    xAxis: {
      show: true,
      axisLine: { color: theme.grid },
      tickLine: { color: theme.grid },
      tickText: { show: true, color: theme.muted },
    },
    yAxis: {
      show: true,
      axisLine: { color: theme.grid },
      tickLine: { color: theme.grid },
      tickText: { show: true, color: theme.muted },
    },
    separator: { color: theme.grid },
    crosshair: {
      show: true,
      horizontal: {
        show: true,
        line: { color: theme.muted },
        text: {
          show: true,
          color: theme.background,
          borderColor: theme.muted,
          backgroundColor: theme.muted,
        },
      },
      vertical: {
        show: true,
        line: { color: theme.muted },
        text: {
          show: true,
          color: theme.background,
          borderColor: theme.muted,
          backgroundColor: theme.muted,
        },
      },
    },
  }
}

function movingAverageIndicator(theme: MarketChartTheme) {
  return {
    name: 'MA',
    paneId: CANDLE_PANE_ID,
    calcParams: [5, 10, 20],
    styles: { lines: indicatorLineStyles(theme) },
  }
}

function volumeIndicator(theme: MarketChartTheme) {
  return {
    name: 'VOL',
    paneId: VOLUME_PANE_ID,
    calcParams: [],
    styles: {
      bars: [{
        upColor: marketChartColorWithAlpha(theme.positive, .5),
        downColor: marketChartColorWithAlpha(theme.negative, .5),
        noChangeColor: marketChartColorWithAlpha(theme.muted, .42),
      }],
    },
  }
}

const localDataLoader: DataLoader = {
  getBars: ({ type, callback }) => {
    if (type === 'init') {
      callback(currentRows, { backward: false, forward: false })
    } else {
      callback([], false)
    }
  },
  subscribeBar: ({ callback }) => {
    updateBar = callback
  },
  unsubscribeBar: () => {
    updateBar = null
  },
}

function applyTheme(): void {
  if (!chart || !container.value) return
  const theme = readMarketChartTheme(container.value)
  chart.setStyles(klineStyles(theme))
  if (maIndicatorId) {
    chart.overrideIndicator({
      ...movingAverageIndicator(theme),
      id: maIndicatorId,
    })
  }
  if (volumeIndicatorId) {
    chart.overrideIndicator({
      ...volumeIndicator(theme),
      id: volumeIndicatorId,
    })
  }
}

function resize(): void {
  if (!chart || !container.value) return
  const width = container.value.clientWidth
  const height = container.value.clientHeight
  if (width <= 0 || height <= 0) return
  chart.resize()
}

function captureViewport(): MarketChartViewport | null {
  if (!chart) return null
  return captureMarketChartViewport(
    chart.getDataList(),
    chart.getVisibleRange(),
    chart.getBarSpace().bar,
  )
}

function restoreViewport(viewport: MarketChartViewport | null): void {
  if (!chart || !viewport || !currentRows.length) return
  chart.setBarSpace(viewport.barSpace)
  const targetRealTo = resolveMarketChartViewportRealTo(currentRows, viewport)
  if (targetRealTo === null) return
  const currentRealTo = chart.getVisibleRange().realTo
  const distance = (currentRealTo - targetRealTo) * chart.getBarSpace().bar
  if (Math.abs(distance) > .01) chart.scrollByDistance(distance, 0)
}

function fitContent(): void {
  if (!chart || !container.value || !currentRows.length) return
  const drawableWidth = chart.getSize(CANDLE_PANE_ID, 'main')?.width
    ?? container.value.clientWidth
  const barSpace = Math.max(1, Math.min(12, (drawableWidth - 18) / currentRows.length))
  chart.setBarSpace(barSpace)
  chart.scrollToRealTime(0)
  fitNextDataset = false
}

function replaceData(preserveViewport: boolean): void {
  if (!chart) return
  const viewport = preserveViewport ? captureViewport() : null
  chart.resetData()
  if (!currentRows.length) {
    fitNextDataset = true
    return
  }
  if (fitNextDataset) fitContent()
  else restoreViewport(viewport)
}

function applyIncrementalUpdate(): boolean {
  const latest = currentRows.at(-1)
  if (!latest || !updateBar) return false
  updateBar(latest)
  return true
}

function synchronizeSymbolMetadata(
  symbol: string,
  points: readonly NormalizedMarketChartPoint[],
  force = false,
): boolean {
  if (!chart) return false
  const inferred = resolveMarketChartSymbolInfo(symbol, points)
  const next = points.length === 0 && appliedSymbol?.ticker === inferred.ticker
    ? { ...inferred, pricePrecision: appliedSymbol.pricePrecision, volumePrecision: appliedSymbol.volumePrecision }
    : inferred
  if (
    !force
    && appliedSymbol?.ticker === next.ticker
    && appliedSymbol.pricePrecision === next.pricePrecision
    && appliedSymbol.volumePrecision === next.volumePrecision
  ) {
    return false
  }
  chart.setSymbol({
    ticker: next.ticker,
    pricePrecision: next.pricePrecision,
    volumePrecision: next.volumePrecision,
  })
  appliedSymbol = next
  return true
}

onMounted(() => {
  if (!container.value) return
  const theme = readMarketChartTheme(container.value)
  chart = init(container.value, {
    locale: props.locale,
    styles: klineStyles(theme),
    layout: {
      pane: { dragEnabled: false, minHeight: 56 },
      yAxis: { inside: false, position: 'right' },
    },
  })
  if (!chart) return

  currentRows = klineRows(props.points)
  renderedPoints = props.points
  maIndicatorId = chart.createIndicator(movingAverageIndicator(theme), true)
  volumeIndicatorId = chart.createIndicator(volumeIndicator(theme))
  chart.setPaneOptions({
    id: VOLUME_PANE_ID,
    dragEnabled: false,
    height: 82,
    minHeight: 58,
    order: 1,
  })
  chart.setOffsetRightDistance(12)
  synchronizeSymbolMetadata(props.symbol, props.points, true)
  chart.setPeriod(marketChartPeriod(props.interval))
  chart.setDataLoader(localDataLoader)
  if (currentRows.length) fitContent()

  resizeObserver = new ResizeObserver(resize)
  resizeObserver.observe(container.value)
  stopObservingTheme = observeMarketChartTheme(
    container.value,
    document.documentElement,
    applyTheme,
  )
})

watch(
  () => ({ interval: props.interval, points: props.points, symbol: props.symbol }),
  (next, previous) => {
    if (!chart) return
    const intervalChanged = next.interval !== previous.interval
    const symbolChanged = next.symbol !== previous.symbol
    const pointsChanged = next.points !== previous.points
    if (intervalChanged) {
      fitNextDataset = true
      pendingPeriod = marketChartPeriod(next.interval)
    }
    if (symbolChanged) fitNextDataset = true
    if ((intervalChanged || symbolChanged) && !pointsChanged) {
      if (symbolChanged) {
        currentRows = klineRows(next.points)
        renderedPoints = next.points
        synchronizeSymbolMetadata(next.symbol, next.points)
        if (currentRows.length) fitContent()
      }
      return
    }

    const update = classifyMarketChartDataUpdate(renderedPoints, next.points)
    const preserveViewport = !fitNextDataset && renderedPoints.length > 0
    const metadataViewport = preserveViewport && (symbolChanged || update === 'replace')
      ? captureViewport()
      : null
    currentRows = klineRows(next.points)
    if (pendingPeriod) {
      renderedPoints = next.points
      chart.setPeriod(pendingPeriod)
      pendingPeriod = null
      if (symbolChanged || update === 'replace') {
        synchronizeSymbolMetadata(next.symbol, next.points)
      }
      if (currentRows.length) fitContent()
      return
    }

    if (
      (symbolChanged || update === 'replace')
      && synchronizeSymbolMetadata(next.symbol, next.points)
    ) {
      renderedPoints = next.points
      if (!currentRows.length) fitNextDataset = true
      else if (fitNextDataset) fitContent()
      else restoreViewport(metadataViewport)
      return
    }

    if (renderedPoints.length <= 1 && next.points.length > renderedPoints.length) {
      fitNextDataset = true
    }
    if (
      !fitNextDataset
      && (update === 'update-last' || update === 'append')
      && applyIncrementalUpdate()
    ) {
      renderedPoints = next.points
      return
    }
    if (update !== 'none') {
      replaceData(preserveViewport)
      renderedPoints = next.points
    }
  },
  { deep: true },
)

watch(() => props.locale, (locale) => {
  chart?.setLocale(locale)
})

onUnmounted(() => {
  resizeObserver?.disconnect()
  stopObservingTheme?.()
  stopObservingTheme = null
  updateBar = null
  if (chart) dispose(chart)
  chart = null
  appliedSymbol = null
  maIndicatorId = null
  volumeIndicatorId = null
})
</script>

<template>
  <div
    ref="container"
    class="market-chart-engine"
    data-kline-provider="klinecharts"
    data-chart-package="klinecharts@10.0.0"
    :data-chart-symbol="symbol"
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
