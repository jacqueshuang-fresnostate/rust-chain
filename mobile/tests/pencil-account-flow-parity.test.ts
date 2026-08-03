import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const sources = {
  message: readView('MessageCenterView'),
  security: readView('SecurityView'),
  kyc: readView('KycView'),
  bindings: readView('AccountBindingsView'),
  referrals: readView('ReferralsView'),
  language: readView('LanguageView'),
  twoFactor: readView('LoginTwoFactorView'),
  forgotPassword: readView('ForgotPasswordView'),
}

const authEntrySources = {
  login: readView('LoginView'),
  register: readView('RegisterView'),
}

const accountCanvasRoots = [
  '.message-center-page',
  '.login-two-factor-page',
  '.auth-page',
  '.security-view',
  '.kyc-page',
  '.account-bindings-page',
  '.referrals-page',
  '.language-page',
] as const

const selectedPageCss = readFileSync(new URL('../src/styles/pencil-selected-pages.css', import.meta.url), 'utf8')
const legacyParityCss = readFileSync(new URL('../src/styles/prototype-parity.css', import.meta.url), 'utf8')

const secondaryViews = [
  sources.security,
  sources.kyc,
  sources.bindings,
  sources.referrals,
  sources.language,
  sources.twoFactor,
  sources.forgotPassword,
]

test('账户所选页面使用组合选择器锁定白色与纯黑根画布且不覆盖登录注册', () => {
  const lightSelectors = accountCanvasRoots.map((root) => `.app-stage .mobile-canvas ${root}`)
  const darkSelectors = accountCanvasRoots.map((root) => `html[data-theme='dark'] .app-stage .mobile-canvas ${root}`)
  const lightDeclarations = declarationsForSelectorGroup(selectedPageCss, lightSelectors)
  const darkDeclarations = declarationsForSelectorGroup(selectedPageCss, darkSelectors)

  assert.equal(declarationValue(lightDeclarations, '--page'), '#ffffff')
  assert.equal(declarationValue(lightDeclarations, '--surface'), '#ffffff')
  assert.equal(declarationValue(lightDeclarations, 'background'), '#ffffff')
  assert.equal(declarationValue(darkDeclarations, '--page'), '#000000')
  assert.equal(declarationValue(darkDeclarations, '--surface'), '#000000')
  assert.equal(declarationValue(darkDeclarations, 'background'), '#000000')

  const sharedDeclarations = declarationsForSelectorGroup(selectedPageCss, ['.pencil-page', '.auth-pencil-page'])
  assert.equal(declarationValue(sharedDeclarations, '--page'), '#f7f9f8')
  assert.equal(declarationValue(sharedDeclarations, '--surface'), '#f7f9f8')
  assert.equal(declarationValue(sharedDeclarations, 'background'), 'var(--page)')
  assert.match(authEntrySources.login, /class="auth-pencil-page login-pencil"/)
  assert.match(authEntrySources.register, /class="auth-pencil-page register-pencil"/)
  assert.doesNotMatch(authEntrySources.login, /class="[^"]*\bauth-page\b/)
  assert.doesNotMatch(authEntrySources.register, /class="[^"]*\bauth-page\b/)
  assert.doesNotMatch([...lightSelectors, ...darkSelectors].join('\n'), /\.auth-pencil-page|\.login-pencil|\.register-pencil/)
  assert.doesNotMatch(selectedPageCss, /:global\s*\(/)
})

test('消息中心映射 56px 返回 Header、四分类和 64px 连续真实公告列表', () => {
  const source = sources.message
  assert.match(source, /data-pencil-source="FkZ6j bRz9K"/)
  assert.match(source, /class="message-root-header"/)
  assert.match(source, /class="message-header-back"[\s\S]*?<ArrowLeft :size="22"/)
  assert.match(source, /void goBackOr\(router, route\.meta\.backFallback \|\| \{ name: 'home' \}\)/)
  assert.match(source, /t\('messageCenter\.markAllReadShort'\)/)
  assert.doesNotMatch(source, /<PageHeader|:back=/)
  assert.match(source, /const MESSAGE_CATEGORIES:[\s\S]*?value: 'all'[\s\S]*?value: 'account'[\s\S]*?value: 'funds'[\s\S]*?value: 'trade'/)
  assert.doesNotMatch(source.match(/const MESSAGE_CATEGORIES[\s\S]*?\n\]/)?.[0] || '', /announcement/)
  assert.match(source, /\.message-root-header\s*\{[\s\S]*?grid-template-columns:\s*40px minmax\(0, 1fr\) 49px;[\s\S]*?height:\s*56px;[\s\S]*?padding:\s*12px 20px 4px;/)
  assert.match(source, /\.message-root-header h1\s*\{[\s\S]*?font-size:\s*22px;[\s\S]*?font-weight:\s*750;/)
  assert.match(source, /\.message-read-all\s*\{[\s\S]*?font-size:\s*12px;[\s\S]*?font-weight:\s*600;/)
  assert.match(source, /\.message-filter-bar\s*\{[\s\S]*?gap:\s*20px;[\s\S]*?height:\s*38px;[\s\S]*?padding:\s*8px 20px 4px;/)
  assert.match(source, /\.message-list\s*\{[\s\S]*?padding:\s*6px 20px 0;/)
  assert.match(source, /\.message-row\s*\{[\s\S]*?height:\s*64px;[\s\S]*?min-height:\s*64px;/)
  assert.doesNotMatch(source, /\.message-row \+ \.message-row[\s\S]*?border-top:/)
  assert.match(source, /\.message-icon\s*\{[\s\S]*?border-radius:\s*50%;[\s\S]*?height:\s*40px;[\s\S]*?width:\s*40px;/)
  assert.doesNotMatch(legacyParityCss, /\.message-icon/)
  assert.match(source, /messages\.value = await fetchNews\(40\)/)
  assert.match(source, /router\.push\(\{ name: 'news-detail', params: \{ id: String\(message\.id\) \} \}\)/)
  assert.doesNotMatch(source, /messages\.value\s*=\s*\[/)
  assert.doesNotMatch(source, /markAllReadLabel|grid-template-columns:\s*repeat\(5/)
})

test('账户二级页统一使用 60px Pencil PageHeader 并从去除原生状态栏后的 Body 起排版', () => {
  for (const source of secondaryViews) {
    assert.match(source, /<PageHeader[\s\S]*?:back="true"/)
    assert.match(source, /<PageHeader[\s\S]*?:pencil="true"/)
    assert.match(source, /class="[^"]*pencil-content/)
    assert.match(source, /env\(safe-area-inset-bottom\)/)
    assert.match(source, /@media \(max-width: 340px\)/)
    assert.match(source, /background-image:\s*none;/)
  }
  assert.match(sources.message, /background-image:\s*none;/)

  for (const source of [sources.security, sources.kyc, sources.bindings, sources.referrals, sources.language]) {
    assert.match(source, /padding-top:\s*6px;|padding:\s*6px 20px/)
  }
  for (const source of [sources.twoFactor, sources.forgotPassword]) {
    assert.match(source, /padding-top:\s*10px;/)
  }
})

test('八个页面保留选中稿坐标锚点和关键行高', () => {
  assert.match(sources.twoFactor, /data-pencil-source="qmNDA kp9wV"/)
  assert.match(sources.twoFactor, /Array\.from\(\{ length: 6 \}/)
  assert.match(sources.twoFactor, /\.otp-control > span:not\(\.sr-only\)[\s\S]*?height:\s*52px;/)
  assert.match(sources.twoFactor, /\.confirm-button\s*\{[\s\S]*?height:\s*48px;/)
  assert.match(sources.twoFactor, /\.login-two-factor-form\s*\{[\s\S]*?gap:\s*14px;/)

  assert.match(sources.forgotPassword, /data-pencil-source="mgAF7 HrPy2"/)
  assert.match(sources.forgotPassword, /v-for="step in 3"/)
  assert.match(sources.forgotPassword, /\.auth-submit\s*\{[\s\S]*?height:\s*48px;/)
  assert.match(sources.forgotPassword, /\.auth-form\s*\{[\s\S]*?gap:\s*14px;/)

  assert.match(sources.security, /data-pencil-source="WZ42z sDl6T"/)
  assert.match(sources.security, /\.state-icon\s*\{[\s\S]*?height:\s*44px;[\s\S]*?width:\s*44px;/)
  assert.match(sources.security, /\.security-method\s*\{[\s\S]*?height:\s*52px;/)
  assert.match(sources.security, /\.security-recovery\s*\{[\s\S]*?height:\s*48px;/)
  assert.match(sources.security, /\.security-content\s*\{[\s\S]*?gap:\s*12px;/)
  assert.match(sources.security, /\.security-view input:focus-visible\s*\{[\s\S]*?outline:\s*0;/)
  assert.match(sources.security, /\.security-methods\s*\{[\s\S]*?gap:\s*12px;/)

  assert.match(sources.kyc, /data-pencil-source="Raoes wJT9Y"/)
  assert.match(sources.kyc, /\.upload-tile\s*\{[\s\S]*?height:\s*72px;/)
  assert.match(sources.kyc, /\.kyc-submit\s*\{[\s\S]*?height:\s*48px;/)
  assert.match(sources.kyc, /\.kyc-content\s*\{[\s\S]*?gap:\s*12px;/)
  assert.match(sources.kyc, /\.kyc-fields\s*\{[\s\S]*?gap:\s*12px;/)
  assert.match(sources.kyc, /\{ kind: 'front'[\s\S]*?\{ kind: 'back'[\s\S]*?\{ kind: 'handheld'/)

  assert.match(sources.bindings, /data-pencil-source="x84Cbv Z0ging"/)
  assert.match(sources.bindings, /\.binding-row\s*\{[\s\S]*?height:\s*52px;/)
  assert.match(sources.bindings, /\.binding-add\s*\{[\s\S]*?height:\s*44px;/)
  assert.match(sources.bindings, /\.bindings-content\s*\{[\s\S]*?gap:\s*10px;/)
  assert.match(sources.bindings, /\.binding-list\s*\{[\s\S]*?gap:\s*10px;/)

  assert.match(sources.referrals, /data-pencil-source="c80gd Bmt4u e4bPj Qy31s"/)
  assert.match(sources.referrals, /\.referral-code\s*\{[\s\S]*?height:\s*56px;/)
  assert.match(sources.referrals, /\.referral-copy-action\s*\{[\s\S]*?height:\s*48px;/)
  assert.match(sources.referrals, /\.invite-row\s*\{[\s\S]*?height:\s*44px;/)
  assert.match(sources.referrals, /\.referrals-content\s*\{[\s\S]*?gap:\s*14px;/)

  assert.match(sources.language, /data-pencil-source="kwFEy yPf6O"/)
  assert.match(sources.language, /\.language-list button\s*\{[\s\S]*?height:\s*52px;/)
  assert.match(sources.language, /\.language-content\s*\{[\s\S]*?gap:\s*8px;/)
  assert.match(sources.language, /\.language-list\s*\{[\s\S]*?gap:\s*8px;/)
})

test('受保护页使用紧凑诚实访客状态和真实 redirect，不恢复旧登录 Hero', () => {
  for (const source of [sources.security, sources.kyc, sources.bindings, sources.referrals]) {
    assert.match(source, /v-if="!session\.isAuthenticated"/)
    assert.match(source, /query: \{ redirect: route\.fullPath \}/)
    assert.match(source, /t\('common\.loginNow'\)/)
    assert.doesNotMatch(source, /LoginRequiredState|PageShell/)
  }
})

test('认证、TOTP、KYC、绑定和邀请继续调用真实接口且不嵌入演示结果', () => {
  for (const call of [
    'changeLoginPassword',
    'changeFundPassword',
    'setFundPassword',
    'setupTwoFactor',
    'confirmTwoFactor',
    'updateLoginTwoFactor',
    'resetUserTwoFactor',
    'resetFundPassword',
  ]) assert.match(sources.security, new RegExp(`\\b${call}\\(`))

  for (const call of ['fetchKycStatus', 'fetchCountries', 'submitKycApplication', 'fileToDataUrl']) {
    assert.match(sources.kyc, new RegExp(`\\b${call}\\(`))
  }
  for (const call of ['fetchThirdPartyBindings', 'bindEmail', 'bindThirdPartyAccount']) {
    assert.match(sources.bindings, new RegExp(`\\b${call}\\(`))
  }
  for (const call of ['fetchReferralCode', 'fetchReferralInvites', 'bindReferralCode']) {
    assert.match(sources.referrals, new RegExp(`\\b${call}\\(`))
  }
  for (const call of ['submitLoginTwoFactor', 'setupLoginTwoFactor', 'confirmLoginTwoFactorSetup', 'resetLoginTwoFactor']) {
    assert.match(sources.twoFactor, new RegExp(`\\b${call}\\(`))
  }
  assert.match(sources.forgotPassword, /resetPasswordWithCode\(\{ email: email\.value, code: code\.value, password: password\.value \}\)/)
  assert.match(sources.forgotPassword, /sanitizeInternalRedirect\(route\.query\.redirect\)/)
  assert.match(sources.twoFactor, /sanitizeInternalRedirect\(route\.query\.redirect\)/)

  const combined = Object.values(sources).join('\n')
  assert.doesNotMatch(combined, /HIPPO88|126\.5|13\*\*\*\*|138\*\*\*\*|21\*\*\*\*/)
  assert.doesNotMatch(combined, /[㐀-鿿]/)
  assert.doesNotMatch(combined, /#[0-9a-f]{3,8}/i)
})

function readView(name: string): string {
  return readFileSync(new URL(`../src/views/${name}.vue`, import.meta.url), 'utf8')
}

function declarationsForSelectorGroup(source: string, expectedSelectors: readonly string[]): string {
  const expected = [...expectedSelectors].sort()
  const rules = source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .matchAll(/([^{}]+)\{([^{}]*)\}/g)
  const matches: string[] = []

  for (const rule of rules) {
    const selectors = rule[1]
      .split(',')
      .map((selector) => selector.trim())
      .filter(Boolean)
      .sort()
    if (selectors.length === expected.length && selectors.every((selector, index) => selector === expected[index])) {
      matches.push(rule[2])
    }
  }

  assert.equal(matches.length, 1, `expected one grouped CSS rule for ${expectedSelectors.join(', ')}`)
  return matches[0]
}

function declarationValue(declarations: string, property: string): string {
  const escapedProperty = property.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = declarations.match(new RegExp(`(?:^|;)\\s*${escapedProperty}\\s*:\\s*([^;]+)`, 'i'))
  assert.ok(match, `missing CSS declaration ${property}`)
  return match[1].trim().toLowerCase()
}
