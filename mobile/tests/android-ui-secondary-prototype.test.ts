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

test('登录与注册逐项映射 Pencil 单页身份区且不叠加额外 Header 动作', () => {
  for (const source of [sources.login, sources.register]) {
    assert.match(source, /class="auth-pencil-page/)
    assert.match(source, /class="auth-pencil-canvas"/)
    assert.match(source, /class="auth-brand-row"/)
    assert.match(source, /class="auth-pencil-title"/)
    assert.match(source, /data-pencil-source=/)
    assert.match(source, /\.auth-brand-row \{[\s\S]*?height: 62px;/)
    assert.doesNotMatch(source, /@click="(?:handleBack|openLanguage)"|<Languages|<X :size="22"/)
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

test('消息、借贷与安全工作台使用选中稿连续画布和真实业务摘要', () => {
  assert.match(sources.messageCenter, /class="page page--plain pencil-page message-center-page"/)
  assert.match(sources.messageCenter, /data-message-workspace="live"/)
  assert.match(sources.security, /class="page page--plain pencil-page security-view"/)
  assert.match(sources.security, /data-security-workspace="live"/)
  assert.doesNotMatch(sources.messageCenter, /page--prototype-grid|secondary-view|secondary-content/)
  assert.doesNotMatch(sources.security, /page--prototype-grid|secondary-view|secondary-content/)
  assert.match(sources.loan, /class="page page--plain pencil-page loan-pencil"/)
  assert.match(sources.loan, /data-pencil-source="kIOBX yrsRy"/)
  assert.match(sources.loan, /data-loan-workspace="live"/)

  assert.match(sources.messageCenter, /messages\.value = await fetchNews\(40\)/)
  assert.match(sources.messageCenter, /class="message-root-header"/)
  assert.match(sources.messageCenter, /class="message-list"[\s\S]*data-message-source="live"/)
  assert.match(sources.messageCenter, /\.message-row,[\s\S]*?min-height: 64px;/)

  assert.match(sources.loan, /const loanAssetFilters = computed/)
  assert.match(sources.loan, /class="loan-access-pencil__summary"/)
  assert.match(sources.loan, /class="pencil-segmented pencil-segmented--soft loan-categories"/)
  assert.match(sources.loan, /class="pencil-note loan-risk-note"/)
  assert.match(sources.loan, /class="loan-hero-pencil"/)

  assert.match(sources.security, /const protectionCount = computed/)
  assert.match(sources.security, /profile\.value\?\.emailVerified/)
  assert.match(sources.security, /twoFactor\.value\?\.totpEnabled/)
  assert.match(sources.security, /class="security-hero"/)
  assert.match(sources.security, /class="security-methods"/)
})
