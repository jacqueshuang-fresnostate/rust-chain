import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import { normalizeMarketChartPoints } from '../src/core/marketChart.ts'
import { quantityForBalancePercentage } from '../src/core/tradeForm.ts'

const chartSource = source('../src/components/MobileMarketChart.vue')
const lightweightChartSource = source('../src/components/LightweightMarketChart.vue')
const chartUtilitySource = source('../src/core/marketChart.ts')
const bookSource = source('../src/components/OrderBookPanel.vue')
const tradeSource = source('../src/views/TradeView.vue')
const modalDialogSource = source('../src/core/modalDialog.ts')
const parityCss = source('../src/styles/prototype-parity.css')
const prototypeCss = source('../src/styles/prototype-base.css')
const secondsSource = source('../src/views/SecondsView.vue')
const marketDetailSource = source('../src/views/MarketDetailView.vue')
const ordersSource = source('../src/views/OrdersView.vue')
const productionSources = [
  chartSource,
  lightweightChartSource,
  bookSource,
  tradeSource,
  secondsSource,
  marketDetailSource,
  ordersSource,
]

test('现货和合约工作台保留真实数据链路并提供完整下单面', () => {
  assert.match(tradeSource, /fetchKlines\(symbol, selectedInterval\)/)
  assert.match(tradeSource, /fetchOrderBook\(symbol\)/)
  assert.match(tradeSource, /fetchRecentTrades\(symbol\)/)
  assert.match(tradeSource, /createMarketDetailStreamSession\(\{/)
  assert.match(tradeSource, /detailStreamSession\.stop\(\)/)
  assert.match(tradeSource, /<MobileMarketChart :points="points" :loading="chartLoading" :interval="interval" :symbol="pairSymbol" \/>/)
  assert.match(tradeSource, /class="chart-panel trade-chart-panel"/)
  assert.match(tradeSource, /v-model="price"/)
  assert.match(tradeSource, /v-model="quantity"/)
  assert.match(tradeSource, /const amountValue = computed\(\{/)
  assert.match(tradeSource, /v-if="isSpotMode"[\s\S]*?class="confirmation-sheet"[\s\S]*?formatAmount\(Number\(amountValue\) \|\| 0\)/)
  assert.match(tradeSource, /class="contract-order-confirm"[\s\S]*?formatAmount\(contractNotionalValue\)[\s\S]*?formatAmount\(contractOrderQuantity\)/)
  assert.match(tradeSource, /fetchWalletAccounts\(\)/)
  assert.match(tradeSource, /fetchMarginWallets\(\)/)
  assert.match(tradeSource, /quantityForBalancePercentage\(\{/)
  assert.doesNotMatch(tradeSource, /const quoteBudget = 100 \* percent/)
  assert.doesNotMatch(tradeSource, /\|\| products\.value\[0\]/)
  assert.match(tradeSource, /class="percent-row"/)
  assert.match(tradeSource, /role="dialog"/)
  assert.match(tradeSource, /aria-modal="true"/)
  assert.match(tradeSource, /useModalDialog\(confirmOpen, confirmDialog, '\[data-dialog-cancel\]'\)/)
  assert.match(modalDialogSource, /document\.body\.style\.overflow = 'hidden'/)
  assert.match(tradeSource, /data-dialog-cancel/)
  assert.match(tradeSource, /watch\(\(\) => route\.query\.mode/)
  assert.doesNotMatch(tradeSource, /class="trade-category"/)
  assert.doesNotMatch(tradeSource, /selectTradeMode/)
  assert.match(tradeSource, /:data-trade-mode="mode"/)
})

test('秒合约保持独立真实产品工作台和市场参考价', () => {
  assert.match(secondsSource, /const nextProducts = await fetchSecondsProducts\(\)/)
  assert.match(secondsSource, /session\.isAuthenticated\s*\? Promise\.allSettled\(\[fetchSecondsOrders\(100\), fetchWalletAccounts\(\)\]\)\s*:\s*Promise\.resolve\(null\)/)
  assert.match(secondsSource, /marketStore\.tickerFor\(selected\.value\?\.symbol \|\| ''\)/)
  assert.match(secondsSource, /selectedLatestPrice > 0 \? formatPrice\(selectedLatestPrice\) : '--'/)
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
  assert.match(lightweightChartSource, /createChart\(container\.value/)
  assert.match(lightweightChartSource, /data-kline-provider="lightweight-charts"/)
  assert.equal(chartSource.match(/<LightweightMarketChart/g)?.length, 1)
  assert.doesNotMatch(chartSource, /KLineChart|TradingViewMarketChart|engine-switch/)
  assert.match(chartSource, /data-chart-state=/)
  assert.match(chartSource, /normalizeMarketChartPoints\(props\.points\)/)
  assert.match(lightweightChartSource, /if \(width <= 0 \|\| height <= 0\) return/)
  assert.match(lightweightChartSource, /datasetKey\(props\.symbol, props\.interval\)/)
  assert.match(lightweightChartSource, /role="region"/)
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

test('订单中心展示 Pencil 双层分类、访客状态和紧凑真实数据面', () => {
  const tabsAt = ordersSource.indexOf('<nav class="pencil-segmented orders-market-tabs"')
  const authAt = ordersSource.indexOf('v-if="!session.isAuthenticated"')
  assert.ok(tabsAt > 0 && authAt > tabsAt, 'tabs should remain visible before the guest state')
  assert.match(ordersSource, /data-pencil-source="kcP5D A85if n6oGO t2GTW4 e5Qs1 hxe8l"/)
  assert.match(ordersSource, /<PageHeader :back="false" :pencil="true"/)
  assert.match(ordersSource, /class="pencil-segmented orders-state-tabs"/)
  assert.match(ordersSource, /class="orders-row"/)
  assert.match(ordersSource, /class="orders-row orders-row--history"/)
  assert.match(ordersSource, /await cancelAllSpotOrders\(spotOrders\.value\.map\(\(order\) => order\.id\)\)/)
  assert.match(ordersSource, /await cancelAllMarginPositions\(\)/)
  assert.match(ordersSource, /await closeAllMarginPositions\(\)/)
  assert.match(ordersSource, /route\.query\.tab === 'positions'/)
  assert.match(ordersSource, /route\.query\.tab === 'history'/)
})

test('交易切片遵守 i18n、Lucide、焦点和窄屏视觉合同', () => {
  for (const viewSource of [secondsSource, marketDetailSource, ordersSource]) {
    assert.match(viewSource, /min-width: 0/)
    assert.match(viewSource, /@media \(max-width: 340px\)/)
    assert.doesNotMatch(viewSource, /<svg/)
    assert.doesNotMatch(viewSource, /\p{Extended_Pictographic}/u)
    assert.doesNotMatch(viewSource, /#[0-9a-f]{3,8}/i)
    assert.doesNotMatch(viewSource, /rgba?\(/i)
    assert.doesNotMatch(viewSource, /[\u3400-\u9fff]/)
  }

  assert.doesNotMatch(tradeSource, /<style scoped|<svg|\p{Extended_Pictographic}/u)
  assert.doesNotMatch(tradeSource, /[\u3400-\u9fff]/)
  assert.doesNotMatch(tradeSource, /'PERPETUAL ORDER'|'SPOT ORDER'/)
  assert.match(parityCss, /@import '\.\/prototype-base\.css';/)
  assert.match(prototypeCss, /\.trade-console\s*\{[\s\S]*?min-width:\s*0/)
  assert.match(prototypeCss, /@media \(max-width: 350px\)/)
  assert.match(secondsSource, /:focus-within/)
  assert.match(secondsSource, /var\(--overlay\)/)
  assert.match(ordersSource, /env\(safe-area-inset-bottom\)/)

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
