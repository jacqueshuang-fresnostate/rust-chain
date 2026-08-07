import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const tradeSource = read('../src/views/TradeView.vue')
const secondsSource = read('../src/views/SecondsView.vue')
const productHubSource = read('../src/views/ProductHubView.vue')
const predictionSource = read('../src/views/PredictionView.vue')
const orderBookSource = read('../src/components/OrderBookPanel.vue')
const selectedPageCss = read('../src/styles/pencil-selected-pages.css')
const legacyPrototypeCss = read('../src/styles/prototype-base.css')

function styleOf(source: string): string {
  return source.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)?.[1] || ''
}

test('现货 yzOPc/bo8k5 模板保持逐字不变，合约使用独立 by3G9/pKHeU 分支', () => {
  const spotStart = tradeSource.indexOf('    <template v-if="isSpotMode">')
  const contractStart = tradeSource.indexOf('    <template v-else>', spotStart)
  assert.ok(spotStart >= 0 && contractStart > spotStart)

  const spotTemplate = tradeSource.slice(spotStart, contractStart)
  const contractTemplate = tradeSource.slice(contractStart, tradeSource.indexOf('    <div v-if="confirmOpen"', contractStart))
  const spotDigest = createHash('sha256').update(spotTemplate).digest('hex')

  assert.equal(spotDigest, '7b3247272adfe69a374bc64452faec8d0ca41367ecc85ecdec7fc6f9436dc444')
  assert.match(spotTemplate, /data-pencil-source="yzOPc-bo8k5"/)
  assert.doesNotMatch(spotTemplate, /by3G9|pKHeU|contract-pencil-/)
  assert.match(contractTemplate, /data-pencil-source="by3G9 pKHeU"/)
  assert.doesNotMatch(contractTemplate, /yzOPc|bo8k5|spot-pencil-workspace/)
})

test('合约为独立二栏下单、五档真实盘口和真实持仓状态面', () => {
  const css = styleOf(tradeSource)
  assert.match(css, /\.contract-pencil-module\s*\{[\s\S]*?grid-template-columns: minmax\(0, 1fr\) 150px;/)
  assert.match(tradeSource, /class="contract-mini-book"[\s\S]*?:asks="asks"[\s\S]*?:bids="bids"[\s\S]*?layout="mini"/)
  assert.match(orderBookSource, /const miniAsks = computed\(\(\) => props\.asks\.slice\(0, 5\)\.reverse\(\)\)/)
  assert.match(orderBookSource, /const miniBids = computed\(\(\) => props\.bids\.slice\(0, 5\)\)/)
  assert.match(tradeSource, /const margin = await fetchMarginWallets\(\)[\s\S]*?marginWallets\.value = margin\.wallets[\s\S]*?marginPositions\.value = margin\.positions/)
  assert.match(tradeSource, /v-if="visibleMarginPositions\.length" class="contract-position-list"/)
  assert.match(tradeSource, /v-else class="contract-position-empty"/)
  assert.match(tradeSource, /fetchOrderBook\(symbol\)/)
  assert.match(tradeSource, /createMarketDetailStreamSession\(\{/)
})

test('秒合约由真实订单切换 VL8er/g9agt 与 Lpt6q/WxeB8 几何并直用现货钱包', () => {
  const css = styleOf(secondsSource)
  assert.match(secondsSource, /data-pencil-source="VL8er g9agt Lpt6q WxeB8"/)
  assert.match(secondsSource, /const activeOrder = computed\(\(\) => orders\.value\.find\(\(order\) => \['opened', 'pending', 'active'\]\.includes\(order\.status\.toLowerCase\(\)\)\) \|\| null\)/)
  assert.match(secondsSource, /:data-seconds-state="activeOrder \? 'active' : 'default'"/)
  assert.doesNotMatch(secondsSource, /secondary-view|secondary-content|page--prototype-grid/)
  assert.match(secondsSource, /v-if="activeOrder" class="seconds-active-order" data-active-order="real"/)
  assert.match(secondsSource, /fetchSecondsProducts\(\)/)
  assert.match(secondsSource, /fetchSecondsOrders\(\)/)
  assert.match(secondsSource, /fetchWalletAccounts\(\)/)
  assert.match(secondsSource, /activeOrder\.entryPrice !== undefined \? formatPrice\(activeOrder\.entryPrice\) : '--'/)
  assert.match(secondsSource, /order\.stakeAmount \* order\.payoutRate/)
  assert.match(secondsSource, /orders\.value = \[openedOrder, \.\.\.orders\.value\.filter/)
  assert.match(secondsSource, /accounts\.value\.find\(\(item\) => item\.assetId === selected\.value\?\.stakeAssetId\)/)
  assert.match(secondsSource, /fetchKlines\(symbol, '1m'\)/)
  assert.match(secondsSource, /await openSecondsOrder\(\{[\s\S]*?productId:[\s\S]*?durationSeconds:[\s\S]*?direction:[\s\S]*?stakeAmount:/)

  assert.match(css, /\.seconds-market-board\s*\{[\s\S]*?padding: 4px 20px 0;/)
  assert.match(css, /\.seconds-micro-chart\s*\{[\s\S]*?height: 170px;/)
  assert.match(css, /\.seconds-active-order\s*\{[\s\S]*?border-radius: 14px;[\s\S]*?padding: 12px 14px;/)
  assert.match(css, /\.seconds-direction-grid button\s*\{\s*min-height: 52px;/)
  assert.match(css, /\.seconds-duration-grid button\s*\{[\s\S]*?height: 36px;[\s\S]*?min-height: 36px;/)
  assert.match(css, /\.seconds-amount-presets\s*\{\s*display: none;/)
  assert.match(css, /\.seconds-submit\s*\{[\s\S]*?border-radius: 26px;[\s\S]*?min-height: 52px;/)
})

test('产品中心仅渲染两条 64px 产品行与一条 48px 产品说明入口', () => {
  const css = styleOf(productHubSource)
  assert.match(productHubSource, /data-pencil-source="Z0B0N6 zMsKE"/)
  assert.equal((productHubSource.match(/class="product-card product-card--secondary product-hub__row"/g) || []).length, 2)
  assert.match(productHubSource, /data-product="prediction"/)
  assert.match(productHubSource, /data-product="news"/)
  assert.match(productHubSource, /<Gauge :size="19"/)
  assert.match(productHubSource, /<Newspaper :size="19"/)
  assert.equal((productHubSource.match(/<ChevronRight :size="18"/g) || []).length, 2)
  assert.match(productHubSource, /<BookOpen :size="16"/)
  assert.match(productHubSource, /t\('products\.hubPrediction'\)[\s\S]*?t\('products\.hubPredictionDescription'\)/)
  assert.match(productHubSource, /t\('products\.hubNews'\)[\s\S]*?t\('products\.hubNewsDescription'\)/)
  assert.match(productHubSource, /t\('products\.hubHelp'\)/)
  assert.doesNotMatch(productHubSource, /CircleDollarSign|products\.introDescription'\) \}\}<\/span>|news\.market/)
  assert.deepEqual({
    prediction: zhCN.products.hubPrediction,
    predictionDescription: zhCN.products.hubPredictionDescription,
    news: zhCN.products.hubNews,
    newsDescription: zhCN.products.hubNewsDescription,
    help: zhCN.products.hubHelp,
  }, {
    prediction: '预测',
    predictionDescription: '交易事件与市场情绪',
    news: '新闻中心',
    newsDescription: '市场观察、产品更新与研究笔记',
    help: '查看产品说明',
  })
  assert.deepEqual({
    prediction: en.products.hubPrediction,
    predictionDescription: en.products.hubPredictionDescription,
    news: en.products.hubNews,
    newsDescription: en.products.hubNewsDescription,
    help: en.products.hubHelp,
  }, {
    prediction: 'Prediction',
    predictionDescription: 'Trade events and market sentiment',
    news: 'News center',
    newsDescription: 'Market insights, product updates, and research notes',
    help: 'View product guide',
  })
  assert.match(productHubSource, /router\.push\(\{ name: 'news' \}\)/)
  assert.match(productHubSource, /router\.push\(\{ name: 'news', query: \{ category: 'product' \} \}\)/)
  assert.doesNotMatch(productHubSource.match(/<template>([\s\S]*?)<\/template>/)?.[1] || '', /v-for="product in/)
  assert.doesNotMatch(productHubSource, /featuredProducts|secondaryProducts|const products = computed/)
  assert.match(legacyPrototypeCss, /\.product-hub\s*\{[\s\S]*?display: grid;[\s\S]*?gap: 14px;/)
  assert.match(css, /\.product-hub\s*\{[\s\S]*?display: block;[\s\S]*?gap: 0;/)
  assert.match(css, /\.product-hub__body\s*\{[\s\S]*?gap: 18px;[\s\S]*?padding: 8px 20px/)
  assert.match(css, /\.product-hub__row\s*\{[\s\S]*?height: 64px;[\s\S]*?min-height: 64px;/)
  assert.match(css, /\.product-hub__help\s*\{[\s\S]*?height: 48px;[\s\S]*?min-height: 48px;/)
})

test('预测页使用 pU7Kz/IcvzQ 的真实市场卡、状态筛选与是/否报价动作', () => {
  const css = styleOf(predictionSource)
  assert.match(predictionSource, /data-pencil-source="pU7Kz IcvzQ CzpTv ZvGMv"/)
  assert.match(predictionSource, /const visibleMarkets = computed\(\(\) => markets\.value\.filter/)
  assert.match(predictionSource, /data-market-source="api"/)
  assert.match(predictionSource, /v-for="market in visibleMarkets"/)
  assert.match(predictionSource, /@click="openOrder\(market, 'yes'\)"/)
  assert.match(predictionSource, /@click="openOrder\(market, 'no'\)"/)
  assert.match(predictionSource, /fetchPredictionMarkets\(\), fetchPredictionConfig\(\)/)
  assert.match(predictionSource, /fetchWalletAccounts\(\), fetchPredictionOrders\(\)/)
  assert.match(predictionSource, /requestPredictionQuote\(\{ marketId: selected\.value\.id, outcome: outcome\.value, assetId: assetId\.value, stakeAmount: amountNumber\.value \}\)/)
  assert.match(predictionSource, /await confirmPredictionQuote\(quote\.value\.quoteId\)/)
  assert.match(predictionSource, /orders\.value = \[createdOrder, \.\.\.orders\.value\.filter/)
  assert.match(predictionSource, /orderStatusLabel\(order\)/)
  assert.match(css, /\.prediction-content\s*\{[\s\S]*?gap: 14px;[\s\S]*?padding: 6px 20px/)
  assert.match(css, /\.prediction-list article\s*\{[\s\S]*?gap: 10px;[\s\S]*?padding: 12px 0 6px;/)
  assert.match(css, /\.prediction-outcomes\s*\{[\s\S]*?grid-template-columns: repeat\(2, minmax\(0, 1fr\)\)/)
  assert.match(css, /\.prediction-outcomes button\s*\{[\s\S]*?height: 38px;[\s\S]*?min-height: 38px;/)
})

test('本轮四页只消费真实状态，不内置画板演示行情、余额或订单', () => {
  const sources = [tradeSource, secondsSource, productHubSource, predictionSource]
  for (const source of sources) {
    assert.doesNotMatch(source, /(?:63,?085|63,?080|01842|1,?284\.00)/)
    assert.doesNotMatch(source, /\b(?:mock|fixture|demoData|fakeOrder|sampleMarket)s?\b/i)
    assert.doesNotMatch(source, /<svg|#[0-9a-f]{3,8}|rgba?\(/i)
  }
})

test('合约、秒合约、产品中心与预测页按选稿使用白色和纯黑根画布且不影响现货', () => {
  const selectedRoots = [
    '.contract-trade',
    '.seconds-page',
    '.product-hub',
    '.prediction-page',
  ]

  for (const selector of selectedRoots) {
    assert.match(selectedPageCss, new RegExp(`\\.app-stage \\.mobile-canvas \\${selector.replace('.', '.')}`))
    assert.match(
      selectedPageCss,
      new RegExp(`html\\[data-theme='dark'\\] \\.app-stage \\.mobile-canvas \\${selector.replace('.', '.')}`),
    )
  }

  assert.match(selectedPageCss, /--page: #ffffff;[\s\S]*?background-color: var\(--page\);[\s\S]*?background-image: none;/)
  assert.match(selectedPageCss, /html\[data-theme='dark'\][\s\S]*?--page: #000000;[\s\S]*?background-color: var\(--page\);[\s\S]*?background-image: none;/)
  assert.doesNotMatch(selectedPageCss, /\.spot-trade/)
})
