import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'
import { effectScope, nextTick, ref } from 'vue'
import { createMarginOrderReview } from '../src/core/marginOrderConfirmation.ts'
import { useModalDialog } from '../src/core/modalDialog.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const tradeSource = read('../src/views/TradeView.vue')
const tradingApiSource = read('../src/api/trading.ts')
const tradeCss = styleOf(tradeSource)
const confirmationTemplate = sliceBetween(
  tradeSource,
  '    <Teleport to="body">\n      <div\n        v-if="confirmOpen"',
  '    </Teleport>',
)
const spotPanel = sliceBetween(
  confirmationTemplate,
  '        <section\n          v-if="isSpotMode"',
  '        <section\n          v-else',
)
const contractPanel = confirmationTemplate.slice(confirmationTemplate.indexOf('        <section\n          v-else'))
const submitOrderSource = sliceBetween(
  tradeSource,
  'async function submitOrder(): Promise<void> {',
  'function trapDialogFocus(event: KeyboardEvent): void {',
)

test('确认层 Teleport 到 body，且仅 contract 分支渲染专属杠杆面板', () => {
  assert.match(confirmationTemplate, /^    <Teleport to="body">/)
  assert.match(confirmationTemplate, /v-if="confirmOpen"[\s\S]*?class="confirmation-layer"/)
  assert.match(confirmationTemplate, /:class="\{ 'contract-order-confirm-layer': !isSpotMode \}"/)
  assert.match(confirmationTemplate, /:data-order-confirm-mode="mode"/)

  assert.match(spotPanel, /v-if="isSpotMode"/)
  assert.match(spotPanel, /class="confirmation-sheet"/)
  assert.match(spotPanel, /t\('common\.price'\)[\s\S]*?moneyText\(spotReview\?\.price\)[\s\S]*?spotReview\?\.quoteAsset/)
  assert.match(spotPanel, /t\('common\.amount'\)[\s\S]*?moneyText\(spotReview\?\.quoteAmount\)[\s\S]*?spotReview\?\.quoteAsset/)
  assert.match(spotPanel, /class="confirmation-actions"[\s\S]*?t\('common\.cancel'\)[\s\S]*?t\('common\.confirm'\)/)
  assert.doesNotMatch(spotPanel, /contract-order-confirm|contractNotional|contractQuantity/)

  assert.match(contractPanel, /class="contract-order-confirm"/)
  assert.match(contractPanel, /aria-labelledby="contract-order-confirm-title"/)
  assert.match(contractPanel, /aria-describedby="contract-order-confirm-risk"/)
  assert.doesNotMatch(contractPanel, /class="confirmation-sheet"|class="confirmation-actions"/)
})

test('杠杆确认明细只组合当前表单、行情、产品设置和后端 Logo', () => {
  assert.match(contractPanel, /<AssetMark[\s\S]*?:symbol="baseAsset"[\s\S]*?:src="ticker\?\.iconUrl"[\s\S]*?:fallback-src="ticker\?\.baseIconUrl"[\s\S]*?:size="36"/)
  assert.match(contractPanel, /\{\{ pairSymbol \}\}/)
  assert.match(contractPanel, /t\('trade\.perpetualShort'\)[\s\S]*?contractOrderReview\.request\.orderType === 'limit'[\s\S]*?trade\.limitOrderShort[\s\S]*?trade\.marketOrderShort/)
  assert.match(contractPanel, /:class="contractOrderReview\.request\.side === 'long' \? 'is-long' : 'is-short'"/)
  assert.match(contractPanel, /t\('rootPrototype\.marginMode'\)[\s\S]*?contractOrderReview\.request\.marginMode === 'cross'/)
  assert.match(contractPanel, /t\('rootPrototype\.leverage'\)[\s\S]*?contractOrderReview\.request\.leverage/)
  assert.match(contractPanel, /t\('trade\.contractReferencePrice'\)[\s\S]*?moneyText\(contractOrderReview\.referencePriceText\)[\s\S]*?quoteAsset/)
  assert.match(contractPanel, /t\('trade\.contractMarginCommitted'\)[\s\S]*?moneyText\(contractOrderReview\.request\.marginAmount\)[\s\S]*?availableAsset/)
  assert.match(contractPanel, /t\('rootPrototype\.estimatedNotional'\)[\s\S]*?moneyText\(contractNotional\)[\s\S]*?availableAsset/)
  assert.match(contractPanel, /t\('trade\.contractEstimatedQuantity'\)[\s\S]*?moneyText\(contractQuantity\)[\s\S]*?baseAsset/)
  assert.match(tradeSource, /function createCurrentMarginOrderReview\(idempotencyKey\?: string\)[\s\S]*?productId: selectedProduct\.value\?\.id \|\| 0,[\s\S]*?marginAmount: quantity\.value,[\s\S]*?orderType: contractOrderType\.value,[\s\S]*?limitPrice: contractLimitPrice\.value,[\s\S]*?referencePrice: currentPrice\.value/)
  assert.match(tradeSource, /const marginOrderDraft = computed\(\(\) => createCurrentMarginOrderReview\(\)\)[\s\S]*?const contractOrderReview = computed\(\(\) => marginReview\.value \|\| marginOrderDraft\.value\)/)
  assert.match(tradeSource, /const contractNotional = computed\(\(\) => contractOrderReview\.value\.estimatedNotionalText\)/)
  assert.match(tradeSource, /const contractQuantity = computed\(\(\) => contractOrderReview\.value\.estimatedQuantityText\)/)
  assert.doesNotMatch(contractPanel, /\bBTC(?:USDT|\/USDT)?\b|64,?090|99,?900/)
})

test('杠杆确认模型以同一组值生成展示估算和真实下单参数', () => {
  const review = createMarginOrderReview({
    productId: 42,
    side: 'buy',
    marginMode: 'cross',
    leverage: 7,
    marginAmount: '0.123456789',
    orderType: 'market',
    limitPrice: '',
    pricePrecision: 8,
    referencePrice: 2.5,
  })

  assert.equal(review.isValid, true)
  assert.deepEqual(review.request, {
    productId: 42,
    side: 'long',
    marginMode: 'cross',
    leverage: 7,
    marginAmount: '0.123456789',
    orderType: 'market',
  })
  assert.equal(review.estimatedNotional, 0.123456789 * 7)
  assert.equal(review.estimatedQuantity, (0.123456789 * 7) / 2.5)
  assert.equal('referencePrice' in review.request, false)

  const movedMarket = createMarginOrderReview({
    productId: 42,
    side: 'buy',
    marginMode: 'cross',
    leverage: 7,
    marginAmount: '0.123456789',
    orderType: 'market',
    limitPrice: '',
    pricePrecision: 8,
    referencePrice: 5,
  })
  assert.deepEqual(movedMarket.request, review.request)
  assert.equal(movedMarket.estimatedQuantity, review.estimatedQuantity / 2)

  const shortReview = createMarginOrderReview({
    productId: 42,
    side: 'sell',
    marginMode: 'isolated',
    leverage: 3,
    marginAmount: '10',
    orderType: 'market',
    limitPrice: '',
    pricePrecision: 8,
    referencePrice: 100,
  })
  assert.equal(shortReview.request.side, 'short')
  assert.equal(shortReview.request.marginMode, 'isolated')

  const unavailableMarket = createMarginOrderReview({
    productId: 42,
    side: 'buy',
    marginMode: 'cross',
    leverage: 7,
    marginAmount: '10',
    orderType: 'market',
    limitPrice: '',
    pricePrecision: 8,
    referencePrice: 0,
  })
  assert.equal(unavailableMarket.isValid, false)
  assert.equal(unavailableMarket.estimatedQuantity, 0)

  const overflowedEstimate = createMarginOrderReview({
    productId: 42,
    side: 'buy',
    marginMode: 'cross',
    leverage: Number.MAX_VALUE,
    marginAmount: '100000000000000000000',
    orderType: 'market',
    limitPrice: '',
    pricePrecision: 8,
    referencePrice: 1,
  })
  assert.equal(overflowedEstimate.isValid, false)
  assert.equal(overflowedEstimate.estimatedNotional, 0)
  assert.equal(overflowedEstimate.estimatedQuantity, 0)
})

test('重试继续调用同一真实杠杆接口，失败留在面板且提交期间防重与禁关', () => {
  assert.match(submitOrderSource, /if \(submitting\.value\) return/)
  assert.match(submitOrderSource, /const spot = spotReview\.value[\s\S]*?const review = marginReview\.value[\s\S]*?const submittedMode = review \? 'contract' : spot \? 'spot' : mode\.value/)
  assert.match(submitOrderSource, /type: spot\.orderType/)
  assert.match(submitOrderSource, /if \(submittedMode === 'spot'\) \{\s*if \(!isLive\.value \|\| !spot\)/)
  assert.equal(submitOrderSource.match(/placeMarginOrder\(/g)?.length, 1)
  assert.match(submitOrderSource, /review\.request\.productId !== product\.id[\s\S]*?validateMarginAmount\(\{[\s\S]*?amount: review\.request\.marginAmount,[\s\S]*?!requestMarginValidation\.isValid[\s\S]*?!review\.isValid[\s\S]*?if \(!review \|\| !review\.marginAmountText\) return[\s\S]*?await placeMarginOrder\(\{\s*\.\.\.review\.request,\s*marginAmount: review\.marginAmountText,\s*price: review\.request\.price,\s*\}\)/)
  assert.match(tradeSource, /const marginSnapshot = mode\.value === 'contract'[\s\S]*?createCurrentMarginOrderReview\(createMarginOrderIdempotencyKey\(\)\)[\s\S]*?marginReview\.value = marginSnapshot/)
  assert.match(tradingApiSource, /idempotency_key: input\.idempotencyKey \|\| createMarginOrderIdempotencyKey\(\)/)

  const failureBranch = submitOrderSource.match(/\} catch \(reason\) \{([\s\S]*?)\} finally \{/)?.[1]
  assert.ok(failureBranch)
  assert.match(failureBranch, /setFeedback\(submittedMode === 'contract'[\s\S]*?marginOrderFailureMessage\(reason\)[\s\S]*?apiErrorMessage\(reason, t\('trade\.orderFailed'\)\)\)/)
  assert.doesNotMatch(failureBranch, /confirmOpen\.value = false|quantity\.value = ''/)

  assert.match(contractPanel, /<footer class="contract-order-confirm__actions">[\s\S]*?v-if="feedback && !feedbackIsPositive"[\s\S]*?class="contract-order-confirm__error"[\s\S]*?role="alert"[\s\S]*?aria-live="assertive"/)
  assert.match(tradeSource, /function closeConfirm\(\): void \{\s*if \(submitting\.value\) return\s*confirmOpen\.value = false/)
  assert.match(confirmationTemplate, /class="confirmation-overlay-dismiss"[\s\S]*?:disabled="submitting"[\s\S]*?@click="closeConfirm"/)
  assert.match(contractPanel, /class="contract-order-confirm__close"[\s\S]*?:disabled="submitting"[\s\S]*?@click="closeConfirm"/)
  assert.match(contractPanel, /class="contract-order-confirm__submit"[\s\S]*?:disabled="submitting \|\| productsLoading"[\s\S]*?@click="submitOrder"/)
  assert.match(contractPanel, /:aria-busy="submitting"/)
})

test('遮罩、关闭、Escape、Tab 循环、滚动锁和焦点恢复合同保持完整', () => {
  assert.match(contractPanel, /data-dialog-cancel[\s\S]*?class="contract-order-confirm__close"/)
  assert.match(contractPanel, /role="dialog"[\s\S]*?aria-modal="true"[\s\S]*?tabindex="-1"[\s\S]*?@keydown="trapDialogFocus"/)
  assert.match(tradeSource, /useModalDialog\(confirmOpen, confirmDialog, '\[data-dialog-cancel\]'\)/)
  assert.match(tradeSource, /setConfirmReturnFocus\(trigger instanceof HTMLElement \? trigger : reviewButton\.value\)/)
  assert.match(tradeSource, /reviewContractOrder\('buy', \$event\)/)
  assert.match(tradeSource, /reviewContractOrder\('sell', \$event\)/)
  assert.match(tradeSource, /function trapDialogFocus\(event: KeyboardEvent\): void \{\s*trapConfirmFocus\(event, closeConfirm\)/)
  assert.match(tradeSource, /watch\(submitting, async \(busy\) => \{[\s\S]*?confirmDialog\.value\?\.focus\(\)/)
})

test('共享对话框行为会锁定滚动、聚焦取消、循环 Tab 并恢复精确触发器', async () => {
  const documentDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'document')
  const htmlElementDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'HTMLElement')
  const originalWarn = console.warn
  let activeElement: FakeDialogElement | null = null
  const body = new FakeDialogElement('body', () => { activeElement = body })
  body.style.overflow = 'clip'
  const trigger = new FakeDialogElement('short-trigger', () => { activeElement = trigger })
  const close = new FakeDialogElement('close', () => { activeElement = close })
  const submit = new FakeDialogElement('submit', () => { activeElement = submit })
  const dialogElement = new FakeDialogElement('dialog', () => { activeElement = dialogElement })
  dialogElement.initial = close
  dialogElement.focusable = [close, submit]
  activeElement = body

  Object.defineProperty(globalThis, 'HTMLElement', {
    configurable: true,
    writable: true,
    value: FakeDialogElement,
  })
  Object.defineProperty(globalThis, 'document', {
    configurable: true,
    writable: true,
    value: {
      body,
      get activeElement() {
        return activeElement
      },
    },
  })

  const open = ref(false)
  const dialog = ref(dialogElement as unknown as HTMLElement)
  const scope = effectScope()
  let modal!: ReturnType<typeof useModalDialog>

  try {
    console.warn = () => {}
    scope.run(() => {
      modal = useModalDialog(open, dialog, '[data-dialog-cancel]')
    })
    console.warn = originalWarn

    modal.setReturnFocus(trigger as unknown as HTMLElement)
    open.value = true
    await nextTick()
    await nextTick()
    assert.equal(body.style.overflow, 'hidden')
    assert.equal(activeElement, close)

    submit.focus()
    const forwardTab = fakeKeyboardEvent('Tab', dialogElement)
    modal.trapFocus(forwardTab.event, () => {})
    assert.equal(forwardTab.prevented(), true)
    assert.equal(activeElement, close)

    const reverseTab = fakeKeyboardEvent('Tab', dialogElement, true)
    modal.trapFocus(reverseTab.event, () => {})
    assert.equal(reverseTab.prevented(), true)
    assert.equal(activeElement, submit)

    dialogElement.focusable = []
    const busyTab = fakeKeyboardEvent('Tab', dialogElement)
    modal.trapFocus(busyTab.event, () => {})
    assert.equal(busyTab.prevented(), true)
    assert.equal(activeElement, dialogElement)

    let closeCalls = 0
    const escape = fakeKeyboardEvent('Escape', dialogElement)
    modal.trapFocus(escape.event, () => { closeCalls += 1 })
    assert.equal(escape.prevented(), true)
    assert.equal(closeCalls, 1)

    open.value = false
    await nextTick()
    await nextTick()
    assert.equal(body.style.overflow, 'clip')
    assert.equal(activeElement, trigger)
  } finally {
    console.warn = originalWarn
    scope.stop()
    restoreGlobalProperty('document', documentDescriptor)
    restoreGlobalProperty('HTMLElement', htmlElementDescriptor)
  }
})

test('专属面板满足三行布局、明暗主题、安全区、320px 和减少动态效果', () => {
  const layerRule = cssRule('.contract-order-confirm-layer')
  const panelRule = cssRule('.contract-order-confirm')
  const spotLayerRule = cssRule(".confirmation-layer[data-order-confirm-mode='spot']")
  const bodyRule = cssRule('.contract-order-confirm__body')
  const actionsRule = cssRule('.contract-order-confirm__actions')
  const errorRule = cssRule('.contract-order-confirm__error')
  const closeRule = cssRule('.contract-order-confirm__close')
  const submitRule = cssRule('.contract-order-confirm__submit')
  const darkRule = cssRule("html[data-theme='dark'] .contract-order-confirm")
  const riskRule = cssRule('.contract-order-confirm__risk')

  assert.match(layerRule, /position: fixed;/)
  assert.match(layerRule, /height: 100dvh;/)
  assert.match(layerRule, /width: min\(100%, 448px\);/)
  assert.match(layerRule, /overflow: hidden;/)
  assert.match(layerRule, /overscroll-behavior: contain;/)
  assert.match(panelRule, /border-radius: 22px 22px 0 0;/)
  assert.match(panelRule, /grid-template-rows: auto minmax\(0, 1fr\) auto;/)
  assert.match(cssRule('.contract-order-confirm__top'), /grid-template-rows: 14px minmax\(44px, auto\);/)
  assert.match(cssRule('.contract-order-confirm__header'), /min-height: 44px;/)
  assert.match(panelRule, /height: min\(620px, calc\(100vh - max\(12px, env\(safe-area-inset-top, 0px\)\)\)\);/)
  assert.match(panelRule, /height: min\(620px, calc\(100dvh - max\(12px, env\(safe-area-inset-top, 0px\)\)\)\);/)
  assert.match(panelRule, /overflow: hidden;/)
  for (const edge of ['top', 'right', 'bottom', 'left']) {
    assert.match(panelRule, new RegExp(`env\\(safe-area-inset-${edge}, 0px\\)`))
  }
  assert.match(bodyRule, /min-height: 0;/)
  assert.match(bodyRule, /overflow-x: hidden;/)
  assert.match(bodyRule, /overflow-y: auto;/)
  assert.match(bodyRule, /overscroll-behavior: contain;/)
  assert.doesNotMatch(actionsRule, /overflow|position: (?:absolute|fixed)/)
  assert.match(errorRule, /max-height: min\(96px, 18dvh\);/)
  assert.match(errorRule, /overflow-y: auto;/)
  assert.match(errorRule, /overscroll-behavior: contain;/)
  assert.match(closeRule, /height: 44px;/)
  assert.match(closeRule, /width: 44px;/)
  assert.match(submitRule, /height: 48px;/)
  assert.match(submitRule, /border-radius: 24px;/)
  assert.match(riskRule, /var\(--confirm-warning-surface\)/)
  assert.match(riskRule, /var\(--confirm-warning-line\)/)
  assert.match(darkRule, /--confirm-page: #0c100e;/)
  assert.match(darkRule, /--confirm-canvas: #070a09;/)
  assert.match(darkRule, /--confirm-line: #29342e;/)
  assert.match(darkRule, /--confirm-text: #f2f7f4;/)
  assert.match(spotLayerRule, /--page: var\(--surface\);/)
  assert.match(spotLayerRule, /--surface-2: var\(--soft\);/)
  assert.match(spotLayerRule, /--text: var\(--ink\);/)
  assert.match(spotLayerRule, /--cyan: var\(--focus\);/)
  assert.match(tradeCss, /@media \(max-width: 820px\) \{[\s\S]*?\.contract-order-confirm-layer \{[\s\S]*?right: 0;[\s\S]*?width: 100%;/)
  assert.match(tradeCss, /@media \(max-width: 340px\) \{[\s\S]*?\.contract-order-confirm \{[\s\S]*?safe-area-inset-left[\s\S]*?safe-area-inset-right/)
  assert.match(tradeCss, /@media \(prefers-reduced-motion: no-preference\) \{[\s\S]*?\.contract-order-confirm \{[\s\S]*?contract-order-confirm-enter/)
  assert.match(tradeCss, /@media \(prefers-reduced-motion: reduce\) \{[\s\S]*?\.contract-order-confirm-layer \*[\s\S]*?transition-duration: \.01ms !important;/)
  assert.doesNotMatch(panelRule, /100vw|overflow-x: auto/)

  const compiled = compileStyle({
    source: tradeCss,
    filename: 'TradeView.vue',
    id: 'data-v-margin-confirm',
    scoped: true,
  })
  assert.deepEqual(compiled.errors, [])
  assert.match(compiled.code, /\.contract-order-confirm\[data-v-margin-confirm\]/)
  assert.match(compiled.code, /html\[data-theme=['"]dark['"]\] \.contract-order-confirm\[data-v-margin-confirm\]/)
  assert.match(compiled.code, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.contract-order-confirm-layer\[data-v-margin-confirm\]/)
  assert.doesNotMatch(compiled.code, /\.trade-view[^,{]*\.contract-order-confirm/)
})

test('专属确认文案中英文对称，模板只使用 i18n 与 Lucide 图标', () => {
  const keys = [
    'contractOrderConfirmTitle',
    'contractOrderConfirmHint',
    'contractReferencePrice',
    'contractMarginCommitted',
    'contractEstimatedQuantity',
    'marketExecutionRiskTitle',
    'marketExecutionRiskDescription',
    'confirmContractOrder',
  ] as const

  for (const key of keys) {
    assert.equal(typeof zhCN.trade[key], 'string')
    assert.equal(typeof en.trade[key], 'string')
    assert.ok(zhCN.trade[key].length > 0)
    assert.ok(en.trade[key].length > 0)
  }
  assert.deepEqual(Object.keys(zhCN.trade).sort(), Object.keys(en.trade).sort())
  assert.doesNotMatch(contractPanel, /[\u3400-\u9fff]/)
  assert.doesNotMatch(contractPanel, /<svg|\p{Extended_Pictographic}/u)
  assert.match(contractPanel, /<TriangleAlert[\s\S]*?aria-hidden="true"/)
  assert.match(contractPanel, /<X :size="20" aria-hidden="true"/)
  assert.match(contractPanel, /<CheckCircle2 v-if="!submitting"[\s\S]*?aria-hidden="true"/)
})

class FakeDialogElement {
  readonly style = { overflow: '' }
  readonly name: string
  initial: FakeDialogElement | null = null
  focusable: FakeDialogElement[] = []
  private readonly onFocus: () => void

  constructor(name: string, onFocus: () => void) {
    this.name = name
    this.onFocus = onFocus
  }

  focus(): void {
    this.onFocus()
  }

  querySelector<T extends Element>(): T | null {
    return this.initial as unknown as T | null
  }

  querySelectorAll<T extends Element>(): NodeListOf<T> {
    return this.focusable as unknown as NodeListOf<T>
  }
}

function fakeKeyboardEvent(key: string, currentTarget: FakeDialogElement, shiftKey = false): {
  event: KeyboardEvent
  prevented: () => boolean
} {
  let defaultPrevented = false
  return {
    event: {
      key,
      shiftKey,
      currentTarget,
      preventDefault() {
        defaultPrevented = true
      },
    } as unknown as KeyboardEvent,
    prevented: () => defaultPrevented,
  }
}

function restoreGlobalProperty(
  key: 'document' | 'HTMLElement',
  descriptor: PropertyDescriptor | undefined,
): void {
  if (descriptor) Object.defineProperty(globalThis, key, descriptor)
  else Reflect.deleteProperty(globalThis, key)
}

function read(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

function styleOf(source: string): string {
  const match = source.match(/<style\s+scoped\s*>([\s\S]*?)<\/style>/)
  assert.ok(match)
  return match[1] || ''
}

function sliceBetween(source: string, startToken: string, endToken: string): string {
  const start = source.indexOf(startToken)
  assert.notEqual(start, -1, `missing start token: ${startToken}`)
  const end = source.indexOf(endToken, start + startToken.length)
  assert.notEqual(end, -1, `missing end token: ${endToken}`)
  return source.slice(start, end)
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
