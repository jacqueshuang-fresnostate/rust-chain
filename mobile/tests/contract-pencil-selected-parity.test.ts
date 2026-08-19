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

test('当前八张 Pencil 杠杆画板均由生产页面声明', () => {
  assert.match(tradeSource, /data-pencil-source="by3G9 pKHeU"/)
  assert.match(sheetsSource, /data-pencil-source="f0L8yf R8t0p aNuw6 PKAcD Crw8v YuKtQ"/)
})

test('杠杆主页面使用真实交易对图标并在当前页打开三个选择弹层', () => {
  assert.match(tradeSource, /<AssetMark :symbol="baseAsset" :src="ticker\?\.iconUrl" :fallback-src="ticker\?\.baseIconUrl" :size="24"/)
  assert.match(tradeSource, /@click="openContractSheet\('pair'\)"/)
  assert.match(tradeSource, /@click="openContractSheet\('marginMode'\)"/)
  assert.match(tradeSource, /@click="openContractSheet\('leverage'\)"/)
  assert.match(tradeSource, /<ContractTradeSheets[\s\S]*?:product="selectedProduct"[\s\S]*?:products="products"/)
  assert.match(tradeSource, /@select-pair="selectContractPair"/)
  assert.match(tradeSource, /router\.replace\(\{[\s\S]*?name: 'trade',[\s\S]*?query: \{ mode: 'contract' \}/)
  assert.match(tradeSource, /t\('trade\.bestBidOffer'\)/)
  assert.match(tradeSource, /t\('trade\.fundingAndCountdown'\)/)
  assert.match(tradeSource, /:mini-levels="6"/)
  assert.match(tradeSource, /:show-mini-precision="false"/)
  assert.match(orderBookSource, /miniLevels\?: number/)
  assert.match(orderBookSource, /showMiniPrecision\?: boolean/)
  assert.match(orderBookSource, /renderedMiniAsks/)
  assert.match(orderBookSource, /renderedMiniBids/)
  assert.doesNotMatch(tradeSource, /@click="changeLeverage"/)
})

test('杠杆主页面保留所选 Pencil 主结构并升级金融操作触控尺寸', () => {
  assert.match(tradeSource, /\.contract-pencil-header \{[^}]*height: 61px;/s)
  assert.match(tradeSource, /\.contract-pencil-module \{[^}]*gap: 12px;[^}]*grid-template-columns: minmax\(0, 1fr\) 150px;[^}]*padding: 2px 16px 4px;/s)
  assert.match(tradeSource, /\.contract-open-close \{[^}]*height: 38px;[^}]*padding: 4px;/s)
  assert.match(tradeSource, /\.contract-mode-row \{[^}]*height: 36px;/s)
  assert.match(tradeSource, /\.contract-price-row \{[^}]*grid-template-columns: minmax\(0, 1fr\) 62px;/s)
  assert.match(tradeSource, /\.contract-amount-field \{[^}]*height: 40px;/s)
  assert.match(tradeSource, /\.contract-percentage \{[^}]*min-height: 92px;/s)
  assert.match(tradeSource, /\.contract-trade \.contract-percentage \.percent-row \{[^}]*grid-auto-rows: 44px;[^}]*grid-template-columns: repeat\(3, minmax\(44px, 1fr\)\);/s)
  assert.match(tradeSource, /\.contract-percentage button \{[^}]*height: 44px;[^}]*min-width: 44px;/s)
  assert.match(tradeSource, /\.contract-balance-rows \{[^}]*gap: 4px;[^}]*grid-template-rows: 44px 18px;/s)
  assert.match(tradeSource, /\.contract-submit \{[^}]*border-radius: 23px;[^}]*height: 46px;/s)
  assert.match(tradeSource, /\.contract-trade \.trade-chart-panel \{[^}]*height: 372px;/s)
  assert.match(tradeSource, /\.contract-position-tabs \{[^}]*height: 37px;[^}]*padding: 8px 20px 4px;/s)
})

test('交易对弹层只组合后端杠杆产品、自选与实时行情数据', () => {
  assert.match(sheetsSource, /props\.products\.map\(\(product\) =>/)
  assert.match(sheetsSource, /marketStore\.tickerFor\(product\.symbol\)/)
  assert.match(sheetsSource, /marketFavorites\.isFavorite\(row\.product\.symbol\)/)
  assert.match(sheetsSource, /sort\(\(left, right\) => \(right\.ticker\?\.volume \|\| 0\) - \(left\.ticker\?\.volume \|\| 0\)\)/)
  assert.match(sheetsSource, /v-for="row in filteredPairRows"/)
  assert.match(sheetsSource, /<AssetMark :symbol="row\.pair\.base" :src="row\.ticker\?\.iconUrl" :fallback-src="row\.ticker\?\.baseIconUrl"/)
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
