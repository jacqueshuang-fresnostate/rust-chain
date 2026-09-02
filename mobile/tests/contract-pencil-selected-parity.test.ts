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
const marginRiskSource = read('../src/core/marginRiskMetrics.ts')
const marginRiskContractSource = `${tradingApiSource}\n${marginRiskSource}`
const ordersSource = read('../src/views/OrdersView.vue')

test('当前八张 Pencil 杠杆画板均由生产页面声明', () => {
  assert.match(tradeSource, /data-pencil-source="cjzfi p6GfgT"/)
  assert.match(tradeSource, /data-pencil-module-source="IpirH mcfEf"/)
  assert.match(sheetsSource, /data-pencil-source="f0L8yf R8t0p aNuw6 PKAcD Crw8v YuKtQ NTiiS CulR4"/)
})

test('杠杆设置触发器按照当前 Pencil 选稿拆为两列设置和独立委托类型行', () => {
  const settingsStart = tradeSource.indexOf('<div class="contract-mode-row contract-settings-row"')
  const settingsEnd = tradeSource.indexOf('</div>', settingsStart)
  const settingsMarkup = tradeSource.slice(settingsStart, settingsEnd)
  const orderTypeStart = tradeSource.indexOf('class="contract-order-type"')

  assert.ok(settingsStart >= 0)
  assert.ok(settingsEnd > settingsStart)
  assert.equal((settingsMarkup.match(/<button/g) ?? []).length, 2)
  assert.ok(orderTypeStart > settingsEnd)
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
  assert.match(orderBookSource, /const miniAskRatio = computed\(\(\) => 100 - miniBidRatio\.value\)/)
  assert.match(orderBookSource, /class="order-book__mini-ratio-label order-book__mini-ratio-label--bid numeric"[\s\S]*?class="order-book__mini-ratio-label order-book__mini-ratio-label--ask numeric"[\s\S]*?class="order-book__mini-ratio-track"/)
  assert.match(orderBookSource, /:aria-label="`\$\{t\('orderBook\.buySide',[\s\S]*?miniBidRatio\}%`"[\s\S]*?:aria-label="`\$\{t\('orderBook\.sellSide',[\s\S]*?miniAskRatio\}%`"/)
  assert.doesNotMatch(tradeSource, /@click="changeLeverage"/)
})

test('杠杆主页面保留所选 Pencil 主结构并升级金融操作触控尺寸', () => {
  assert.match(tradeSource, /\.contract-pencil-header \{[^}]*height: calc\(58px \+ env\(safe-area-inset-top\)\);[^}]*padding: env\(safe-area-inset-top\) 14px 0;/s)
  assert.match(tradeSource, /\.contract-pencil-module \{[^}]*gap: 10px;[^}]*grid-template-columns: 202px minmax\(150px, 1fr\);[^}]*height: 500px;[^}]*padding: 2px 14px 8px;/s)
  assert.match(tradeSource, /\.contract-open-close \{[^}]*height: 30px;[^}]*padding: 2px;[^}]*top: 0;/s)
  assert.match(tradeSource, /\.contract-open-close button \{[^}]*height: 24px;[^}]*min-height: 24px;/s)
  assert.match(tradeSource, /\.contract-open-close button\.active \{[^}]*background: var\(--contract-accent\);[^}]*color: #07110d;/s)
  assert.match(tradeSource, /\.contract-mode-row \{[^}]*gap: 6px;[^}]*grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);[^}]*height: 38px;[^}]*top: 36px;/s)
  assert.match(tradeSource, /\.contract-mode-row button,[\s\S]*?border-radius: 7px;[\s\S]*?height: 38px;[\s\S]*?justify-content: space-between;[\s\S]*?min-height: 38px;[\s\S]*?padding: 0 11px;/)
  assert.match(tradeSource, /\.contract-order-type \{[^}]*height: 40px;[^}]*left: 0;[^}]*position: absolute;[^}]*right: 0;[^}]*top: 80px;[^}]*width: 100%;/s)
  assert.match(tradeSource, /\.contract-price-row \{[^}]*grid-template-columns: minmax\(0, 138px\) 58px;[^}]*height: 54px;[^}]*top: 126px;/s)
  assert.match(tradeSource, /\.contract-field \{[^}]*border: 1px solid transparent;[^}]*border-radius: 8px;/s)
  assert.match(tradeSource, /\.contract-field:focus-within \{[^}]*border-color: var\(--contract-accent\);[^}]*box-shadow:/s)
  assert.match(tradeSource, /\.contract-field input \{[^}]*border: 0;[^}]*box-shadow: none;[^}]*outline: 0;/s)
  assert.match(tradeSource, /\.contract-price-field \{[^}]*grid-template-rows: 13px 22px;[^}]*height: 54px;[^}]*padding: 7px 10px;/s)
  assert.match(tradeSource, /\.contract-price-field input \{[^}]*font-size: 17px;[^}]*height: 22px;/s)
  assert.match(tradeSource, /\.contract-amount-field \{[^}]*height: 48px;[^}]*top: 186px;/s)
  assert.match(tradeSource, /\.contract-amount-field \{[^}]*grid-template-rows: 13px 20px;[^}]*padding: 6px 11px;/s)
  assert.match(tradeSource, /\.contract-amount-field input \{[^}]*font-size: 15px;[^}]*height: 20px;/s)
  assert.match(tradeSource, /\.contract-percentage \{[^}]*height: 32px;[^}]*top: 240px;/s)
  assert.match(tradeSource, /\.contract-percentage__slider \{[^}]*grid-template-columns: minmax\(0, 1fr\) 34px;[^}]*height: 32px;/s)
  assert.match(tradeSource, /\.contract-percentage__input \{[^}]*height: 44px;[^}]*min-height: 44px;/s)
  assert.match(tradeSource, /id="contract-percentage-input"[\s\S]*?type="range"[\s\S]*?min="0"[\s\S]*?max="100"[\s\S]*?step="1"[\s\S]*?@input="setContractPercentageFromSlider"/)
  assert.doesNotMatch(tradeSource, /class="contract-percentage"[\s\S]*?v-for="value in \[0, 25, 50, 75, 100\]"[\s\S]*?class="contract-available-row"/)
  assert.match(tradeSource, /\.contract-available-row \{[^}]*height: 13px;[^}]*top: 278px;/s)
  assert.match(tradeSource, /\.contract-tpsl \{[^}]*height: 16px;[^}]*top: 297px;/s)
  assert.match(tradeSource, /\.contract-open-meta--long \{ top: 319px; \}/)
  assert.match(tradeSource, /\.contract-open-meta--short \{ top: 401px; \}/)
  assert.match(tradeSource, /\.contract-submit \{[^}]*border-radius: 21px;[^}]*height: 42px;/s)
  assert.match(tradeSource, /\.contract-submit \{[^}]*align-items: center;[^}]*display: flex;[^}]*justify-content: center;[^}]*text-align: center;/s)
  assert.match(tradeSource, /\.contract-submit--long,[^}]*top: 353px;/s)
  assert.match(tradeSource, /\.contract-submit--short \{[^}]*top: 435px;/s)
  assert.match(tradeSource, /t\('trade\.priceField', \{ asset: quoteAsset \}\)/)
  assert.match(tradeSource, /t\('trade\.marginField', \{ asset: availableAsset \}\)/)
  assert.match(tradeSource, /class="contract-percentage__value numeric"[\s\S]*?percentage === null \? '—' : `\$\{percentage\}%`/)
  assert.match(tradeSource, /t\('trade\.longActionCompact', \{ leverage: longLeverage \}\)/)
  assert.match(tradeSource, /t\('trade\.shortActionCompact', \{ leverage: shortLeverage \}\)/)
  assert.match(tradeSource, /\.contract-trade \.trade-chart-panel \{[^}]*height: 490px;/s)
  assert.match(tradeSource, /\.contract-mini-book :deep\(\.order-book__mini-ratio\) \{[^}]*grid-template-rows: 12px 6px;[^}]*height: 30px;/s)
  assert.match(tradeSource, /\.contract-mini-book :deep\(\.order-book__mini-ratio-track\) \{[\s\S]*?var\(--contract-accent\) 0 var\(--mini-bid-ratio\),[\s\S]*?var\(--contract-negative\) var\(--mini-bid-ratio\) 100%/)
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
  assert.match(tradingApiSource, /export interface MarginUserSetting \{[\s\S]*?leverage: number \| null[\s\S]*?longLeverage: number \| null[\s\S]*?shortLeverage: number \| null[\s\S]*?marginMode: 'cross' \| 'isolated' \| null/)
  assert.match(tradingApiSource, /client\.get<[\s\S]*?requestUrl\(`\/margin\/settings\/\$\{productId\}`\)/)
  assert.match(tradingApiSource, /axios\.isAxiosError\(error\) && error\.response\?\.status === 404[\s\S]*?return \{ leverage: null, longLeverage: null, shortLeverage: null, marginMode: null \}/)
  assert.match(tradingApiSource, /mapMarginUserLeverageSetting\(response\.data\)/)
  assert.match(tradingApiSource, /updateMarginMode\(productId: number, mode: 'cross' \| 'isolated'\)/)
  assert.match(tradingApiSource, /marginMode: 'cross' \| 'isolated'/)

  assert.match(tradeSource, /const setting = await fetchMarginSetting\(product\.id\)/)
  assert.match(tradeSource, /const nextLongLeverage = setting\.longLeverage/)
  assert.match(tradeSource, /const nextShortLeverage = setting\.shortLeverage/)
  assert.match(tradeSource, /product\.leverageLevels\.includes\(nextLongLeverage\)/)
  assert.match(tradeSource, /product\.leverageLevels\.includes\(nextShortLeverage\)/)
  assert.match(tradeSource, /product\.marginModes\.includes\(setting\.marginMode\)/)
  assert.match(tradeSource, /await updateMarginLeverage\(product\.id, \{[\s\S]*?longLeverage: nextLongLeverage,[\s\S]*?shortLeverage: nextShortLeverage,[\s\S]*?\}\)/)
  assert.match(tradeSource, /await updateMarginMode\(product\.id, nextMode\)/)
  assert.match(tradeSource, /createMarginOrderReview\(\{[\s\S]*?marginMode: marginMode\.value,[\s\S]*?leverage: leverage\.value/)
  assert.match(tradeSource, /placeMarginOrder\(\{\s*\.\.\.review\.request,\s*marginAmount: review\.marginAmountText,\s*price: review\.request\.price,\s*\}\)/)
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
  assert.match(tradingApiSource, /crossAccounts: response\.data\.cross_accounts\.map\(mapMarginCrossAccount\)/)
  assert.match(tradingApiSource, /requestUrl\(`\/margin\/positions\/\$\{encodeURIComponent\(positionId\)\}\/risk`\)/)
  for (const field of [
    'unrealized_pnl',
    'position_quantity',
    'return_rate',
    'margin_ratio',
    'estimated_liquidation_price',
    'liquidation_distance_rate',
    'cross_account_risk',
    'reference_pair_id',
    'price_assumption',
    'liquidation_buffer',
    'net_quantity',
    'gross_quantity',
    'estimate_status',
    'conditional_liquidation_price',
    'conditional_liquidation_distance_rate',
    'marks_observed_at_min',
    'marks_observed_at_max',
  ]) {
    assert.match(marginRiskContractSource, new RegExp(field))
  }
  assert.match(tradingApiSource, /crossAccountRisk: mapStrictMarginCrossAccountRisk\(risk\.cross_account_risk\)/)
  assert.match(tradingApiSource, /function mapStrictMarginCrossAccountRisk[\s\S]*?const mapped = mapMarginCrossAccountRisk\(risk\)/)
  assert.match(tradeSource, /crossAccountRisk: risk\?\.crossAccountRisk/)
  assert.match(tradeSource, /t\('trade\.estimatedAccountLiquidationPrice'\)/)
  assert.match(tradeSource, /t\('trade\.noStableSingleLiquidationPrice'\)/)
  assert.match(tradeSource, /role="note"[\s\S]*?t\('trade\.crossAccountLiquidationAssumption'\)/)
  assert.match(tradingApiSource, /function mapMarginBatchAction[\s\S]*?positions:[\s\S]*?failures:/)
  assert.match(tradeSource, /const result = await closeAllMarginPositions[\s\S]*?result\.failures\.length[\s\S]*?positionsPartiallyClosed/)
  assert.match(ordersSource, /cancelAllMarginPositions\(\)\)\.failures\.length[\s\S]*?batchActionPartial/)
  assert.match(ordersSource, /closeAllMarginPositions\(\)\)\.failures\.length[\s\S]*?batchActionPartial/)
  assert.match(tradeSource, /:disabled="!selectedProduct\?\.strategyOrdersSupported"/)
  assert.match(tradeSource, /if \(tab === 'strategy' && !selectedProduct\.value\?\.strategyOrdersSupported\) return/)
  assert.match(tradeSource, /createMarginAccountReconciliationLifecycle\(\{[\s\S]*?reconcile: reconcileMarginAccount/)
  assert.match(tradeSource, /marginAccountReconciliation\.startPolling\(\)/)
  assert.match(tradeSource, /marginAccountReconciliation\.stop\(\)/)
})

test('持仓页签显示真实可见数量且持仓卡按 Pencil 顺序复用三段操作', () => {
  assert.match(tradeSource, /t\('trade\.positionAssetsTab', \{ count: visibleMarginPositions\.length \}\)/)
  assert.equal(zhCN.trade.positionAssetsTab, '持仓 ({count})')
  assert.equal(en.trade.positionAssetsTab, 'Positions ({count})')

  const positionCardStart = tradeSource.indexOf('<article v-for="position in visibleMarginPositions"')
  const pendingOrdersStart = tradeSource.indexOf("<div v-else-if=\"contractWorkspaceTab === 'orders'", positionCardStart)
  assert.ok(positionCardStart >= 0 && pendingOrdersStart > positionCardStart)
  const positionCards = tradeSource.slice(positionCardStart, pendingOrdersStart)
  assertOrdered(positionCards, [
    'data-position-action="take-profit-stop-loss"',
    'data-position-action="close"',
    'data-position-action="market-close-all"',
  ])
  assert.match(positionCards, /data-position-action="take-profit-stop-loss"[\s\S]*?:disabled="!productForPosition\(position\)\?\.takeProfitStopLossSupported \|\| positionActionSaving !== null \|\| bulkCloseSaving"/)
  const takeProfitStopLossAction = positionCards.slice(
    positionCards.indexOf('data-position-action="take-profit-stop-loss"'),
    positionCards.indexOf('data-position-action="close"'),
  )
  assert.doesNotMatch(takeProfitStopLossAction, /@click=|fetch\(|post\(/)
  assert.match(positionCards, /data-position-action="close"[\s\S]*?aria-haspopup="dialog"[\s\S]*?@click="openMarginCloseSheet\(position, \$event\)"/)
  assert.match(positionCards, /data-position-action="market-close-all"[\s\S]*?@click="performPositionAction\(position, 'market-close-all'\)"/)
  assert.doesNotMatch(positionCards, /performBulkClose|closeAllMarginPositions/)
  assert.equal(zhCN.trade.positionActions, '持仓操作')
  assert.equal(en.trade.positionActions, 'Position actions')
  assert.equal(zhCN.trade.marketCloseAll, '市价全平')
  assert.equal(en.trade.marketCloseAll, 'Market close all')
  assert.equal(zhCN.trade.confirmMarketCloseAll, '确认市价全平')
  assert.equal(en.trade.confirmMarketCloseAll, 'Confirm market close')
})

test('普通平仓使用选中稿滑动弹窗，卡内市价全平继续使用独立二次确认', () => {
  const openCloseSheet = sliceSourceFunction('function openMarginCloseSheet', 'function closeMarginCloseSheet')
  const confirmClose = sliceSourceFunction('async function confirmMarginClose', 'async function performPositionAction')
  const quickAction = sliceSourceFunction('async function performPositionAction', 'async function performBulkClose')
  assert.match(tradeSource, /type PositionActionType = 'close' \| 'market-close-all' \| 'cancel'/)
  assert.doesNotMatch(openCloseSheet, /closeMarginPosition\(/)
  assert.match(confirmClose, /positionActionSaving\.value = \{ id: position\.id, type: 'close' \}[\s\S]*?createMarginCloseIdempotencyKey\(\)[\s\S]*?await closeMarginPosition\(position\.id, \{[\s\S]*?percentage: attempt\.percentage,[\s\S]*?idempotencyKey: attempt\.idempotencyKey/)
  assert.equal([...confirmClose.matchAll(/closeMarginPosition\(position\.id, \{/g)].length, 1)
  assert.match(quickAction, /armedPositionAction\.value\?\.id !== position\.id[\s\S]*?bulkCloseArmed\.value = false[\s\S]*?armedPositionAction\.value = \{ id: position\.id, type: action \}[\s\S]*?return/)
  assert.match(quickAction, /const closesPosition = action === 'market-close-all'/)
  assert.match(quickAction, /if \(closesPosition\) await closeMarginPosition\(position\.id\)/)
  assert.match(quickAction, /else await cancelMarginPosition\(position\.id\)/)
  assert.doesNotMatch(`${confirmClose}\n${quickAction}`, /closeAllMarginPositions/)

  const positionCards = tradeSource.slice(
    tradeSource.indexOf('<article v-for="position in visibleMarginPositions"'),
    tradeSource.indexOf("<div v-else-if=\"contractWorkspaceTab === 'orders'"),
  )
  assert.match(positionCards, /data-position-action="close"[\s\S]*?:aria-expanded="marginClosePositionId === position\.id"[\s\S]*?openMarginCloseSheet/)
  assert.doesNotMatch(positionCards, /data-position-action="close"[\s\S]*?armedPositionAction\.type === 'close'/)
  assert.match(positionCards, /data-position-action="market-close-all"[\s\S]*?armedPositionAction\.type === 'market-close-all'[\s\S]*?trade\.confirmMarketCloseAll/)
  assert.match(positionCards, /data-position-action="market-close-all"[\s\S]*?:disabled="positionActionSaving !== null \|\| bulkCloseSaving"/)
})

test('顶部一键平仓保留 currentPairOnly 条件作用域且与卡内单仓动作分离', () => {
  const bulkClose = sliceSourceFunction('async function performBulkClose', 'async function applyContractLeverage')
  assert.match(bulkClose, /!visibleMarginPositions\.value\.length/)
  assert.match(bulkClose, /!selectedProduct\.value\?\.bulkCloseSupported/)
  assert.match(bulkClose, /if \(!bulkCloseArmed\.value\) \{[\s\S]*?armedPositionAction\.value = null[\s\S]*?bulkCloseArmed\.value = true[\s\S]*?return/)
  assert.match(bulkClose, /await closeAllMarginPositions\(currentPairOnly\.value \? selectedProduct\.value\?\.id : undefined\)/)
  assert.doesNotMatch(bulkClose, /closeMarginPosition\(/)
  assert.equal([...tradeSource.matchAll(/@click="performBulkClose"/g)].length, 1)
})

test('切换当前交易对作用域会撤销旧确认意图且保存中不可变更作用域', () => {
  const scopeToggle = sliceSourceFunction('function toggleCurrentPairScope', 'function selectContractWorkspaceTab')
  assert.match(scopeToggle, /if \(positionActionSaving\.value \|\| bulkCloseSaving\.value\) return/)
  assertOrdered(scopeToggle, [
    'currentPairOnly.value = !currentPairOnly.value',
    'armedPositionAction.value = null',
    'bulkCloseArmed.value = false',
  ])
  assert.equal([...tradeSource.matchAll(/@click="toggleCurrentPairScope"/g)].length, 2)
  assert.match(tradeSource, /class="contract-current-pair"[\s\S]*?:aria-pressed="currentPairOnly"[\s\S]*?:disabled="bulkCloseSaving \|\| positionActionSaving !== null"[\s\S]*?@click="toggleCurrentPairScope"/)
  assert.match(tradeSource, /class="contract-filter-control"[\s\S]*?:aria-label="t\('trade\.positionFilter'\)"[\s\S]*?:aria-pressed="currentPairOnly"[\s\S]*?:disabled="bulkCloseSaving \|\| positionActionSaving !== null"[\s\S]*?@click="toggleCurrentPairScope"/)
})

test('持仓三枚按钮复刻独立间距、42px 视觉面与 44px 触控合同', () => {
  assert.match(tradeSource, /--contract-position-action-surface: #ffffff;[\s\S]*?--contract-position-action-border: #087b52;[\s\S]*?--contract-position-action-text: #087b52;/)
  assert.match(tradeSource, /html\[data-theme='dark'\] \.contract-trade \{[\s\S]*?--contract-position-action-surface: #121714;[\s\S]*?--contract-position-action-border: #202923;[\s\S]*?--contract-position-action-text: var\(--contract-text\);/)
  assert.match(tradeSource, /\.contract-position-card \{[^}]*display: grid;[^}]*gap: 12px;[^}]*grid-template-rows: auto auto auto 44px;[^}]*min-height: 272px;/s)
  assert.match(tradeSource, /\.contract-liquidation-distance small \{[^}]*grid-column: 1 \/ -1;[^}]*line-height: 12px;[^}]*min-width: 0;[^}]*overflow-wrap: anywhere;/s)
  assert.match(tradeSource, /\.contract-position-actions \{[^}]*display: grid;[^}]*gap: 10px;[^}]*grid-template-columns: repeat\(3, minmax\(0, 1fr\)\);[^}]*height: 44px;[^}]*min-width: 0;[^}]*width: 100%;/s)
  assert.match(tradeSource, /\.contract-position-actions button \{[^}]*background: transparent;[^}]*border: 0;[^}]*border-radius: 12px;[^}]*height: 44px;[^}]*min-height: 44px;[^}]*min-width: 0;[^}]*position: relative;/s)
  assert.match(tradeSource, /\.contract-position-actions button::before \{[^}]*background: var\(--contract-position-action-surface\);[^}]*border: 1px solid var\(--contract-position-action-border\);[^}]*border-radius: 12px;[^}]*inset: 1px 0;/s)
  assert.match(tradeSource, /\.contract-position-actions button:active:not\(:disabled\)::before \{[^}]*transform: translateY\(1px\);/s)
  assert.match(tradeSource, /\.contract-position-actions button:disabled \{[^}]*opacity: \.58;/s)
  assert.doesNotMatch(tradeSource, /\.contract-position-actions button \+ button|\.contract-position-actions button:first-child|\.contract-position-actions button:last-child/)
  assert.match(tradeSource, /\.contract-position-tabs button:focus-visible,[\s\S]*?\.contract-workspace-panel button:focus-visible \{[\s\S]*?outline: 2px solid var\(--focus\);/)
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
  assert.match(sheetsSource, /v-for="level in longQuickLeverageLevels"/)
  assert.match(sheetsSource, /v-for="level in shortQuickLeverageLevels"/)
  assert.match(sheetsSource, /leverageLevels\.value\.includes\(draftLongLeverage\.value\)/)
  assert.match(sheetsSource, /leverageLevels\.value\.includes\(draftShortLeverage\.value\)/)
  assert.match(sheetsSource, /props\.product\?\.marginModes \|\| \[\]/)
  assert.match(sheetsSource, /v-for="item in supportedMarginModes"/)
  assert.match(sheetsSource, /supportedMarginModes\.value\.includes\(draftMarginMode\.value\)/)
  assert.match(sheetsSource, /trade\.marginModeNoticeDescription/)
})

test('四个底部弹层满足对话框、焦点、触控、主题和安全区合同', () => {
  assert.match(sheetsSource, /<Teleport to="body">/)
  assert.match(sheetsSource, /role="dialog"[\s\S]*?aria-modal="true"/)
  assert.match(sheetsSource, /useModalDialog\(dialogOpen, dialog, '\[data-dialog-initial\]'\)/)
  assert.match(sheetsSource, /trapFocus\(event, requestClose\)/)
  assert.match(sheetsSource, /height: min\(840px,/) // leverage
  assert.match(sheetsSource, /height: min\(446px,/) // margin mode
  assert.match(sheetsSource, /height: min\(620px,/) // pair
  assert.match(sheetsSource, /env\(safe-area-inset-bottom\)/)
  assert.match(sheetsSource, /\.contract-sheet__close \{[\s\S]*?height: 44px;[\s\S]*?width: 44px;/)
  assert.match(sheetsSource, /\.contract-leverage-step \{[^}]*height: 42px;[^}]*width: 42px;/s)
  assert.match(sheetsSource, /\.contract-leverage-step::after \{[^}]*inset: -1px;/s)
  assert.match(sheetsSource, /\.contract-leverage-option::after,[\s\S]*?inset: -3px 0;/)
  assert.match(sheetsSource, /\.contract-leverage-more::after \{[^}]*inset-inline: -6px;/s)
  assert.match(sheetsSource, /\.contract-pair-search \{[^}]*height: 40px;/s)
  assert.match(sheetsSource, /\.contract-pair-search input \{[^}]*height: 44px;/s)
  assert.match(sheetsSource, /<button data-dialog-initial class="contract-sheet__close"[^>]*?@click="requestClose">[\s\S]*?<\/button>[\s\S]*?<label class="contract-pair-search">/)
  assert.doesNotMatch(sheetsSource, /<input[^>]*data-dialog-initial[^>]*v-model="searchQuery"/)
  assert.match(sheetsSource, /@media \(max-width: 820px\)/)
  assert.match(sheetsSource, /@media \(max-width: 340px\)/)
  assert.match(sheetsSource, /@media \(prefers-reduced-motion: no-preference\)/)
  assert.match(sheetsSource, /html\[data-theme='dark'\] \.contract-sheet/)
  assert.doesNotMatch(sheetsSource, /<svg|\p{Extended_Pictographic}/u)
})

test('四个底部弹层复刻所选画板的内容轨道与间距', () => {
  assert.match(sheetsSource, /grid-template-rows: 14px 36px auto auto;/)
  assert.match(sheetsSource, /row-gap: 14px;/)
  assert.match(sheetsSource, /\.contract-sheet--marginMode \{[^}]*grid-template-rows: 14px 45px auto auto;/s)
  assert.match(sheetsSource, /\.contract-sheet--pair \{[^}]*grid-template-rows: 14px 45px 40px 22px 322px auto;[^}]*row-gap: 10px;/s)
  assert.match(sheetsSource, /\.contract-sheet--leverage \{[^}]*grid-template-rows: 34px minmax\(0, 1fr\) 52px;[^}]*height: min\(840px,/s)
  assert.match(sheetsSource, /\.contract-leverage-stepper \{[^}]*height: 64px;/s)
  assert.match(sheetsSource, /\.contract-leverage-window \{[^}]*height: 46px;[^}]*border-radius: 23px;/s)
  assert.match(sheetsSource, /\.contract-leverage-info \{[^}]*min-height: 103px;/s)
  assert.match(sheetsSource, /\.contract-leverage-info--short \{[^}]*min-height: 75px;/s)
  assert.match(sheetsSource, /\.contract-sheet-notice \{[^}]*height: 33px;/s)
  assert.match(sheetsSource, /\.contract-sheet__submit \{[^}]*border-radius: 24px;[^}]*height: 48px;/s)
  assert.match(sheetsSource, /\.contract-sheet--leverage \.contract-sheet__submit \{[^}]*height: 52px;[^}]*border-radius: 26px;/s)
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
  assert.equal(zhCN.trade.bestBidOffer, '最优价')
  assert.equal(en.trade.bestBidOffer, 'Best')
  assert.equal(zhCN.trade.longActionCompact, '开多 {leverage}x')
  assert.equal(zhCN.trade.shortActionCompact, '开空 {leverage}x')
})

function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}

function assertOrdered(source: string, markers: string[]): void {
  let cursor = -1
  for (const marker of markers) {
    const next = source.indexOf(marker, cursor + 1)
    assert.ok(next > cursor, `expected ${marker} after previous marker`)
    cursor = next
  }
}

function sliceSourceFunction(startToken: string, endToken: string): string {
  const start = tradeSource.indexOf(startToken)
  assert.notEqual(start, -1, `missing start token: ${startToken}`)
  const end = tradeSource.indexOf(endToken, start + startToken.length)
  assert.notEqual(end, -1, `missing end token: ${endToken}`)
  return tradeSource.slice(start, end)
}
