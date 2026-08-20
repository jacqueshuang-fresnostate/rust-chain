import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'
import {
  classifyMarginOrderBackendBoundaryError,
  createMarginOrderReview,
} from '../src/core/marginOrderConfirmation.ts'
import {
  clampMarginShortcutAmount,
  mapMarginProductMarginLimits,
  marginShortcutAvailable,
  quantityForBalancePercentage,
  validateMarginAmount,
} from '../src/core/tradeForm.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const tradingApiSource = read('../src/api/trading.ts')
const typesSource = read('../src/core/types.ts')
const tradeSource = read('../src/views/TradeView.vue')
const tradeCss = styleOf(tradeSource)

test('max_margin DTO 边界保留正值，缺失或非法非正值映射为 null', () => {
  assert.deepEqual(mapMarginProductMarginLimits({
    min_margin: '10.00000000',
    max_margin: '125.50000000',
  }), {
    minMargin: 10,
    maxMargin: 125.5,
  })

  for (const max_margin of [undefined, null, '', '   ', 0, '0', -1, '-2', 'invalid', '1e2', '0x10', '+5', Number.POSITIVE_INFINITY, true]) {
    assert.equal(mapMarginProductMarginLimits({ min_margin: '10', max_margin }).maxMargin, null)
  }

  assert.match(tradingApiSource, /max_margin\?: string \| number \| null/)
  assert.match(tradingApiSource, /const marginLimits = mapMarginProductMarginLimits\(product\)/)
  assert.match(tradingApiSource, /minMargin: marginLimits\.minMargin,[\s\S]*?maxMargin: marginLimits\.maxMargin/)
  assert.match(typesSource, /interface MarginProduct \{[\s\S]*?minMargin: number[\s\S]*?maxMargin: number \| null/)
})

test('合约百分比以钱包可用额与产品上限的较小值为唯一基数', () => {
  assert.equal(marginShortcutAvailable(1_000, 120), 120)
  assert.equal(marginShortcutAvailable(80, 120), 80)
  assert.equal(marginShortcutAvailable(1_000, null), 1_000)
  assert.equal(marginShortcutAvailable(1_000, 0), 1_000)
  const preciseMaximum = 100.000000009
  assert.ok(Number(preciseMaximum.toFixed(8)) > preciseMaximum)
  assert.equal(
    clampMarginShortcutAmount(Number(preciseMaximum.toFixed(8)), 1_000, preciseMaximum),
    preciseMaximum,
  )

  assert.equal(quantityForBalancePercentage({
    available: 1_000,
    maximum: 120,
    mode: 'contract',
    percentage: .25,
    price: 50,
    side: 'buy',
  }), 30)
  assert.equal(quantityForBalancePercentage({
    available: 1_000,
    maximum: 120,
    mode: 'contract',
    percentage: .37,
    price: 50,
    side: 'buy',
  }), 44.4)
  assert.equal(quantityForBalancePercentage({
    available: 1_000,
    maximum: 120,
    mode: 'contract',
    percentage: 1,
    price: 50,
    side: 'sell',
  }), 120)
  assert.equal(quantityForBalancePercentage({
    available: 1_000,
    maximum: null,
    mode: 'contract',
    percentage: .5,
    price: 0,
    side: 'buy',
  }), 500)
  assert.equal(quantityForBalancePercentage({
    available: 1_000,
    maximum: 120,
    mode: 'spot',
    percentage: .5,
    price: 50,
    side: 'buy',
  }), 10)
})

test('最小与最大保证金边界包含端点，并直接决定确认快照是否有效', () => {
  assert.deepEqual(validateMarginAmount({ amount: 9.99, minMargin: 10, maxMargin: 100 }).error, 'below-minimum')
  assert.equal(validateMarginAmount({ amount: 10, minMargin: 10, maxMargin: 100 }).isValid, true)
  assert.equal(validateMarginAmount({ amount: 100, minMargin: 10, maxMargin: 100 }).isValid, true)
  assert.deepEqual(validateMarginAmount({ amount: 100.01, minMargin: 10, maxMargin: 100 }).error, 'above-maximum')
  assert.deepEqual(validateMarginAmount({ amount: Number.NaN, minMargin: 10, maxMargin: 100 }).error, 'invalid')
  assert.equal(validateMarginAmount({ amount: 10_000, minMargin: 10, maxMargin: null }).isValid, true)

  const belowMinimum = createMarginOrderReview({
    productId: 42,
    side: 'buy',
    marginMode: 'cross',
    leverage: 5,
    marginAmount: 9,
    minMargin: 10,
    maxMargin: 100,
    orderType: 'market',
    limitPrice: '',
    pricePrecision: 8,
    referencePrice: 50,
  })
  assert.equal(belowMinimum.marginAmountValidation.error, 'below-minimum')
  assert.equal(belowMinimum.isValid, false)

  const atMaximum = createMarginOrderReview({
    productId: 42,
    side: 'sell',
    marginMode: 'isolated',
    leverage: 5,
    marginAmount: 100,
    minMargin: 10,
    maxMargin: 100,
    orderType: 'market',
    limitPrice: '',
    pricePrecision: 8,
    referencePrice: 50,
  })
  assert.equal(atMaximum.marginAmountValidation.isValid, true)
  assert.equal(atMaximum.isValid, true)
  assert.equal(atMaximum.request.marginAmount, 100)
})

test('字段、打开确认层和最终 placeMarginOrder 共用同一校验结果', () => {
  const reviewOrderSource = sliceFunction('function reviewOrder(event?: Event): void {', 'function reviewContractOrder')
  const submitOrderSource = sliceFunction('async function submitOrder(): Promise<void> {', 'function trapDialogFocus')

  assert.match(tradeSource, /createMarginOrderReview\(\{[\s\S]*?minMargin: selectedProduct\.value\?\.minMargin,[\s\S]*?maxMargin: selectedProduct\.value\?\.maxMargin/)
  assert.match(tradeSource, /marginAmountError = computed\([\s\S]*?marginOrderDraft\.value\.marginAmountValidation/)
  assert.match(reviewOrderSource, /!marginOrderDraft\.value\.marginAmountValidation\.isValid[\s\S]*?marginAmountValidationMessage/)
  assert.match(submitOrderSource, /validateMarginAmount\(\{[\s\S]*?amount: review\.request\.marginAmount,[\s\S]*?minMargin: product\.minMargin,[\s\S]*?maxMargin: product\.maxMargin,[\s\S]*?!requestMarginValidation\.isValid[\s\S]*?marginAmountValidationMessage\(requestMarginValidation\)[\s\S]*?await placeMarginOrder\(review\.request\)/)
  assert.equal(submitOrderSource.match(/placeMarginOrder\(/g)?.length, 1)
  assert.match(tradeSource, /maximum: mode\.value === 'contract' \? selectedProduct\.value\?\.maxMargin : null/)
  assert.match(tradeSource, /clampMarginShortcutAmount\(roundedQuantity, availableBalance\.value, selectedProduct\.value\?\.maxMargin\)/)
  assert.match(tradeSource, /contractShortcutAvailable = computed\(\(\) => marginShortcutAvailable\(/)
  assert.match(tradeSource, /const percentage = ref<number \| null>\(0\)/)
  assert.match(tradeSource, /function setContractPercentageFromSlider\(event: Event\): void \{[\s\S]*?setQuantity\(value\)/)
  assert.match(tradeSource, /function clearPercentageSelection\(\): void \{\s*percentage\.value = null\s*\}/)
  assert.match(tradeSource, /@input="clearPercentageSelection"/)
  assert.match(tradeSource, /:aria-invalid="marginAmountError \? 'true' : 'false'"/)
  assert.match(tradeSource, /:aria-errormessage="marginAmountError \? 'contract-margin-error' : undefined"/)
  assert.match(tradeSource, /id="contract-margin-range"[\s\S]*?id="contract-margin-error"[\s\S]*?role="alert"/)
})

test('后端最小与最大竞态错误被稳定识别并转换为对称本地化文案', () => {
  assert.equal(
    classifyMarginOrderBackendBoundaryError('validation error: margin amount is below product minimum'),
    'below-minimum',
  )
  assert.equal(
    classifyMarginOrderBackendBoundaryError('Validation Error: margin amount exceeds product maximum'),
    'above-maximum',
  )
  assert.equal(classifyMarginOrderBackendBoundaryError('validation error: leverage is unsupported'), null)

  const keys = [
    'marginRangeWithMaximum',
    'marginRangeWithoutMaximum',
    'invalidMarginAmount',
    'marginBelowMinimum',
    'marginAboveMaximum',
    'marginMinimumChanged',
    'marginMaximumChanged',
    'marginMaximumShortcut',
  ] as const
  for (const key of keys) {
    assert.equal(typeof zhCN.trade[key], 'string')
    assert.equal(typeof en.trade[key], 'string')
    assert.ok(zhCN.trade[key].length > 0)
    assert.ok(en.trade[key].length > 0)
  }

  assert.match(tradeSource, /boundary === 'below-minimum'[\s\S]*?loadMarginProducts\(\{ preserveExistingOnError: true \}\)[\s\S]*?t\('trade\.marginMinimumChanged'\)/)
  assert.match(tradeSource, /boundary === 'above-maximum'[\s\S]*?loadMarginProducts\(\{ preserveExistingOnError: true \}\)[\s\S]*?t\('trade\.marginMaximumChanged'\)/)
  assert.match(tradeSource, /const requestVersion = \+\+marginProductsRequestVersion[\s\S]*?requestVersion !== marginProductsRequestVersion[\s\S]*?if \(!options\.preserveExistingOnError\) \{[\s\S]*?products\.value = \[\]/)
})

test('杠杆连续滑杆、设置、BBO、资产与主操作遵守 Pencil 几何和完整交互状态', () => {
  const compiled = compileStyle({
    source: tradeCss,
    filename: 'TradeView.vue',
    id: 'data-v-margin-boundary',
    scoped: true,
  })
  assert.deepEqual(compiled.errors, [])

  assert.match(cssRule('.contract-percentage'), /height: 32px;/)
  assert.match(cssRule('.contract-percentage'), /top: 188px;/)
  assert.match(cssRule('.contract-percentage__slider'), /grid-template-columns: minmax\(0, 1fr\) 34px;/)
  assert.match(cssRule('.contract-percentage__slider'), /height: 32px;/)
  assert.match(cssRule('.contract-percentage__input'), /height: 44px;/)
  assert.match(cssRule('.contract-percentage__input'), /min-height: 44px;/)
  assert.match(cssRule('.contract-percentage__input::-webkit-slider-runnable-track'), /height: 4px;/)
  assert.match(cssRule('.contract-percentage__input::-webkit-slider-thumb'), /height: 18px;/)
  assert.match(cssRule('.contract-percentage__input::-webkit-slider-thumb'), /width: 18px;/)
  assert.match(cssRule('.contract-percentage__input:focus-visible::-webkit-slider-thumb'), /0 0 0 4px var\(--contract-accent\)/)
  assert.match(cssRule('.contract-percentage__auth-trigger'), /height: 44px;/)
  assert.match(cssRule('.contract-percentage__value'), /text-align: right;/)

  assert.match(cssRule('.contract-mode-row button,\n.contract-order-type'), /height: 32px;/)
  assert.match(cssRule('.contract-price-row > button'), /height: 56px;/)
  assert.match(cssRule('.contract-amount-field input'), /height: 20px;/)
  assert.match(cssRule('.contract-header-control'), /height: 44px;/)
  assert.match(cssRule('.contract-header-control'), /width: 44px;/)
  assert.match(cssRule('.contract-position-tabs button'), /height: 44px;/)
  assert.match(cssRule('.contract-submit'), /height: 42px;/)
  assert.match(cssRule('.contract-submit'), /align-items: center;/)
  assert.match(cssRule('.contract-submit'), /display: flex;/)
  assert.match(cssRule('.contract-submit'), /justify-content: center;/)
  assert.match(cssRule('.contract-submit'), /text-align: center;/)
  assert.match(cssRule('.contract-submit--long,\n.contract-trade .contract-submit--long.submit-order'), /top: 301px;/)
  assert.match(cssRule('.contract-submit--short'), /top: 383px;/)
  assert.match(cssRule('.contract-submit:disabled'), /opacity: \.62;/)
  assert.match(cssRule('.contract-header-control:active:not(:disabled)'), /transform: scale\(\.94\);/)
  assert.match(tradeCss, /\.contract-pencil-header button:focus-visible,[\s\S]*?outline: 2px solid var\(--focus\);[\s\S]*?outline-offset: 2px;/)
  assert.match(tradeCss, /@media \(prefers-reduced-motion: reduce\) \{[\s\S]*?\.trade-view button:active,[\s\S]*?transform: none;/)
  assert.match(tradeCss, /\.trade-view \.contract-header-control:active:not\(:disabled\),[\s\S]*?\.trade-view \.contract-pencil-module button:active:not\(:disabled\) \{\s*transform: none;\s*\}/)
})

function read(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

function styleOf(source: string): string {
  const match = source.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)
  assert.ok(match)
  return match[1] || ''
}

function sliceFunction(startToken: string, endToken: string): string {
  const start = tradeSource.indexOf(startToken)
  assert.notEqual(start, -1, `missing start token: ${startToken}`)
  const end = tradeSource.indexOf(endToken, start + startToken.length)
  assert.notEqual(end, -1, `missing end token: ${endToken}`)
  return tradeSource.slice(start, end)
}

function cssRule(selector: string): string {
  const marker = `${selector} {`
  const start = tradeCss.indexOf(marker)
  assert.notEqual(start, -1, `missing CSS rule ${selector}`)
  const openingBrace = tradeCss.indexOf('{', start)
  let depth = 0
  for (let index = openingBrace; index < tradeCss.length; index += 1) {
    if (tradeCss[index] === '{') depth += 1
    if (tradeCss[index] === '}') depth -= 1
    if (depth === 0) return tradeCss.slice(openingBrace + 1, index)
  }
  assert.fail(`unterminated CSS rule ${selector}`)
}
