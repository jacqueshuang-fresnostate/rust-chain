import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import {
  calculateMarketMovingAverages,
  calculateSimpleMovingAverage,
  latestMarketMovingAverages,
} from '../src/core/marketIndicators.ts'
import {
  captureMarketChartLogicalViewport,
  captureMarketChartViewport,
  classifyMarketChartDataUpdate,
  DEFAULT_MARKET_CHART_ENGINE,
  loadMarketChartEngine,
  MARKET_CHART_ENGINE_STORAGE_KEY,
  marketChartPeriod,
  normalizeMarketChartTicker,
  persistMarketChartEngine,
  resolveMarketChartSymbolInfo,
  resolveMarketChartLogicalRange,
  resolveMarketChartViewportRealTo,
  type MarketChartEngineStorage,
} from '../src/core/marketChartEngine.ts'

const marketDetailSource = source('../src/views/MarketDetailView.vue')
const chartSource = source('../src/components/MobileMarketChart.vue')
const klineChartSource = source('../src/components/KLineChartMarketChart.vue')
const tradingViewChartSource = source('../src/components/TradingViewMarketChart.vue')
const orderBookSource = source('../src/components/OrderBookPanel.vue')
const tradeSource = source('../src/views/TradeView.vue')
const packageJson = JSON.parse(source('../package.json')) as {
  dependencies: Record<string, string>
}
const packageLock = JSON.parse(source('../package-lock.json')) as {
  packages: Record<string, { version?: string }>
}
const marketTemplate = marketDetailSource.match(/<template>[\s\S]*?<\/template>/)?.[0] ?? ''
const marketStyle = marketDetailSource.match(/<style scoped>[\s\S]*?<\/style>/)?.[0] ?? ''
const marketScopedCss = marketDetailSource.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1] ?? ''

test('行情详情按参考层级组成真实紧凑工作台', () => {
  const hierarchy = [
    'class="market-detail__header"',
    'class="market-detail__rail"',
    'class="market-detail__summary"',
    'class="market-detail__chart-panel"',
    'class="market-detail__microstructure"',
    'class="market-detail__actions"',
  ].map((value) => marketTemplate.indexOf(value))

  assert.ok(hierarchy.every((index) => index >= 0))
  assert.deepEqual([...hierarchy].sort((left, right) => left - right), hierarchy)
  assert.match(marketTemplate, /<AssetMark[\s\S]*:symbol="baseAsset"[\s\S]*:src="ticker\?\.iconUrl"[\s\S]*:size="24"/)
  assert.match(marketTemplate, /market-detail__instrument[\s\S]*t\('marketDetail\.selectPair'\)[\s\S]*<ChevronDown/)
  assert.match(marketTemplate, /market-detail__favorite[\s\S]*<Star[\s\S]*<Share2/)
  assert.doesNotMatch(marketTemplate, /market-detail__instrument-title|market-detail__microquote|t\('marketDetail\.spot'\)/)
  assert.match(marketTemplate, /t\('marketDetail\.high24h'\)/)
  assert.match(marketTemplate, /t\('marketDetail\.low24h'\)/)
  assert.match(marketTemplate, /t\('marketDetail\.volume24h'\)/)
  assert.match(marketTemplate, /t\('marketDetail\.turnover24h'\)[\s\S]*<dd class="numeric">-- \{\{ quoteAsset \}\}<\/dd>/)
  assert.match(marketTemplate, /formatObservedTime\(observedAt\)/)
  assert.match(marketDetailSource, /trades\.value\[0\]\?\.price[\s\S]*latestCandle\.value\?\.close[\s\S]*ticker\.value\?\.lastPrice/)
  assert.match(marketDetailSource, /const hasLatestPrice = computed\(\(\) => Number\.isFinite\(latestPrice\.value\) && latestPrice\.value > 0\)/)
  assert.match(marketTemplate, /<strong[\s\S]*v-if="hasLatestPrice"[\s\S]*formatPrice\(latestPrice\)/)
  assert.match(marketDetailSource, /\(\(latestPrice\.value - market\.openPrice\) \/ market\.openPrice\) \* 100/)
  assert.match(marketTemplate, /formatPercent\(latestChangePercent\)/)
  assert.match(marketDetailSource, /onDepth: \(_context, snapshot\) => \{[\s\S]*liveDetailActive\.value = true/)
  assert.match(marketDetailSource, /startLiveDetail\([\s\S]*liveDetailActive\.value = false/)
  assert.match(marketTemplate, /liveDetailActive[\s\S]*t\('common\.liveData'\)[\s\S]*t\('marketDetail\.snapshotData'\)/)
})

test('导航 rail、底部 action deck 和面板切换全部连接真实锚点或命名路由', () => {
  assert.match(marketTemplate, /aria-controls="market-chart"[\s\S]*scrollToSection\('chart'\)/)
  assert.match(marketTemplate, /aria-controls="market-overview"[\s\S]*scrollToSection\('overview'\)/)
  assert.equal((marketTemplate.match(/<nav class="market-detail__rail"[\s\S]*?<\/nav>/)?.[0].match(/<button/g) || []).length, 2)
  assert.doesNotMatch(marketTemplate, /href="#market-(?:chart|order-book|latest-trades)"/)
  assert.match(marketDetailSource, /target\?\.scrollIntoView\(\{[\s\S]*block: 'start'/)
  assert.match(marketDetailSource, /function openPairPicker\(\)[\s\S]*name: 'markets'/)
  assert.match(marketDetailSource, /name: 'trade',[\s\S]*params: \{ symbol: pairSymbol\.value\.replace\('\/', '_'\) \}/)
  assert.match(marketDetailSource, /query: mode === 'contract' \? \{ mode: 'contract' \} : undefined/)
  assert.match(marketDetailSource, /name: 'orders',[\s\S]*query: \{ symbol: pairSymbol\.value\.replace\('\/', '_'\) \}/)
  assert.match(marketTemplate, /market-detail__actions[\s\S]*openTrade\('contract'\)[\s\S]*@click="openOrders"[\s\S]*openTrade\('spot'\)/)

  assert.match(marketTemplate, /role="tablist"/)
  assert.match(marketTemplate, /:aria-pressed="marketDataPanel === 'orderBook'"/)
  assert.match(marketTemplate, /:aria-pressed="marketDataPanel === 'trades'"/)
  assert.match(marketTemplate, /:tabindex="marketDataPanel === 'orderBook' \? 0 : -1"/)
  assert.match(marketTemplate, /@keydown="handleMarketDataTabKeydown\(\$event, 'trades'\)"/)
  assert.match(marketDetailSource, /event\.key === 'ArrowRight'[\s\S]*event\.key === 'Home'[\s\S]*target\?\.focus\(\)/)
  assert.match(marketTemplate, /role="tabpanel"/)
  assert.match(marketTemplate, /v-show="marketDataPanel === 'orderBook'"/)
  assert.match(marketTemplate, /v-show="marketDataPanel === 'trades'"/)
})

test('MA5、MA10、MA20 由真实收盘价计算并随形成中蜡烛更新', () => {
  const points = Array.from({ length: 20 }, (_, index) => ({
    time: 1_720_000_000 + index * 60,
    close: index + 1,
  }))
  const averages = calculateMarketMovingAverages(points)

  assert.equal(averages.ma5.length, 16)
  assert.equal(averages.ma10.length, 11)
  assert.equal(averages.ma20.length, 1)
  assert.deepEqual(latestMarketMovingAverages(averages), {
    ma5: 18,
    ma10: 15.5,
    ma20: 10.5,
  })

  const livePoints = [...points.slice(0, -1), { ...points.at(-1)!, close: 30 }]
  assert.deepEqual(latestMarketMovingAverages(calculateMarketMovingAverages(livePoints)), {
    ma5: 20,
    ma10: 16.5,
    ma20: 11,
  })
  assert.deepEqual(calculateSimpleMovingAverage(points, 0), [])
})

test('本地双引擎共享真实数据、指标、成交量与稳定视口', () => {
  assert.equal(packageJson.dependencies.klinecharts, '10.0.0')
  assert.equal(packageJson.dependencies['lightweight-charts'], '5.2.0')
  assert.equal(packageLock.packages['node_modules/klinecharts']?.version, '10.0.0')
  assert.equal(packageLock.packages['node_modules/lightweight-charts']?.version, '5.2.0')

  assert.match(chartSource, /const engine = ref<MarketChartEngine>\(loadMarketChartEngine\(\)\)/)
  assert.match(chartSource, /\['klinecharts', 'tradingview'\]/)
  assert.match(chartSource, /v-if="engine === 'klinecharts'"/)
  assert.match(chartSource, /<TradingViewMarketChart[\s\S]*v-else/)
  assert.match(chartSource, /:points="normalizedPoints"/)
  assert.match(chartSource, /:moving-averages="movingAverages"/)
  assert.match(chartSource, /<KLineChartMarketChart[\s\S]*:symbol="symbol"/)
  assert.match(chartSource, /const chartLocale = computed\(\(\) => locale\.value === 'en' \? 'en-US' : 'zh-CN'\)/)
  assert.equal(chartSource.match(/:locale="chartLocale"/g)?.length, 2)
  assert.match(chartSource, /role="radiogroup"/)
  assert.match(chartSource, /aria-orientation="horizontal"/)
  assert.match(chartSource, /role="radio"/)
  assert.match(chartSource, /:aria-checked="engine === option"/)
  assert.match(chartSource, /event\.key === 'ArrowLeft'[\s\S]*event\.key === 'Home'/)
  assert.match(chartSource, /event\.key === 'ArrowRight'[\s\S]*event\.key === 'End'/)
  assert.match(chartSource, /persistMarketChartEngine\(value\)/)
  assert.match(chartSource, /if \(props\.compactEngineSwitch && closeCompactMenu\) \{[\s\S]*compactEngineMenuOpen\.value = false[\s\S]*compactEngineTrigger\.value\?\.focus\(\)/)
  assert.match(chartSource, /data-fit-policy="initial-or-interval"/)
  assert.match(chartSource, /mobile-market-chart__engine-switch button[\s\S]*min-height: 44px/)
  assert.match(chartSource, /:aria-expanded="compactEngineMenuOpen"[\s\S]*<Settings2/)
  assert.match(chartSource, /compactEngineMenuOpen\.value = false[\s\S]*compactEngineTrigger\.value\?\.focus\(\)/)
  assert.match(marketTemplate, /<MobileMarketChart[\s\S]*:interval="interval"[\s\S]*show-engine-switch[\s\S]*compact-engine-switch/)

  assert.match(klineChartSource, /from 'klinecharts'/)
  assert.match(klineChartSource, /data-kline-provider="klinecharts"/)
  assert.match(klineChartSource, /data-chart-package="klinecharts@10\.0\.0"/)
  assert.match(klineChartSource, /setDataLoader\(localDataLoader\)/)
  assert.match(klineChartSource, /callback\(currentRows, \{ backward: false, forward: false \}\)/)
  assert.match(klineChartSource, /name: 'MA'[\s\S]*calcParams: \[5, 10, 20\]/)
  assert.match(klineChartSource, /name: 'VOL'[\s\S]*calcParams: \[\]/)
  assert.match(klineChartSource, /volume: point\.volume/)
  assert.match(klineChartSource, /updateBar\(latest\)/)
  assert.match(klineChartSource, /captureViewport\(\)[\s\S]*restoreViewport\(viewport\)/)
  assert.match(klineChartSource, /captureMarketChartViewport\([\s\S]*resolveMarketChartViewportRealTo\(/)
  assert.match(klineChartSource, /scrollByDistance\(distance, 0\)/)
  assert.match(klineChartSource, /pendingPeriod = marketChartPeriod\(next\.interval\)/)
  assert.match(klineChartSource, /if \(\(intervalChanged \|\| symbolChanged\) && !pointsChanged\)/)
  assert.match(klineChartSource, /observeMarketChartTheme\(/)
  assert.match(klineChartSource, /watch\(\(\) => props\.locale[\s\S]*chart\?\.setLocale\(locale\)/)
  assert.match(klineChartSource, /if \(chart\) dispose\(chart\)/)

  const symbolIndex = klineChartSource.indexOf('synchronizeSymbolMetadata(props.symbol, props.points, true)')
  const periodIndex = klineChartSource.indexOf('chart.setPeriod(marketChartPeriod(props.interval))')
  const loaderIndex = klineChartSource.indexOf('chart.setDataLoader(localDataLoader)')
  assert.ok(symbolIndex > 0 && periodIndex > symbolIndex && loaderIndex > periodIndex)

  assert.equal(tradingViewChartSource.match(/chart\.addSeries\(LineSeries/g)?.length, 3)
  assert.match(tradingViewChartSource, /chart\.addSeries\(HistogramSeries/)
  assert.match(tradingViewChartSource, /value: point\.volume/)
  assert.equal(tradingViewChartSource.match(/attributionLogo: false/g)?.length, 2)
  assert.doesNotMatch(tradingViewChartSource, /attributionLogo: true|tv-attr-logo/)
  assert.match(tradingViewChartSource, /data-kline-provider="tradingview"/)
  assert.match(tradingViewChartSource, /data-chart-package="lightweight-charts@5\.2\.0"/)
  assert.match(tradingViewChartSource, /candles\?\.update\(candleRow\(point\)\)/)
  assert.match(tradingViewChartSource, /volume\?\.update\(volumeRow\(point, theme\)\)/)
  assert.equal(tradingViewChartSource.match(/timeScale\(\)\.fitContent\(\)/g)?.length, 1)
  assert.match(tradingViewChartSource, /if \(fitKeyChanged && !pointsChanged\) return/)
  assert.match(tradingViewChartSource, /renderAllData\(false, viewport\)/)
  assert.match(tradingViewChartSource, /captureMarketChartLogicalViewport\([\s\S]*resolveMarketChartLogicalRange\(/)
  assert.match(tradingViewChartSource, /requestAnimationFrame\(\(\) => \{[\s\S]*restoreViewport\(viewport\)/)
  assert.match(tradingViewChartSource, /watch\(\(\) => props\.locale[\s\S]*localization: \{ locale \}/)
  assert.match(tradingViewChartSource, /pinch: true/)
  for (const engineSource of [klineChartSource, tradingViewChartSource]) {
    assert.match(engineSource, /\.market-chart-engine\s*\{[\s\S]*height: 100%;[\s\S]*min-height: 0;/)
    assert.doesNotMatch(engineSource, /\.market-chart-engine\s*\{[\s\S]*min-height: 220px;/)
  }
  assert.match(tradeSource, /<MobileMarketChart :points="points" :loading="chartLoading" :interval="interval" :symbol="pairSymbol" \/>/)

  const chartRuntimeSources = [chartSource, klineChartSource, tradingViewChartSource]
  for (const runtimeSource of chartRuntimeSources) {
    assert.doesNotMatch(runtimeSource, /<iframe|<script[^>]+src=|<a\b|href=|https?:\/\/|cdn\.|TradingView\.widget|KLineChartPro|datafeed/i)
    assert.doesNotMatch(runtimeSource, /marketDetailStream|fetchKlines|WebSocket/)
  }
})

test('KLineChart 使用实际交易对与行情量级精度，并隐藏重复内置图例', () => {
  assert.equal(normalizeMarketChartTicker(' btc_usdt '), 'BTC/USDT')
  assert.deepEqual(resolveMarketChartSymbolInfo('btc_usdt', [{
    close: 67_432.12345678,
    volume: 12.345678,
  }]), {
    ticker: 'BTC/USDT',
    pricePrecision: 2,
    volumePrecision: 2,
  })
  assert.deepEqual(resolveMarketChartSymbolInfo('pepe-usdt', [{
    close: .000012345678,
    volume: 12_345.678,
  }]), {
    ticker: 'PEPE/USDT',
    pricePrecision: 8,
    volumePrecision: 0,
  })

  assert.doesNotMatch(klineChartSource, /ticker:\s*['"]HIPPO['"]|pricePrecision:\s*8,\s*volumePrecision:\s*8/)
  assert.equal(klineChartSource.match(/showRule: 'none'/g)?.length, 2)
  assert.match(klineChartSource, /candle:[\s\S]*tooltip:[\s\S]*showRule: 'none'/)
  assert.match(klineChartSource, /indicator:[\s\S]*tooltip:[\s\S]*showRule: 'none'/)
  assert.match(klineChartSource, /crosshair:\s*\{[\s\S]*show: true[\s\S]*horizontal:[\s\S]*vertical:/)
  assert.match(klineChartSource, /synchronizeSymbolMetadata\(props\.symbol, props\.points, true\)/)
  assert.match(klineChartSource, /update === 'update-last' \|\| update === 'append'[\s\S]*applyIncrementalUpdate\(\)/)
  assert.match(marketTemplate, /market-detail__indicator-legend[\s\S]*MA5[\s\S]*MA10[\s\S]*MA20[\s\S]*candleVolume/)
  assert.match(marketTemplate, /<MobileMarketChart[\s\S]*:symbol="pairSymbol"[\s\S]*show-engine-switch/)
})

test('图表引擎偏好默认、本地持久化和形成中蜡烛分类可独立验证', () => {
  const values = new Map<string, string>()
  const storage: MarketChartEngineStorage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => { values.set(key, value) },
  }
  assert.equal(DEFAULT_MARKET_CHART_ENGINE, 'klinecharts')
  assert.equal(loadMarketChartEngine(storage), 'klinecharts')
  persistMarketChartEngine('tradingview', storage)
  assert.equal(values.get(MARKET_CHART_ENGINE_STORAGE_KEY), 'tradingview')
  assert.equal(loadMarketChartEngine(storage), 'tradingview')
  values.set(MARKET_CHART_ENGINE_STORAGE_KEY, 'unknown')
  assert.equal(loadMarketChartEngine(storage), 'klinecharts')
  assert.deepEqual(marketChartPeriod('4h'), { type: 'hour', span: 4 })
  assert.deepEqual(marketChartPeriod('bad'), { type: 'minute', span: 15 })

  const first = { time: 1, open: 1, high: 2, low: .5, close: 1.5, volume: 3 }
  const forming = { time: 2, open: 2, high: 3, low: 1, close: 2.5, volume: 4 }
  assert.equal(classifyMarketChartDataUpdate([], [first]), 'replace')
  assert.equal(classifyMarketChartDataUpdate([first, forming], [first, forming]), 'none')
  assert.equal(classifyMarketChartDataUpdate(
    [first, forming],
    [first, { ...forming, close: 2.75 }],
  ), 'update-last')
  assert.equal(classifyMarketChartDataUpdate(
    [first],
    [first, forming],
  ), 'append')
  assert.equal(classifyMarketChartDataUpdate(
    [first, forming],
    [{ ...first, close: 1.25 }, forming],
  ), 'replace')

  const rows = Array.from({ length: 100 }, (_, index) => ({ timestamp: 1_000 + index * 60 }))
  const viewport = captureMarketChartViewport(rows, { to: 72, realTo: 72 }, 6)
  assert.deepEqual(viewport, {
    barSpace: 6,
    anchorTimestamp: rows[71]?.timestamp,
    anchorOffset: 1,
  })
  assert.equal(resolveMarketChartViewportRealTo(rows, viewport!), 72)
  assert.equal(resolveMarketChartViewportRealTo(
    [{ timestamp: 940 }, ...rows],
    viewport!,
  ), 73)
  assert.equal(captureMarketChartViewport([], { to: 0, realTo: 0 }, 6), null)

  const logicalRows = rows.map((row) => ({ time: row.timestamp }))
  const logicalViewport = captureMarketChartLogicalViewport(
    logicalRows,
    { from: 24, to: 72 },
  )
  assert.deepEqual(logicalViewport, {
    rangeWidth: 48,
    anchorTimestamp: logicalRows[71]?.time,
    anchorOffset: 1,
  })
  assert.deepEqual(resolveMarketChartLogicalRange(logicalRows, logicalViewport!), {
    from: 24,
    to: 72,
  })
  assert.deepEqual(resolveMarketChartLogicalRange(
    [{ time: 940 }, ...logicalRows],
    logicalViewport!,
  ), {
    from: 25,
    to: 73,
  })
  assert.equal(captureMarketChartLogicalViewport([], { from: 0, to: 1 }), null)
})

test('图表沉浸展开使用 CSS fixed、安全区、Escape 和可还原滚动锁', () => {
  assert.match(marketDetailSource, /Maximize2/)
  assert.match(marketDetailSource, /Minimize2/)
  assert.match(marketTemplate, /:data-chart-mode="chartExpanded \? 'expanded' : 'inline'"/)
  assert.match(marketTemplate, /:role="chartExpanded \? 'dialog' : 'region'"/)
  assert.match(marketTemplate, /:aria-modal="chartExpanded \? 'true' : undefined"/)
  assert.match(marketTemplate, /:aria-label="chartExpanded \? t\('marketDetail\.collapseChart'\) : t\('marketDetail\.expandChart'\)"/)
  assert.match(marketDetailSource, /event\.key === 'Escape'/)
  assert.match(marketDetailSource, /event\.key !== 'Tab'[\s\S]*querySelectorAll<HTMLElement>[\s\S]*last\?\.focus\(\)/)
  assert.match(marketDetailSource, /document\.body\.style\.overflow = 'hidden'/)
  assert.match(marketDetailSource, /document\.body\.style\.position = 'fixed'/)
  assert.match(marketDetailSource, /window\.addEventListener\('keydown', handleChartKeydown\)/)
  assert.match(marketDetailSource, /window\.removeEventListener\('keydown', handleChartKeydown\)/)
  assert.match(marketDetailSource, /function restoreChartScroll[\s\S]*document\.documentElement\.style\.scrollBehavior = 'auto'[\s\S]*window\.scrollTo\(0, lock\.scrollY\)[\s\S]*lock\.rootScrollBehavior/)
  assert.match(marketDetailSource, /window\.scrollTo\(0, lock\.scrollY\)/)
  assert.equal(marketDetailSource.match(/focus\(\{ preventScroll: true \}\)/g)?.length, 2)
  assert.match(marketDetailSource, /chartExpanded\.value = false[\s\S]*const lock = releaseChartScroll\(\)[\s\S]*nextTick\(\(\) => \{[\s\S]*restoreChartScroll\(lock\)/)
  assert.match(marketDetailSource, /onBeforeUnmount\(\(\) => \{[\s\S]*const lock = releaseChartScroll\(\)[\s\S]*restoreChartScroll\(lock\)/)
  assert.match(marketStyle, /\.market-detail\.view-stack\s*\{[\s\S]*will-change: opacity/)
  assert.match(marketStyle, /\.market-detail__chart-panel\.is-expanded[\s\S]*position: fixed/)
  assert.match(marketStyle, /\.market-detail__chart-panel\.is-expanded[\s\S]*height: 100dvh/)
  assert.match(marketStyle, /env\(safe-area-inset-top\)/)
  assert.match(marketStyle, /env\(safe-area-inset-bottom\)/)
  assert.match(marketStyle, /padding: 0 env\(safe-area-inset-right\) 0 env\(safe-area-inset-left\)/)
  assert.doesNotMatch(marketDetailSource, /requestFullscreen|exitFullscreen|fullscreenElement/)
})

test('订单簿保持 stacked/split 兼容并为选中详情提供七行 paired 布局', () => {
  assert.match(orderBookSource, /layout\?: 'stacked' \| 'split' \| 'paired' \| 'matrix'/)
  assert.match(orderBookSource, /layout: 'stacked'/)
  assert.match(orderBookSource, /:data-layout="layout"/)
  assert.match(orderBookSource, /v-if="layout === 'split'"/)
  assert.match(orderBookSource, /splitAsks = computed\(\(\) => props\.asks\.slice\(0, 6\)\)/)
  assert.match(orderBookSource, /visibleBids = computed\(\(\) => props\.bids\.slice\(0, 6\)\)/)
  assert.match(orderBookSource, /data-book-side="bid"/)
  assert.match(orderBookSource, /data-book-side="ask"/)
  assert.match(orderBookSource, /:style="\{ width: width\(item\.quantity\) \}"/)
  assert.match(orderBookSource, /order-book__split-last b\s*\{[\s\S]*color: var\(--ink\)/)
  assert.match(orderBookSource, /grid-template-columns: minmax\(0, 1fr\) 1px minmax\(0, 1fr\)/)
  assert.match(orderBookSource, /matrixMode = computed\(\(\) => props\.layout === 'paired' \|\| props\.layout === 'matrix'\)/)
  assert.match(orderBookSource, /props\.bids\.slice\(0, 7\)/)
  assert.match(orderBookSource, /props\.asks\.slice\(0, 7\)/)
  assert.match(orderBookSource, /Array\.from\(\{ length: 7 \}/)
  assert.match(orderBookSource, /role="columnheader" aria-colspan="2"/)
  assert.match(orderBookSource, /<template v-if="hasRows">[\s\S]*order-book__matrix-row/)
  assert.match(orderBookSource, /order-book__matrix-row[\s\S]*row\.bid[\s\S]*row\.ask/)
  assert.match(orderBookSource, /\.order-book--matrix\s*\{[\s\S]*height: 272px/)
  assert.match(orderBookSource, /grid-template-columns: minmax\(0, \.8fr\) minmax\(0, 1\.05fr\) minmax\(0, 1\.05fr\) minmax\(0, \.8fr\)/)
  assert.match(orderBookSource, /@media \(max-width: 340px\)/)
  assert.match(marketTemplate, /<OrderBookPanel[\s\S]*layout="paired"/)

  const tradeOrderBooks = [...tradeSource.matchAll(/<OrderBookPanel[\s\S]*?\/>/g)].map(([markup]) => markup)
  assert.ok(tradeOrderBooks.length > 0)
  assert.ok(tradeOrderBooks.some((markup) => /layout="split"/.test(markup)))
})

test('页面不暴露伪造控件，并具备 320px、横屏、触控和减弱动效合同', () => {
  assert.doesNotMatch(marketTemplate, /marketDetail\.(?:grid|alert|updates)/)
  assert.doesNotMatch(marketTemplate, /markPrice|ranking|leverage|strategy/i)
  assert.doesNotMatch(marketDetailSource, /ticker\.value\.volume\s*\*|volume\s*\*\s*latestPrice/)
  assert.match(marketStyle, /overflow-x: clip/)
  assert.match(marketStyle, /min-height: 44px/)
  assert.match(marketStyle, /@media \(max-width: 340px\)/)
  assert.match(marketStyle, /@media \(orientation: landscape\) and \(max-height: 600px\)/)
  assert.match(marketStyle, /@media \(prefers-reduced-motion: reduce\)/)
  assert.match(marketStyle, /padding: 0 0 calc\(67px \+ env\(safe-area-inset-bottom\)\)/)
  assert.match(marketStyle, /\.market-detail__header\s*\{[\s\S]*height: calc\(64px \+ env\(safe-area-inset-top\)\)/)
  assert.match(marketStyle, /\.market-detail__rail\s*\{[\s\S]*height: 42px/)
  assert.match(marketStyle, /\.market-detail__summary\s*\{[\s\S]*height: 112px/)
  assert.match(marketStyle, /\.market-detail__intervals\s*\{[\s\S]*height: 48px/)
  assert.match(marketStyle, /\.market-detail__chart\s*\{[\s\S]*height: 204px/)
  assert.match(marketStyle, /\.market-detail__chart-panel\.is-expanded\s*\{[\s\S]*grid-template-rows: calc\(48px \+ env\(safe-area-inset-top\)\) minmax\(0, 1fr\)/)
  assert.match(marketStyle, /\.market-detail__chart-panel\.is-expanded \.market-detail__chart\s*\{[\s\S]*height: auto;[\s\S]*min-height: 0;/)
  assert.match(marketStyle, /\.market-detail__indicator-legend\s*\{[\s\S]*height: 28px/)
  assert.match(marketStyle, /\.market-detail__data-tabs\s*\{[\s\S]*height: 48px/)
  assert.match(marketStyle, /\.market-detail__data-panel\s*\{[\s\S]*height: 272px/)
  assert.match(marketStyle, /\.market-detail__actions\s*\{[\s\S]*grid-template-columns: 40px 40px minmax\(0, 1fr\)[\s\S]*height: calc\(67px \+ env\(safe-area-inset-bottom\)\)/)
  assert.match(marketStyle, /\.market-detail__actions \.is-primary\s*\{[\s\S]*background: var\(--detail-action\)[\s\S]*height: 52px/)
  assert.match(marketStyle, /\.is-ma5\s*\{[\s\S]*color: var\(--detail-positive\)/)
  assert.match(marketStyle, /\.is-ma10\s*\{[\s\S]*color: var\(--detail-negative\)/)
  assert.match(marketStyle, /\.is-ma20\s*\{[\s\S]*color: var\(--market-detail-ma20\)/)

  for (const key of [
    'orderBook.buySide',
    'orderBook.sellSide',
    'marketDetail.chart',
    'marketDetail.chartWorkstation',
    'marketDetail.chartEngine',
    'marketDetail.klineChartEngine',
    'marketDetail.tradingViewEngine',
    'marketDetail.snapshotData',
    'marketDetail.expandChart',
    'marketDetail.collapseChart',
    'marketDetail.marketData',
    'marketDetail.spotTrade',
    'marketDetail.orders',
  ]) {
    assert.notEqual(resolveMessage(zhCN, key), undefined, `zh-CN missing ${key}`)
    assert.notEqual(resolveMessage(en, key), undefined, `en missing ${key}`)
  }
})

test('浅色主题规则经 Vue scoped CSS 编译后仍保留局部后代选择器', () => {
  const compiled = compileStyle({
    source: marketScopedCss,
    filename: 'MarketDetailView.vue',
    id: 'data-v-market-detail',
    scoped: true,
  })

  assert.deepEqual(compiled.errors, [])
  assert.match(compiled.code, /\.market-detail\[data-v-market-detail\]\s*\{/)
  assert.match(
    compiled.code,
    /\.market-detail__actions \.is-primary\[data-v-market-detail\]\s*\{/,
  )
})

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}
