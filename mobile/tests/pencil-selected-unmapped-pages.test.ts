import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const views = {
  assets: read('../src/views/AssetsView.vue'),
  profile: read('../src/views/ProfileView.vue'),
  orders: read('../src/views/OrdersView.vue'),
  login: read('../src/views/LoginView.vue'),
  register: read('../src/views/RegisterView.vue'),
  news: read('../src/views/NewsView.vue'),
  newsDetail: read('../src/views/NewsDetailView.vue'),
  swap: read('../src/views/SwapView.vue'),
  earn: read('../src/views/EarnView.vue'),
  loan: read('../src/views/LoanView.vue'),
  newCoins: read('../src/views/NewCoinsView.vue'),
  newCoinDetail: read('../src/views/NewCoinDetailView.vue'),
}
const appSource = read('../src/App.vue')
const mainSource = read('../src/main.ts')
const routerSource = read('../src/router/index.ts')
const pageHeaderSource = read('../src/components/PageHeader.vue')
const selectedCss = read('../src/styles/pencil-selected-pages.css')

test('此前未映射页面均声明当前 Pencil 选中画板来源', () => {
  const expected: Record<keyof typeof views, string> = {
    assets: 'CUK3y i6YDBr p61z2Q Q4JYj v6phV TuWXq tPkL1 tPkD1',
    profile: 'dUqOS duJTW S23rM S0Bj8',
    orders: 'kcP5D A85if n6oGO t2GTW4 e5Qs1 hxe8l',
    login: 'u99Fpg WNbsc',
    register: 'MCuqb RGYGj',
    news: 'VGPW0 b6EGF',
    newsDetail: 'Q50Rgr ASvmq',
    swap: 'x9T4CL eXdnN sf288 xvVss',
    earn: 'zIzOm tCHZ9 nqP6W aXxul',
    loan: 'kIOBX yrsRy',
    newCoins: 'oOJ0q ZTtvY',
    newCoinDetail: 'nFwYy B6Qh9J',
  }
  for (const [name, id] of Object.entries(expected) as [keyof typeof views, string][]) {
    assert.match(views[name], new RegExp(`data-pencil-source="${id}"`), `${name} lost its selected Pencil source`)
  }
})

test('选中稿共享头部、字段、弹层和根壳职责保持一致', () => {
  assert.match(mainSource, /import '\.\/styles\/pencil-selected-pages\.css'/)
  assert.match(pageHeaderSource, /pencil\?: boolean/)
  assert.match(pageHeaderSource, /pencil \? 'pencil-page-header' : 'secondary-header'/)
  assert.match(pageHeaderSource, /height: 60px/)
  assert.match(pageHeaderSource, /z-index: var\(--layer-sticky-header\)/)
  assert.match(selectedCss, /\.pencil-field__shell:focus-within[\s\S]*?box-shadow: 0 0 0 2px var\(--focus-ring\)/)
  assert.match(selectedCss, /\.pencil-sheet-mask[\s\S]*?z-index: var\(--layer-overlay\)/)
  assert.match(selectedCss, /@media \(max-width: 340px\)/)
  assert.match(selectedCss, /@media \(prefers-reduced-motion: reduce\)/)
  assert.match(appSource, /\['home', 'markets'\]\.includes\(String\(route\.name \|\| ''\)\)/)
  assert.match(routerSource, /path: '\/orders'[\s\S]*?meta: \{ depth: 1, backFallback: '\/' \}/)
})

test('资产与我的页面锁定 390px 选中稿几何和 Lucide 图标', () => {
  assert.match(views.assets, /\.assets-hero \{[\s\S]*?height: 236px;[\s\S]*?padding: 18px 20px 16px/)
  assert.match(views.assets, /\.assets-hero--member[\s\S]*?align-content: center;[\s\S]*?grid-template-rows: auto 66px/)
  assert.match(views.assets, /\.assets-member-summary__value strong[\s\S]*?font-size: clamp\(30px, 8vw, 34px\)/)
  assert.match(views.assets, /\.assets-hero-actions button[\s\S]*?height: 66px[\s\S]*?min-height: 66px/)
  assert.match(views.assets, /\.assets-holding-row[\s\S]*?min-height: 52px/)
  assert.match(views.assets, /class="assets-balance-toggle"[\s\S]*?<Eye v-if="balanceVisible" :size="14"/)
  assert.match(views.assets, /<ArrowDownToLine[\s\S]*?<ArrowUpFromLine[\s\S]*?<ArrowLeftRight[\s\S]*?<ReceiptText/)
  assert.match(views.assets, /openProtectedRoute\('wallet-ledger'\)[\s\S]*?<ReceiptText/)
  assert.match(views.assets, /openProtectedRoute\('withdrawal-records'\)[\s\S]*?<ArrowUpFromLine/)
  assert.doesNotMatch(views.assets, /ArrowUpToLine|ArrowRightLeft/)
  for (const key of ['assets.guestTitle', 'assets.loginViewAssets', 'assets.accountBalances', 'assets.allAccounts', 'assets.holdings', 'assets.spotHoldings', 'assets.marginHoldings', 'assets.holdingCount', 'assets.availableFrozenSummary', 'assets.quickLedger', 'assets.fundLedger', 'assets.fundLedgerDescription', 'assets.withdrawalRecordsDescription', 'assets.quickRecharge', 'assets.quickRechargeDescription']) {
    assert.match(views.assets, new RegExp(key.replace('.', '\\.')))
  }

  assert.match(views.profile, /\.profile-pencil__content[\s\S]*?gap: 10px[\s\S]*?padding-top: 10px/)
  assert.match(views.profile, /\.profile-identity-pencil[\s\S]*?height: 72px/)
  assert.match(views.profile, /\.profile-auth-actions[\s\S]*?height: 58px/)
  assert.match(views.profile, /\.profile-status-row[\s\S]*?height: 44px/)
  assert.match(views.profile, /\.profile-group[\s\S]*?height: 201px/)
  assert.match(views.profile, /\.profile-group--support[\s\S]*?height: 195px/)
  assert.match(views.profile, /<Settings :size="20"/)
  assert.match(views.profile, /profile\.securityCenter/)
  assert.match(views.profile, /profile\.accountBindings/)
  assert.match(views.profile, /<UserPlus :size="18"/)
  assert.match(views.profile, /profile\.referrals[\s\S]*?profile\.referralDescription/)
  assert.doesNotMatch(views.profile, /<Settings2/)
})

test('订单页标签边界和 64px 数据行与选中稿一致', () => {
  assert.match(views.orders, /--pencil-root-header-margin: 4px/)
  assert.match(views.orders, /\.orders-pencil__content[\s\S]*?padding-top: 4px/)
  assert.match(views.orders, /\.orders-market-tabs[\s\S]*?height: 45px/)
  assert.match(views.orders, /\.orders-state-tabs[\s\S]*?height: 34px[\s\S]*?margin-top: 4px/)
  assert.match(views.orders, /\.orders-list,[\s\S]*?margin-top: 4px/)
  assert.match(views.orders, /\.orders-row \{[\s\S]*?grid-template-rows: 20px 16px[\s\S]*?height: 64px/)
  assert.match(views.orders, /t\('orders\.historyOrdersTab'\)/)
  assert.doesNotMatch(views.orders, /class="orders-toolbar"/)
  const actions = views.orders.match(/<template #actions>([\s\S]*?)<\/template>/)?.[1] || ''
  assert.equal((actions.match(/<button\b/g) || []).length, 1)
})

test('登录注册页锁定品牌、标题、字段和动作纵向坐标', () => {
  for (const source of [views.login, views.register]) {
    assert.match(source, /\.auth-pencil-canvas[\s\S]*?gap: 12px/)
    assert.match(source, /\.auth-brand-row[\s\S]*?height: 62px/)
    assert.match(source, /\.auth-brand-row img[\s\S]*?height: 34px[\s\S]*?width: 136px/)
    assert.match(source, /\.auth-pencil-title[\s\S]*?height: 88px/)
    assert.match(source, /\.auth-pencil-title h1[\s\S]*?font-size: 24px[\s\S]*?font-weight: 750[\s\S]*?line-height: 35px/)
    assert.match(source, /\.auth-pencil-title p[\s\S]*?line-height: 17px[\s\S]*?margin: 8px 0 0/)
    assert.match(source, /\.auth-pencil-field[\s\S]*?height: 48px/)
    assert.doesNotMatch(source, /t\('common\.close'\)|t\('language\.entry'\)/)
  }

  assert.match(views.login, /auth\.pencilLoginDescription/)
  assert.match(views.login, /auth\.keepSignedIn/)
  assert.match(views.login, /auth\.newDeviceTwoFactor/)
  assert.match(views.login, /\.auth-method-tabs,[\s\S]*?height: 26px/)
  assert.match(views.login, /\.auth-pencil-field[\s\S]*?margin-top: 12px/)
  assert.match(views.login, /\.auth-form-meta[\s\S]*?height: 16px[\s\S]*?margin-top: 12px/)
  assert.match(views.login, /\.login-submit-wrap[\s\S]*?height: 56px[\s\S]*?margin-top: 12px[\s\S]*?padding-top: 8px/)
  assert.match(views.login, /\.auth-switch[\s\S]*?height: 33px/)
  assert.match(views.login, /\.auth-security-note[\s\S]*?height: 24px/)

  assert.match(views.register, /auth\.pencilRegisterDescription/)
  assert.match(views.register, /\.register-fields[\s\S]*?gap: 12px/)
  assert.match(views.register, /\.register-confirm-field[\s\S]*?grid-template-rows: 48px 20px[\s\S]*?height: 68px/)
  assert.match(views.register, /auth\.pencilRegisterTitle/)
  assert.match(views.register, /\.terms-row[\s\S]*?height: 16px[\s\S]*?margin-top: 0/)
  assert.match(views.register, /\.auth-switch[\s\S]*?height: 33px/)
  assert.match(views.register, /\.register-submit-wrap[\s\S]*?height: 56px[\s\S]*?padding-top: 8px/)
})

test('视觉重构没有移除真实接口、状态与危险操作复核', () => {
  assert.match(views.assets, /fetchWalletAccounts\(\)[\s\S]*?fetchMarginWallets\(\)/)
  assert.match(views.profile, /fetchUserProfile\(\)[\s\S]*?fetchKycStatus\(\)/)
  assert.match(views.orders, /fetchOpenSpotOrders\(30, signal\)[\s\S]*?fetchMarginPositions\('opened', 30, signal\)/)
  assert.match(views.login, /loginWithPassword\(account\.value, password\.value(?:,\s*cfTurnstileToken\.value\s*\|\|\s*undefined)?\)/)
  assert.match(views.register, /registerWithEmail\(\{ email: email\.value, password: password\.value/)
  assert.match(views.news, /rows\.value = await fetchNews\(50\)/)
  assert.match(views.newsDetail, /<NewsRichText :blocks="detail\.content"/)
  assert.doesNotMatch(views.newsDetail, /v-html/)
  assert.match(views.swap, /const requestAmount = amountText\.value[\s\S]*requestConvertQuote\(selectedPair\.value, requestAmount\)/)
  assert.match(views.earn, /const requestAmount = amountText\.value[\s\S]*subscribeEarnProduct\(selected\.value\.id, requestAmount\)/)
  assert.match(views.loan, /applyLoan\(\{[\s\S]*?repayLoanOrder\(order\.id\)/)
  assert.match(views.newCoins, /fetchNewCoinProjects\(\)/)
  assert.match(views.newCoinDetail, /subscribeNewCoin\(\{[\s\S]*?createNewCoinPurchase\(\{/)
  for (const source of [views.assets, views.orders, views.swap, views.earn, views.loan, views.newCoinDetail]) {
    assert.match(source, /role="dialog"/)
    assert.match(source, /aria-modal="true"/)
  }
})

test('选中页面静态文案在中英文资源中保持对称', () => {
  const keys = new Set<string>()
  for (const source of Object.values(views)) {
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
