import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const tradeSource = source('../src/views/TradeView.vue')
const secondsSource = source('../src/views/SecondsView.vue')
const parityCss = source('../src/styles/prototype-parity.css')
const selectedCss = source('../src/styles/pencil-selected-pages.css')
const routerSource = source('../src/router/index.ts')
const tradeCss = styleOf(tradeSource)
const secondsCss = styleOf(secondsSource)

test('现货、合约与秒合约保持三个独立路由和真实业务语义', () => {
  assert.match(routerSource, /path: '\/trade\/:symbol\?', name: 'trade'/)
  assert.match(routerSource, /path: '\/seconds', alias: '\/products\/seconds', name: 'seconds'/)
  assert.match(tradeSource, /const mode = ref<'spot' \| 'contract'>\(route\.query\.mode === 'contract' \? 'contract' : 'spot'\)/)
  assert.match(tradeSource, /watch\(\(\) => route\.query\.mode/)
  assert.match(tradeSource, /navigation\.rememberTradeMode\(mode\.value\)/)
  assert.match(tradeSource, /query: \{ purpose: 'trade', mode: mode\.value \}/)
  assert.match(tradeSource, /:data-trade-mode="mode"/)
  assert.match(tradeSource, /:data-trade-surface="mode"/)
  assert.doesNotMatch(tradeSource, /class="trade-category"|selectTradeMode/)
})

test('交易工作台保留行情、K 线、盘口、余额、委托与下单处理函数', () => {
  assert.match(tradeSource, /fetchKlines\(symbol, selectedInterval\)/)
  assert.match(tradeSource, /fetchOrderBook\(symbol\)/)
  assert.match(tradeSource, /fetchRecentTrades\(symbol\)/)
  assert.match(tradeSource, /createMarketDetailStreamSession\(\{/)
  assert.match(tradeSource, /detailStreamSession\.replace\(symbol, selectedInterval, version\)/)
  assert.match(tradeSource, /mergeMarketTrades\(trades\.value, trade, 16\)/)
  assert.match(tradeSource, /marketStore\.tickerFor\(pairSymbol\.value\)/)
  assert.match(tradeSource, /fetchWalletAccounts\(\)/)
  assert.match(tradeSource, /fetchMarginWallets\(\)/)
  assert.match(tradeSource, /quantityForBalancePercentage\(\{/)
  assert.match(tradeSource, /await placeSpotOrder\(\{[\s\S]*?symbol: pairSymbol\.value,[\s\S]*?quantity: orderAmount,/)
  assert.match(tradeSource, /createMarginOrderReview\(\{[\s\S]*?productId: selectedProduct\.value\?\.id \|\| 0,[\s\S]*?marginAmount: Number\(quantity\.value\),/)
  assert.match(tradeSource, /await placeMarginOrder\(review\.request\)/)
  assert.match(tradeSource, /selectContractWorkspaceTab\('positions'\)/)
  assert.match(tradeSource, /await closeMarginPosition\(position\.id\)/)
  assert.match(tradeSource, /await cancelMarginPosition\(position\.id\)/)
  assert.match(tradeSource, /await closeAllMarginPositions\(currentPairOnly\.value \? selectedProduct\.value\?\.id : undefined\)/)
  assert.doesNotMatch(tradeSource, /await closeAllMarginPositions\(selectedProduct\.value\??\.id\)/)
  assert.match(tradeSource, /fetchMarginPositionRisk\(position\.id\)/)
  assert.match(tradeSource, /v-for="time in \['1m', '5m', '15m', '1h', '1d'\]"/)
  assert.doesNotMatch(tradeSource, /\['1m', '15m', '1h', '4h', '1d'\]/)
  assert.match(tradeSource, /<OrderBookPanel[\s\S]*?class="trade-order-book"[\s\S]*?layout="split"/)
  assert.match(tradeSource, /data-market-data-panel="marketDataPanel"/)
  assert.match(tradeSource, /class="spot-recent-trades"/)
  assert.match(tradeSource, /<MobileMarketChart :points="points" :loading="chartLoading" :interval="interval" :symbol="pairSymbol" \/>/)
})

test('秒合约继续直用现货钱包、后台产品周期与真实下单接口且没有划转入口', () => {
  assert.match(secondsSource, /const nextProducts = await fetchSecondsProducts\(\)/)
  assert.match(secondsSource, /Promise\.allSettled\(\[fetchSecondsOrders\(100\), fetchWalletAccounts\(\)\]\)/)
  assert.match(secondsSource, /accounts\.value\.find\(\(item\) => item\.assetId === selected\.value\?\.stakeAssetId\)/)
  assert.match(secondsSource, /selected\.value\?\.cycles\.find\(\(item\) => item\.id === selectedCycleId\.value\)/)
  assert.match(secondsSource, /function setDirection\(nextDirection: 'up' \| 'down'\)/)
  assert.match(secondsSource, /const review = orderReview\.value[\s\S]*?await openSecondsOrder\(\{[\s\S]*?productId: review\.productId,[\s\S]*?durationSeconds: review\.durationSeconds,[\s\S]*?direction: review\.direction,[\s\S]*?stakeAmount: review\.stakeAmount,[\s\S]*?idempotencyKey: review\.idempotencyKey,/)
  assert.match(secondsSource, /marketStore\.tickerFor\(selected\.value\?\.symbol \|\| ''\)/)
  assert.doesNotMatch(secondsSource, /\b(?:transfer|fetchMarginWallets|updateMarginLeverage|placeMarginOrder)\b|划转/iu)
})

test('两类页面采用单一价格主角和连续 Instrument plate', () => {
  assert.match(tradeSource, /data-instrument-hero="pair-price"/)
  assert.match(tradeSource, /data-instrument-plate="market-and-order"/)
  assert.match(secondsSource, /data-instrument-hero="pair-price"/)
  assert.match(secondsSource, /data-instrument-plate="market-and-order"/)

  assertOrdered(tradeSource, [
    'data-instrument-hero="pair-price"',
    'class="contract-header-identity"',
    'class="contract-pencil-module"',
    'class="chart-panel trade-chart-panel"',
    'class="trade-order-book"',
    'class="trade-console"',
  ])
  assertOrdered(secondsSource, [
    'data-instrument-hero="pair-price"',
    'class="seconds-pair-field"',
    'class="seconds-trading-operation"',
    'class="seconds-market-status"',
    'class="seconds-price-panel"',
    'class="seconds-micro-chart"',
    'class="instrument-plate seconds-order-console"',
    'class="seconds-duration-grid"',
    'class="seconds-cycle-limit"',
    'class="seconds-amount-field"',
    'class="seconds-direction-grid"',
    'class="button button--primary button--full seconds-submit"',
    'class="seconds-orders-workspace"',
    'class="seconds-order-filters"',
    'class="seconds-active-order-list"',
  ])
  assert.doesNotMatch(secondsSource, /seconds-session-records|ordersSection|scrollToOrders/)

  assert.match(tradeSource, /<style\s+scoped\s*>/)
  assert.match(secondsSource, /<style\s+scoped\s*>/)
  assert.match(tradeCss, /\.trade-workspace\s*\{[\s\S]*?background: var\(--surface\);[\s\S]*?border-bottom: 1px solid var\(--line-strong\);/)
  assert.match(secondsCss, /\.seconds-workspace\s*\{[\s\S]*?display: block;[\s\S]*?width: 100%;/)
  assert.match(tradeCss, /border-radius: 0;/)
  assert.match(secondsCss, /\.seconds-order-console\s*\{[\s\S]*?border-radius: 0;/)
})

test('交易表单保持可用触控，Seconds 视觉几何锁定 30/38/40/44px 选中稿', () => {
  assert.match(tradeCss, /\.input-stack \.field-shell\s*\{[\s\S]*?height: 52px;[\s\S]*?min-height: 52px;/)
  assert.match(tradeCss, /\.input-stack \.field-shell input\s*\{[\s\S]*?min-height: 44px;/)
  assert.match(tradeCss, /\.side-switch button\s*\{[\s\S]*?min-height: 50px;/)
  assert.match(tradeCss, /\.amount-control input\[type="range"\]\s*\{[\s\S]*?height: 44px;[\s\S]*?min-height: 44px;/)
  assert.match(tradeCss, /\.contract-percentage__input\s*\{[\s\S]*?height: 44px;[\s\S]*?min-height: 44px;/)
  assert.match(tradeCss, /\.submit-order\s*\{\s*min-height: 52px;/)
  assert.match(tradeCss, /\.input-stack \.field-shell:focus-within\s*\{[\s\S]*?border-color: var\(--focus\);[\s\S]*?box-shadow: 0 0 0 3px var\(--focus-ring\);/)

  assert.match(secondsCss, /\.seconds-pair-field select\s*\{[\s\S]*?height: 44px;[\s\S]*?inset: -11px 0 auto;/)
  assert.match(secondsCss, /\.seconds-order-console\s*\{[\s\S]*?grid-template-rows: 30px 26px 38px 40px 44px;[\s\S]*?height: 202px;/)
  assert.match(secondsCss, /\.seconds-duration-grid button\s*\{[\s\S]*?height: 30px;/)
  assert.match(secondsCss, /\.seconds-duration-grid button::before\s*\{[\s\S]*?inset: -8px 0;/)
  assert.match(secondsCss, /\.seconds-amount-field\s*\{[\s\S]*?height: 38px;/)
  assert.match(secondsCss, /\.seconds-amount-field::before\s*\{[\s\S]*?inset: -4px 0;/)
  assert.match(secondsCss, /\.seconds-direction-grid button\s*\{[\s\S]*?height: 40px;/)
  assert.match(secondsCss, /\.seconds-direction-grid button::before\s*\{[\s\S]*?inset: -3px 0;/)
  assert.match(secondsCss, /\.seconds-submit\s*\{[\s\S]*?height: 44px;[\s\S]*?min-height: 44px !important;/)
  assert.match(secondsCss, /\.seconds-pair-field:focus-within\s*\{[\s\S]*?outline: 2px solid var\(--focus\);[\s\S]*?outline-offset: 3px;/)
  assert.match(secondsCss, /\.seconds-amount-field:focus-within\s*\{[\s\S]*?border-color: var\(--focus\);[\s\S]*?box-shadow: 0 0 0 3px var\(--focus-ring\);/)

  assert.doesNotMatch(tradeCss, /box-shadow:\s*inset 3px 0/)
  assert.doesNotMatch(secondsCss, /box-shadow:\s*inset 3px 0/)
})

test('320–448px 响应式、安全区和低动态合同不产生工作区固定遮挡', () => {
  for (const width of [320, 360, 390, 448]) {
    const tradeInnerWidth = width - 32
    const secondsInnerWidth = width - 40
    assert.ok(tradeInnerWidth >= 5 * 44, `${width}px interval rail must fit five touch targets`)
    assert.ok((secondsInnerWidth - 18) / 4 >= 44, `${width}px duration rail must show four usable targets`)
  }

  assert.match(tradeCss, /\.trade-quote > div:first-child strong\s*\{[\s\S]*?overflow-wrap: normal;[\s\S]*?white-space: nowrap;/)
  assert.match(tradeCss, /@media \(max-width: 340px\)\s*\{[\s\S]*?\.trade-quote\s*\{[\s\S]*?grid-template-columns: minmax\(0, 1fr\) minmax\(96px, \.62fr\);[\s\S]*?\.trade-quote > div:first-child strong\s*\{\s*font-size: 29px;/)

  for (const [css, sourceName] of [[tradeCss, 'trade'], [secondsCss, 'seconds']] as const) {
    assert.match(css, /overflow-x: clip;/, `${sourceName} should clip decorative x overflow`)
    assert.match(css, /env\(safe-area-inset-bottom\)/, `${sourceName} should reserve the bottom safe area`)
    assert.match(css, /@media \(max-width: 340px\)/, `${sourceName} should handle 320px layouts`)
    assert.match(css, /@media \(prefers-reduced-motion: reduce\)/, `${sourceName} should disable nonessential motion`)
    assert.doesNotMatch(css, /width:\s*100vw/)
    if (sourceName === 'trade') {
      assert.doesNotMatch(css, /overflow-x:\s*auto/)
      assert.match(css, /--contract-bg: #f7f9f8;/)
      assert.match(css, /html\[data-theme='dark'\] \.contract-trade \{[\s\S]*?--contract-bg: #070a09;/)
      assert.match(css, /@media \(max-width: 359px\)[\s\S]*?grid-template-columns: minmax\(0, 1fr\) 132px;/)
    } else {
      assert.match(css, /\.seconds-duration-scroll\s*\{[\s\S]*?overflow-x: auto;/)
      assert.match(css, /\.seconds-trading-operation\s*\{[\s\S]*?height: 420px;/)
      assert.match(css, /\.seconds-orders-workspace\s*\{[\s\S]*?padding: 12px 20px calc\(16px \+ env\(safe-area-inset-bottom\)\);/)
      assert.doesNotMatch(css, /#[0-9a-f]{3,8}|rgba?\(/i)
    }
  }

  const spotOrderTypeLayerIndex = tradeCss.indexOf('.spot-order-type-layer {')
  assert.notEqual(spotOrderTypeLayerIndex, -1, 'spot order-type sheet must own an independent style boundary')
  const tradeConfirmationIndex = tradeCss.indexOf('.confirmation-layer {')
  assert.ok(tradeConfirmationIndex > spotOrderTypeLayerIndex, 'trade confirmation styles must remain after the order-type sheet')
  const tradeBeforeConfirmation = tradeCss.slice(0, tradeConfirmationIndex)
  const tradeWithoutOrderTypeLayer = tradeBeforeConfirmation.replace(/\.spot-order-type-layer\s*\{[^}]*\}/, '')
  assert.doesNotMatch(tradeWithoutOrderTypeLayer, /position:\s*fixed/)
  assert.match(tradeSource, /<Teleport to="body">[\s\S]*?class="spot-order-type-layer"/)
  assert.match(tradeCss.slice(spotOrderTypeLayerIndex), /^\.spot-order-type-layer\s*\{[^}]*inset:\s*0;[^}]*position:\s*fixed;[^}]*\}/)
  const secondsSettlementLayerIndex = secondsCss.indexOf('.seconds-settlement-layer {')
  assert.notEqual(
    secondsSettlementLayerIndex,
    -1,
    'seconds settlement result must own an independent style boundary',
  )
  const secondsMaskIndex = secondsCss.indexOf('.seconds-mask {')
  assert.notEqual(secondsMaskIndex, -1, 'seconds confirmation mask must own an independent style boundary')
  assert.ok(
    secondsMaskIndex > secondsSettlementLayerIndex,
    'seconds confirmation styles must remain after the settlement-result island',
  )
  const secondsBeforeMask = secondsCss.slice(0, secondsMaskIndex)
  const secondsWithoutSettlementLayer = secondsBeforeMask.replace(
    /\.seconds-settlement-layer\s*\{[^}]*\}/,
    '',
  )
  assert.doesNotMatch(secondsWithoutSettlementLayer, /position:\s*fixed/)
  assert.match(
    secondsSource,
    /<Teleport to="body">\s*<Transition name="seconds-result-reveal"[\s\S]*?class="seconds-settlement-layer"/,
  )
  assert.match(
    secondsCss.slice(secondsSettlementLayerIndex),
    /^\.seconds-settlement-layer\s*\{[^}]*pointer-events:\s*none;[^}]*position:\s*fixed;[^}]*\}/,
  )
  assert.match(secondsSource, /<Teleport to="body">[\s\S]*?class="confirmation-layer seconds-mask"/)
  assert.match(secondsCss.slice(secondsMaskIndex), /^\.seconds-mask\s*\{[^}]*inset:\s*0;[^}]*position:\s*fixed;[^}]*\}/)
  assert.doesNotMatch(`${tradeSource}\n${secondsSource}`, /<svg|\p{Extended_Pictographic}/u)
})

test('交易与秒合约使用薄荷主动作，Seconds 使用独立选中稿主题令牌', () => {
  assert.match(parityCss, /\.trade-view \.trade-instrument-hero\s*\{[\s\S]*?var\(--signal-green\)/)
  assert.match(parityCss, /\.trade-view \.submit-order,[\s\S]*?background:\s*var\(--accent\)/)
  assert.match(selectedCss, /\.app-stage \.mobile-canvas \.seconds-page\s*\{[\s\S]*?--seconds-page: #ffffff;[\s\S]*?--seconds-signal: #43efa9;/)
  assert.match(selectedCss, /html\[data-theme='dark'\] \.app-stage \.mobile-canvas \.seconds-page\s*\{[\s\S]*?--seconds-page: #000000;[\s\S]*?--seconds-card-surface: #0c100e;/)
  assert.match(secondsCss, /\.seconds-submit\s*\{[\s\S]*?background: var\(--seconds-signal\) !important;/)
  assert.doesNotMatch(secondsSource, /01842|t\('seconds\.currentRound'\)/)
  assert.match(secondsSource, /nearestSelectedActiveOrder[\s\S]*?t\('seconds\.activeRoundStatus'/)
})

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

function styleOf(fileSource: string): string {
  const match = fileSource.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)
  assert.ok(match, 'expected a scoped style block')
  return match[1] || ''
}

function assertOrdered(fileSource: string, markers: string[]): void {
  let cursor = -1
  for (const marker of markers) {
    const next = fileSource.indexOf(marker, cursor + 1)
    assert.ok(next > cursor, `expected ${marker} after previous marker`)
    cursor = next
  }
}
