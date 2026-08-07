import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const marketDetailSource = readFileSync(new URL('../src/views/MarketDetailView.vue', import.meta.url), 'utf8')
const newsSource = readFileSync(new URL('../src/views/NewsView.vue', import.meta.url), 'utf8')
const newsDetailSource = readFileSync(new URL('../src/views/NewsDetailView.vue', import.meta.url), 'utf8')
const assetMarkSource = readFileSync(new URL('../src/components/AssetMark.vue', import.meta.url), 'utf8')
const loginRequiredSource = readFileSync(new URL('../src/components/LoginRequiredState.vue', import.meta.url), 'utf8')
const chartSource = readFileSync(new URL('../src/components/MobileMarketChart.vue', import.meta.url), 'utf8')
const klineChartSource = readFileSync(new URL('../src/components/KLineChartMarketChart.vue', import.meta.url), 'utf8')
const tradingViewChartSource = readFileSync(new URL('../src/components/TradingViewMarketChart.vue', import.meta.url), 'utf8')
const chartThemeSource = readFileSync(new URL('../src/core/marketChartTheme.ts', import.meta.url), 'utf8')
const orderBookSource = readFileSync(new URL('../src/components/OrderBookPanel.vue', import.meta.url), 'utf8')
const selectedPageCss = readFileSync(new URL('../src/styles/pencil-selected-pages.css', import.meta.url), 'utf8')

const ownedSources = [
  marketDetailSource,
  newsSource,
  newsDetailSource,
  assetMarkSource,
  loginRequiredSource,
  chartSource,
  klineChartSource,
  tradingViewChartSource,
  orderBookSource,
]

test('行情详情保留真实 ticker、K 线、盘口、成交和交易导航合同', () => {
  assert.match(marketDetailSource, /marketStore\.tickerFor\(pairSymbol\.value\)/)
  assert.match(marketDetailSource, /marketStore\.refresh\(forceMarket\)/)
  assert.match(marketDetailSource, /fetchKlines\(pairSymbol\.value, interval\.value\)/)
  assert.match(marketDetailSource, /fetchOrderBook\(pairSymbol\.value\)/)
  assert.match(marketDetailSource, /fetchRecentTrades\(pairSymbol\.value\)/)
  assert.match(marketDetailSource, /Promise\.allSettled/)
  assert.match(marketDetailSource, /<MobileMarketChart[\s\S]*:points="points"[\s\S]*:interval="interval"[\s\S]*show-engine-switch/)
  assert.match(marketDetailSource, /<OrderBookPanel[\s\S]*:bids="bids"[\s\S]*:asks="asks"[\s\S]*:current-price="latestPrice"/)
  assert.match(marketDetailSource, /query: mode === 'contract' \? \{ mode: 'contract' \} : undefined/)
  assert.match(marketDetailSource, /openTrade\('spot'\)/)
  assert.match(marketDetailSource, /openTrade\('contract'\)/)
  assert.match(marketDetailSource, /goBackOr\(router, \{ name: 'markets' \}\)/)
})

test('公告列表和详情保留真实 API、语言映射与命名路由合同', () => {
  assert.match(newsSource, /rows\.value = await fetchNews\(50\)/)
  assert.match(newsSource, /router\.push\(\{ name: 'news-detail', params: \{ id: notice\.id \} \}\)/)
  assert.match(newsSource, /apiErrorMessage\(reason, t\('news\.loadFailed'\)\)/)
  assert.match(newsDetailSource, /fetchNewsDetail\(id\)/)
  assert.match(newsDetailSource, /fetchNews\(6\)\.catch\(\(\) => \[\]\)/)
  assert.match(newsDetailSource, /watch\(\(\) => props\.id,[\s\S]*\{ immediate: true \}\)/)
  assert.match(newsDetailSource, /<NewsRichText :blocks="detail\.content" :empty-text="t\('news\.emptyContent'\)" \/>/)
  assert.doesNotMatch(newsDetailSource, /v-html/)
  assert.match(newsDetailSource, /apiErrorMessage\(reason, t\('news\.detailLoadFailed'\)\)/)
})

test('共享行情与登录支撑组件使用真实数据、主题变量和安全回退', () => {
  assert.match(assetMarkSource, /fallbackSrc\?: string/)
  assert.match(assetMarkSource, /\[props\.src, props\.fallbackSrc\]/)
  assert.match(assetMarkSource, /@error="imageIndex \+= 1"/)
  assert.match(assetMarkSource, /watch\(\[\(\) => props\.src, \(\) => props\.fallbackSrc\]/)
  assert.match(assetMarkSource, /var\(--positive\)/)
  assert.match(loginRequiredSource, /query: \{ redirect: route\.fullPath \}/)
  assert.match(loginRequiredSource, /t\('common\.loginRequiredTitle'\)/)

  assert.match(chartSource, /normalizeMarketChartPoints\(props\.points\)/)
  assert.match(chartSource, /v-if="engine === 'klinecharts'"/)
  assert.match(chartSource, /<TradingViewMarketChart[\s\S]*v-else/)
  assert.match(klineChartSource, /init\(container\.value/)
  assert.match(klineChartSource, /createIndicator\(movingAverageIndicator\(theme\), true\)/)
  assert.match(klineChartSource, /createIndicator\(volumeIndicator\(theme\)\)/)
  assert.match(tradingViewChartSource, /createChart\(container\.value/)
  assert.match(tradingViewChartSource, /chart\.addSeries\(CandlestickSeries/)
  assert.match(tradingViewChartSource, /chart\.addSeries\(HistogramSeries/)
  assert.match(chartThemeSource, /getPropertyValue\('--surface'\)/)
  assert.match(chartThemeSource, /getPropertyValue\('--positive'\)/)
  for (const engineSource of [klineChartSource, tradingViewChartSource]) {
    assert.match(engineSource, /observeMarketChartTheme\([\s\S]*container\.value,[\s\S]*document\.documentElement,[\s\S]*applyTheme/)
    assert.match(engineSource, /stopObservingTheme\?\.\(\)/)
  }
  assert.match(klineChartSource, /data-kline-provider="klinecharts"/)
  assert.match(tradingViewChartSource, /data-kline-provider="tradingview"/)

  assert.match(orderBookSource, /asks\.slice\(0, 6\)\.reverse\(\)/)
  assert.match(orderBookSource, /bids\.slice\(0, 6\)/)
  assert.match(orderBookSource, /width\(item\.quantity\)/)
  assert.match(orderBookSource, /loading \? t\('common\.loading'\) : t\('common\.marketUnavailable'\)/)
})

test('行情、公告与共享支撑切片满足主题、触控、窄屏和 Lucide 契约', () => {
  for (const source of ownedSources) {
    assert.doesNotMatch(source, /<svg/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
    assert.doesNotMatch(source, /[\u3400-\u9fff]/)
    assert.doesNotMatch(source, /#[0-9a-f]{3,8}/i)
    assert.doesNotMatch(source, /background:\s*(?:white|rgb\()/i)
  }

  for (const source of [marketDetailSource]) {
    assert.match(source, /@media \(max-width: 340px\)/)
    assert.match(source, /env\(safe-area-inset-/)
    assert.match(source, /min-height: (?:4[4-9]|[5-9]\d)px/)
  }
  for (const source of [newsSource, newsDetailSource]) {
    assert.match(source, /@media \(max-width: 340px\)/)
    assert.match(source, /data-pencil-source=/)
  }
  assert.match(selectedPageCss, /env\(safe-area-inset-bottom\)/)
  assert.match(selectedPageCss, /min-height:\s*(?:44|4[5-9]|[5-9]\d)px/)

  assert.match(marketDetailSource, /position: sticky/)
  assert.match(marketDetailSource, /z-index: var\(--layer-sticky-header\)/)
  assert.match(loginRequiredSource, /min-height: 46px/)
})

test('切片使用的固定文案键全部存在于中英文资源', () => {
  const keys = new Set<string>()
  for (const source of ownedSources) {
    for (const match of source.matchAll(/\bt\('([^']+)'/g)) keys.add(match[1])
  }

  for (const key of keys) {
    assert.notEqual(resolveMessage(zhCN, key), undefined, `zh-CN missing ${key}`)
    assert.notEqual(resolveMessage(en, key), undefined, `en missing ${key}`)
  }
})

function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}
