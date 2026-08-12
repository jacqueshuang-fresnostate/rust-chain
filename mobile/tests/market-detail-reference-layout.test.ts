import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
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
  classifyMarketChartDataUpdate,
  resolveMarketChartLogicalRange,
} from '../src/core/marketChartRuntime.ts'

const marketDetailSource = source('../src/views/MarketDetailView.vue')
const chartSource = source('../src/components/MobileMarketChart.vue')
const lightweightChartSource = source('../src/components/LightweightMarketChart.vue')
const orderBookSource = source('../src/components/OrderBookPanel.vue')
const tradeSource = source('../src/views/TradeView.vue')
const prototypeParityCss = source('../src/styles/prototype-parity.css')
const packageJson = JSON.parse(source('../package.json')) as {
  dependencies: Record<string, string>
}
const packageLock = JSON.parse(source('../package-lock.json')) as {
  packages: Record<string, { version?: string }>
}
const marketTemplate = marketDetailSource.match(/<template>[\s\S]*?<\/template>/)?.[0] ?? ''
const marketStyle = marketDetailSource.match(/<style scoped>[\s\S]*?<\/style>/)?.[0] ?? ''
const marketScopedCss = marketDetailSource.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1] ?? ''

type CssBlock = { header: string, body: string }

function cssBlocks(css: string): CssBlock[] {
  const blocks: CssBlock[] = []
  let cursor = 0

  while (cursor < css.length) {
    const openingBrace = css.indexOf('{', cursor)
    if (openingBrace < 0) break

    let depth = 1
    let closingBrace = -1
    for (let index = openingBrace + 1; index < css.length; index += 1) {
      if (css[index] === '{') depth += 1
      if (css[index] !== '}') continue
      depth -= 1
      if (depth !== 0) continue
      closingBrace = index
      break
    }

    assert.notEqual(closingBrace, -1, 'CSS block is not closed')
    const header = css
      .slice(cursor, openingBrace)
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .trim()
    if (header) {
      blocks.push({
        header,
        body: css.slice(openingBrace + 1, closingBrace),
      })
    }
    cursor = closingBrace + 1
  }

  return blocks
}

function cssBlockBody(css: string, header: string): string {
  const block = cssBlocks(css).find((candidate) => candidate.header === header)
  assert.ok(block, `CSS block not found: ${header}`)
  return block.body
}

function cssRuleBody(css: string, selector: string): string {
  const block = cssBlocks(css).find((candidate) => (
    !candidate.header.startsWith('@')
    && candidate.header.split(',').some((part) => part.trim() === selector)
  ))
  assert.ok(block, `CSS rule not found: ${selector}`)
  return block.body
}

function cssDeclarationValue(body: string, property: string): string {
  const escapedProperty = property.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = new RegExp(`(?:^|\\n)\\s*${escapedProperty}\\s*:\\s*([^;]+);`).exec(body)
  assert.ok(match, `CSS declaration not found: ${property}`)
  return match[1].trim()
}

function cssSpecificity(selector: string): [number, number, number] {
  const idCount = selector.match(/#[\w-]+/g)?.length ?? 0
  const classLikeCount = selector.match(/\.[\w-]+|\[[^\]]+\]|:(?!:)[\w-]+/g)?.length ?? 0
  const elementCount = selector
    .replace(/#[\w-]+|\.[\w-]+|\[[^\]]+\]|::?[\w-]+/g, ' ')
    .split(/[\s>+~]+/)
    .filter((part) => /^[a-z][\w-]*$/i.test(part))
    .length
  return [idCount, classLikeCount, elementCount]
}

function compareSpecificity(
  left: [number, number, number],
  right: [number, number, number],
): number {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index]
  }
  return 0
}

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
  assert.match(marketDetailSource, /const latestPrice = computed\(\(\) => ticker\.value\?\.lastPrice \?\? 0\)/)
  assert.doesNotMatch(marketDetailSource, /const latestPrice = computed\(\(\) => \([\s\S]*trades\.value\[0\]\?\.price/)
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

test('本地 Lightweight Charts 单引擎保留真实数据、指标、成交量与稳定视口', () => {
  assert.equal(packageJson.dependencies.klinecharts, undefined)
  assert.equal(packageJson.dependencies['lightweight-charts'], '5.2.0')
  assert.equal(packageLock.packages['node_modules/klinecharts'], undefined)
  assert.equal(packageLock.packages['node_modules/lightweight-charts']?.version, '5.2.0')
  assert.equal(existsSync(new URL('../src/components/KLineChartMarketChart.vue', import.meta.url)), false)
  assert.equal(existsSync(new URL('../src/components/TradingViewMarketChart.vue', import.meta.url)), false)
  assert.equal(existsSync(new URL('../src/core/marketChartEngine.ts', import.meta.url)), false)

  assert.match(chartSource, /import LightweightMarketChart from '@\/components\/LightweightMarketChart\.vue'/)
  assert.equal(chartSource.match(/<LightweightMarketChart/g)?.length, 1)
  assert.match(chartSource, /:points="normalizedPoints"/)
  assert.match(chartSource, /:moving-averages="movingAverages"/)
  assert.match(chartSource, /:symbol="symbol"/)
  assert.match(chartSource, /const chartLocale = computed\(\(\) => locale\.value === 'en' \? 'en-US' : 'zh-CN'\)/)
  assert.equal(chartSource.match(/:locale="chartLocale"/g)?.length, 1)
  assert.match(chartSource, /data-fit-policy="initial-or-dataset"/)
  assert.match(chartSource, /data-chart-engine="lightweight-charts"/)
  assert.doesNotMatch(chartSource, /engine-switch|radiogroup|Settings2|localStorage/)
  assert.match(marketTemplate, /<MobileMarketChart[\s\S]*:interval="interval"[\s\S]*\/>/)
  assert.doesNotMatch(marketTemplate, /show-engine-switch|compact-engine-switch/)

  assert.equal(lightweightChartSource.match(/chart\.addSeries\(LineSeries/g)?.length, 3)
  assert.match(lightweightChartSource, /chart\.addSeries\(HistogramSeries/)
  assert.match(lightweightChartSource, /value: point\.volume/)
  assert.equal(lightweightChartSource.match(/attributionLogo: true/g)?.length, 1)
  assert.doesNotMatch(lightweightChartSource, /attributionLogo: false/)
  assert.match(lightweightChartSource, /data-kline-provider="lightweight-charts"/)
  assert.match(lightweightChartSource, /data-chart-package="lightweight-charts@5\.2\.0"/)
  assert.match(lightweightChartSource, /candles\?\.update\(candleRow\(point\)\)/)
  assert.match(lightweightChartSource, /volume\?\.update\(volumeRow\(point, theme\)\)/)
  assert.equal(lightweightChartSource.match(/timeScale\(\)\.fitContent\(\)/g)?.length, 1)
  assert.match(lightweightChartSource, /datasetKey\(props\.symbol, props\.interval\)/)
  assert.match(lightweightChartSource, /const fitKeyChanged = next\.key !== previous\.key/)
  assert.match(lightweightChartSource, /if \(fitKeyChanged\) \{[\s\S]*scheduleViewportRestore\(null\)[\s\S]*fitNextDataset = true[\s\S]*\}/)
  assert.match(lightweightChartSource, /if \(fitKeyChanged && !pointsChanged\) return/)
  assert.match(lightweightChartSource, /renderAllData\(false, viewport\)/)
  assert.match(lightweightChartSource, /captureMarketChartLogicalViewport\([\s\S]*resolveMarketChartLogicalRange\(/)
  assert.match(lightweightChartSource, /requestAnimationFrame\(\(\) => \{[\s\S]*restoreViewport\(viewport\)/)
  assert.match(lightweightChartSource, /watch\(\(\) => props\.locale[\s\S]*localization: \{ locale \}/)
  assert.match(lightweightChartSource, /horzTouchDrag: true[\s\S]*vertTouchDrag: false/)
  assert.match(lightweightChartSource, /pinch: true/)
  assert.match(lightweightChartSource, /kineticScroll: \{ mouse: false, touch: true \}/)
  assert.match(lightweightChartSource, /if \(width <= 0 \|\| height <= 0\) return/)
  assert.match(lightweightChartSource, /resizeObserver\?\.disconnect\(\)/)
  assert.match(lightweightChartSource, /chart\?\.remove\(\)/)
  assert.match(lightweightChartSource, /role="region"/)
  assert.doesNotMatch(lightweightChartSource, /role="img"/)
  assert.match(lightweightChartSource, /\.market-chart-engine\s*\{[\s\S]*height: 100%;[\s\S]*min-height: 0;/)
  assert.match(tradeSource, /<MobileMarketChart :points="points" :loading="chartLoading" :interval="interval" :symbol="pairSymbol" \/>/)

  const chartRuntimeSources = [chartSource, lightweightChartSource]
  for (const runtimeSource of chartRuntimeSources) {
    assert.doesNotMatch(runtimeSource, /<iframe|<script[^>]+src=|https?:\/\/|cdn\.|TradingView\.widget|KLineChartPro|datafeed/i)
    assert.doesNotMatch(runtimeSource, /marketDetailStream|fetchKlines|WebSocket/)
  }
})

test('行情详情继续展示真实均线与成交量并传递交易对数据集键', () => {
  assert.match(marketTemplate, /market-detail__indicator-legend[\s\S]*MA5[\s\S]*MA10[\s\S]*MA20[\s\S]*candleVolume/)
  assert.match(marketTemplate, /<MobileMarketChart[\s\S]*:points="points"[\s\S]*:interval="interval"[\s\S]*\/>/)
  assert.match(marketTemplate, /:symbol="pairSymbol"/)
  assert.doesNotMatch(marketTemplate, /show-engine-switch|compact-engine-switch/)
})

test('形成中蜡烛分类和 timestamp 锚定视口可独立验证', () => {
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

  const logicalRows = Array.from({ length: 100 }, (_, index) => ({ time: 1_000 + index * 60 }))
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

test('图表切换按钮使用双主题毛玻璃正方形并保持左上角定位', () => {
  const toggleSelector = '.market-detail .market-detail__chart > button.market-detail__chart-toggle'
  const expandedToggleSelector = '.market-detail__chart-panel.is-expanded .market-detail__chart > button.market-detail__chart-toggle'
  const legacyDarkSelector = '.app-stage.theme-dark .mobile-canvas .market-detail__chart-toggle'
  const inlineRule = cssRuleBody(marketScopedCss, toggleSelector)
  const activeRule = cssRuleBody(marketScopedCss, `${toggleSelector}:active`)
  const focusRule = cssRuleBody(marketScopedCss, `${toggleSelector}:focus-visible`)
  const expandedRule = cssRuleBody(marketScopedCss, expandedToggleSelector)
  const reducedMotionRule = cssBlockBody(
    marketScopedCss,
    '@media (prefers-reduced-motion: reduce)',
  )
  const reducedToggleRule = cssRuleBody(reducedMotionRule, toggleSelector)
  const reducedActiveRule = cssRuleBody(
    reducedMotionRule,
    `${toggleSelector}:active`,
  )
  const standardBackdrop = cssDeclarationValue(inlineRule, 'backdrop-filter')
  const webkitBackdrop = cssDeclarationValue(inlineRule, '-webkit-backdrop-filter')
  const blurStrength = standardBackdrop.match(/\bblur\(([\d.]+)px\)/)?.[1]
  const saturationStrength = standardBackdrop.match(/\bsaturate\(([\d.]+)%\)/)?.[1]

  assert.match(inlineRule, /\bwidth\s*:\s*44px\s*;/)
  assert.match(inlineRule, /\bheight\s*:\s*44px\s*;/)
  assert.match(inlineRule, /\bborder-radius\s*:\s*12px\s*;/)
  assert.match(inlineRule, /\bdisplay\s*:\s*flex\s*;/)
  assert.match(inlineRule, /\balign-items\s*:\s*center\s*;/)
  assert.match(inlineRule, /\bjustify-content\s*:\s*center\s*;/)
  assert.match(
    inlineRule,
    /\bbackground\s*:\s*linear-gradient\([^;]*var\(--detail-surface\)[^;]*transparent[^;]*var\(--detail-background\)[^;]*transparent[^;]*\)\s*;/,
  )
  assert.equal(webkitBackdrop, standardBackdrop)
  assert.ok(blurStrength && Number(blurStrength) > 0)
  assert.ok(saturationStrength && Number(saturationStrength) > 100)
  assert.match(inlineRule, /\bborder\s*:\s*1px solid color-mix\([^;]*var\(--detail-line\)[^;]*\)\s*;/)
  assert.match(cssDeclarationValue(inlineRule, 'box-shadow'), /\binset\s+0\s+1px\s+0\b/)
  assert.match(
    cssDeclarationValue(inlineRule, 'box-shadow'),
    /,\s*0\s+[\d.]+px\s+[\d.]+px\b/,
  )
  assert.match(cssDeclarationValue(inlineRule, 'transition'), /\btransform\b/)
  assert.match(inlineRule, /\bleft\s*:\s*16px\s*;/)
  assert.match(inlineRule, /\btop\s*:\s*12px\s*;/)
  assert.doesNotMatch(inlineRule, /\bright\s*:/)

  assert.match(activeRule, /\btransform\s*:\s*translateY\(1px\)\s*;/)
  assert.match(cssDeclarationValue(activeRule, 'box-shadow'), /\binset\s+0\s+1px\s+0\b/)
  assert.match(cssDeclarationValue(activeRule, 'box-shadow'), /,\s*0\s+[\d.]+px\s+[\d.]+px\b/)
  assert.match(focusRule, /\boutline\s*:\s*2px solid var\(--focus\)\s*;/)
  assert.match(focusRule, /\boutline-offset\s*:\s*3px\s*;/)
  assert.match(cssDeclarationValue(focusRule, 'box-shadow'), /\binset\s+0\s+1px\s+0\b/)
  assert.match(cssDeclarationValue(focusRule, 'box-shadow'), /,\s*0\s+[\d.]+px\s+[\d.]+px\b/)
  assert.match(reducedToggleRule, /\btransition\s*:\s*none\s*;/)
  assert.match(reducedActiveRule, /\btransform\s*:\s*none\s*;/)

  assert.match(expandedRule, /\bleft\s*:\s*10px\s*;/)
  assert.match(expandedRule, /\btop\s*:\s*8px\s*;/)
  assert.doesNotMatch(expandedRule, /\bright\s*:/)

  assert.doesNotMatch(
    [inlineRule, activeRule, focusRule].join('\n'),
    /#0b1811|rgba\(\s*11\s*,\s*24\s*,\s*17\s*,/i,
  )

  const legacyDarkRule = cssRuleBody(prototypeParityCss, legacyDarkSelector)
  assert.match(legacyDarkRule, /\bbox-shadow\s*:/)

  const compiled = compileStyle({
    source: marketScopedCss,
    filename: 'MarketDetailView.vue',
    id: 'data-v-market-detail',
    scoped: true,
  })
  assert.deepEqual(compiled.errors, [])
  const compiledToggleRules = cssBlocks(compiled.code).filter((block) => (
    block.header.includes('.market-detail__chart-toggle[data-v-market-detail]')
    && (block.body.includes('backdrop-filter')
      || block.header.endsWith(':active')
      || block.header.endsWith(':focus-visible'))
  ))
  assert.equal(compiledToggleRules.length, 3)
  const compiledBaseRule = compiledToggleRules.find((block) => block.body.includes('backdrop-filter'))
  const compiledExpandedRule = cssBlocks(compiled.code).find((block) => (
    block.header.includes('.market-detail__chart-toggle[data-v-market-detail]')
    && block.body.includes('left: 10px')
    && block.body.includes('top: 8px')
  ))
  assert.ok(compiledBaseRule)
  assert.ok(compiledExpandedRule)
  assert.ok(
    compareSpecificity(
      cssSpecificity(compiledExpandedRule.header),
      cssSpecificity(compiledBaseRule.header),
    ) > 0,
    `${compiledExpandedRule.header} must outrank ${compiledBaseRule.header}`,
  )
  for (const block of compiledToggleRules) {
    assert.ok(
      compareSpecificity(cssSpecificity(block.header), cssSpecificity(legacyDarkSelector)) > 0,
      `${block.header} must outrank the legacy dark-theme shadow rule`,
    )
  }
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
