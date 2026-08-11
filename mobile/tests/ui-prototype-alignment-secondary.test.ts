import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const viewNames = [
  'AccountBindingsView',
  'DepositAssetView',
  'DepositDetailView',
  'DepositNetworkView',
  'EarnView',
  'ForgotPasswordView',
  'KycView',
  'LanguageView',
  'LoanView',
  'LoginTwoFactorView',
  'LoginView',
  'MessageCenterView',
  'NewCoinDetailView',
  'NewCoinRecordsView',
  'NewCoinsView',
  'NewsDetailView',
  'NewsView',
  'PredictionView',
  'QuickRechargeView',
  'ReferralsView',
  'RegisterView',
  'SecurityView',
  'SwapView',
  'WalletLedgerView',
  'WithdrawAssetView',
  'WithdrawView',
  'WithdrawalRecordsView',
] as const

const sources = Object.fromEntries(viewNames.map((name) => [
  name,
  readFileSync(new URL(`../src/views/${name}.vue`, import.meta.url), 'utf8'),
])) as Record<(typeof viewNames)[number], string>
const pageHeaderSource = readFileSync(new URL('../src/components/PageHeader.vue', import.meta.url), 'utf8')
const routerSource = readFileSync(new URL('../src/router/index.ts', import.meta.url), 'utf8')
const prototypeCss = readFileSync(new URL('../src/styles/prototype-base.css', import.meta.url), 'utf8')
const prototypeManagedContent = new Set(['LoanView', 'MessageCenterView', 'SecurityView'])
const selectedCss = readFileSync(new URL('../src/styles/pencil-selected-pages.css', import.meta.url), 'utf8')
const pencilSelectedViews = new Set([
  'EarnView',
  'LoanView',
  'LoginView',
  'NewCoinDetailView',
  'NewCoinsView',
  'NewsDetailView',
  'NewsView',
  'RegisterView',
  'SwapView',
])

test('二级页面使用场景 Header 或完整认证身份区', () => {
  for (const name of viewNames) {
    const source = sources[name]
    if (name === 'LoginView') {
      assert.match(source, /class="auth-brand-row"/)
      assert.match(source, /data-pencil-source="u99Fpg WNbsc"/)
      continue
    }
    if (name === 'RegisterView') {
      assert.match(source, /class="auth-brand-row"/)
      assert.match(source, /data-pencil-source="MCuqb RGYGj"/)
      continue
    }
    if (name === 'MessageCenterView') {
      assert.match(source, /<header class="message-root-header">/)
      assert.doesNotMatch(source, /<PageHeader/)
      continue
    }
    assert.match(source, /<PageHeader[\s\S]*?:back="true"/, `${name} missing explicit secondary back action`)
    if (source.includes(':pencil="true"')) {
      assert.match(source, /<PageHeader[\s\S]*?:pencil="true"/, `${name} missing Pencil header mode`)
    } else {
      assert.match(source, /<PageHeader[\s\S]*?:eyebrow=/, `${name} missing header eyebrow`)
      assert.match(source, /<PageHeader[\s\S]*?:subtitle=/, `${name} missing header subtitle`)
    }
  }
})

test('新闻页复用共享 44px 返回入口并以产品中心作为直开兜底', () => {
  const source = sources.NewsView

  assert.match(source, /<PageHeader class="news-pencil__header" :back="true" :pencil="true"/)
  assert.doesNotMatch(source, /\bArrowLeft\b|\bgoBackOr\b/)
  assert.match(pageHeaderSource, /import \{ ArrowLeft \} from 'lucide-vue-next'/)
  assert.match(pageHeaderSource, /import \{ goBackOr \} from '@\/core\/navigation'/)
  assert.match(pageHeaderSource, /class="icon-button page-header__back"/)
  assert.match(pageHeaderSource, /:aria-label="showBack \? t\('common\.back'\) : undefined"/)
  assert.match(
    pageHeaderSource,
    /\.pencil-page-header :deep\(\.icon-button\)\s*\{[\s\S]*?height: 44px !important;[\s\S]*?width: 44px !important;/,
  )
  assert.match(
    routerSource,
    /path: '\/news', name: 'news', component: NewsView, meta: \{ showBottomNav: false, depth: 1, backFallback: '\/products' \}/,
  )

  assert.match(source, /normalizeNewsCategory\(route\.query\.category\)/)
  assert.match(source, /@click="searchOpen = !searchOpen"/)
  assert.match(source, /@click="activeCategory = category\.value"/)
})

test('重点业务页保留各自的信息层级和完整状态表面', () => {
  assert.match(sources.MessageCenterView, /data-pencil-source="FkZ6j bRz9K t7j6n eSMHf"/)
  assert.match(sources.MessageCenterView, /class="message-root-header"/)
  assert.match(sources.MessageCenterView, /class="message-list"/)
  assert.match(sources.MessageCenterView, /\.message-filter-bar\s*\{[\s\S]*?display: flex;[\s\S]*?height: 38px;/)
  assert.equal((sources.MessageCenterView.match(/\{ value: '(?:all|account|funds|trade)'/g) || []).length, 4)
  assert.doesNotMatch(sources.MessageCenterView, /inbox-summary|grid-template-columns: repeat\(5/)
  assert.match(sources.MessageCenterView, /message-row--unread/)

  assert.match(sources.LoanView, /class="loan-hero-pencil"/)
  assert.match(sources.LoanView, /class="loan-product-pencil"/)
  assert.match(sources.LoanView, /class="loan-application-pencil"/)
  assert.match(sources.LoanView, /:class="\{ 'is-invalid': amountInvalid \}"/)
  assert.match(sources.LoanView, /:disabled="submitting \|\| \(session\.isAuthenticated && !canApply\)"/)

  assert.match(
    sources.SecurityView,
    /class="security-hero"[\s\S]*?:data-protection-score="securityReady \? protectionPercent : '--'"/,
  )
  assert.match(sources.SecurityView, /class="security-methods"/)
  assert.match(sources.SecurityView, /class="security-method__state is-positive"/)
  assert.match(
    sources.SecurityView,
    /\.security-hero,\s*\.compact-state,\s*\.account-login-state\s*\{[\s\S]*?grid-template-columns: 44px minmax\(0, 1fr\);/,
  )
  assert.match(
    sources.SecurityView,
    /\.security-methods\s*\{[\s\S]*?display: flex;[\s\S]*?flex-direction: column;[\s\S]*?gap: 12px;/,
  )
  assert.match(
    sources.SecurityView,
    /\.security-method\s*\{[\s\S]*?height: 52px;[\s\S]*?min-height: 52px;/,
  )
  assert.match(sources.SecurityView, /canUpdateLoginPassword/)
  assert.match(sources.SecurityView, /canUpdateFundPassword/)

  for (const name of ['DepositDetailView', 'WithdrawView', 'WalletLedgerView', 'WithdrawalRecordsView'] as const) {
    assert.match(sources[name], /(?:network|address|fee|balance|record|ledger)/i)
  }
  for (const name of ['EarnView', 'NewCoinsView', 'PredictionView', 'SwapView'] as const) {
    assert.match(sources[name], /(?:overview|workspace|project|product|history|orders|holdings)/i)
  }
})

test('组合输入把焦点、错误和禁用反馈放在完整字段容器', () => {
  for (const name of [
    'AccountBindingsView',
    'DepositAssetView',
    'EarnView',
    'ForgotPasswordView',
    'KycView',
    'LoanView',
    'LoginTwoFactorView',
    'LoginView',
    'NewCoinDetailView',
    'NewCoinRecordsView',
    'PredictionView',
    'QuickRechargeView',
    'ReferralsView',
    'RegisterView',
    'SecurityView',
    'SwapView',
    'WithdrawAssetView',
    'WithdrawView',
  ] as const) {
    const contracts = pencilSelectedViews.has(name) ? `${sources[name]}\n${selectedCss}` : sources[name]
    assert.match(contracts, /:focus-within/, `${name} missing container focus`)
  }
  assert.match(sources.KycView, /\.kyc-field:focus-within/)
  assert.match(selectedCss, /\.pencil-field__shell\.is-invalid/)
  assert.match(sources.WithdrawView, /aria-invalid/)
  assert.match(sources.QuickRechargeView, /aria-invalid/)
})

test('全部二级页面遵守窄屏、主题变量、无固定宽屏溢出和 Lucide 合同', () => {
  for (const [name, source] of Object.entries(sources)) {
    const styles = source.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1] || ''
    const contracts = pencilSelectedViews.has(name) ? `${styles}\n${selectedCss}` : styles
    assert.match(contracts, /@media \(max-width: 340px\)/, `${name} missing 340px layout`)
    if (prototypeManagedContent.has(name) && !pencilSelectedViews.has(name)) {
      assert.match(
        prototypeCss,
        /\.secondary-content\s*\{[\s\S]*?env\(safe-area-inset-bottom\)/,
        `${name} missing shared safe area`,
      )
    } else {
      assert.match(contracts, /env\(safe-area-inset-bottom\)/, `${name} missing safe area`)
    }
    assert.doesNotMatch(styles, /(?:^|\n)\s*width:\s*(?:[5-9]\d{2}|\d{4,})px/, `${name} can overflow a phone viewport`)
    assert.doesNotMatch(contracts, /rgba?\(11,\s*24,\s*17/i)
    assert.doesNotMatch(source, /<svg/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
  }
})

test('二级页面新增静态文案全部来自完整的中英文资源', () => {
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
