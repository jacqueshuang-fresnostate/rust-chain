import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const sources = {
  login: readFileSync(new URL('../src/views/LoginView.vue', import.meta.url), 'utf8'),
  register: readFileSync(new URL('../src/views/RegisterView.vue', import.meta.url), 'utf8'),
  swap: readFileSync(new URL('../src/views/SwapView.vue', import.meta.url), 'utf8'),
  newCoinDetail: readFileSync(new URL('../src/views/NewCoinDetailView.vue', import.meta.url), 'utf8'),
  withdraw: readFileSync(new URL('../src/views/WithdrawView.vue', import.meta.url), 'utf8'),
  messageCenter: readFileSync(new URL('../src/views/MessageCenterView.vue', import.meta.url), 'utf8'),
  loan: readFileSync(new URL('../src/views/LoanView.vue', import.meta.url), 'utf8'),
  security: readFileSync(new URL('../src/views/SecurityView.vue', import.meta.url), 'utf8'),
}
const modalHelperSource = readFileSync(new URL('../src/core/modalDialog.ts', import.meta.url), 'utf8')

test('登录与注册工具栏使用原型三层 sticky 信息 Header', () => {
  for (const source of [sources.login, sources.register]) {
    assert.match(source, /class="auth-topbar__copy"/)
    assert.match(source, /\.auth-topbar \{[^}]*background: var\(--surface\)/)
    assert.match(source, /\.auth-topbar \{[^}]*border-bottom: 1px solid var\(--line\)/)
    assert.match(source, /\.auth-topbar \{[^}]*grid-template-columns: 44px minmax\(0, 1fr\) 44px/)
    assert.match(source, /\.auth-topbar \{[^}]*position: sticky/)
    assert.match(source, /\.auth-topbar \{[^}]*z-index: var\(--layer-sticky-header\)/)
    assert.match(source, /\.auth-topbar__copy span \{[^}]*color: var\(--positive\)/)
    assert.match(source, /\.auth-topbar__copy small \{[^}]*color: var\(--muted\)/)
  }
})

test('提币、闪兑与新币资金动作进入可访问底部复核层', () => {
  for (const source of [sources.withdraw, sources.swap, sources.newCoinDetail]) {
    assert.match(source, /role="dialog"/)
    assert.match(source, /aria-modal="true"/)
    assert.match(source, /aria-labelledby=/)
    assert.match(source, /aria-describedby=/)
    assert.match(source, /@click\.self=/)
    assert.match(source, /useModalDialog/)
    assert.match(source, /trapReviewFocus\(event, closeReview\)/)
    assert.match(source, /@keydown="handleReviewKeydown"/)
    assert.match(source, /data-dialog-cancel/)
    assert.match(source, /background: var\(--overlay\)/)
    assert.match(source, /z-index: var\(--layer-overlay\)/)
    assert.match(source, /env\(safe-area-inset-bottom\)/)
  }
  assert.match(modalHelperSource, /event\.key === 'Escape'/)
  assert.match(modalHelperSource, /event\.key !== 'Tab'/)
  assert.match(modalHelperSource, /document\.body\.style\.overflow = 'hidden'/)
  assert.match(modalHelperSource, /returnFocus\?\.focus\(\)/)
})

test('复核层没有改变真实校验、请求与载荷合同', () => {
  assert.match(sources.withdraw, /@submit\.prevent="requestSubmit"/)
  assert.match(sources.withdraw, /numericAmount\.value > available\.value/)
  assert.match(sources.withdraw, /await submitWithdrawal\(\{[\s\S]*assetSymbol: asset\.value\.symbol,[\s\S]*network: selectedNetwork\.value \|\| undefined,[\s\S]*address: address\.value,[\s\S]*amount: numericAmount\.value,[\s\S]*fee: fee\.value,[\s\S]*fundPassword: fundPassword\.value \|\| undefined,[\s\S]*totpCode: totpCode\.value \|\| undefined,/)

  assert.match(sources.swap, /quote\.value = await requestConvertQuote\(selectedPair\.value, amountNumber\.value\)/)
  assert.match(sources.swap, /quote\.value\.expiresAt <= Date\.now\(\)/)
  assert.match(sources.swap, /await confirmConvertQuote\(quote\.value\.quoteId\)/)
  assert.match(sources.swap, /@click="openReview"/)

  assert.match(sources.newCoinDetail, /paymentAmount\.value > selectedAccount\.value\.available/)
  assert.match(sources.newCoinDetail, /await subscribeNewCoin\(\{[\s\S]*symbol: project\.value\.symbol,[\s\S]*quoteAssetId: quoteAssetId\.value,[\s\S]*quoteAmount: amountNumber\.value,[\s\S]*issuePrice: project\.value\.issuePrice,/)
  assert.match(sources.newCoinDetail, /await createNewCoinPurchase\(\{[\s\S]*symbol: project\.value\.symbol,[\s\S]*pairId: project\.value\.postListingPairId,[\s\S]*price: executionPrice\.value,[\s\S]*quantity: amountNumber\.value,/)
  assert.match(sources.newCoinDetail, /@click="requestSubmit"/)
})

test('消息、借贷与安全工作台使用共享原型网格和真实业务摘要', () => {
  for (const source of [sources.messageCenter, sources.loan, sources.security]) {
    assert.match(source, /page--prototype-grid/)
    assert.match(source, /data-(?:message|loan|security)-workspace="live"/)
  }

  assert.match(sources.messageCenter, /messages\.value = await fetchNews\(40\)/)
  assert.match(sources.messageCenter, /message-summary__metrics/)
  assert.match(sources.messageCenter, /var\(--signal-coral\)/)

  assert.match(sources.loan, /collateralProductCount/)
  assert.match(sources.loan, /creditProductCount/)
  assert.match(sources.loan, /session\.isAuthenticated \? orders\.length : '--'/)
  assert.match(sources.loan, /loan-overview__metrics/)

  assert.match(sources.security, /const protectionCount = computed/)
  assert.match(sources.security, /profile\.value\?\.emailVerified/)
  assert.match(sources.security, /twoFactor\.value\?\.totpEnabled/)
  assert.match(sources.security, /security-overview__checks/)
})
