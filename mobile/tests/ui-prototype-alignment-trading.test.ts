import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import { normalizeMarketChartPoints } from '../src/core/marketChart.ts'
import { quantityForBalancePercentage } from '../src/core/tradeForm.ts'

const chartSource = source('../src/components/MobileMarketChart.vue')
const chartUtilitySource = source('../src/core/marketChart.ts')
const bookSource = source('../src/components/OrderBookPanel.vue')
const tradeSource = source('../src/views/TradeView.vue')
const secondsSource = source('../src/views/SecondsView.vue')
const marketDetailSource = source('../src/views/MarketDetailView.vue')
const ordersSource = source('../src/views/OrdersView.vue')
const productionSources = [
  chartSource,
  bookSource,
  tradeSource,
  secondsSource,
  marketDetailSource,
  ordersSource,
]

test('现货和合约工作台保留真实数据链路并提供完整下单面', () => {
  assert.match(tradeSource, /fetchKlines\(pairSymbol\.value, interval\.value\)/)
  assert.match(tradeSource, /fetchOrderBook\(pairSymbol\.value\)/)
  assert.match(tradeSource, /<MobileMarketChart :points="points" :loading="chartLoading" \/>/)
  assert.match(tradeSource, /class="trade-chart-panel"/)
  assert.match(tradeSource, /v-model="price"/)
  assert.match(tradeSource, /v-model="quantity"/)
  assert.match(tradeSource, /v-model="amountValue"/)
  assert.match(tradeSource, /fetchWalletAccounts\(\)/)
  assert.match(tradeSource, /fetchMarginWallets\(\)/)
  assert.match(tradeSource, /quantityForBalancePercentage\(\{/)
  assert.doesNotMatch(tradeSource, /const quoteBudget = 100 \* percent/)
  assert.doesNotMatch(tradeSource, /\|\| products\.value\[0\]/)
  assert.match(tradeSource, /class="percent-row"/)
  assert.match(tradeSource, /role="dialog"/)
  assert.match(tradeSource, /aria-modal="true"/)
  assert.match(tradeSource, /document\.body\.style\.overflow = 'hidden'/)
  assert.match(tradeSource, /data-dialog-cancel/)
  assert.match(tradeSource, /watch\(\(\) => route\.query\.mode/)
  assert.doesNotMatch(tradeSource, /class="trade-category"/)
  assert.doesNotMatch(tradeSource, /selectTradeMode/)
  assert.match(tradeSource, /:data-trade-mode="mode"/)
})

test('秒合约保持独立真实产品工作台和市场参考价', () => {
  assert.match(secondsSource, /const productsRequest = fetchSecondsProducts\(\)/)
  assert.match(secondsSource, /session\.isAuthenticated\s*\?\s*await Promise\.all\(\[productsRequest, fetchSecondsOrders\(\), fetchWalletAccounts\(\)\]\)\s*:\s*\[await productsRequest, \[\], \[\]\]/)
  assert.match(secondsSource, /marketStore\.tickerFor\(selected\.value\?\.symbol \|\| ''\)/)
  assert.match(secondsSource, /selectedTicker \? formatPrice\(selectedTicker\.lastPrice\) : '--'/)
  assert.match(secondsSource, /class="seconds-market-board"/)
  assert.match(secondsSource, /class="seconds-direction-grid"/)
  assert.match(secondsSource, /class="seconds-duration-grid"/)
  assert.match(secondsSource, /class="seconds-amount-presets"/)
  assert.match(secondsSource, /role="dialog"/)
  assert.match(secondsSource, /await openSecondsOrder\(\{/)
})

test('行情详情使用真实 K 线、深度、成交和双交易入口', () => {
  assert.match(marketDetailSource, /fetchKlines\(pairSymbol\.value, interval\.value\)/)
  assert.match(marketDetailSource, /fetchOrderBook\(pairSymbol\.value\)/)
  assert.match(marketDetailSource, /fetchRecentTrades\(pairSymbol\.value\)/)
  assert.match(marketDetailSource, /data-market-workspace="live"/)
  assert.match(marketDetailSource, /<MobileMarketChart/)
  assert.match(marketDetailSource, /<OrderBookPanel/)
  assert.match(marketDetailSource, /openTrade\('spot'\)/)
  assert.match(marketDetailSource, /openTrade\('contract'\)/)
})

test('图表和盘口在空数据与重复时间点下仍保留稳定画布', () => {
  assert.match(chartSource, /createChart\(container\.value/)
  assert.match(chartSource, /data-kline-provider="tradingview"/)
  assert.match(chartSource, /data-chart-state=/)
  assert.match(chartSource, /normalizeMarketChartPoints\(props\.points\)/)
  assert.match(chartSource, /if \(width <= 0 \|\| height <= 0\) return/)
  assert.match(chartUtilitySource, /const unique = new Map<number, NormalizedMarketChartPoint>\(\)/)
  assert.match(chartUtilitySource, /sort\(\(left, right\) => left\.time - right\.time\)/)
  assert.match(chartSource, /min-width: 0/)
  assert.match(chartSource, /overflow: hidden/)
  assert.match(bookSource, /data-book-side="ask"/)
  assert.match(bookSource, /data-book-side="bid"/)
  assert.match(bookSource, /:aria-busy="loading"/)
  assert.match(bookSource, /@media \(max-width: 340px\)/)
})

test('图表时间归一化兼容秒与毫秒并在归一化后去重排序', () => {
  const points = normalizeMarketChartPoints([
    { time: 1_720_000_000_000, open: 2, high: 3, low: 1, close: 2.5, volume: 4 },
    { time: 1_710_000_000, open: 1, high: 2, low: .5, close: 1.5, volume: 3 },
    { time: 1_720_000_000, open: 3, high: 4, low: 2, close: 3.5, volume: 5 },
    { time: Number.NaN, open: 1, high: 2, low: .5, close: 1.5, volume: 3 },
  ])

  assert.deepEqual(points.map((point) => point.time), [1_710_000_000, 1_720_000_000])
  assert.equal(points[1]?.open, 3)
})

test('百分比数量使用真实可用余额并区分现货买卖与保证金金额', () => {
  assert.equal(quantityForBalancePercentage({
    available: 1_000,
    mode: 'spot',
    percentage: .25,
    price: 50,
    side: 'buy',
  }), 5)
  assert.equal(quantityForBalancePercentage({
    available: 8,
    mode: 'spot',
    percentage: .5,
    price: 50,
    side: 'sell',
  }), 4)
  assert.equal(quantityForBalancePercentage({
    available: 600,
    mode: 'contract',
    percentage: .5,
    price: 50,
    side: 'buy',
  }), 300)
  assert.equal(quantityForBalancePercentage({
    available: 600,
    mode: 'spot',
    percentage: .5,
    price: 0,
    side: 'buy',
  }), 0)
})

test('订单中心始终展示场景标题、等宽三栏和紧凑真实数据面', () => {
  const tabsAt = ordersSource.indexOf('<nav class="order-tabs"')
  const authAt = ordersSource.indexOf('v-if="!session.isAuthenticated"')
  assert.ok(tabsAt > 0 && authAt > tabsAt, 'tabs should remain visible before the guest state')
  assert.match(ordersSource, /:eyebrow="t\('orders\.category'\)"/)
  assert.match(ordersSource, /:subtitle="t\('orders\.loginDescription'\)"/)
  assert.match(ordersSource, /:back="true"/)
  assert.match(ordersSource, /grid-template-columns: repeat\(3, minmax\(0, 1fr\)\)/)
  assert.match(ordersSource, /class="order-card"/)
  assert.match(ordersSource, /class="history-row"/)
  assert.match(ordersSource, /await cancelAllSpotOrders\(spotOrders\.value\.map\(\(order\) => order\.id\)\)/)
  assert.match(ordersSource, /await cancelAllMarginPositions\(\)/)
  assert.match(ordersSource, /await closeAllMarginPositions\(\)/)
  assert.match(ordersSource, /route\.query\.tab === 'positions'/)
  assert.match(ordersSource, /route\.query\.tab === 'history'/)
})

test('交易切片遵守 i18n、Lucide、焦点和窄屏视觉合同', () => {
  for (const viewSource of [tradeSource, secondsSource, marketDetailSource, ordersSource]) {
    assert.match(viewSource, /min-width: 0/)
    assert.match(viewSource, /@media \(max-width: 340px\)/)
    assert.doesNotMatch(viewSource, /<svg/)
    assert.doesNotMatch(viewSource, /\p{Extended_Pictographic}/u)
    assert.doesNotMatch(viewSource, /#[0-9a-f]{3,8}/i)
    assert.doesNotMatch(viewSource, /rgba?\(/i)
    assert.doesNotMatch(viewSource, /[\u3400-\u9fff]/)
  }

  assert.match(tradeSource, /:focus-within/)
  assert.match(secondsSource, /:focus-within/)
  assert.match(tradeSource, /var\(--overlay\)/)
  assert.match(secondsSource, /var\(--overlay\)/)
  assert.match(ordersSource, /padding-bottom: calc\(28px \+ env\(safe-area-inset-bottom\)\)/)

  const keys = new Set<string>()
  for (const fileSource of productionSources) {
    for (const match of fileSource.matchAll(/\bt\('([^']+)'/g)) keys.add(match[1])
  }
  for (const key of keys) {
    assert.notEqual(resolveMessage(zhCN, key), undefined, `zh-CN missing ${key}`)
    assert.notEqual(resolveMessage(en, key), undefined, `en missing ${key}`)
  }
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
