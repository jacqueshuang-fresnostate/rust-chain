import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const sources = {
  message: readFileSync(new URL('../src/views/MessageCenterView.vue', import.meta.url), 'utf8'),
  loan: readFileSync(new URL('../src/views/LoanView.vue', import.meta.url), 'utf8'),
  security: readFileSync(new URL('../src/views/SecurityView.vue', import.meta.url), 'utf8'),
}
const pageHeaderSource = readFileSync(new URL('../src/components/PageHeader.vue', import.meta.url), 'utf8')
const baseCss = readFileSync(new URL('../src/styles/base.css', import.meta.url), 'utf8')
const prototypeCss = readFileSync(new URL('../src/styles/prototype-base.css', import.meta.url), 'utf8')
const parityCss = readFileSync(new URL('../src/styles/prototype-parity.css', import.meta.url), 'utf8')
const selectedCss = readFileSync(new URL('../src/styles/pencil-selected-pages.css', import.meta.url), 'utf8')

function templateOf(source: string): string {
  return source.match(/<template>([\s\S]*?)<style scoped>/)?.[1] || ''
}

function styleOf(source: string): string {
  return source.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1] || ''
}

test('B2 三个工作台保留真实 API、路由、存储与确认动作', () => {
  assert.match(sources.message, /messages\.value = await fetchNews\(40\)/)
  assert.match(sources.message, /function markAllRead\(\): void/)
  assert.match(sources.message, /readIds\.value = new Set\(\[\.\.\.readIds\.value, \.\.\.messages\.value\.map/)
  assert.match(sources.message, /globalThis\.localStorage\?\.getItem\(READ_IDS_STORAGE_KEY\)/)
  assert.match(sources.message, /globalThis\.localStorage\?\.setItem\(READ_IDS_STORAGE_KEY, JSON\.stringify\(values\)\)/)
  assert.match(sources.message, /router\.push\(\{ name: 'news-detail', params: \{ id: String\(message\.id\) \} \}\)/)
  assert.match(sources.message, /type MessageCategory = 'all' \| 'account' \| 'funds' \| 'trade'/)
  assert.match(sources.message, /const visibleMessages = computed\(\(\) => activeCategory\.value === 'all' \? messages\.value : \[\]\)/)
  assert.doesNotMatch(sources.message, /messages\.value\s*=\s*\[/)

  for (const call of [
    'fetchLoanProducts',
    'fetchLoanOrders',
    'fetchWalletAccounts',
    'applyLoan',
    'cancelLoanOrder',
    'repayLoanOrder',
  ]) {
    assert.match(sources.loan, new RegExp(`\\b${call}\\(`), `${call} contract was dropped`)
  }
  assert.match(sources.loan, /router\.push\(\{ name: 'login', query: \{ redirect: '\/products\/loan' \} \}\)/)
  assert.match(sources.loan, /role="dialog"/)
  assert.match(sources.loan, /aria-modal="true"/)
  assert.match(sources.loan, /event\.key === 'Escape'/)
  assert.match(sources.loan, /event\.key !== 'Tab'/)
  assert.match(sources.loan, /data-dialog-cancel/)
  assert.match(sources.loan, /returnFocus\?\.focus\(\)/)

  for (const call of [
    'fetchUserProfile',
    'fetchTwoFactorStatus',
    'changeLoginPassword',
    'setupTwoFactor',
    'confirmTwoFactor',
    'updateLoginTwoFactor',
    'sendUserTwoFactorResetCode',
    'resetUserTwoFactor',
    'setFundPassword',
    'changeFundPassword',
    'sendFundPasswordResetCode',
    'resetFundPassword',
  ]) {
    assert.match(sources.security, new RegExp(`\\b${call}\\(`), `${call} contract was dropped`)
  }
  assert.match(sources.security, /toDataURL\(setup\.value\.otpAuthUri/)
  assert.match(sources.security, /navigator\.clipboard\.writeText\(setup\.value\.secret\)/)
  assert.match(
    sources.security,
    /function openLogin\(\): void \{\s*void router\.push\(\{ name: 'login', query: \{ redirect: route\.fullPath \} \}\)\s*\}/,
  )
})

test('消息空态、贷款空产品和访客安全状态只呈现一个诚实主舞台', () => {
  const messageTemplate = templateOf(sources.message)
  assert.match(messageTemplate, /class="message-root-header"/)
  assert.match(messageTemplate, /class="message-list"[\s\S]*?data-message-source="live"/)
  assert.match(messageTemplate, /v-if="loading && !messages\.length" class="message-state"/)
  assert.match(messageTemplate, /v-else-if="error && !messages\.length" class="message-state message-state--error"/)
  assert.match(messageTemplate, /v-for="message in visibleMessages"/)
  assert.match(messageTemplate, /v-if="!loading && !error && !visibleMessages\.length" class="message-empty-state"/)
  assert.match(messageTemplate, /class="message-empty-state__plate"><BellOff :size="24"/)
  assert.match(messageTemplate, /<strong>\{\{ emptyTitle \}\}<\/strong>/)
  assert.match(messageTemplate, /<small>\{\{ emptyDescription \}\}<\/small>/)
  assert.doesNotMatch(messageTemplate, /inbox-summary|message-tools|inbox-state|message-timeline/)

  const loanTemplate = templateOf(sources.loan)
  assert.match(loanTemplate, /:data-loan-state="loanWorkspaceState"/)
  assert.match(loanTemplate, /v-for="product in visibleProducts"[\s\S]*?class="loan-product-pencil"/)
  assert.match(loanTemplate, /v-if="!visibleProducts\.length" class="pencil-state loan-products-empty"/)
  assert.match(loanTemplate, /<section v-if="session\.isAuthenticated" class="pencil-section loan-orders-pencil">/)
  assert.match(loanTemplate, /<div v-else class="pencil-state">/)
  assert.doesNotMatch(sources.loan, /loan-card--placeholder/)
  assert.doesNotMatch(loanTemplate, /v-if="!products\.length" class="loan-card/)
  assert.doesNotMatch(loanTemplate, /class="product-terms"><b>--<\/b><b>--<\/b><b>--<\/b>/)

  const securityTemplate = templateOf(sources.security)
  assert.match(securityTemplate, /<section v-if="!session\.isAuthenticated" class="account-login-state"/)
  assert.match(securityTemplate, /<section v-else-if="loading" class="compact-state"/)
  assert.match(securityTemplate, /<section v-else-if="error && !securityReady" class="compact-state compact-state--error"/)
  assert.match(securityTemplate, /<template v-else-if="securityReady">/)
  const guestStage = securityTemplate.match(/<section v-if="!session\.isAuthenticated"[\s\S]*?<\/section>/)?.[0] || ''
  assert.ok(guestStage, 'guest security stage is missing')
  assert.doesNotMatch(guestStage, /['"]--['"]|>\s*--\s*</)
  for (const task of ['two-factor', 'password', 'funds']) {
    assert.ok(
      securityTemplate.indexOf(`data-security-task="${task}"`) > securityTemplate.indexOf('v-else-if="securityReady"'),
      `${task} must be progressively disclosed after authenticated security state`,
    )
  }
})

test('三个工作台遵守 44px、窄屏、主题、安全区和低动态合同', () => {
  for (const [name, source] of Object.entries(sources)) {
    const template = templateOf(source)
    const styles = styleOf(source)
    const contractStyles = name === 'loan' ? `${styles}\n${selectedCss}` : styles
    if (name === 'message') {
      assert.match(source, /class="message-root-header"/)
      assert.doesNotMatch(source, /<PageHeader/)
    } else {
      assert.match(source, /<PageHeader/)
    }
    assert.match(source, /<style scoped>/)
    assert.match(contractStyles, /min-height:\s*(?:44|4[5-9]|5[0-2])px/)
    assert.match(contractStyles, /min-width:\s*0/)
    assert.match(contractStyles, /@media \(max-width: 340px\)/)
    assert.match(contractStyles, /@media \(prefers-reduced-motion: reduce\)/)
    assert.match(contractStyles, /var\(--(?:surface|page|ink|text|muted|line|focus)/)
    assert.match(contractStyles, /focus-(?:visible|within)/)
    assert.doesNotMatch(styles, /#[0-9a-f]{3,8}|rgba?\(/i, `${name} bypasses shared theme tokens`)
    assert.doesNotMatch(styles, /(?:^|\n)\s*width:\s*(?:[5-9]\d{2}|\d{4,})px/, `${name} can overflow a phone viewport`)
    assert.doesNotMatch(template, /<svg\b|\p{Extended_Pictographic}/u)
  }

  assert.match(selectedCss, /--page: #f7f9f8;/)
  assert.match(selectedCss, /--page: #000000;/)
  assert.match(selectedCss, /--ink: #f2f7f4;/)

  assert.match(styleOf(sources.message), /\.message-filter-bar\s*\{[\s\S]*?display: flex;[\s\S]*?gap: 20px;[\s\S]*?height: 38px;/)
  assert.doesNotMatch(styleOf(sources.message), /grid-template-columns: repeat\(5, minmax\(0, 1fr\)\)/)
  assert.match(styleOf(sources.loan), /\.loan-products-pencil\s*\{[\s\S]*?display: grid/)
  assert.match(styleOf(sources.loan), /@media \(max-width: 340px\)/)
  assert.match(selectedCss, /env\(safe-area-inset-bottom\)/)
  assert.match(styleOf(sources.security), /\.security-field\s*\{[\s\S]*?border: 1px solid transparent;[\s\S]*?padding: 6px 10px;/)
  assert.match(styleOf(sources.security), /\.security-field input\s*\{[\s\S]*?min-height: 32px;[\s\S]*?outline: 0;/)
  assert.match(styleOf(sources.security), /\.security-field:focus-within\s*\{[\s\S]*?border-color: var\(--positive\);[\s\S]*?var\(--focus-ring\)/)
  assert.match(styleOf(sources.security), /\.security-view input:focus-visible\s*\{[\s\S]*?box-shadow: none;[\s\S]*?outline: 0;/)

  assert.match(pageHeaderSource, /pencil \? 'pencil-page-header' : 'secondary-header'/)
  assert.match(pageHeaderSource, /class="icon-button page-header__back"/)
  assert.match(parityCss, /\.page-header__title\s*\{[\s\S]*?color: var\(--text\)/)
  assert.match(baseCss, /--app-max-width:\s*448px/)
  assert.match(baseCss, /\.app-frame\s*\{[\s\S]*?max-width: var\(--app-max-width\);[\s\S]*?overflow-x: clip;/)
  assert.match(prototypeCss, /\.secondary-content\s*\{[\s\S]*?env\(safe-area-inset-bottom\)/)
})

test('明暗主题的主标题、状态文案和主操作使用高对比语义令牌', () => {
  const messageCss = styleOf(sources.message)
  const loanCss = styleOf(sources.loan)
  const securityCss = styleOf(sources.security)

  assert.match(messageCss, /\.message-root-header h1\s*\{[\s\S]*?color: var\(--ink\)/)
  assert.match(messageCss, /\.message-read-all\s*\{[\s\S]*?color: var\(--positive\)/)
  assert.match(messageCss, /\.message-read-all:disabled\s*\{[\s\S]*?color: var\(--muted\)/)
  assert.match(messageCss, /\.message-row-copy small,[\s\S]*?\.message-state small\s*\{[\s\S]*?color: var\(--muted\)/)
  assert.match(messageCss, /\.message-state > button\s*\{[\s\S]*?color: var\(--positive\)/)

  assert.match(loanCss, /\.loan-access-pencil__summary > div > span\s*\{[\s\S]*?color: var\(--muted\)/)
  assert.match(loanCss, /\.loan-access-pencil__icon\s*\{[\s\S]*?color: var\(--positive\)/)
  assert.match(loanCss, /\.loan-access-pencil button\s*\{[\s\S]*?background: var\(--ink\);[\s\S]*?color: var\(--surface\)/)
  assert.match(selectedCss, /\.pencil-primary\s*\{[\s\S]*?background: var\(--accent\);[\s\S]*?color: var\(--on-accent\)/)

  assert.match(securityCss, /\.security-feedback p\s*\{[\s\S]*?color: var\(--positive\)/)
  assert.match(securityCss, /\.security-feedback \.security-feedback--error\s*\{[\s\S]*?color: var\(--negative\)/)
  assert.match(securityCss, /\.security-method strong\s*\{[\s\S]*?color: var\(--ink\)/)
  assert.match(securityCss, /\.security-method small\s*\{[\s\S]*?color: var\(--muted\)/)
  assert.match(securityCss, /\.security-method__state\.is-positive\s*\{[\s\S]*?color: var\(--positive\)/)
  assert.match(securityCss, /\.account-login-state \.pencil-primary,[\s\S]*?min-height: 44px;/)
})

test('消息、借贷和安全页使用当前 Pencil 画板而不恢复旧网格 PageShell', () => {
  assert.match(sources.message, /data-pencil-source="FkZ6j bRz9K t7j6n eSMHf"/)
  assert.match(sources.message, /class="message-root-header"/)
  assert.match(sources.message, /class="message-list"/)
  assert.match(sources.loan, /data-pencil-source="kIOBX yrsRy"/)
  assert.match(sources.loan, /class="loan-hero-pencil"/)
  assert.match(sources.security, /data-pencil-source="WZ42z sDl6T"/)
  assert.match(sources.security, /class="security-hero"/)
  assert.match(sources.security, /class="security-methods"/)
  assert.match(sources.loan, /class="loan-product-pencil"/)
  for (const source of Object.values(sources)) {
    assert.doesNotMatch(source, /page--prototype-grid|secondary-view|secondary-content/)
  }
  assert.match(
    selectedCss,
    /\.pencil-page,\s*\.auth-pencil-page\s*\{[\s\S]*?background: var\(--page\)/,
  )
  assert.match(
    selectedCss,
    /\.app-stage \.mobile-canvas \.contract-trade,\s*\.app-stage \.mobile-canvas \.seconds-page,\s*\.app-stage \.mobile-canvas \.product-hub,\s*\.app-stage \.mobile-canvas \.prediction-page\s*\{[\s\S]*?--page: #ffffff;[\s\S]*?--surface: #ffffff;[\s\S]*?background-color: var\(--page\);[\s\S]*?background-image: none;/,
  )
  assert.match(
    selectedCss,
    /html\[data-theme='dark'\] \.app-stage \.mobile-canvas \.contract-trade,\s*html\[data-theme='dark'\] \.app-stage \.mobile-canvas \.seconds-page,\s*html\[data-theme='dark'\] \.app-stage \.mobile-canvas \.product-hub,\s*html\[data-theme='dark'\] \.app-stage \.mobile-canvas \.prediction-page\s*\{[\s\S]*?--page: #000000;[\s\S]*?--surface: #000000;[\s\S]*?background-color: var\(--page\);[\s\S]*?background-image: none;/,
  )
})

test('B2 新增模板文案全部复用既有中英文键', () => {
  const keys = new Set<string>()
  for (const source of Object.values(sources)) {
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
