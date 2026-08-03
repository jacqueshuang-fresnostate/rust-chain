import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const assetsSource = read('../src/views/AssetsView.vue')
const profileSource = read('../src/views/ProfileView.vue')
const selectedCss = read('../src/styles/pencil-selected-pages.css')

test('资产页映射 Pencil 选中稿并只使用真实资产生成分布', () => {
  assert.match(assetsSource, /data-pencil-source="CUK3y i6YDBr"/)
  assert.match(assetsSource, /class="page pencil-page pencil-root-page assets-pencil"/)
  assert.match(assetsSource, /const hasHoldings = computed\(\(\) => accountDataAvailable\.value/)
  assert.match(assetsSource, /const hasAllocation = computed\(\(\) => hasHoldings\.value && totalEstimate\.value > 0\)/)
  assert.match(assetsSource, /const allocationRows = computed\(\(\) =>/)
  assert.match(assetsSource, /\.filter\(\(row\) => row\.amount > 0 && row\.value > 0\)/)
  assert.match(assetsSource, /v-if="accountDataAvailable && hasAllocation"/)
  assert.match(assetsSource, /v-else class="pencil-state assets-distribution__state"/)
  assert.match(assetsSource, /v-if="!session\.isAuthenticated" class="pencil-primary"/)
  assert.doesNotMatch(assetsSource, /placeholder-|symbol:\s*'--'|rootPrototype\.todayReturn/)
})

test('资产页保留钱包、划转、资金路由与可访问确认层', () => {
  assert.match(assetsSource, /Promise\.all\(\[marketStore\.refresh\(\), fetchWalletAccounts\(\), fetchMarginWallets\(\)\]\)/)
  assert.match(assetsSource, /await transferWalletFunds\(transferAsset\.value, transferFrom\.value, to, transferValue\)/)
  assert.match(assetsSource, /transferValue > transferAvailable\.value/)
  assert.match(assetsSource, /useModalDialog\(transferOpen, transferDialog\)/)
  assert.match(assetsSource, /trapTransferFocus\(event, closeTransfer\)/)
  assert.match(assetsSource, /role="dialog"/)
  assert.match(assetsSource, /aria-modal="true"/)
  assert.match(assetsSource, /data-dialog-cancel/)
  for (const routeName of ['deposit-asset', 'withdraw-asset', 'wallet-ledger', 'withdrawal-records', 'quick-recharge']) {
    assert.match(assetsSource, new RegExp(`'${routeName}'`))
  }
  assert.match(assetsSource, /query: \{ redirect: '\/assets' \}/)
})

test('我的页同时覆盖 Pencil 访客与会员状态并保留真实账户动作', () => {
  assert.match(profileSource, /data-pencil-source="dUqOS duJTW S23rM S0Bj8"/)
  assert.match(profileSource, /class="page pencil-page pencil-root-page profile-pencil"/)
  assert.match(profileSource, /v-if="!session\.isAuthenticated" class="profile-auth-actions"/)
  assert.match(profileSource, /name: 'login'[\s\S]*?redirect: '\/profile'/)
  assert.match(profileSource, /name: 'register'/)
  assert.match(profileSource, /Promise\.all\(\[fetchUserProfile\(\), fetchKycStatus\(\)\]\)/)
  assert.match(profileSource, /await updateUsername\(nameDraft\.value\)/)
  assert.match(profileSource, /await uploadUserAvatar\(file\)/)
  assert.match(profileSource, /navigator\.clipboard\.writeText\(String\(profile\.value\.id\)\)/)
  assert.match(profileSource, /useModalDialog\(editOpen, profileDialog, '\[autofocus\]'\)/)
  assert.match(profileSource, /session\.logout\(\)/)
  assert.match(profileSource, /router\.replace\('\/'\)/)
  for (const routeName of ['kyc', 'security', 'account-bindings', 'language', 'message-center']) {
    assert.match(profileSource, new RegExp(`name: '${routeName}'`))
  }
  assert.doesNotMatch(profileSource, /name: 'referrals'/)
  assert.doesNotMatch(profileSource, /rootPrototype\.(?:tradingDays|winRate|profitFactor)|LEVEL --/)
})

test('资产与我的共享选中页触控、聚焦、窄屏、安全区和低动态合同', () => {
  for (const source of [assetsSource, profileSource]) {
    assert.match(source, /<PageHeader[^>]*:pencil="true"/)
    assert.match(source, /<style scoped>/)
    assert.match(source, /@media \(max-width: 340px\)/)
    assert.match(source, /var\(--(?:surface|ink|muted|line|accent)/)
    assert.doesNotMatch(source, /#[\da-f]{3,8}|<svg|\p{Extended_Pictographic}/iu)
  }
  assert.match(selectedCss, /env\(safe-area-inset-bottom\)/)
  assert.match(selectedCss, /\.pencil-field__shell:focus-within/)
  assert.match(selectedCss, /min-height:\s*(?:44|4[5-9]|[5-9]\d)px/)
  assert.match(selectedCss, /@media \(prefers-reduced-motion: reduce\)/)
})
