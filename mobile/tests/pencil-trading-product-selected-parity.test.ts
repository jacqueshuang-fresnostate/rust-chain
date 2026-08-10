import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'
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
const baseCss = read('../src/styles/base.css')

function styleOf(source: string): string {
  return source.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)?.[1] || ''
}

function blockOf(source: string, marker: string): string {
  const markerIndex = source.indexOf(marker)
  assert.notEqual(markerIndex, -1, `missing block marker: ${marker}`)
  const openIndex = source.indexOf('{', markerIndex)
  assert.notEqual(openIndex, -1, `missing opening brace: ${marker}`)

  let depth = 0
  for (let index = openIndex; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1
    if (source[index] !== '}') continue
    depth -= 1
    if (depth === 0) return source.slice(openIndex + 1, index)
  }

  assert.fail(`missing closing brace: ${marker}`)
}

function replaceExactlyOnce(source: string, current: string, prior: string, label: string): string {
  const first = source.indexOf(current)
  assert.notEqual(first, -1, `missing normalization source: ${label}`)
  assert.equal(source.indexOf(current, first + current.length), -1, `duplicate normalization source: ${label}`)
  return `${source.slice(0, first)}${prior}${source.slice(first + current.length)}`
}

test('现货 yzOPc/bo8k5 模板仅定向调整订单类型入口与持仓归属，合约仍使用独立分支', () => {
  const spotStart = tradeSource.indexOf('    <template v-if="isSpotMode">')
  const contractStart = tradeSource.indexOf('    <template v-else>', spotStart)
  const orderTypeTeleportStart = tradeSource.indexOf('    <Teleport to="body">', contractStart)
  assert.ok(spotStart >= 0 && contractStart > spotStart)
  assert.ok(orderTypeTeleportStart > contractStart)

  const spotTemplate = tradeSource.slice(spotStart, contractStart)
  const contractTemplate = tradeSource.slice(contractStart, orderTypeTeleportStart)
  const currentTrigger = spotTemplate.match(/          <button\n            class="spot-type-field"[\s\S]*?          <\/button>/)?.[0]
  assert.ok(currentTrigger)
  const priorTrigger = `          <button
            class="spot-type-field"
            type="button"
            :aria-label="t('trade.category')"
            @click="toggleSpotOrderType"
          >
            <Info :size="14" aria-hidden="true" />
            <strong>{{ orderType === 'limit' ? t('trade.limitOrderShort') : t('trade.marketOrderShort') }}</strong>
            <ChevronDown :size="15" aria-hidden="true" />
          </button>`
  const accountStart = spotTemplate.indexOf('      <div class="spot-account-workspace"')
  const accountEnd = spotTemplate.indexOf('      <button\n        class="spot-chart-entry"', accountStart)
  assert.ok(accountStart >= 0 && accountEnd > accountStart)
  const currentAccountWorkspace = spotTemplate.slice(accountStart, accountEnd)
  const currentHoldingsEntry = `          <span
            id="spot-holdings-label"
            class="spot-account-current active"
            aria-current="true"
          >
            {{ t('orders.positions') }}
          </span>`
  const currentPanelOpening = `        <section
          id="spot-holdings-panel"
          class="spot-holdings-panel"
          role="region"
          aria-labelledby="spot-holdings-label"
        >
`
  const currentContext = `          <div class="spot-holdings-context">
            <span><i aria-hidden="true" />{{ t('trade.onlyCurrent') }}</span>
            <button type="button" @click="openAssets">{{ t('common.viewAll') }}</button>
          </div>`
  const statesStart = currentAccountWorkspace.indexOf('          <div v-if="balancesLoading"')
  const statesEnd = currentAccountWorkspace.lastIndexOf('        </section>\n      </div>\n\n')
  assert.ok(statesStart >= 0 && statesEnd > statesStart)
  const currentStateBranches = currentAccountWorkspace.slice(statesStart, statesEnd)
  let normalizedAccountWorkspace = replaceExactlyOnce(
    currentAccountWorkspace,
    '      <div class="spot-account-workspace">',
    `      <section class="spot-account-workspace" :aria-label="t('trade.positionsAndAssets')">`,
    'account workspace element',
  )
  normalizedAccountWorkspace = replaceExactlyOnce(
    normalizedAccountWorkspace,
    `          <button type="button" @click="openOrders('spot')">`,
    `          <button class="active" type="button" @click="openOrders('spot')">`,
    'orders entry state',
  )
  normalizedAccountWorkspace = replaceExactlyOnce(
    normalizedAccountWorkspace,
    currentHoldingsEntry,
    `          <button type="button" @click="openOrders('positions')">
            {{ t('trade.positionsAndAssets') }} <ChevronDown :size="12" aria-hidden="true" />
          </button>`,
    'current holdings label',
  )
  normalizedAccountWorkspace = replaceExactlyOnce(
    normalizedAccountWorkspace,
    currentPanelOpening,
    '',
    'holdings panel opening',
  )
  normalizedAccountWorkspace = replaceExactlyOnce(
    normalizedAccountWorkspace,
    currentContext,
    `        <div class="spot-order-filter">
          <span><i aria-hidden="true" />{{ t('trade.onlyCurrent') }}</span>
          <button type="button" @click="openOrders('spot')">{{ t('orders.cancelAll') }}</button>
        </div>`,
    'holdings context row',
  )
  normalizedAccountWorkspace = replaceExactlyOnce(
    normalizedAccountWorkspace,
    currentStateBranches,
    currentStateBranches.replace(/^ {2}/gm, ''),
    'wallet state indentation',
  )
  normalizedAccountWorkspace = replaceExactlyOnce(
    normalizedAccountWorkspace,
    '\n        </section>\n      </div>\n\n',
    '\n      </section>\n\n',
    'holdings panel closing',
  )
  let normalizedSpotTemplate = replaceExactlyOnce(spotTemplate, currentTrigger, priorTrigger, 'order type trigger')
  normalizedSpotTemplate = replaceExactlyOnce(
    normalizedSpotTemplate,
    currentAccountWorkspace,
    normalizedAccountWorkspace,
    'account workspace',
  )
  const priorSpotDigest = createHash('sha256').update(normalizedSpotTemplate).digest('hex')

  assert.equal(priorSpotDigest, '7b3247272adfe69a374bc64452faec8d0ca41367ecc85ecdec7fc6f9436dc444')
  assert.match(spotTemplate, /data-pencil-source="yzOPc-bo8k5"/)
  assert.match(currentTrigger, /:aria-label="t\('trade\.orderTypeTrigger'/)
  assert.match(currentTrigger, /aria-haspopup="dialog"[\s\S]*?:aria-expanded="spotOrderTypeOpen"[\s\S]*?aria-controls="spot-order-type-dialog"[\s\S]*?@click="openSpotOrderTypeSheet"/)
  assert.match(currentAccountWorkspace, /id="spot-holdings-label"[\s\S]*?aria-current="true"[\s\S]*?t\('orders\.positions'\)/)
  assert.doesNotMatch(currentHoldingsEntry, /<button|aria-controls|role="tab"/)
  assert.match(currentAccountWorkspace, /id="spot-holdings-panel"[\s\S]*?aria-labelledby="spot-holdings-label"/)
  assert.match(currentAccountWorkspace, /t\('trade\.onlyCurrent'\)[\s\S]*?@click="openAssets"[^>]*>\{\{ t\('common\.viewAll'\) \}\}/)
  assert.doesNotMatch(currentAccountWorkspace, /orders\.cancelAll|openOrders\('positions'\)/)
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
  assert.match(secondsSource, /const activeOrders = computed\(\(\) => activeSecondsOrders\(orders\.value\)\)/)
  assert.match(secondsSource, /:data-seconds-state="activeOrders\.length \? 'active' : 'default'"/)
  assert.doesNotMatch(secondsSource, /secondary-view|secondary-content|page--prototype-grid/)
  assert.match(secondsSource, /v-if="activeOrders\.length"[\s\S]*?data-active-order-list="all"[\s\S]*?v-for="order in activeOrders"/)
  assert.match(secondsSource, /fetchSecondsProducts\(\)/)
  assert.match(secondsSource, /fetchSecondsOrders\(100\)/)
  assert.match(secondsSource, /fetchWalletAccounts\(\)/)
  assert.match(secondsSource, /order\.entryPrice !== undefined \? formatPrice\(order\.entryPrice\) : '--'/)
  assert.match(secondsSource, /return secondsOrderEstimatedProfit\(order\)/)
  assert.match(secondsSource, /orders\.value = upsertSecondsOrder\(orders\.value, openedOrder\)/)
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

test('秒合约确认层 Teleport 到 body 并将固定操作区与可滚动明细分离', () => {
  const css = styleOf(secondsSource)
  const teleportedLayer = secondsSource.match(/<Teleport to="body">([\s\S]*?)<\/Teleport>/)?.[1]
  assert.ok(teleportedLayer)
  assert.match(teleportedLayer, /<div v-if="confirmOpen && selected && cycle" class="confirmation-layer seconds-mask" @click\.self="closeConfirm">/)
  assert.match(teleportedLayer, /ref="confirmDialog"[\s\S]*?role="dialog"[\s\S]*?aria-modal="true"[\s\S]*?@keydown="trapDialogFocus"/)

  const bodyBoundary = teleportedLayer.match(
    /<\/header>\s*<div class="seconds-dialog__body">([\s\S]*?)<\/div>\s*<div class="confirmation-actions dialog-actions">/,
  )
  assert.ok(bodyBoundary)
  assert.match(bodyBoundary[1], /<p id="seconds-confirm-summary">/)
  assert.match(bodyBoundary[1], /<dl class="confirmation-detail">/)
  assert.match(bodyBoundary[1], /<p v-if="error" class="dialog-feedback" role="alert">/)
  assert.doesNotMatch(bodyBoundary[1], /confirmation-actions|dialog-actions/)

  assert.match(secondsSource, /function closeConfirm\(\): void \{\s*if \(submitting\.value\) return\s*confirmOpen\.value = false\s*\}/)
  assert.match(secondsSource, /if \(event\.key === 'Escape'\) \{[\s\S]*?event\.preventDefault\(\)[\s\S]*?closeConfirm\(\)[\s\S]*?return\s*\}/)
  assert.match(secondsSource, /if \(event\.key !== 'Tab' \|\| !confirmDialog\.value\) return/)
  assert.match(secondsSource, /event\.shiftKey && document\.activeElement === first[\s\S]*?last\.focus\(\)[\s\S]*?document\.activeElement === last[\s\S]*?first\.focus\(\)/)
  assert.match(secondsSource, /previousBodyOverflow = document\.body\.style\.overflow[\s\S]*?document\.body\.style\.overflow = 'hidden'[\s\S]*?\[data-dialog-cancel\][\s\S]*?\.focus\(\)/)
  assert.match(secondsSource, /document\.body\.style\.overflow = previousBodyOverflow[\s\S]*?await nextTick\(\)[\s\S]*?returnFocus\?\.focus\(\)[\s\S]*?returnFocus = null/)
  assert.match(secondsSource, /onBeforeUnmount\(\(\) => \{[\s\S]*?document\.body\.style\.overflow = previousBodyOverflow[\s\S]*?\}\)/)

  assert.match(css, /(?:^|\n)\.seconds-mask\s*\{/)
  assert.doesNotMatch(css, /\.seconds-page\s+\.seconds-mask\s*\{/)

  const maskRule = blockOf(css, '.seconds-mask {')
  assert.match(maskRule, /--page: var\(--background\);/)
  assert.match(maskRule, /--surface-2: var\(--soft\);/)
  assert.match(maskRule, /--text: var\(--ink\);/)
  assert.match(maskRule, /\bbackground: var\(--overlay\);/)
  assert.match(maskRule, /\bbox-sizing: border-box;/)
  assert.match(maskRule, /\bheight: 100dvh;/)
  assert.match(maskRule, /\binset: 0;/)
  assert.match(maskRule, /\bposition: fixed;/)
  assert.match(maskRule, /padding:[\s\S]*?env\(safe-area-inset-top\)[\s\S]*?env\(safe-area-inset-right\)[\s\S]*?env\(safe-area-inset-bottom\)[\s\S]*?env\(safe-area-inset-left\)/)

  for (const themeMarker of [":root[data-theme='light'] {", ":root[data-theme='dark'] {"]) {
    const themeRule = blockOf(baseCss, themeMarker)
    for (const token of ['background', 'surface', 'soft', 'ink', 'accent', 'focus', 'negative', 'overlay']) {
      assert.match(themeRule, new RegExp(`--${token}:`), `${themeMarker} must own --${token}`)
    }
  }

  const dialogRule = blockOf(css, '.seconds-dialog {')
  assert.match(dialogRule, /\bgrid-template-rows: auto minmax\(0, 1fr\) auto;/)
  assert.match(dialogRule, /\bmax-height: calc\(100dvh - max\(16px, env\(safe-area-inset-top\)\) - max\(16px, env\(safe-area-inset-bottom\)\)\);/)
  assert.match(dialogRule, /\boverflow: hidden;/)
  assert.match(dialogRule, /\boverscroll-behavior: auto;/)
  assert.doesNotMatch(dialogRule, /overflow-y:\s*auto|overscroll-behavior:\s*contain/)

  const bodyRule = blockOf(css, '.seconds-dialog__body {')
  assert.match(bodyRule, /\bgap: 15px;/)
  assert.match(bodyRule, /\bmin-height: 0;/)
  assert.match(bodyRule, /\boverflow-x: hidden;/)
  assert.match(bodyRule, /\boverflow-y: auto;/)
  assert.match(bodyRule, /\boverscroll-behavior: contain;/)

  const actionsRule = blockOf(css, '.dialog-actions {')
  assert.match(actionsRule, /\bgrid-template-columns: minmax\(0, \.8fr\) minmax\(0, 1\.2fr\);/)
  const buttonRule = blockOf(css, '.dialog-actions .button {')
  assert.match(buttonRule, /\bmin-height: 48px;/)
  const narrowRule = blockOf(blockOf(css, '@media (max-width: 340px) {'), '.dialog-actions {')
  assert.match(narrowRule, /\bgrid-template-columns: 1fr;/)

  const compiled = compileStyle({
    source: css,
    filename: 'SecondsView.vue',
    id: 'data-v-seconds-confirm',
    scoped: true,
  })
  assert.deepEqual(compiled.errors, [])
  assert.match(compiled.code, /\.seconds-mask\[data-v-seconds-confirm\]\s*\{/)
  assert.match(compiled.code, /\.seconds-dialog\[data-v-seconds-confirm\]\s*\{[\s\S]*?overflow: hidden;[\s\S]*?overscroll-behavior: auto;/)
  assert.match(compiled.code, /\.seconds-dialog__body\[data-v-seconds-confirm\]\s*\{[\s\S]*?overflow-y: auto;[\s\S]*?overscroll-behavior: contain;/)
  assert.match(compiled.code, /\.seconds-dialog button\[data-v-seconds-confirm\]:focus-visible/)
  assert.doesNotMatch(compiled.code, /\.seconds-page[^,{]*\.seconds-mask/)
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
