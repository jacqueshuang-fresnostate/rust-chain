import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const tradeSource = source('../src/views/TradeView.vue')
const secondsSource = source('../src/views/SecondsView.vue')
const parityCss = source('../src/styles/prototype-parity.css')
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
  assert.match(secondsSource, /await openSecondsOrder\(\{[\s\S]*?productId: selected\.value\.id,[\s\S]*?durationSeconds: cycle\.value\.durationSeconds,[\s\S]*?direction: direction\.value,[\s\S]*?stakeAmount: amountNumber\.value,/)
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
    'class="field seconds-pair-field"',
    'class="seconds-direction-grid"',
    'class="seconds-duration-grid"',
    'class="field seconds-amount-field"',
    'class="seconds-order-summary"',
  ])
  assert.doesNotMatch(secondsSource, /seconds-session-records|seconds-orders|ordersSection|scrollToOrders/)

  assert.match(tradeSource, /<style\s+scoped\s*>/)
  assert.match(secondsSource, /<style\s+scoped\s*>/)
  assert.match(tradeCss, /\.trade-workspace\s*\{[\s\S]*?background: var\(--surface\);[\s\S]*?border-bottom: 1px solid var\(--line-strong\);/)
  assert.match(secondsCss, /\.seconds-workspace\s*\{[\s\S]*?gap: 0;/)
  assert.match(tradeCss, /border-radius: 0;/)
  assert.match(secondsCss, /border-radius: 0;/)
})

test('表单、切换、百分比与主按钮满足 44–52px 和完整聚焦环合同', () => {
  assert.match(tradeCss, /\.input-stack \.field-shell\s*\{[\s\S]*?height: 52px;[\s\S]*?min-height: 52px;/)
  assert.match(tradeCss, /\.input-stack \.field-shell input\s*\{[\s\S]*?min-height: 44px;/)
  assert.match(tradeCss, /\.side-switch button\s*\{[\s\S]*?min-height: 50px;/)
  assert.match(tradeCss, /\.amount-control input\[type="range"\]\s*\{[\s\S]*?height: 44px;[\s\S]*?min-height: 44px;/)
  assert.match(tradeCss, /\.percent-row button\s*\{[\s\S]*?min-height: 48px;[\s\S]*?min-width: 44px;/)
  assert.match(tradeCss, /\.submit-order\s*\{\s*min-height: 52px;/)
  assert.match(tradeCss, /\.input-stack \.field-shell:focus-within\s*\{[\s\S]*?border-color: var\(--focus\);[\s\S]*?box-shadow: 0 0 0 3px var\(--focus-ring\);/)

  assert.match(secondsCss, /\.seconds-select-shell\s*\{[\s\S]*?height: 44px;[\s\S]*?min-height: 44px;/)
  assert.match(secondsCss, /\.seconds-amount-field > div\s*\{[\s\S]*?height: 52px;[\s\S]*?min-height: 52px;/)
  assert.match(secondsCss, /\.seconds-direction-grid button\s*\{\s*min-height: 52px;/)
  assert.match(secondsCss, /\.seconds-submit\s*\{[\s\S]*?min-height: 52px;/)
  assert.match(secondsCss, /\.seconds-select-shell:focus-within\s*\{[\s\S]*?border-color: var\(--focus\);[\s\S]*?box-shadow: inset 0 0 0 1px var\(--focus\);/)
  assert.match(secondsCss, /\.seconds-amount-field:focus-within > div\s*\{[\s\S]*?border-color: var\(--focus\);[\s\S]*?box-shadow: 0 0 0 3px var\(--focus-ring\);/)

  assert.doesNotMatch(tradeCss, /box-shadow:\s*inset 3px 0/)
  assert.doesNotMatch(secondsCss, /box-shadow:\s*inset 3px 0/)
})

test('320–448px 响应式、安全区和低动态合同不产生工作区固定遮挡', () => {
  for (const width of [320, 360, 390, 448]) {
    const tradeInnerWidth = width - 32
    const secondsInnerWidth = width - 32
    assert.ok(tradeInnerWidth >= 5 * 44, `${width}px interval rail must fit five touch targets`)
    assert.ok(secondsInnerWidth >= 3 * 44, `${width}px duration rail must fit three touch targets`)
  }

  assert.match(tradeCss, /\.trade-quote > div:first-child strong\s*\{[\s\S]*?overflow-wrap: normal;[\s\S]*?white-space: nowrap;/)
  assert.match(tradeCss, /@media \(max-width: 340px\)\s*\{[\s\S]*?\.trade-quote\s*\{[\s\S]*?grid-template-columns: minmax\(0, 1fr\) minmax\(96px, \.62fr\);[\s\S]*?\.trade-quote > div:first-child strong\s*\{\s*font-size: 29px;/)

  for (const [css, sourceName] of [[tradeCss, 'trade'], [secondsCss, 'seconds']] as const) {
    assert.match(css, /overflow-x: clip;/, `${sourceName} should clip decorative x overflow`)
    assert.match(css, /env\(safe-area-inset-bottom\)/, `${sourceName} should reserve the bottom safe area`)
    assert.match(css, /@media \(max-width: 340px\)/, `${sourceName} should handle 320px layouts`)
    assert.match(css, /@media \(prefers-reduced-motion: reduce\)/, `${sourceName} should disable nonessential motion`)
    assert.doesNotMatch(css, /width:\s*100vw|overflow-x:\s*auto/)
    if (sourceName === 'trade') {
      assert.match(css, /--contract-bg: #f7f9f8;/)
      assert.match(css, /html\[data-theme='dark'\] \.contract-trade \{[\s\S]*?--contract-bg: #070a09;/)
      assert.match(css, /@media \(max-width: 359px\)[\s\S]*?grid-template-columns: minmax\(0, 1fr\) 132px;/)
    } else {
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

test('交易与秒合约使用首页薄荷主动作和连续面板层级', () => {
  assert.match(parityCss, /\.trade-view \.trade-instrument-hero\s*\{[\s\S]*?var\(--signal-green\)/)
  assert.match(parityCss, /\.trade-view \.submit-order,[\s\S]*?\.seconds-page \.seconds-submit\s*\{[\s\S]*?background:\s*var\(--accent\)/)
  assert.match(parityCss, /\.seconds-page \.seconds-market-board\s*\{[\s\S]*?var\(--signal-green\)/)
  assert.match(parityCss, /\.seconds-page \.seconds-market-board::after\s*\{\s*content:\s*none;\s*\}/)
  assert.doesNotMatch(secondsSource, /seconds-round-row|t\('seconds\.currentRound'\)/)
  assert.match(parityCss, /\.seconds-page \.seconds-order-console\s*\{[\s\S]*?border-radius:\s*0/)
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
