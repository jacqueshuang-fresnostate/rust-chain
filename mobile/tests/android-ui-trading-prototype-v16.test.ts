import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { normalizeMarketChartPoints } from '../src/core/marketChart.ts'
import { quantityForBalancePercentage } from '../src/core/tradeForm.ts'

const tradeSource = source('../src/views/TradeView.vue')
const secondsSource = source('../src/views/SecondsView.vue')
const marketDetailSource = source('../src/views/MarketDetailView.vue')
const ordersSource = source('../src/views/OrdersView.vue')
const chartSource = source('../src/components/MobileMarketChart.vue')
const bookSource = source('../src/components/OrderBookPanel.vue')

test('v16 现货与合约保留独立路由、实时报价、盘口和真实下单链路', () => {
  assert.match(tradeSource, /:data-trade-mode="mode"/)
  assert.match(tradeSource, /data-market-quote="live"/)
  assert.match(tradeSource, /data-order-surface="live"/)
  assert.match(tradeSource, /t\('marketDetail\.high24h'\)/)
  assert.match(tradeSource, /t\('marketDetail\.low24h'\)/)
  assert.match(tradeSource, /<MobileMarketChart :points="points" :loading="chartLoading" \/>/)
  assert.match(tradeSource, /<OrderBookPanel/)
  assert.match(tradeSource, /fetchWalletAccounts\(\)/)
  assert.match(tradeSource, /fetchMarginWallets\(\)/)
  assert.match(tradeSource, /quantityForBalancePercentage\(\{/)
  assert.match(tradeSource, /await placeSpotOrder\(\{/)
  assert.match(tradeSource, /await placeMarginOrder\(\{/)
  assert.match(tradeSource, /watch\(\(\) => route\.query\.mode/)
  assert.doesNotMatch(tradeSource, /class="trade-category"/)
  assert.doesNotMatch(tradeSource, /selectTradeMode/)
})

test('余额百分比严格使用真实可用额并区分现货买卖与合约保证金', () => {
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
    percentage: 2,
    price: 0,
    side: 'buy',
  }), 600)
  assert.equal(quantityForBalancePercentage({
    available: Number.POSITIVE_INFINITY,
    mode: 'spot',
    percentage: .5,
    price: 50,
    side: 'buy',
  }), 0)
})

test('秒合约显示真实参考价、预计返还、确认与订单记录', () => {
  assert.match(secondsSource, /const productsRequest = fetchSecondsProducts\(\)/)
  assert.match(secondsSource, /session\.isAuthenticated\s*\?\s*await Promise\.all\(\[productsRequest, fetchSecondsOrders\(\), fetchWalletAccounts\(\)\]\)\s*:\s*\[await productsRequest, \[\], \[\]\]/)
  assert.match(secondsSource, /function reviewOrder\(\): void \{\s*if \(!session\.isAuthenticated\)/)
  assert.match(secondsSource, /marketStore\.tickerFor\(selected\.value\?\.symbol \|\| ''\)/)
  assert.match(secondsSource, /amountNumber\.value \* \(1 \+ payoutRate\.value\)/)
  assert.match(secondsSource, /:data-seconds-market="selected \? 'live' : loading \? 'loading' : 'empty'"/)
  assert.match(secondsSource, /data-session-feedback="created"/)
  assert.match(secondsSource, /t\('marketDetail\.latestPrice'\)/)
  assert.match(secondsSource, /role="dialog"/)
  assert.match(secondsSource, /aria-modal="true"/)
  assert.match(secondsSource, /await openSecondsOrder\(\{/)
})

test('行情详情与订单中心保持真实数据、二级操作面和危险操作复核', () => {
  assert.match(marketDetailSource, /data-market-workspace="live"/)
  assert.match(marketDetailSource, /fetchKlines\(pairSymbol\.value, interval\.value\)/)
  assert.match(marketDetailSource, /fetchOrderBook\(pairSymbol\.value\)/)
  assert.match(marketDetailSource, /fetchRecentTrades\(pairSymbol\.value\)/)
  assert.match(marketDetailSource, /openTrade\('spot'\)/)
  assert.match(marketDetailSource, /openTrade\('contract'\)/)

  assert.match(ordersSource, /data-orders-workspace="live"/)
  assert.match(ordersSource, /class="order-tabs"/)
  assert.match(ordersSource, /role="dialog"/)
  assert.match(ordersSource, /aria-modal="true"/)
  assert.match(ordersSource, /document\.body\.style\.overflow = 'hidden'/)
  assert.match(ordersSource, /data-dialog-cancel/)
  assert.match(ordersSource, /await cancelSpotOrder\(order\.id\)/)
  assert.match(ordersSource, /await cancelAllSpotOrders\(spotOrders\.value\.map\(\(order\) => order\.id\)\)/)
  assert.match(ordersSource, /await cancelAllMarginPositions\(\)/)
  assert.match(ordersSource, /await closeAllMarginPositions\(\)/)
})

test('图表与盘口使用稳定实时数据画布并兼容秒和毫秒时间戳', () => {
  assert.match(chartSource, /data-kline-provider="tradingview"/)
  assert.match(chartSource, /if \(width <= 0 \|\| height <= 0\) return/)
  assert.match(bookSource, /visibleAsks = computed/)
  assert.match(bookSource, /visibleBids = computed/)
  assert.match(bookSource, /t\('marketDetail\.price'\)/)
  assert.match(bookSource, /t\('marketDetail\.quantity'\)/)

  const points = normalizeMarketChartPoints([
    { time: 1_720_000_000_000, open: 2, high: 3, low: 1, close: 2.5, volume: 4 },
    { time: 1_710_000_000, open: 1, high: 2, low: .5, close: 1.5, volume: 3 },
    { time: 1_720_000_000, open: 3, high: 4, low: 2, close: 3.5, volume: 5 },
  ])
  assert.deepEqual(points.map((point) => point.time), [1_710_000_000, 1_720_000_000])
  assert.equal(points[1]?.open, 3)
})

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}
