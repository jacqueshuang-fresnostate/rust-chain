import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const sources = {
  login: readFileSync(new URL('../src/views/LoginView.vue', import.meta.url), 'utf8'),
  register: readFileSync(new URL('../src/views/RegisterView.vue', import.meta.url), 'utf8'),
  forgotPassword: readFileSync(new URL('../src/views/ForgotPasswordView.vue', import.meta.url), 'utf8'),
  loginTwoFactor: readFileSync(new URL('../src/views/LoginTwoFactorView.vue', import.meta.url), 'utf8'),
  kyc: readFileSync(new URL('../src/views/KycView.vue', import.meta.url), 'utf8'),
  bindings: readFileSync(new URL('../src/views/AccountBindingsView.vue', import.meta.url), 'utf8'),
  referrals: readFileSync(new URL('../src/views/ReferralsView.vue', import.meta.url), 'utf8'),
  language: readFileSync(new URL('../src/views/LanguageView.vue', import.meta.url), 'utf8'),
}

const allSources = Object.values(sources)
const selectedCss = readFileSync(new URL('../src/styles/pencil-selected-pages.css', import.meta.url), 'utf8')

test('登录与注册保留配置、挑战、重定向和请求载荷合同', () => {
  assert.match(sources.login, /fetchLoginConfig\(\)/)
  assert.match(sources.login, /loginWithPassword\(account\.value, password\.value\)/)
  assert.match(sources.login, /const safeRedirect = computed\(\(\) => sanitizeInternalRedirect\(route\.query\.redirect\)\)/)
  assert.match(sources.login, /name: 'login-two-factor', query: \{ challenge: result\.challengeId, redirect: safeRedirect\.value \}/)
  assert.match(sources.login, /name: 'login-two-factor', query: \{ setup: result\.setupChallengeId, redirect: safeRedirect\.value \}/)
  assert.match(sources.login, /replaceAuthStep\(router, \{ name, query: \{ redirect: safeRedirect\.value \} \}\)/)
  assert.match(sources.login, /data-pencil-source="u99Fpg WNbsc"/)
  assert.doesNotMatch(sources.login, /handleBack|openLanguage|name: 'language'/)
  assert.match(sources.login, /session\.sync\(\)/)
  assert.match(sources.login, /replaceAuthStep\(router, safeRedirect\.value\)/)

  assert.match(sources.register, /Promise\.allSettled\(\[fetchCountries\(\), fetchRegisterConfig\(\)\]\)/)
  assert.match(sources.register, /await sendRegistrationCode\(email\.value\)/)
  assert.match(
    sources.register,
    /registerWithEmail\(\{ email: email\.value, password: password\.value, code: code\.value, countryCode: countryCode\.value, inviteCode: inviteCode\.value \}\)/,
  )
  assert.match(sources.register, /emailCodeRequired\.value && !code\.value\.trim\(\)/)
  assert.match(sources.register, /inviteCodeRequired\.value && !inviteCode\.value\.trim\(\)/)
  assert.match(sources.register, /const safeRedirect = computed\(\(\) => sanitizeInternalRedirect\(route\.query\.redirect\)\)/)
  assert.match(sources.register, /createLoginRedirectTarget\(safeRedirect\.value\)/)
  assert.match(sources.register, /data-pencil-source="MCuqb RGYGj"/)
  assert.match(sources.register, /function returnToLogin\(\): void \{\s*void replaceAuthStep\(router, loginTarget\.value\)\s*\}/)
  assert.doesNotMatch(sources.register, /handleBack|openLanguage|name: 'language'/)
  assert.match(sources.register, /session\.sync\(\)/)
  assert.match(sources.register, /replaceAuthStep\(router, safeRedirect\.value\)/)
  assert.match(sources.register, /replaceAuthStep\(router, loginTarget\.value\)/)
})

test('密码找回和登录二次验证保留发码、重置与安全跳转合同', () => {
  assert.match(sources.forgotPassword, /sendPasswordResetCode\(email\.value\)/)
  assert.match(
    sources.forgotPassword,
    /resetPasswordWithCode\(\{ email: email\.value, code: code\.value, password: password\.value \}\)/,
  )
  assert.match(sources.forgotPassword, /const safeRedirect = computed\(\(\) => sanitizeInternalRedirect\(route\.query\.redirect\)\)/)
  assert.match(sources.forgotPassword, /createLoginRedirectTarget\(safeRedirect\.value\)/)
  assert.match(sources.forgotPassword, /replaceAuthStep\(router, loginTarget\.value\)/)
  assert.match(sources.forgotPassword, /:fallback="loginTarget"/)
  assert.match(sources.forgotPassword, /:prefer-fallback="true"/)

  assert.match(sources.loginTwoFactor, /submitLoginTwoFactor\(challengeId\.value, code\.value\)/)
  assert.match(sources.loginTwoFactor, /setupLoginTwoFactor\(setupChallengeId\.value\)/)
  assert.match(sources.loginTwoFactor, /confirmLoginTwoFactorSetup\(setupChallengeId\.value, setupCode\.value\)/)
  assert.match(sources.loginTwoFactor, /toDataURL\(nextSetup\.otpAuthUri/)
  assert.match(sources.loginTwoFactor, /setup\.secret/)
  assert.match(sources.loginTwoFactor, /sendLoginTwoFactorResetCode\(challengeId\.value\)/)
  assert.match(sources.loginTwoFactor, /resetLoginTwoFactor\(challengeId\.value, resetCode\.value\)/)
  assert.match(sources.loginTwoFactor, /const safeRedirect = computed\(\(\) => sanitizeInternalRedirect\(route\.query\.redirect\)\)/)
  assert.match(sources.loginTwoFactor, /createLoginRedirectTarget\(safeRedirect\.value\)/)
  assert.match(sources.loginTwoFactor, /session\.sync\(\)/)
  assert.match(sources.loginTwoFactor, /replaceAuthStep\(router, safeRedirect\.value\)/)
  assert.match(sources.loginTwoFactor, /await returnToLogin\(\)/)
  assert.match(sources.loginTwoFactor, /replaceAuthStep\(router, loginTarget\.value\)/)
  assert.match(sources.loginTwoFactor, /@click="returnToLogin"/)
})

test('认证与语言跳转不会把敏感表单字段写入 URL 查询参数', () => {
  for (const source of [
    sources.login,
    sources.register,
    sources.forgotPassword,
    sources.language,
  ]) {
    assert.doesNotMatch(
      source,
      /query:\s*\{[^}]*\b(?:account|email|password|confirmation|code|inviteCode)\b[^}]*\}/,
    )
  }
})

test('KYC 保留国家规则、文件校验、图片转换和完整提交载荷', () => {
  assert.match(sources.kyc, /Promise\.all\(\[fetchKycStatus\(\), fetchCountries\(\)\]\)/)
  assert.match(sources.kyc, /file\.size > maxDocumentSize\.value/)
  assert.match(sources.kyc, /file\.type\.startsWith\('image\/'\)/)
  assert.match(sources.kyc, /reader\.readAsDataURL\(file\)/)
  assert.match(sources.kyc, /requiresHandheld\.value && !documents\.value\.handheld/)
  for (const field of [
    'realName',
    'submissionType',
    'enterpriseName',
    'businessRegistrationNumber',
    'country',
    'idNumber',
    'documentType',
    'documentFrontImage',
    'documentBackImage',
    'documentHandheldImage',
  ]) {
    assert.match(sources.kyc, new RegExp(`${field}:`))
  }
  assert.match(sources.kyc, /await submitKycApplication\(\{/)
  assert.match(sources.kyc, /await load\(\)/)
  assert.match(sources.kyc, /accept="image\/\*"/)
})

test('账户绑定、邀请和语言设置保留真实读写行为', () => {
  assert.match(sources.bindings, /Promise\.all\(\[fetchUserProfile\(\), fetchThirdPartyBindings\(\)\]\)/)
  assert.match(sources.bindings, /sendEmailBindCode\(email\.value\)/)
  assert.match(sources.bindings, /bindEmail\(email\.value, emailCode\.value\)/)
  assert.match(sources.bindings, /provider: provider\.value/)
  assert.match(sources.bindings, /accountIdentifier: accountIdentifier\.value/)
  assert.match(sources.bindings, /displayName: displayName\.value/)
  assert.match(sources.bindings, /role="dialog"/)
  assert.match(sources.bindings, /aria-modal="true"/)
  assert.match(sources.bindings, /@keydown="handleProviderDialogKeydown"/)

  assert.match(sources.referrals, /Promise\.all\(\[fetchReferralCode\(\), fetchReferralInvites\(\)\]\)/)
  assert.match(sources.referrals, /navigator\.clipboard\.writeText\(code\.value\.code\)/)
  assert.match(sources.referrals, /document\.execCommand\('copy'\)/)
  assert.match(sources.referrals, /bindReferralCode\(bindCode\.value\)/)

  assert.match(sources.language, /normalizeMobileLocale\(locale\.value\)/)
  assert.match(sources.language, /v-for="option in SUPPORTED_LOCALES"/)
  assert.match(sources.language, /setAppLocale\(nextLocale\)/)
  assert.match(sources.language, /sanitizeInternalRedirect\(route\.query\.back, '\/profile'\)/)
  assert.match(sources.language, /:fallback="backTarget"/)
  assert.match(sources.language, /role="radiogroup"/)
  assert.match(sources.language, /role="radio"/)
})

test('访问、身份与设置视图遵守主题、触控、窄屏和 Lucide 合同', () => {
  for (const source of allSources) {
    assert.match(source, /@media \(max-width: 340px\)/)
    assert.match(source, /env\(safe-area-inset-bottom\)/)
    assert.match(source, /var\(--(?:background|surface|page)\)/)
    assert.match(source, /min-height:\s*(?:4[4-9]|[5-9]\d)px/)
    assert.match(source, /from 'lucide-vue-next'/)
    assert.doesNotMatch(source, /#[0-9a-f]{3,8}/i)
    assert.doesNotMatch(source, /background:\s*(?:white|rgb\()/i)
    assert.doesNotMatch(source, /color:\s*(?:white|black)\b/i)
    assert.doesNotMatch(source, /<svg/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
    assert.doesNotMatch(source, /[\u3400-\u9fff]/)
  }

  for (const source of [
    sources.login,
    sources.register,
    sources.forgotPassword,
    sources.loginTwoFactor,
    sources.kyc,
    sources.bindings,
    sources.referrals,
  ]) {
    assert.match(`${source}\n${selectedCss}`, /(?:focus-within|\.input)/)
  }
})

test('视图引用的固定文案键在中英文资源中均存在', () => {
  const keys = new Set<string>()
  for (const source of allSources) {
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
