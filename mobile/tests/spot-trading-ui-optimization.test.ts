import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const tradeSource = source('../src/views/TradeView.vue')
const tradeCss = styleOf(tradeSource)
const orderBookSource = source('../src/components/OrderBookPanel.vue')
const appSource = source('../src/App.vue')
const baseCss = source('../src/styles/base.css')

test('现货工作台以 REST 快照启动并由单一 WebSocket 会话持续更新三类市场数据', () => {
  assert.match(tradeSource, /createMarketDetailStreamSession\(\{/)
  assert.match(tradeSource, /getUrl: publicMarketWebSocketUrl/)
  assert.match(tradeSource, /onDepth: \(_context, snapshot\) => \{[\s\S]*?bids\.value = snapshot\.bids[\s\S]*?asks\.value = snapshot\.asks/)
  assert.match(tradeSource, /onTrade: \(_context, trade\) => \{[\s\S]*?mergeMarketTrades\(trades\.value, trade, 16\)/)
  assert.match(tradeSource, /onKlines: \(_context, nextPoints\) => \{[\s\S]*?points\.value = nextPoints/)
  assert.match(tradeSource, /Promise\.allSettled\(\[[\s\S]*?fetchKlines\(symbol, selectedInterval\),[\s\S]*?fetchOrderBook\(symbol\),[\s\S]*?fetchRecentTrades\(symbol\),/)
  assert.match(tradeSource, /detailStreamSession\.resolveKlineRequest\(klineRequest, restPoints\)/)
  assert.match(tradeSource, /mergeMarketTradeHistory\(trades\.value, restTrades, 16\)/)
  assert.match(tradeSource, /detailStreamSession\.stop\(\)/)
})

test('现货默认层级直接映射 Pencil 选中的左右工作台、账户区和折叠图表入口', () => {
  for (const marker of [
    'data-pencil-source="yzOPc-bo8k5"',
    'class="spot-pencil-header"',
    'class="spot-pencil-workspace"',
    'class="spot-order-console"',
    'class="spot-mini-book"',
    'class="spot-account-workspace"',
    'class="spot-chart-entry"',
    'class="spot-chart-drawer"',
    'class="spot-market-data__tabs"',
    'class="spot-recent-trades"',
  ]) {
    assert.match(tradeSource, new RegExp(escapeRegExp(marker)))
  }
  assert.match(tradeSource, /<template v-if="isSpotMode">/)
  assert.match(tradeSource, /layout="mini"/)
  assert.match(tradeSource, /v-if="spotChartOpen" id="spot-local-chart"/)
  assert.match(tradeSource, /role="tablist"/)
  assert.match(tradeSource, /aria-controls="spot-order-book-panel"/)
  assert.match(tradeSource, /aria-controls="spot-trades-panel"/)
  assert.match(tradeSource, /<OrderBookPanel[\s\S]*?layout="split"/)
  assert.doesNotMatch(tradeSource, /<svg|\p{Extended_Pictographic}/u)
})

test('现货拥有 Pencil 二级 Header，根 Logo Header 不再叠加', () => {
  assert.match(appSource, /const showRootHeader = computed\(\(\) => \([\s\S]*?\['home', 'markets'\]/)
  assert.match(appSource, /<RootHeader v-if="showRootHeader" \/>/)
  assert.match(tradeSource, /class="spot-pencil-header"[\s\S]*?class="spot-header-control"[\s\S]*?@click="goBack"/)
  assert.match(tradeSource, /<AssetMark :symbol="baseAsset" :src="ticker\?\.iconUrl" :fallback-src="ticker\?\.baseIconUrl" :size="24"/)
  assert.match(tradeSource, /@click="toggleFavorite"[\s\S]*?<Star :size="23"/)
  assert.match(tradeSource, /@click="shareMarket"[\s\S]*?<Share2 :size="22"/)
  assert.match(tradeSource, /function goBack\(\): void \{\s*void goBackOr\(router, \{ name: 'markets' \}\)/)
  assert.match(tradeCss, /\.spot-pencil-header\s*\{[\s\S]*?height: 64px;[\s\S]*?position: sticky;[\s\S]*?z-index: 42;/)
  assert.match(tradeCss, /\.spot-header-control\s*\{[\s\S]*?height: 44px;[\s\S]*?width: 44px;/)
})

test('Pencil 390px 几何与 320px 紧凑盘口都不产生横向溢出', () => {
  assert.match(tradeCss, /\.spot-pencil-workspace\s*\{[\s\S]*?gap: 14px;[\s\S]*?grid-template-columns: minmax\(0, 1fr\) 148px;[\s\S]*?padding: 8px 16px 10px;/)
  assert.match(tradeCss, /\.spot-order-console\s*\{[\s\S]*?gap: 10px;/)
  assert.match(tradeCss, /\.spot-side-switch\s*\{[\s\S]*?height: 40px;/)
  assert.match(tradeCss, /\.spot-trade \.spot-type-field\s*\{\s*min-height: 40px;/)
  assert.match(tradeCss, /\.spot-field-shell\s*\{[\s\S]*?height: 44px;/)
  assert.match(tradeCss, /\.spot-submit-order\s*\{[\s\S]*?height: 46px;/)
  assert.match(tradeCss, /@media \(max-width: 340px\)[\s\S]*?grid-template-columns: minmax\(0, 1fr\) 124px;/)
  assert.match(orderBookSource, /layout\?: 'stacked' \| 'split' \| 'paired' \| 'matrix' \| 'mini'/)
  assert.match(orderBookSource, /const miniAsks = computed\(\(\) => props\.asks\.slice\(0, 5\)\.reverse\(\)\)/)
  assert.match(orderBookSource, /const miniBids = computed\(\(\) => props\.bids\.slice\(0, 5\)\)/)
  assert.match(tradeCss, /\.spot-market-data__tabs button\s*\{[\s\S]*?min-height: 50px;[\s\S]*?min-width: 44px;/)
  assert.match(tradeCss, /\.spot-field-shell:focus-within\s*\{[\s\S]*?border-color: var\(--focus\);[\s\S]*?box-shadow: 0 0 0 3px var\(--focus-ring\);/)
  assert.match(tradeCss, /\.spot-field-shell input:focus-visible\s*\{[\s\S]*?box-shadow: none;[\s\S]*?outline: 0;/)
  assert.doesNotMatch(tradeCss, /\.spot-pencil-workspace :is\(button, input\):focus-visible/)
  assert.match(baseCss, /\.sr-only\s*\{[\s\S]*?clip-path: inset\(50%\);[\s\S]*?position: absolute;/)
  assert.doesNotMatch(tradeCss, /width:\s*100vw|overflow-x:\s*auto/)
})

test('新增现货状态文案保持中英文键一致', () => {
  for (const key of ['liveMarket', 'restAndSocket', 'depthLive', 'klineLive', 'depthSnapshot', 'klineSnapshot', 'tradeTime', 'noRecentTrades', 'limitOrderShort', 'marketOrderShort', 'turnover', 'available', 'onlyCurrent', 'spotAssetEmpty', 'spotAssetEmptyHint'] as const) {
    assert.equal(typeof zhCN.trade[key], 'string')
    assert.equal(typeof en.trade[key], 'string')
    assert.ok(zhCN.trade[key].length > 0)
    assert.ok(en.trade[key].length > 0)
  }
})

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

function styleOf(fileSource: string): string {
  const match = fileSource.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)
  assert.ok(match)
  return match[1] || ''
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
