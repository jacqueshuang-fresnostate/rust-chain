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
const prototypeCss = readFileSync(new URL('../src/styles/prototype-base.css', import.meta.url), 'utf8')
const prototypeManagedContent = new Set(['LoanView', 'MessageCenterView', 'SecurityView'])

test('二级页面使用场景 Header 或完整认证身份区', () => {
  for (const name of viewNames) {
    const source = sources[name]
    if (name === 'LoginView') {
      assert.match(source, /class="login-panel__logo"/)
      assert.match(source, /class="auth-progress"/)
      continue
    }
    if (name === 'RegisterView') {
      assert.match(source, /class="register-form__intro"/)
      assert.match(source, /auth\.stepProgress/)
      continue
    }
    assert.match(source, /<PageHeader[\s\S]*?:back="true"/, `${name} missing explicit secondary back action`)
    assert.match(source, /<PageHeader[\s\S]*?:eyebrow=/, `${name} missing header eyebrow`)
    assert.match(source, /<PageHeader[\s\S]*?:subtitle=/, `${name} missing header subtitle`)
  }
})

test('重点业务页保留各自的信息层级和完整状态表面', () => {
  assert.match(sources.MessageCenterView, /class="inbox-summary"/)
  assert.match(sources.MessageCenterView, /grid-template-columns: repeat\(2, minmax\(0, 1fr\)\)/)
  assert.match(sources.MessageCenterView, /\.message-filter-bar\s*\{[\s\S]*?grid-template-columns: repeat\(5, minmax\(0, 1fr\)\)/)
  assert.match(sources.MessageCenterView, /message-row--unread/)

  assert.match(sources.LoanView, /\.loan-list \{\s*display: grid;[\s\S]*grid-template-columns: repeat\(2, minmax\(0, 1fr\)\)/)
  assert.match(sources.LoanView, /@media \(max-width: 340px\) \{[\s\S]*\.loan-list \{\s*grid-template-columns: 1fr;/)
  assert.match(sources.LoanView, /:class="\{ 'is-invalid': amountInvalid \}"/)
  assert.match(sources.LoanView, /:disabled="submitting \|\| \(session\.isAuthenticated && !canApply\)"/)

  assert.match(sources.SecurityView, /class="protection-overview"/)
  assert.match(sources.SecurityView, /class="security-checklist"/)
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
    assert.match(sources[name], /:focus-within/, `${name} missing container focus`)
  }
  assert.match(sources.KycView, /\.kyc-field:focus-within/)
  assert.match(sources.LoanView, /\.loan-field\.is-invalid:focus-within/)
  assert.match(sources.WithdrawView, /aria-invalid/)
  assert.match(sources.QuickRechargeView, /aria-invalid/)
})

test('全部二级页面遵守窄屏、硬边界、主题变量和 Lucide 合同', () => {
  for (const [name, source] of Object.entries(sources)) {
    const styles = source.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1] || ''
    assert.match(styles, /@media \(max-width: 340px\)/, `${name} missing 340px layout`)
    if (prototypeManagedContent.has(name)) {
      assert.match(
        prototypeCss,
        /\.secondary-content\s*\{[\s\S]*?env\(safe-area-inset-bottom\)/,
        `${name} missing shared safe area`,
      )
    } else {
      assert.match(styles, /env\(safe-area-inset-bottom\)/, `${name} missing safe area`)
    }
    assert.doesNotMatch(styles, /border-radius:\s*(?:[1-9]\d+|999)px/, `${name} has a radius over 8px`)
    assert.doesNotMatch(styles, /rgba?\(11,\s*24,\s*17/i)
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
