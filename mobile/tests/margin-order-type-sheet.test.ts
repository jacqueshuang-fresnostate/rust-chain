import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  isFilledMarginPosition,
  isPendingMarginPosition,
  marginLimitPriceFromBbo,
  parseMarginOrderTypes,
  preferredMarginOrderType,
} from '../src/core/marginOrder.ts'
import { createMarginOrderReview } from '../src/core/marginOrderConfirmation.ts'
import { validateMarginLimitPrice } from '../src/core/tradeForm.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const apiSource = read('../src/api/trading.ts')
const sheetsSource = read('../src/components/ContractTradeSheets.vue')
const tradeSource = read('../src/views/TradeView.vue')
const ordersSource = read('../src/views/OrdersView.vue')

test('杠杆订单类型只保留后端能力，失效时按 Pencil 默认优先回落限价或首个真实能力', () => {
  assert.deepEqual(parseMarginOrderTypes(undefined), [])
  assert.deepEqual(parseMarginOrderTypes([' LIMIT ', 'market', 'stop', 'limit']), ['limit', 'market'])
  assert.deepEqual(parseMarginOrderTypes('market, limit, trigger'), ['market', 'limit'])

  assert.equal(preferredMarginOrderType(null, ['limit', 'market']), 'limit')
  assert.equal(preferredMarginOrderType(null, ['market', 'limit']), 'limit')
  assert.equal(preferredMarginOrderType('limit', ['market', 'limit']), 'limit')
  assert.equal(preferredMarginOrderType('market', ['market', 'limit']), 'market')
  assert.equal(preferredMarginOrderType('limit', ['market']), 'market')
  assert.equal(preferredMarginOrderType('market', ['limit']), 'limit')
  assert.equal(preferredMarginOrderType('market', []), null)

  assert.match(apiSource, /const orderTypes = parseMarginOrderTypes\(response\.data\.capabilities\?\.order_types\)/)
  assert.match(apiSource, /orderTypes: \[\.\.\.orderTypes\]/)
  assert.doesNotMatch(apiSource, /orderTypes:\s*\[['"]market['"]\]/)
  assert.match(tradeSource, /contractOrderType\.value = preferredMarginOrderType\([\s\S]*?nextSelected\?\.orderTypes \|\| \[\]/)
})

test('限价必须为正数且符合交易对价格精度，尾随零不扩大有效精度', () => {
  assert.equal(validateMarginLimitPrice({ price: '100.1200', pricePrecision: 2 }).isValid, true)
  assert.equal(validateMarginLimitPrice({ price: '100.123', pricePrecision: 2 }).error, 'precision')
  assert.equal(validateMarginLimitPrice({ price: '0', pricePrecision: 8 }).error, 'invalid')
  assert.equal(validateMarginLimitPrice({ price: '-1', pricePrecision: 8 }).error, 'invalid')
  assert.equal(validateMarginLimitPrice({ price: '', pricePrecision: 8 }).error, 'required')
  assert.equal(validateMarginLimitPrice({ price: '1.', pricePrecision: 8 }).error, 'invalid')
  assert.equal(validateMarginLimitPrice({ price: '.5', pricePrecision: 8 }).normalized, '0.5')
  assert.equal(validateMarginLimitPrice({ price: '1', pricePrecision: null }).error, 'precision-unavailable')
})

test('确认快照冻结订单类型、限价、参考价和幂等键，市价请求始终省略价格', () => {
  const market = createMarginOrderReview({
    productId: 7,
    side: 'buy',
    marginMode: 'cross',
    leverage: 5,
    marginAmount: '20',
    orderType: 'market',
    limitPrice: '88.88',
    pricePrecision: 2,
    idempotencyKey: 'market-frozen-key',
    minMargin: 10,
    maxMargin: 100,
    referencePrice: 100,
  })
  assert.equal(market.isValid, true)
  assert.deepEqual(market.request, {
    productId: 7,
    side: 'long',
    marginMode: 'cross',
    leverage: 5,
    marginAmount: '20',
    orderType: 'market',
    idempotencyKey: 'market-frozen-key',
  })

  const limit = createMarginOrderReview({
    productId: 7,
    side: 'sell',
    marginMode: 'isolated',
    leverage: 3,
    marginAmount: '25',
    orderType: 'limit',
    limitPrice: '105.50',
    pricePrecision: 2,
    idempotencyKey: 'limit-frozen-key',
    minMargin: 10,
    maxMargin: 100,
    referencePrice: 101,
  })
  assert.equal(limit.isValid, true)
  assert.equal(limit.referencePrice, 101)
  assert.equal(limit.request.orderType, 'limit')
  assert.equal(limit.request.price, '105.5')
  assert.equal(limit.request.idempotencyKey, 'limit-frozen-key')
  assert.match(tradeSource, /const marginSnapshot = mode\.value === 'contract'[\s\S]*?createCurrentMarginOrderReview\(createMarginOrderIdempotencyKey\(\)\)[\s\S]*?marginReview\.value = marginSnapshot/)
  assert.match(tradeSource, /await placeMarginOrder\(\{\s*\.\.\.review\.request,\s*marginAmount: review\.marginAmountText,\s*price: review\.request\.price,\s*\}\)/)
})

test('BBO 做多回填卖一、做空回填买一，持仓与挂单按可空入场价分流', () => {
  const book = {
    bids: [{ price: 99 }, { price: 100 }],
    asks: [{ price: 102 }, { price: 101 }],
    latestPrice: 100.5,
  }
  assert.equal(marginLimitPriceFromBbo({ side: 'buy', ...book }), 101)
  assert.equal(marginLimitPriceFromBbo({ side: 'sell', ...book }), 100)
  assert.equal(marginLimitPriceFromBbo({ side: 'buy', bids: [], asks: [], latestPrice: 100.5 }), 100.5)

  assert.equal(isFilledMarginPosition({ entryPrice: 0 }), false)
  assert.equal(isFilledMarginPosition({ entryPrice: null }), false)
  assert.equal(isFilledMarginPosition({ entryPrice: 99 }), true)
  assert.equal(isPendingMarginPosition({ entryPrice: null, status: 'opened' }), true)
  assert.equal(isPendingMarginPosition({ entryPrice: null, status: 'canceled' }), false)
  assert.match(ordersSource, /cancelablePositions = computed\(\(\) => openedPositions\.value\.filter\(isPendingMarginPosition\)\)/)
  assert.match(ordersSource, /closablePositions = computed\(\(\) => openedPositions\.value\.filter\(isFilledMarginPosition\)\)/)
  assert.match(tradeSource, /filledMarginPositions = computed\(\(\) => marginPositions\.value\.filter\(\(position\) => \([\s\S]*?isFilledMarginPosition\(position\)/)
  assert.match(tradeSource, /pendingMarginOrders = computed\(\(\) => marginPositions\.value\.filter\(\(position\) => \([\s\S]*?isPendingMarginPosition\(position\)/)
  assert.match(tradeSource, /visibleMarginPositions = computed[\s\S]*?filledMarginPositions\.value/)
  assert.match(tradeSource, /visiblePendingMarginOrders = computed[\s\S]*?pendingMarginOrders\.value/)
  assert.match(tradeSource, /watch\(\(\) => selectedProduct\.value\?\.id \?\? null,[\s\S]*?contractLimitPrice\.value = ''/)
  assert.match(tradeSource, /contractOrderType\.value === 'limit'[\s\S]*?!contractLimitPrice\.value\.trim\(\)[\s\S]*?fillContractLimitPrice\(\)/)
})

test('杠杆订单类型底部弹层只在显式选项时提交，关闭路径不改值', () => {
  assert.match(sheetsSource, /type ContractSheet = 'pair' \| 'leverage' \| 'marginMode' \| 'orderType' \| null/)
  assert.match(sheetsSource, /supportedOrderTypes = computed<MarginOrderType\[\]>\(\(\) => props\.product\?\.orderTypes \|\| \[\]\)/)
  assert.match(sheetsSource, /v-for="item in supportedOrderTypes"/)
  assert.match(sheetsSource, /@click="selectOrderType\(item\)"/)
  assert.match(sheetsSource, /function selectOrderType\(orderType: MarginOrderType\)[\s\S]*?emit\('selectOrderType', orderType\)/)
  const closeFunction = sheetsSource.match(/function requestClose\(\): void \{[\s\S]*?\n\}/)?.[0] || ''
  assert.doesNotMatch(closeFunction, /selectOrderType|emit\('selectOrderType'/)
  assert.match(sheetsSource, /useModalDialog\(dialogOpen, dialog, '\[data-dialog-initial\]'\)/)
  assert.match(sheetsSource, /@click="requestClose"[\s\S]*?@keydown="handleKeydown"/)
  assert.match(sheetsSource, /\.contract-mode-options button \{[\s\S]*?min-height: 64px;/)
  assert.match(sheetsSource, /html\[data-theme='dark'\] \.contract-sheet/)
  assert.match(sheetsSource, /@media \(max-width: 340px\)/)
  assert.match(sheetsSource, /@media \(prefers-reduced-motion: no-preference\)/)

  const openSheetFunction = tradeSource.slice(
    tradeSource.indexOf('function openContractSheet'),
    tradeSource.indexOf('function selectContractOrderType'),
  )
  assert.match(openSheetFunction, /sheet === 'orderType'[\s\S]*?contractSheet\.value = sheet[\s\S]*?return[\s\S]*?if \(!session\.isAuthenticated\)/)
  assert.ok(
    openSheetFunction.indexOf("sheet === 'orderType'") < openSheetFunction.indexOf('!session.isAuthenticated'),
    '订单类型选择应在访客态公开，认证只约束需要持久化的倍数和保证金模式',
  )
})

test('杠杆 API 按冻结类型组装请求，市价不带 price，限价才带 price', () => {
  assert.match(apiSource, /order_type: input\.orderType/)
  assert.match(apiSource, /if \(input\.orderType === 'limit'\) \{[\s\S]*?payload\.price = price/)
  assert.doesNotMatch(apiSource, /order_type: 'market'/)
  assert.match(apiSource, /const entryPriceText = nullableTradingDecimal\(position\.entry_price/)
  assert.match(apiSource, /const limitPriceText = nullableTradingDecimal\(position\.limit_price/)
  assert.match(apiSource, /entryPrice: nullableDecimalDisplayNumber\(entryPriceText\)/)
  assert.match(apiSource, /limitPrice: limitPriceText/)
  assert.match(tradeSource, /:readonly="contractOrderType !== 'limit'"/)
  assert.match(tradeSource, /:aria-errormessage="contractOrderType === 'limit'[\s\S]*?contract-limit-price-error/)
  assert.match(tradeSource, /@click="fillContractLimitPrice"/)
  assert.match(tradeSource, /contractOrderReview\.request\.orderType === 'limit'[\s\S]*?contractOrderReview\.request\.price/)

  const keys = [
    'marginOrderTypeSheetTitle',
    'marginOrderTypeSheetHint',
    'orderTypeUnavailableShort',
    'marginMarketOrderDescription',
    'marginLimitOrderDescription',
    'marginLimitPriceRequired',
    'marginLimitPriceInvalid',
    'marginLimitPricePrecision',
    'limitExecutionRiskTitle',
    'limitExecutionRiskDescription',
  ] as const
  for (const key of keys) {
    assert.equal(typeof zhCN.trade[key], 'string')
    assert.equal(typeof en.trade[key], 'string')
    assert.ok(zhCN.trade[key].length > 0)
    assert.ok(en.trade[key].length > 0)
  }
})

function read(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}
