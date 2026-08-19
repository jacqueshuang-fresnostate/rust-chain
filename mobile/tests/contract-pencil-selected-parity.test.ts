import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const tradeSource = read('../src/views/TradeView.vue')
const sheetsSource = read('../src/components/ContractTradeSheets.vue')
const orderBookSource = read('../src/components/OrderBookPanel.vue')
const tradingApiSource = read('../src/api/trading.ts')
const ordersSource = read('../src/views/OrdersView.vue')

test('当前八张 Pencil 杠杆画板均由生产页面声明', () => {
  assert.match(tradeSource, /data-pencil-source="cjzfi p6GfgT"/)
  assert.match(sheetsSource, /data-pencil-source="f0L8yf R8t0p aNuw6 PKAcD Crw8v YuKtQ"/)
})

test('杠杆主页面使用后台产品图标并在当前页打开四个选择弹层', () => {
  assert.match(tradeSource, /<AssetMark :symbol="baseAsset" :src="selectedProduct\?\.logoUrl \|\| ticker\?\.iconUrl" :fallback-src="ticker\?\.baseIconUrl" :size="28"/)
  assert.match(tradeSource, /@click="openContractSheet\('pair'\)"/)
  assert.match(tradeSource, /@click="openContractSheet\('marginMode'\)"/)
  assert.match(tradeSource, /@click="openContractSheet\('leverage'\)"/)
  assert.match(tradeSource, /@click="openContractSheet\('orderType'\)"/)
  assert.match(tradeSource, /<ContractTradeSheets[\s\S]*?:product="selectedProduct"[\s\S]*?:products="products"/)
  assert.match(tradeSource, /@select-pair="selectContractPair"/)
  assert.match(tradeSource, /router\.replace\(\{[\s\S]*?name: 'trade',[\s\S]*?query: \{ mode: 'contract' \}/)
  assert.match(tradeSource, /t\('trade\.bestBidOffer'\)/)
  assert.match(tradeSource, /t\('trade\.hourlyInterestAndCycle'\)/)
  assert.match(tradeSource, /:mini-ask-levels="6"/)
  assert.match(tradeSource, /:mini-bid-levels="7"/)
  assert.match(tradeSource, /:mini-precision="contractBookPrecision"/)
  assert.match(tradeSource, /:show-mini-precision="true"/)
  assert.match(orderBookSource, /miniAskLevels\?: number/)
  assert.match(orderBookSource, /miniBidLevels\?: number/)
  assert.match(orderBookSource, /miniPrecision\?: string/)
  assert.match(orderBookSource, /showMiniPrecision\?: boolean/)
  assert.match(orderBookSource, /renderedMiniAsks/)
  assert.match(orderBookSource, /renderedMiniBids/)
  assert.doesNotMatch(tradeSource, /@click="changeLeverage"/)
})

test('杠杆主页面保留所选 Pencil 主结构并升级金融操作触控尺寸', () => {
  assert.match(tradeSource, /\.contract-pencil-header \{[^}]*height: calc\(58px \+ env\(safe-area-inset-top\)\);[^}]*padding: env\(safe-area-inset-top\) 14px 0;/s)
  assert.match(tradeSource, /\.contract-pencil-module \{[^}]*gap: 10px;[^}]*grid-template-columns: 202px minmax\(150px, 1fr\);[^}]*height: 460px;[^}]*padding: 2px 14px 8px;/s)
  assert.match(tradeSource, /\.contract-open-close \{[^}]*height: 30px;[^}]*padding: 2px;[^}]*top: 0;/s)
  assert.match(tradeSource, /\.contract-mode-row \{[^}]*grid-template-columns: 54px 48px minmax\(0, 1fr\);[^}]*height: 32px;[^}]*top: 36px;/s)
  assert.match(tradeSource, /\.contract-price-row \{[^}]*grid-template-columns: minmax\(0, 138px\) 58px;[^}]*height: 56px;[^}]*top: 74px;/s)
  assert.match(tradeSource, /\.contract-amount-field \{[^}]*height: 46px;[^}]*top: 136px;/s)
  assert.match(tradeSource, /\.contract-percentage \{[^}]*height: 32px;[^}]*top: 188px;/s)
  assert.match(tradeSource, /\.contract-trade \.contract-percentage \.percent-row \{[^}]*grid-template-columns: repeat\(5, minmax\(0, 1fr\)\);[^}]*height: 32px;/s)
  assert.match(tradeSource, /\.contract-percentage button,[\s\S]*?height: 44px;[\s\S]*?min-height: 44px;/)
  assert.match(tradeSource, /\.contract-available-row \{[^}]*height: 13px;[^}]*top: 226px;/s)
  assert.match(tradeSource, /\.contract-tpsl \{[^}]*height: 16px;[^}]*top: 245px;/s)
  assert.match(tradeSource, /\.contract-open-meta--long \{ top: 267px; \}/)
  assert.match(tradeSource, /\.contract-open-meta--short \{ top: 349px; \}/)
  assert.match(tradeSource, /\.contract-submit \{[^}]*border-radius: 21px;[^}]*height: 42px;/s)
  assert.match(tradeSource, /\.contract-submit--long,[^}]*top: 301px;/s)
  assert.match(tradeSource, /\.contract-submit--short \{[^}]*top: 383px;/s)
  assert.match(tradeSource, /\.contract-trade \.trade-chart-panel \{[^}]*height: 450px;/s)
  assert.match(tradeSource, /\.contract-position-tabs \{[^}]*height: 44px;[^}]*min-height: 44px;[^}]*padding: 0 10px 0 14px;/s)
})

test('交易对弹层只组合后端杠杆产品、自选与实时行情数据', () => {
  assert.match(sheetsSource, /props\.products\.map\(\(product\) =>/)
  assert.match(sheetsSource, /marketStore\.tickerFor\(product\.symbol\)/)
  assert.match(sheetsSource, /marketFavorites\.isFavorite\(row\.product\.symbol\)/)
  assert.match(sheetsSource, /sort\(\(left, right\) => \(right\.ticker\?\.volume \|\| 0\) - \(left\.ticker\?\.volume \|\| 0\)\)/)
  assert.match(sheetsSource, /v-for="row in filteredPairRows"/)
  assert.match(sheetsSource, /<AssetMark :symbol="row\.pair\.base" :src="row\.product\.logoUrl \|\| row\.ticker\?\.iconUrl" :fallback-src="row\.ticker\?\.baseIconUrl"/)
  assert.match(sheetsSource, /row\.ticker \? formatPrice\(row\.ticker\.lastPrice\) : '--'/)
  assert.match(sheetsSource, /v-if="productsLoading" class="contract-pair-state"/)
  assert.match(sheetsSource, /v-else-if="productsError" class="contract-pair-state" role="alert"/)
  assert.match(sheetsSource, /emit\('retryProducts'\)/)
  assert.doesNotMatch(sheetsSource, /BTC|ETH|SOL|64,?090|99,?900/)
})

test('用户保存的保证金模式和杠杆倍数由后端设置接口驱动', () => {
  assert.match(tradingApiSource, /export interface MarginUserSetting \{[\s\S]*?leverage: number \| null[\s\S]*?marginMode: 'cross' \| 'isolated' \| null/)
  assert.match(tradingApiSource, /client\.get<[\s\S]*?requestUrl\(`\/margin\/settings\/\$\{productId\}`\)/)
  assert.match(tradingApiSource, /axios\.isAxiosError\(error\) && error\.response\?\.status === 404[\s\S]*?return \{ leverage: null, marginMode: null \}/)
  assert.match(tradingApiSource, /updateMarginMode\(productId: number, mode: 'cross' \| 'isolated'\)/)
  assert.match(tradingApiSource, /marginMode: 'cross' \| 'isolated'/)

  assert.match(tradeSource, /const setting = await fetchMarginSetting\(product\.id\)/)
  assert.match(tradeSource, /product\.leverageLevels\.includes\(setting\.leverage\)/)
  assert.match(tradeSource, /product\.marginModes\.includes\(setting\.marginMode\)/)
  assert.match(tradeSource, /await updateMarginLeverage\(product\.id, nextLeverage\)/)
  assert.match(tradeSource, /await updateMarginMode\(product\.id, nextMode\)/)
  assert.match(tradeSource, /createMarginOrderReview\(\{[\s\S]*?marginMode: marginMode\.value,[\s\S]*?leverage: leverage\.value/)
  assert.match(tradeSource, /placeMarginOrder\(review\.request\)/)
})

test('公开产品、能力、钱包和风险 DTO 被杠杆工作区完整消费', () => {
  const productLoader = tradeSource.slice(
    tradeSource.indexOf('async function loadMarginProducts'),
    tradeSource.indexOf('function applyMarginProductDefaults'),
  )
  assert.match(productLoader, /await fetchMarginProducts\(\)/)
  assert.doesNotMatch(productLoader, /session\.isAuthenticated/)

  assert.match(tradingApiSource, /margin_asset\?: string \| number/)
  assert.match(tradingApiSource, /logo_url\?: string \| null/)
  assert.match(tradingApiSource, /maintenance_margin_rate\?: string \| number/)
  assert.match(tradingApiSource, /hourly_interest_rate\?: string \| number/)
  assert.match(tradingApiSource, /take_profit_stop_loss\?: boolean/)
  assert.match(tradingApiSource, /strategy_orders\?: boolean/)
  assert.match(tradingApiSource, /bulk_close\?: boolean/)
  assert.match(tradingApiSource, /position_risk\?: boolean/)
  assert.match(tradingApiSource, /crossAccounts: \(response\.data\.cross_accounts \|\| \[\]\)\.map/)
  assert.match(tradingApiSource, /requestUrl\(`\/margin\/positions\/\$\{encodeURIComponent\(positionId\)\}\/risk`\)/)
  for (const field of [
    'unrealized_pnl',
    'position_quantity',
    'return_rate',
    'margin_ratio',
    'estimated_liquidation_price',
    'liquidation_distance_rate',
  ]) {
    assert.match(tradingApiSource, new RegExp(field))
  }
  assert.match(tradingApiSource, /function mapMarginBatchAction[\s\S]*?positions:[\s\S]*?failures:/)
  assert.match(tradeSource, /const result = await closeAllMarginPositions[\s\S]*?result\.failures\.length[\s\S]*?positionsPartiallyClosed/)
  assert.match(ordersSource, /const result = await cancelAllMarginPositions\(\)[\s\S]*?result\.failures\.length[\s\S]*?batchCancelPartial/)
  assert.match(ordersSource, /const result = await closeAllMarginPositions\(\)[\s\S]*?result\.failures\.length[\s\S]*?batchClosePartial/)
  assert.match(tradeSource, /:disabled="!selectedProduct\?\.strategyOrdersSupported"/)
  assert.match(tradeSource, /if \(tab === 'strategy' && !selectedProduct\.value\?\.strategyOrdersSupported\) return/)
  assert.match(tradeSource, /marginRiskRefreshTimer = window\.setInterval[\s\S]*?loadMarginPositionRisks\(\)[\s\S]*?5_000/)
  assert.match(tradeSource, /window\.clearInterval\(marginRiskRefreshTimer\)/)
})

test('Header 更多菜单支持键盘打开、循环导航、Escape 关闭和焦点恢复', () => {
  assert.match(tradeSource, /ref="contractMoreButton"[\s\S]*?@keydown="handleContractMoreButtonKeydown"/)
  assert.match(tradeSource, /ref="contractMoreMenu"[\s\S]*?role="menu"[\s\S]*?@keydown="handleContractMoreKeydown"/)
  assert.match(tradeSource, /event\.key === 'Escape'[\s\S]*?closeContractMore\(\)/)
  assert.match(tradeSource, /event\.key === 'ArrowDown'[\s\S]*?event\.key === 'ArrowUp'/)
  assert.match(tradeSource, /contractMoreButton\.value\?\.focus\(\)/)
  assert.match(tradeSource, /backdrop-filter: blur\(18px\) saturate\(135%\)/)
})

test('倍数与保证金模式仅暴露当前产品配置的真实选项', () => {
  assert.match(sheetsSource, /props\.product\?\.leverageLevels \|\| \[\]/)
  assert.match(sheetsSource, /v-for="level in quickLeverageLevels"/)
  assert.match(sheetsSource, /leverageLevels\.value\.includes\(draftLeverage\.value\)/)
  assert.match(sheetsSource, /props\.product\?\.marginModes \|\| \[\]/)
  assert.match(sheetsSource, /v-for="item in supportedMarginModes"/)
  assert.match(sheetsSource, /supportedMarginModes\.value\.includes\(draftMarginMode\.value\)/)
  assert.match(sheetsSource, /trade\.marginModeNoticeDescription/)
})

test('三个底部弹层满足对话框、焦点、触控、主题和安全区合同', () => {
  assert.match(sheetsSource, /<Teleport to="body">/)
  assert.match(sheetsSource, /role="dialog"[\s\S]*?aria-modal="true"/)
  assert.match(sheetsSource, /useModalDialog\(dialogOpen, dialog, '\[data-dialog-initial\]'\)/)
  assert.match(sheetsSource, /trapFocus\(event, requestClose\)/)
  assert.match(sheetsSource, /height: min\(500px,/) // leverage
  assert.match(sheetsSource, /height: min\(446px,/) // margin mode
  assert.match(sheetsSource, /height: min\(620px,/) // pair
  assert.match(sheetsSource, /env\(safe-area-inset-bottom\)/)
  assert.match(sheetsSource, /\.contract-sheet__close \{[\s\S]*?height: 44px;[\s\S]*?width: 44px;/)
  assert.match(sheetsSource, /\.contract-leverage-slider \{[\s\S]*?height: 44px;/)
  assert.match(sheetsSource, /\.contract-pair-search \{[^}]*height: 40px;/s)
  assert.match(sheetsSource, /\.contract-pair-search input \{[^}]*height: 44px;/s)
  assert.match(sheetsSource, /@media \(max-width: 820px\)/)
  assert.match(sheetsSource, /@media \(max-width: 340px\)/)
  assert.match(sheetsSource, /@media \(prefers-reduced-motion: no-preference\)/)
  assert.match(sheetsSource, /html\[data-theme='dark'\] \.contract-sheet/)
  assert.doesNotMatch(sheetsSource, /<svg|\p{Extended_Pictographic}/u)
})

test('三个底部弹层复刻所选画板的内容轨道与间距', () => {
  assert.match(sheetsSource, /grid-template-rows: 14px 36px auto auto;/)
  assert.match(sheetsSource, /row-gap: 14px;/)
  assert.match(sheetsSource, /\.contract-sheet--marginMode \{[^}]*grid-template-rows: 14px 45px auto auto;/s)
  assert.match(sheetsSource, /\.contract-sheet--pair \{[^}]*grid-template-rows: 14px 45px 40px 22px 322px auto;[^}]*row-gap: 10px;/s)
  assert.match(sheetsSource, /\.contract-leverage-card \{[^}]*height: 126px;/s)
  assert.match(sheetsSource, /\.contract-leverage-quick \{[^}]*grid-template-columns: repeat\(6, minmax\(0, 1fr\)\);[^}]*height: 34px;/s)
  assert.match(sheetsSource, /\.contract-scope-row \{[^}]*height: 44px;/s)
  assert.match(sheetsSource, /\.contract-sheet-notice \{[^}]*height: 33px;/s)
  assert.match(sheetsSource, /\.contract-sheet__submit \{[^}]*border-radius: 24px;[^}]*height: 48px;/s)
  assert.match(sheetsSource, /\.contract-mode-options button \{[^}]*height: 64px;/s)
  assert.match(sheetsSource, /\.contract-pair-filters \{[^}]*height: 22px;/s)
  assert.match(sheetsSource, /\.contract-pair-list \{[^}]*height: 322px;/s)
  assert.match(sheetsSource, /\.contract-pair-row \{[^}]*height: 52px;/s)
  assert.match(sheetsSource, /@media \(max-width: 340px\) \{[\s\S]*?\.contract-sheet-notice \{\s*height: auto;/)
})

test('新增杠杆弹层文案中英文资源对称且模板无固定中文', () => {
  const keys = new Set<string>()
  for (const source of [tradeSource, sheetsSource]) {
    for (const match of source.matchAll(/\bt\('([^']+)'/g)) keys.add(match[1])
  }
  for (const key of keys) {
    assert.notEqual(resolveMessage(zhCN, key), undefined, `zh-CN missing ${key}`)
    assert.notEqual(resolveMessage(en, key), undefined, `en missing ${key}`)
  }

  const template = sheetsSource.match(/<template>([\s\S]*?)<\/template>/)?.[1] || ''
  assert.doesNotMatch(template, /[\u3400-\u9fff]/)
})

function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}
