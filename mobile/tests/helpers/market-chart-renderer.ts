import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import ts from 'typescript'
import { effectScope, nextTick, reactive, ref, watch } from 'vue'
import { resolveMarketChartPriceFormat, type NormalizedMarketChartPoint } from '../../src/core/marketChart.ts'
import * as runtime from '../../src/core/marketChartRuntime.ts'
import { calculateMarketMovingAverages } from '../../src/core/marketIndicators.ts'

type Range = { from: number, to: number }
type Row = { time: number, [key: string]: unknown }
type Options = Record<string, unknown>

export function chartPoints(count: number, start = 1_700_000_000): NormalizedMarketChartPoint[] {
  return Array.from({ length: count }, (_, index) => ({
    time: start + index * 60, open: 10, high: 12, low: 9, close: 11, volume: index + 1,
  }))
}

// Execute the actual SFC setup script with real Vue scheduling and production
// helpers. Only browser/chart I/O is fake; no renderer decision is copied here.
export function mountMarketChart(initial: {
  points?: NormalizedMarketChartPoint[]
  historyLoading?: boolean
} = {}) {
  const component = readFileSync(new URL('../../src/components/LightweightMarketChart.vue', import.meta.url), 'utf8')
  const script = component.match(/<script setup lang="ts">([\s\S]*?)<\/script>/)?.[1]
  assert.ok(script)
  const syntax = ts.createSourceFile('chart.ts', script, ts.ScriptTarget.Latest, true)
  const source = syntax.statements.filter((item) => !ts.isImportDeclaration(item))
    .map((item) => item.getText(syntax)).join('\n')
  const output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
    reportDiagnostics: true,
  })
  assert.equal(output.diagnostics?.length ?? 0, 0)

  const points = initial.points ?? []
  const props = reactive({
    points,
    movingAverages: calculateMarketMovingAverages(points),
    symbol: 'BTCUSDT', interval: '1m', locale: 'en-US', label: 'Market',
    historyLoading: initial.historyLoading ?? false,
  })
  let visibleRange: Range | null = null
  let fitCount = 0
  let removed = false
  let resizeDisconnected = false
  let themeDisconnected = false
  let themeCallback = () => {}
  let nextFrame = 1
  const frames = new Map<number, FrameRequestCallback>()
  const restoredRanges: Range[] = []
  const chartOptions: Options[] = []
  const resizeCalls: number[][] = []
  const listeners = new Map<string, Set<EventListener>>()
  const rangeListeners = new Set<(range: Range | null) => void>()
  const emitted: string[] = []
  const container = {
    clientWidth: 350, clientHeight: 300,
    addEventListener(name: string, listener: EventListener) {
      const entries = listeners.get(name) ?? new Set<EventListener>()
      entries.add(listener)
      listeners.set(name, entries)
    },
    removeEventListener(name: string, listener: EventListener) { listeners.get(name)?.delete(listener) },
  }

  const series: Array<{
    kind: string, options: Options[], rows: Row[], sets: number, updates: number,
    setData: (rows: Row[]) => void, update: (row: Row) => void,
    applyOptions: (options: Options) => void,
    priceScale: () => { applyOptions: (options: Options) => void },
  }> = []
  const timeScale = {
    getVisibleLogicalRange: () => visibleRange && { ...visibleRange },
    subscribeVisibleLogicalRangeChange(callback: (range: Range | null) => void) { rangeListeners.add(callback) },
    unsubscribeVisibleLogicalRangeChange(callback: (range: Range | null) => void) { rangeListeners.delete(callback) },
    setVisibleLogicalRange(range: Range) {
      assert.equal(removed, false)
      visibleRange = { ...range }
      restoredRanges.push({ ...range })
      rangeListeners.forEach((callback) => callback(visibleRange))
    },
    fitContent() {
      fitCount += 1
      visibleRange = { from: -.5, to: (series[0]?.rows.length ?? 0) - .5 }
      rangeListeners.forEach((callback) => callback(visibleRange))
    },
  }
  // Approximate native append shifting only. These tests verify application
  // calls, not native canvas geometry or the library's internal scheduling.
  function nativeAppendRange(previous: Row[], next: Row[]) {
    const last = previous.length - 1
    if (visibleRange && previous.length && next.length > previous.length
      && visibleRange.from <= last && visibleRange.to >= last
      && next[0].time >= previous[0].time) {
      const delta = next.length - previous.length
      visibleRange = { from: visibleRange.from + delta, to: visibleRange.to + delta }
    }
  }
  const chart = {
    timeScale: () => timeScale,
    addSeries(kind: string, options: Options) {
      const result = {
        kind, options: [options], rows: [] as Row[], sets: 0, updates: 0,
        setData(rows: Row[]) {
          if (kind === 'candles') nativeAppendRange(this.rows, rows)
          this.rows = rows
          this.sets += 1
        },
        update(row: Row) {
          const rows = [...this.rows]
          if (rows.at(-1)?.time === row.time) rows[rows.length - 1] = row
          else rows.push(row)
          if (kind === 'candles') nativeAppendRange(this.rows, rows)
          this.rows = rows
          this.updates += 1
        },
        applyOptions(options: Options) { this.options.push(options) },
        priceScale: () => ({ applyOptions: (_options: Options) => {} }),
      }
      series.push(result)
      return result
    },
    applyOptions(options: Options) { chartOptions.push(options) },
    resize(width: number, height: number) { resizeCalls.push([width, height]) },
    remove() { removed = true },
  }
  const theme = {
    background: '#ffffff', muted: '#666666', grid: '#cccccc', positive: '#00aa00',
    negative: '#aa0000', ma5: '#112233', ma10: '#445566', ma20: '#778899',
  }
  const mounts: Array<() => void> = []
  const unmounts: Array<() => void> = []
  let resizeCallback = () => {}
  const bindings = {
    ...runtime,
    ref, watch,
    defineProps: () => props,
    defineEmits: () => (event: string) => emitted.push(event),
    withDefaults: (value: typeof props) => value,
    onMounted: (callback: () => void) => mounts.push(callback),
    onUnmounted: (callback: () => void) => unmounts.push(callback),
    resolveMarketChartPriceFormat,
    readMarketChartTheme: () => theme,
    marketChartColorWithAlpha: (color: string) => color,
    observeMarketChartTheme: (_container: unknown, _root: unknown, callback: () => void) => {
      themeCallback = callback
      return () => { themeDisconnected = true }
    },
    createChart: (_container: unknown, options: Options) => {
      chartOptions.push(options)
      return chart
    },
    CandlestickSeries: 'candles', HistogramSeries: 'volume', LineSeries: 'line',
    ColorType: { Solid: 'solid' },
    getComputedStyle: () => ({ fontFamily: 'sans-serif' }),
    document: { documentElement: {} },
    ResizeObserver: class {
      constructor(callback: () => void) { resizeCallback = callback }
      observe() {}
      disconnect() { resizeDisconnected = true }
    },
    requestAnimationFrame: (callback: FrameRequestCallback) => {
      const id = nextFrame++
      frames.set(id, callback)
      return id
    },
    cancelAnimationFrame: (id: number) => frames.delete(id),
  }
  const setup = new Function(...Object.keys(bindings), `${output.outputText}\nreturn { container };`) as (
    ...args: unknown[]
  ) => { container: { value: unknown } }
  const scope = effectScope()
  const exposed = scope.run(() => setup(...Object.values(bindings)))!
  exposed.container.value = container
  mounts.forEach((callback) => callback())

  return {
    props, series, chartOptions, restoredRanges, resizeCalls, container, emitted,
    get range() { return visibleRange && { ...visibleRange } },
    get fits() { return fitCount },
    get pendingFrames() { return frames.size },
    get listenerCount() { return [...listeners.values()].reduce((sum, entries) => sum + entries.size, 0) },
    get rangeListenerCount() { return rangeListeners.size },
    get removed() { return removed },
    get observersDisconnected() { return resizeDisconnected && themeDisconnected },
    async update(next: Partial<typeof props>) {
      Object.assign(props, next)
      if (next.points) props.movingAverages = calculateMarketMovingAverages(next.points)
      await nextTick()
    },
    setRange(range: Range) { visibleRange = { ...range } },
    notifyRange(range: Range | null) {
      visibleRange = range && { ...range }
      rangeListeners.forEach((callback) => callback(visibleRange))
    },
    gesture(type: string, extra: Record<string, unknown> = {}) {
      for (const listener of listeners.get(type) ?? []) listener({ type, ...extra } as unknown as Event)
    },
    flushFrames() {
      const callbacks = [...frames.values()]
      frames.clear()
      callbacks.forEach((callback) => callback(0))
    },
    theme: () => themeCallback(),
    resize: () => resizeCallback(),
    unmount() {
      exposed.container.value = null
      unmounts.forEach((callback) => callback())
      scope.stop()
    },
  }
}
