import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const tradeSource = readFileSync(new URL('../src/views/TradeView.vue', import.meta.url), 'utf8')
const secondsSource = readFileSync(new URL('../src/views/SecondsView.vue', import.meta.url), 'utf8')
const loanSource = readFileSync(new URL('../src/views/LoanView.vue', import.meta.url), 'utf8')

test('交易页保留现货、合约和 mode 路由合同', () => {
  assert.match(tradeSource, /await placeSpotOrder\(\{\s*symbol: pairSymbol\.value,\s*side: side\.value,\s*type: submittedOrderType,\s*price: limitPrice,\s*quantity: amount,/)
  assert.match(tradeSource, /await placeMarginOrder\(\{\s*productId: selectedProduct\.value\.id,\s*side: side\.value === 'buy' \? 'long' : 'short',\s*marginMode: marginMode\.value,\s*leverage: leverage\.value,\s*marginAmount: amount,/)
  assert.match(tradeSource, /await updateMarginLeverage\(product\.id, nextLeverage\)/)
  assert.match(tradeSource, /watch\(\(\) => route\.query\.mode/)
  assert.match(tradeSource, /navigation\.rememberTradeMode\(mode\.value\)/)
  assert.match(tradeSource, /mode\.value !== 'contract' \|\| !session\.isAuthenticated/)
  assert.match(tradeSource, /v-if="mode === 'contract' && !session\.isAuthenticated"/)
  assert.match(tradeSource, /v-if="mode !== 'contract' \|\| session\.isAuthenticated" class="trade-columns"/)
  assert.doesNotMatch(tradeSource, /class="trade-category"/)
  assert.doesNotMatch(tradeSource, /selectTradeMode/)
  assert.match(tradeSource, /:data-trade-mode="mode"/)
  assert.match(tradeSource, /class="order-book-shell"/)
})

test('秒合约页保留真实产品、钱包、下单和历史合同', () => {
  assert.match(secondsSource, /const productsRequest = fetchSecondsProducts\(\)/)
  assert.match(secondsSource, /session\.isAuthenticated\s*\?\s*await Promise\.all\(\[productsRequest, fetchSecondsOrders\(\), fetchWalletAccounts\(\)\]\)\s*:\s*\[await productsRequest, \[\], \[\]\]/)
  assert.match(secondsSource, /await openSecondsOrder\(\{\s*productId: selected\.value\.id,\s*durationSeconds: cycle\.value\.durationSeconds,\s*direction: direction\.value,\s*stakeAmount: amountNumber\.value,/)
  assert.match(secondsSource, /class="seconds-direction-grid"/)
  assert.match(secondsSource, /class="seconds-duration-grid"/)
  assert.match(secondsSource, /class="seconds-amount-field"/)
  assert.match(secondsSource, /class="seconds-orders"/)
  assert.doesNotMatch(secondsSource, /cancelSecondsOrder|\/seconds-contracts\/orders\/\$\{[^}]+\}\/cancel/)
})

test('借贷页保留真实申请、撤销、还款并开放逾期还款', () => {
  assert.match(loanSource, /await applyLoan\(\{[\s\S]*productId: selected\.value\.id,[\s\S]*amount: amountNumber\.value,[\s\S]*collateralAssetId:[\s\S]*collateralAmount:/)
  assert.match(loanSource, /await cancelLoanOrder\(order\.id\)/)
  assert.match(loanSource, /await repayLoanOrder\(order\.id\)/)
  assert.match(loanSource, /status === 'disbursed' \|\| status === 'overdue'/)
  assert.match(loanSource, /requestOrderAction\(order\)/)
  assert.match(loanSource, /confirmOrderAction/)
})

test('三页满足聚焦、确认层、触控和窄屏合同', () => {
  for (const source of [tradeSource, secondsSource, loanSource]) {
    assert.match(source, /:focus-within/)
    assert.match(source, /min-height: 44px/)
    assert.match(source, /env\(safe-area-inset-/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
  }

  assert.match(secondsSource, /role="dialog"/)
  assert.match(secondsSource, /aria-modal="true"/)
  assert.match(secondsSource, /event\.key === 'Escape'/)
  assert.match(secondsSource, /event\.key !== 'Tab'/)
  assert.match(loanSource, /role="dialog"/)
  assert.match(loanSource, /aria-modal="true"/)
  assert.match(loanSource, /event\.key === 'Escape'/)
  assert.match(loanSource, /event\.key !== 'Tab'/)
  assert.match(tradeSource, /@media \(max-width: 340px\)/)
  assert.match(secondsSource, /@media \(max-width: 340px\)/)
  assert.match(loanSource, /@media \(max-width: 340px\)/)
  assert.match(loanSource, /\.loan-list \{\s*grid-template-columns: 1fr;/)
})

test('三个视图使用的静态文案键在中英文资源中均存在', () => {
  const keys = new Set<string>()
  for (const source of [tradeSource, secondsSource, loanSource]) {
    for (const match of source.matchAll(/\bt\('([^']+)'/g)) keys.add(match[1])
  }

  for (const key of keys) {
    assert.notEqual(resolveMessage(zhCN, key), undefined, `zh-CN missing ${key}`)
    assert.notEqual(resolveMessage(en, key), undefined, `en missing ${key}`)
  }
})

function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}
