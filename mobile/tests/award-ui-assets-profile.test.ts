import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const assetsSource = read('../src/views/AssetsView.vue')
const profileSource = read('../src/views/ProfileView.vue')
const selectedCss = read('../src/styles/pencil-selected-pages.css')
const sha256 = (path: string): string => createHash('sha256')
  .update(readFileSync(new URL(path, import.meta.url)))
  .digest('hex')

test('资产页映射四个 Pencil 画板并只使用真实资产生成持仓', () => {
  assert.match(assetsSource, /data-pencil-source="CUK3y i6YDBr p61z2Q Q4JYj v6phV TuWXq"/)
  assert.match(assetsSource, /class="page pencil-page pencil-root-page assets-pencil"/)
  assert.match(assetsSource, /const holdingRows = computed<AssetHoldingRow\[\]>/)
  assert.match(assetsSource, /\.filter\(\(row\) => row\.amount > 0\)/)
  assert.match(assetsSource, /return right\.estimatedValue - left\.estimatedValue/)
  assert.match(assetsSource, /estimatedValue: estimateAssetValue\(row\.symbol, amount\)/)
  assert.match(assetsSource, /Number\.isFinite\(lastPrice\)[\s\S]*?: null/)
  assert.match(assetsSource, /v-if="!session\.isAuthenticated" class="pencil-content assets-pencil__guest-content"/)
  assert.match(assetsSource, /v-else[\s\S]*?class="pencil-content assets-pencil__member-content"/)
  assert.match(assetsSource, /v-else-if="hasHoldings" class="assets-holdings__list"/)
  assert.match(assetsSource, /t\('assets\.availableFrozenSummary'/)
  assert.match(assetsSource, /t\('assets\.estimateUnavailable'\)/)
  assert.match(assetsSource, /t\('rootPrototype\.todayReturn'\)[\s\S]*?<strong class="pencil-numeric" :class="todayReturnPresentation\.tone">\{\{ todayReturnPresentation\.amount \}\}<\/strong>/)
  assert.match(assetsSource, /const QUOTE_ASSET_SYMBOL = 'USDT'/)
  assert.match(assetsSource, /if \(symbol === QUOTE_ASSET_SYMBOL\) return amount/)
  assert.doesNotMatch(assetsSource, /STABLE_ASSET_SYMBOLS|\['USDT', 'USDC', 'USD'\]/)
  assert.match(assetsSource, /minimumFractionDigits: 2,[\s\S]*?maximumFractionDigits: 2/)
  assert.doesNotMatch(assetsSource, /placeholder-|symbol:\s*'--'|24,806|1,204\.55|4\.85%/)

  const guestBranch = assetsSource.slice(
    assetsSource.indexOf('class="pencil-content assets-pencil__guest-content"'),
    assetsSource.indexOf('class="pencil-content assets-pencil__member-content"'),
  )
  assert.match(guestBranch, /assets-hero--guest[\s\S]*?assets-guest-login/)
  assert.doesNotMatch(guestBranch, /assets-hero-actions|assets-holdings|assets-tools|openTransfer/)
  assert.match(assetsSource, /v-if="session\.isAuthenticated && transferOpen" class="confirmation-layer assets-transfer-layer"/)
  assert.equal(assetsSource.match(/class="assets-holdings__empty-icon"/g)?.length, 1)
})

test('资产 Hero 使用跟随主题的两张跟踪生产素材', () => {
  assert.match(assetsSource, /assetsHeroLight from '@\/assets\/assets\/assets-hero-light\.jpg'/)
  assert.match(assetsSource, /assetsHeroDark from '@\/assets\/assets\/assets-hero-dark\.jpg'/)
  assert.match(assetsSource, /v-show="!theme\.isDark"[\s\S]*?:src="assetsHeroLight"/)
  assert.match(assetsSource, /v-show="theme\.isDark"[\s\S]*?:src="assetsHeroDark"/)
  assert.ok(existsSync(new URL('../src/assets/assets/assets-hero-light.jpg', import.meta.url)))
  assert.ok(existsSync(new URL('../src/assets/assets/assets-hero-dark.jpg', import.meta.url)))
  assert.equal(sha256('../src/assets/assets/assets-hero-light.jpg'), 'eb1d0237547675fd61694cd07879b09b30ad8fa976541978501eb04bb246f5dc')
  assert.equal(sha256('../src/assets/assets/assets-hero-dark.jpg'), '52d3e5fab2674d3693a8a3e574f557ca5c89a4d46ea6bdb7cc3f8b3fa6af7691')
  assert.match(assetsSource, /assets-hero__overlay--dark[\s\S]*?background: color-mix\(in srgb, var\(--page\) 25%, transparent\)/)
  assert.doesNotMatch(assetsSource, /mobile\/pencil|@\/\.\.\/pencil|generated-178568/)
})

test('资产页保留钱包、划转、资金路由与可访问确认层', () => {
  assert.match(assetsSource, /Promise\.all\(\[[\s\S]*marketStore\.refresh\(\),[\s\S]*fetchWalletAccounts\(\),[\s\S]*fetchMarginWallets\(\),[\s\S]*\]\)/)
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
  for (const routeName of ['kyc', 'security', 'account-bindings', 'referrals', 'language', 'help-support']) {
    assert.match(profileSource, new RegExp(`name: '${routeName}'`))
  }
  assert.match(profileSource, /<UserPlus :size="18"/)
  assert.match(profileSource, /profile\.referrals[\s\S]*?profile\.referralDescription/)
  assert.match(profileSource, /@click="router\.push\(\{ name: 'referrals' \}\)"/)
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
