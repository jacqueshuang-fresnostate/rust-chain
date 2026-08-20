import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const tradeSource = source('../src/views/TradeView.vue')
const tradeCss = styleOf(tradeSource)
const orderBookSource = source('../src/components/OrderBookPanel.vue')
const appSource = source('../src/App.vue')
const routerSource = source('../src/router/index.ts')
const baseCss = source('../src/styles/base.css')
const modalDialogSource = source('../src/core/modalDialog.ts')

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

test('现货钱包持有资产归属当前持仓面板，委托与历史仅导航到权威订单页', () => {
  const spotStart = tradeSource.indexOf('    <template v-if="isSpotMode">')
  const contractStart = tradeSource.indexOf('    <template v-else>', spotStart)
  assert.ok(spotStart >= 0 && contractStart > spotStart)
  const spotTemplate = tradeSource.slice(spotStart, contractStart)
  const accountStart = spotTemplate.indexOf('      <div class="spot-account-workspace"')
  const accountEnd = spotTemplate.indexOf('      <button\n        class="spot-chart-entry"', accountStart)
  assert.ok(accountStart >= 0 && accountEnd > accountStart)
  const accountWorkspace = spotTemplate.slice(accountStart, accountEnd)
  const accountNav = accountWorkspace.match(/<nav class="spot-account-tabs"[\s\S]*?<\/nav>/)?.[0]
  assert.ok(accountNav)

  const ordersEntry = accountNav.match(/<button type="button" @click="openOrders\('spot'\)">[\s\S]*?<\/button>/)?.[0]
  assert.ok(ordersEntry)
  assert.match(ordersEntry, /t\('trade\.orders'\)/)
  assert.doesNotMatch(ordersEntry, /class="active"|aria-current/)

  const holdingsEntry = accountNav.match(/<span\s+id="spot-holdings-label"[\s\S]*?<\/span>/)?.[0]
  assert.ok(holdingsEntry)
  assert.match(holdingsEntry, /class="spot-account-current active"/)
  assert.match(holdingsEntry, /aria-current="true"/)
  assert.match(holdingsEntry, /t\('orders\.positions'\)/)
  assert.doesNotMatch(holdingsEntry, /@click|openOrders|ChevronDown|aria-controls|role="tab"/)
  assert.doesNotMatch(accountNav, /<button[^>]+id="spot-holdings-label"/)
  assert.match(accountNav, /:aria-label="t\('trade\.orderHistory'\)" @click="openOrders\('history'\)"/)

  assert.match(accountWorkspace, /id="spot-holdings-panel"[\s\S]*?role="region"[\s\S]*?aria-labelledby="spot-holdings-label"/)
  assert.match(accountWorkspace, /class="spot-holdings-context"[\s\S]*?t\('trade\.onlyCurrent'\)[\s\S]*?@click="openAssets"[^>]*>\{\{ t\('common\.viewAll'\) \}\}/)
  assert.match(accountWorkspace, /v-if="balancesLoading" class="spot-account-state" role="status"/)
  assert.match(accountWorkspace, /v-else-if="balancesError" class="spot-account-state" role="alert"/)
  assert.match(accountWorkspace, /v-else-if="session\.isAuthenticated && spotVisibleBalances\.length" class="spot-balance-preview"/)
  assert.match(accountWorkspace, /v-for="wallet in spotVisibleBalances"/)
  assert.match(accountWorkspace, /v-else class="spot-account-state"[\s\S]*?class="spot-account-actions"/)
  assert.doesNotMatch(accountWorkspace, /orders\.cancelAll|openOrders\('positions'\)|trade\.positionsAndAssets/)
  assert.doesNotMatch(spotTemplate, /openOrders\('positions'\)/)

  assert.match(tradeSource, /const spotVisibleBalances = computed\(\(\) => spotWallets\.value\.filter\(\(wallet\) => \([\s\S]*?\[baseAsset\.value, quoteAsset\.value\]\.includes\(wallet\.symbol\)[\s\S]*?wallet\.available \+ wallet\.frozen \+ wallet\.locked > 0/)
  assert.match(tradeSource, /else \{\s*const wallets = await fetchWalletAccounts\(\)\s*if \(!isCurrentTradingBalancesRequest[\s\S]*?spotWallets\.value = wallets\s*marginPositions\.value = \[\]/)
  const tradingImport = tradeSource.match(/import \{[\s\S]*?\} from '@\/api\/trading'/)?.[0]
  assert.ok(tradingImport)
  assert.doesNotMatch(tradingImport, /\b(?:fetchSpotOrders|fetchOpenSpotOrders|fetchSpotOrderHistory|cancelSpotOrder|cancelAllSpotOrders|cancelAllMarginPositions)\b/)
  assert.doesNotMatch(tradeSource, /from ['"]@\/api\/orders['"]/)

  assert.match(functionBody('openOrders'), /router\.push\(\{ name: 'orders', query: \{ tab \} \}\)/)
  assert.match(routerSource, /\{ path: '\/orders', name: 'orders', component: OrdersView/)
  assert.match(tradeSource, /function openAssets\(\): void \{[\s\S]*?redirect: '\/assets'[\s\S]*?router\.push\(\{ name: 'assets' \}\)[\s\S]*?\n\}/)
  assert.deepEqual({ zh: zhCN.trade.onlyCurrent, en: en.trade.onlyCurrent }, {
    zh: '只看当前交易对',
    en: 'Current pair only',
  })
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
  assert.match(tradeCss, /\.spot-trade \.spot-type-field\s*\{\s*min-height: 44px;/)
  assert.match(tradeCss, /\.spot-field-shell\s*\{[\s\S]*?height: 44px;/)
  assert.match(tradeCss, /\.spot-submit-order\s*\{[\s\S]*?height: 46px;/)
  assert.match(tradeCss, /@media \(max-width: 340px\)[\s\S]*?grid-template-columns: minmax\(0, 1fr\) 124px;/)
  assert.match(orderBookSource, /layout\?: 'stacked' \| 'split' \| 'paired' \| 'matrix' \| 'mini'/)
  assert.match(orderBookSource, /const miniAsks = computed\(\(\) => props\.asks\.slice\(0, 5\)\.reverse\(\)\)/)
  assert.match(orderBookSource, /const miniBids = computed\(\(\) => props\.bids\.slice\(0, 5\)\)/)
  assert.match(tradeCss, /\.spot-market-data__tabs button\s*\{[\s\S]*?min-height: 50px;[\s\S]*?min-width: 44px;/)
  assert.match(cssRule('.spot-account-workspace'), /border-top: 1px solid var\(--line\);/)
  assert.match(cssRule('.spot-account-tabs'), /min-height: 48px;/)
  assert.match(cssRule('.spot-account-tabs :is(button, .spot-account-current)'), /min-height: 44px;/)
  assert.match(cssRule('.spot-holdings-panel'), /min-height: 232px;/)
  assert.match(cssRule('.spot-holdings-context'), /height: 34px;[\s\S]*?min-height: 34px;/)
  assert.match(cssRule('.spot-holdings-context button'), /min-height: 44px;[\s\S]*?min-width: 44px;/)
  assert.match(cssRule('.spot-account-state > button'), /min-height: 44px;/)
  const accountBorder = cssRule('.spot-account-workspace').match(/border-top: (\d+)px/)?.[1]
  const tabsHeight = cssRule('.spot-account-tabs').match(/min-height: (\d+)px/)?.[1]
  const panelHeight = cssRule('.spot-holdings-panel').match(/min-height: (\d+)px/)?.[1]
  const contextHeight = cssRule('.spot-holdings-context').match(/height: (\d+)px/)?.[1]
  const stateHeight = cssRule('.spot-account-state').match(/min-height: (\d+)px/)?.[1]
  const balancesHeight = cssRule('.spot-balance-preview').match(/min-height: (\d+)px/)?.[1]
  assert.ok(accountBorder && tabsHeight && panelHeight && contextHeight && stateHeight && balancesHeight)
  assert.equal(Number(panelHeight), Number(contextHeight) + Number(stateHeight))
  assert.equal(Number(panelHeight), Number(contextHeight) + Number(balancesHeight))
  assert.equal(Number(accountBorder) + Number(tabsHeight) + Number(panelHeight), 281)
  assert.match(tradeCss, /\.spot-field-shell:focus-within\s*\{[\s\S]*?border-color: var\(--focus\);[\s\S]*?box-shadow: 0 0 0 3px var\(--focus-ring\);/)
  assert.match(tradeCss, /\.spot-field-shell input:focus-visible\s*\{[\s\S]*?box-shadow: none;[\s\S]*?outline: 0;/)
  assert.doesNotMatch(tradeCss, /\.spot-pencil-workspace :is\(button, input\):focus-visible/)
  assert.match(baseCss, /\.sr-only\s*\{[\s\S]*?clip-path: inset\(50%\);[\s\S]*?position: absolute;/)
  assert.doesNotMatch(tradeCss, /width:\s*100vw|overflow-x:\s*auto/)
})

test('现货订单类型触发器只打开选择层，显式选择才更改表单和下单参数', () => {
  const openBody = functionBody('openSpotOrderTypeSheet')
  const closeBody = functionBody('closeSpotOrderTypeSheet')
  const selectBody = functionBody('selectSpotOrderType')

  assert.match(openBody, /spotOrderTypeOpen\.value = true/)
  assert.doesNotMatch(openBody, /orderType\.value/)
  assert.match(closeBody, /spotOrderTypeOpen\.value = false/)
  assert.doesNotMatch(closeBody, /orderType\.value/)
  assert.match(selectBody, /orderType\.value = type[\s\S]*?closeSpotOrderTypeSheet\(\)/)
  assert.doesNotMatch(selectBody, /(?:price|quantity|percentage)\.value\s*=/)
  assert.doesNotMatch(tradeSource, /toggleSpotOrderType/)

  assert.match(tradeSource, /class="spot-type-field"[\s\S]*?:aria-expanded="spotOrderTypeOpen"[\s\S]*?aria-controls="spot-order-type-dialog"[\s\S]*?@click="openSpotOrderTypeSheet"/)
  assert.match(tradeSource, /@click="selectSpotOrderType\('limit'\)"/)
  assert.match(tradeSource, /@click="selectSpotOrderType\('market'\)"/)
  assert.match(tradeSource, /const selectedOrderType = computed\(\(\) => mode\.value === 'contract' \? contractOrderType\.value : orderType\.value\)/)
  assert.match(tradeSource, /const effectivePrice = computed\(\(\) => \{[\s\S]*?return orderType\.value === 'limit' \? Number\(price\.value\) : currentPrice\.value/)
  assert.match(tradeSource, /:readonly="orderType === 'market'"/)
  assert.match(tradeSource, /const submittedMode = mode\.value[\s\S]*?const submittedOrderType = orderType\.value/)
  assert.match(tradeSource, /placeSpotOrder\(\{[\s\S]*?type: submittedOrderType,[\s\S]*?price: limitPrice,[\s\S]*?quantity: orderAmount,/)
  const modeWatch = tradeSource.match(/watch\(\(\) => route\.query\.mode,[\s\S]*?\}, \{ immediate: true \}\)/)?.[0]
  assert.ok(modeWatch)
  assert.match(modeWatch, /if \(mode\.value === 'contract'\) closeSpotOrderTypeSheet\(\)/)
  assert.doesNotMatch(modeWatch, /orderType\.value\s*=/)
})

test('现货订单类型层 Teleport 到 body，三种取消路径均不改变当前选择', () => {
  const teleported = tradeSource.match(/<Teleport to="body">([\s\S]*?class="spot-order-type-layer"[\s\S]*?)<\/Teleport>/)?.[1]
  assert.ok(teleported)
  assert.match(teleported, /v-if="isSpotMode && spotOrderTypeOpen"/)
  assert.match(teleported, /class="spot-order-type-overlay"[\s\S]*?tabindex="-1"[\s\S]*?@click="closeSpotOrderTypeSheet"/)
  assert.match(teleported, /class="spot-order-type-sheet__close"[\s\S]*?@click="closeSpotOrderTypeSheet"/)
  assert.match(teleported, /@keydown="handleSpotOrderTypeKeydown"/)
  assert.doesNotMatch(teleported, /confirmOpen|confirmation-sheet|submitOrder/)
  assert.match(tradeSource, /function handleSpotOrderTypeKeydown\(event: KeyboardEvent\): void \{\s*trapSpotOrderTypeFocus\(event, closeSpotOrderTypeSheet\)\s*\}/)
  assert.match(modalDialogSource, /if \(event\.key === 'Escape'\) \{[\s\S]*?close\(\)/)

  const dismissCode = [
    functionBody('closeSpotOrderTypeSheet'),
    functionBody('handleSpotOrderTypeKeydown'),
    teleported,
  ].join('\n')
  assert.doesNotMatch(dismissCode, /orderType\.value\s*=/)

  assert.match(tradeSource, /<Teleport to="body">\s*<div\s+v-if="confirmOpen"[\s\S]*?<section\s+v-if="isSpotMode"\s+ref="confirmDialog"\s+class="confirmation-sheet"[\s\S]*?@keydown="trapDialogFocus"/)
  assert.match(functionBody('openSpotOrderTypeSheet'), /if \(confirmOpen\.value\) return/)
  assert.match(functionBody('reviewOrder'), /if \(spotOrderTypeOpen\.value\) return/)
  assert.match(tradeSource, /useModalDialog\(confirmOpen, confirmDialog, '\[data-dialog-cancel\]'\)/)
  assert.match(modalDialogSource, /onBeforeUnmount\(\(\) => \{[\s\S]*?document\.body\.style\.overflow = previousBodyOverflow/)
})

test('现货订单类型层复用共享焦点合同，具备完整对话框和选中语义', () => {
  assert.match(tradeSource, /useModalDialog\(\s*spotOrderTypeOpen,\s*spotOrderTypeDialog,\s*'\[data-order-type-current="true"\]',\s*\)/)
  assert.match(tradeSource, /aria-haspopup="dialog"/)
  assert.match(tradeSource, /role="dialog"[\s\S]*?aria-modal="true"[\s\S]*?aria-labelledby="spot-order-type-title"[\s\S]*?aria-describedby="spot-order-type-hint"/)
  assert.match(tradeSource, /id="spot-order-type-title"[\s\S]*?t\('trade\.orderTypeSheetTitle'\)/)
  assert.match(tradeSource, /id="spot-order-type-hint"[\s\S]*?t\('trade\.orderTypeSheetHint'\)/)
  assert.equal(tradeSource.match(/:aria-pressed="orderType === '(?:limit|market)'"/g)?.length, 2)
  assert.equal(tradeSource.match(/:data-order-type-current="orderType === '(?:limit|market)'"/g)?.length, 2)
  assert.match(tradeSource, /:class="\{ active: orderType === 'limit' \}"/)
  assert.match(tradeSource, /:class="\{ active: orderType === 'market' \}"/)
  assert.match(tradeSource, /<CheckCircle2 v-if="orderType === 'limit'"/)
  assert.match(tradeSource, /<CheckCircle2 v-if="orderType === 'market'"/)

  assert.match(modalDialogSource, /previousBodyOverflow = document\.body\.style\.overflow[\s\S]*?document\.body\.style\.overflow = 'hidden'/)
  assert.match(modalDialogSource, /document\.body\.style\.overflow = previousBodyOverflow[\s\S]*?returnFocus\?\.focus\(\)/)
  assert.match(modalDialogSource, /event\.shiftKey && document\.activeElement === first[\s\S]*?last\.focus\(\)[\s\S]*?document\.activeElement === last[\s\S]*?first\.focus\(\)/)
})

test('现货订单类型层保持 44px 触摸目标、安全区、可见焦点和明暗主题语义', () => {
  const layerRule = cssRule('.spot-order-type-layer')
  const sheetRule = cssRule('.spot-order-type-sheet')
  const closeRule = cssRule('.spot-order-type-sheet__close')
  const optionRule = cssRule('.spot-order-type-options > button')
  const focusRule = cssRule('.spot-order-type-layer button:focus-visible')
  const sheetCss = tradeCss.slice(
    tradeCss.indexOf('.spot-order-type-layer {'),
    tradeCss.indexOf('.spot-field-shell {'),
  )

  assert.match(layerRule, /\bposition: fixed;/)
  assert.match(layerRule, /\binset: 0;/)
  assert.match(layerRule, /\bheight: 100vh;/)
  assert.match(layerRule, /\bheight: 100dvh;/)
  assert.match(layerRule, /\bwidth: 100%;/)
  assert.match(layerRule, /\boverflow: hidden;/)
  assert.match(layerRule, /\boverscroll-behavior: contain;/)
  assert.match(sheetRule, /\bbox-sizing: border-box;/)
  assert.match(sheetRule, /\bmax-width: 448px;/)
  assert.match(sheetRule, /\bwidth: 100%;/)
  assert.match(sheetRule, /max-height: calc\(100dvh - max\(16px, env\(safe-area-inset-top\)\)\);/)
  assert.match(sheetRule, /padding:[\s\S]*?env\(safe-area-inset-right\)[\s\S]*?env\(safe-area-inset-bottom\)[\s\S]*?env\(safe-area-inset-left\)/)
  assert.match(closeRule, /\bheight: 44px;/)
  assert.match(closeRule, /\bwidth: 44px;/)
  assert.match(optionRule, /\bmin-height: 64px;/)
  assert.match(focusRule, /outline: 2px solid var\(--focus\);[\s\S]*?outline-offset: 2px;/)
  assert.match(tradeCss, /\.spot-type-field\s*\{[\s\S]*?height: 44px;/)
  assert.match(tradeCss, /\.spot-type-field\[aria-expanded='true'\][\s\S]*?box-shadow: 0 0 0 3px var\(--focus-ring\);/)

  for (const token of ['surface-elevated', 'field-surface', 'ink', 'muted-strong', 'positive-soft', 'overlay', 'focus']) {
    assert.match(sheetCss, new RegExp(`var\\(--${token}\\)`), `missing theme token --${token}`)
  }
  assert.doesNotMatch(sheetCss, /#[\da-f]{3,8}\b|\brgba?\(/i)
  assert.doesNotMatch(sheetCss, /\b100vw\b|overflow-x:\s*auto/)
  for (const selector of [":root,\n:root[data-theme='light']", ":root[data-theme='dark']"]) {
    const themeRule = cssRule(selector, baseCss)
    for (const token of ['surface-elevated', 'field-surface', 'ink', 'muted-strong', 'positive-soft', 'overlay', 'focus']) {
      assert.match(themeRule, new RegExp(`--${token}:`), `${selector} missing --${token}`)
    }
  }

  const reducedMotionRule = tradeCss.match(/@media \(prefers-reduced-motion: reduce\) \{([\s\S]*?)\n\}/)?.[1]
  assert.ok(reducedMotionRule)
  assert.match(reducedMotionRule, /\.spot-order-type-layer \*/)
  assert.match(reducedMotionRule, /\.spot-order-type-layer button:active[\s\S]*?transform: none;/)

  const compiled = compileStyle({
    source: tradeCss,
    filename: 'TradeView.vue',
    id: 'data-v-spot-order-type',
    scoped: true,
  })
  assert.deepEqual(compiled.errors, [])
  assert.match(compiled.code, /\.spot-order-type-layer\[data-v-spot-order-type\]\s*\{[\s\S]*?position: fixed;/)
  assert.match(compiled.code, /\.spot-order-type-sheet\[data-v-spot-order-type\]\s*\{[\s\S]*?max-width: 448px;/)
  assert.match(compiled.code, /\.spot-order-type-layer button\[data-v-spot-order-type\]:focus-visible/)
  assert.match(compiled.code, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.spot-order-type-layer\[data-v-spot-order-type\]/)
  assert.doesNotMatch(compiled.code, /\.trade-view[^,{]*\.spot-order-type-layer/)
})

test('新增现货状态文案保持中英文键一致', () => {
  for (const key of ['liveMarket', 'restAndSocket', 'depthLive', 'klineLive', 'depthSnapshot', 'klineSnapshot', 'tradeTime', 'noRecentTrades', 'limitOrderShort', 'marketOrderShort', 'orderTypeTrigger', 'orderTypeSheetTitle', 'orderTypeSheetHint', 'limitOrderDescription', 'marketOrderDescription', 'turnover', 'available', 'onlyCurrent', 'spotAssetEmpty', 'spotAssetEmptyHint'] as const) {
    assert.equal(typeof zhCN.trade[key], 'string')
    assert.equal(typeof en.trade[key], 'string')
    assert.ok(zhCN.trade[key].length > 0)
    assert.ok(en.trade[key].length > 0)
  }
  assert.deepEqual(Object.keys(zhCN.trade).sort(), Object.keys(en.trade).sort())
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

function functionBody(name: string): string {
  const match = tradeSource.match(new RegExp(`function ${name}\\([^)]*\\): void \\{([\\s\\S]*?)\\n\\}`))
  assert.ok(match, `missing function ${name}`)
  return match[1] || ''
}

function cssRule(selector: string, css = tradeCss): string {
  const match = css.match(new RegExp(`${escapeRegExp(selector)}\\s*\\{([\\s\\S]*?)\\}`))
  assert.ok(match, `missing CSS rule ${selector}`)
  return match[1] || ''
}
