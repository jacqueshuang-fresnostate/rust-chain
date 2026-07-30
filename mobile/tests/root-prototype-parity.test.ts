import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const appSource = read('../src/App.vue')
const navSource = read('../src/components/AppBottomNav.vue')
const headerSource = read('../src/components/RootHeader.vue')
const parityCss = read('../src/styles/prototype-parity.css')
const prototypeCss = read('../src/styles/prototype-base.css')
const mainSource = read('../src/main.ts')
const routerSource = read('../src/router/index.ts')
const viteSource = read('../vite.config.ts')
const views = {
  home: read('../src/views/HomeView.vue'),
  markets: read('../src/views/MarketsView.vue'),
  trade: read('../src/views/TradeView.vue'),
  assets: read('../src/views/AssetsView.vue'),
  profile: read('../src/views/ProfileView.vue'),
}

function assertOrdered(source: string, selectors: string[]): void {
  let previous = -1
  for (const selector of selectors) {
    const current = source.indexOf(selector, previous + 1)
    assert.ok(current > previous, `${selector} must follow the previous prototype block`)
    previous = current
  }
}

function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}

test('共享样式机械导入受检原型 CSS，并只提供 Vue/API 兼容桥接', () => {
  assert.match(parityCss, /@import '\.\/prototype-base\.css';/)
  assert.match(mainSource, /import '\.\/styles\/prototype-parity\.css'/)
  assert.match(viteSource, /tailwindcss:\s*fileURLToPath\(new URL\('\.\/src\/styles\/tailwind-source-reset\.css'/)
  assert.match(prototypeCss, /\.app-stage\.theme-light\s*\{/)
  assert.match(prototypeCss, /\.topbar\s*\{[\s\S]*?height:\s*64px/)
  assert.match(prototypeCss, /\.bottom-nav\s*\{[\s\S]*?height:\s*84px/)
  assert.match(prototypeCss, /grid-template-columns:\s*repeat\(7,\s*minmax\(44px,\s*1fr\)\)/)
  assert.match(prototypeCss, /\.bottom-nav \.seconds-nav-action span\s*\{[\s\S]*?height:\s*48px/)
  assert.match(parityCss, /\.bottom-nav button\s*\{[\s\S]*?min-height:\s*66px/)
})

test('根壳使用原型 Geist 变量字体并保留中文系统字体回退', () => {
  assert.match(
    parityCss,
    /@font-face\s*\{[\s\S]*?font-family:\s*'Geist';[\s\S]*?font-weight:\s*100 900;[\s\S]*?\.\.\/assets\/fonts\/geist-98bbbccb\.woff2/,
  )
  assert.match(
    parityCss,
    /@font-face\s*\{[\s\S]*?font-family:\s*'Geist Mono';[\s\S]*?font-weight:\s*100 900;[\s\S]*?\.\.\/assets\/fonts\/geist-mono-013b2f2f\.woff2/,
  )
  assert.match(
    parityCss,
    /body,\s*#app\s*\{[\s\S]*?font-family:\s*'Geist',\s*'PingFang SC',\s*'Hiragino Sans GB',\s*'Microsoft YaHei',\s*sans-serif;/,
  )
  assert.match(
    parityCss,
    /\.app-stage\s*\{[\s\S]*?--font-geist-sans:\s*'Geist';[\s\S]*?--font-geist-mono:\s*'Geist Mono';[\s\S]*?font-family:\s*var\(--font-geist-sans\),\s*'PingFang SC',\s*'Hiragino Sans GB',\s*'Microsoft YaHei',\s*sans-serif;/,
  )
})

test('zh-CN 根页面装饰眉题保持原型英文原文', () => {
  const labels = {
    marketPulse: 'MARKET PULSE',
    assetField: 'ASSET FIELD',
    allocationLabel: 'ALLOCATION',
    holdingsLabel: 'HOLDINGS',
    accountsLabel: 'ACCOUNTS',
    guestModeEyebrow: 'GUEST MODE',
    verifiedMemberEyebrow: 'VERIFIED MEMBER',
    accountMatrix: 'ACCOUNT MATRIX',
  }

  for (const [key, label] of Object.entries(labels)) {
    assert.equal(resolveMessage(zhCN, `rootPrototype.${key}`), label)
  }
})

test('根壳层保持原型舞台、64px 顶栏、路由栈、PWA 状态与异形导航顺序', () => {
  assertOrdered(appSource, [
    'class="app-stage"',
    'class="stage-art"',
    'class="app-frame mobile-canvas"',
    'class="ambient-layer"',
    'class="route-veil"',
    '<PwaStatus />',
    '<RootHeader v-if="showBottomNav" />',
    'class="app-route-host"',
    '<AppBottomNav v-if="showBottomNav" />',
  ])
  assert.equal((appSource.match(/<PwaStatus \/>/g) || []).length, 1)
  assert.match(headerSource, /class="topbar root-header"/)
  assert.match(headerSource, /class="brand-button root-header__brand"/)
  assert.match(headerSource, /class="brand-logo"/)
  assert.match(headerSource, /class="topbar-actions action-cluster root-header__actions"/)
  assert.match(headerSource, /class="icon-button has-dot root-header__control root-header__message"/)
  assert.match(appSource, /class="stage-art" aria-hidden="true"/)
  for (const key of [
    'desktopStageIndex',
    'desktopStageMottoLine1',
    'desktopStageMottoLine2',
    'desktopStageMottoLine3',
    'desktopStagePairBtc',
    'desktopStagePairEth',
    'desktopStagePairSol',
    'desktopStageInstrument',
    'desktopStageLocation',
  ]) assert.match(appSource, new RegExp(`t\\('rootPrototype\\.${key}'\\)`))
  assert.doesNotMatch(appSource, /SIGNAL THEATRE|SIGNALS|LIVE EXCHANGE INSTRUMENT|HONG KONG/)
})

test('七项根导航保持独立 Spot/Contract、抬升 Seconds 与类型化 replace', () => {
  const keys = [...navSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'spot', 'seconds', 'contract', 'assets', 'profile'])
  assert.match(navSource, /to:\s*\{\s*name:\s*'trade',\s*params:[\s\S]*?key:\s*'seconds'/)
  assert.match(navSource, /query:\s*\{\s*mode:\s*'contract'\s*\}/)
  assert.match(navSource, /'seconds-nav-action': item\.primary/)
  assert.match(navSource, /function selectRoot\(to: RouteLocationRaw\)/)
  assert.match(navSource, /router\.replace\(to\)/)
  assert.doesNotMatch(navSource, /<RouterLink|<svg|\p{Extended_Pictographic}/u)
})

test('Home、Markets、Trade、Assets、Profile 使用原型块级顺序且不带 scoped 覆盖', () => {
  assertOrdered(views.home, [
    'class="view home-view prototype-root-view"',
    'class="home-workspace"',
    'class="portfolio-overview"',
    'class="funding-actions"',
    'class="shortcut-section"',
    'class="market-brief"',
    'class="home-market-section"',
    'class="home-benefits"',
  ])
  assertOrdered(views.markets, [
    'class="view markets-view prototype-root-view"',
    'class="page-intro"',
    'class="search-field"',
    'class="filter-rail"',
    'class="market-index"',
    'class="market-table-head"',
    'class="market-list"',
  ])
  assertOrdered(views.trade, [
    'class="view trade-view prototype-root-view"',
    'class="trade-heading"',
    'class="trade-quote"',
    'class="chart-panel trade-chart-panel"',
    'class="trade-console"',
  ])
  assertOrdered(views.assets, [
    'class="view assets-view prototype-root-view"',
    'class="page-intro compact"',
    'class="asset-hero"',
    'class="asset-actions"',
    'class="content-section allocation-section"',
    'class="allocation-track"',
    'class="account-list"',
    'class="content-section"',
  ])
  assertOrdered(views.profile, [
    'class="view profile-view prototype-root-view"',
    'class="profile-identity"',
    'class="dual-actions profile-auth-actions"',
    'class="content-section"',
    'class="profile-identity"',
    'class="profile-metrics"',
    'class="settings-list"',
  ])

  for (const source of Object.values(views)) {
    assert.doesNotMatch(source, /<style scoped/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
  }
  assert.match(views.home, /<svg viewBox="0 0 360 84"[\s\S]*?d="M0 67 C28 62 39 70 62 57/)
  assert.match(views.markets, /<svg[\s\S]*?class="sparkline"[\s\S]*?<polyline/)
  for (const source of [views.trade, views.assets, views.profile]) assert.doesNotMatch(source, /<svg/)
})

test('根视图继续调用真实 API/store，访客与加载错误状态不改变原型骨架', () => {
  for (const contract of [
    /fetchWalletAccounts\(\)/,
    /fetchMarginWallets\(\)/,
    /marketStore\.startLiveUpdates\(\)/,
    /fetchNews\(\)/,
  ]) assert.match(views.home, contract)

  assert.match(views.markets, /marketStore\.startLiveUpdates\(\)/)
  assert.match(views.markets, /router\.push\(\{ name: 'market-detail'/)
  assert.match(views.trade, /fetchKlines\(pairSymbol\.value, interval\.value\)/)
  assert.match(views.trade, /fetchOrderBook\(pairSymbol\.value\)/)
  assert.match(views.trade, /await placeSpotOrder\(\{/)
  assert.match(views.trade, /await placeMarginOrder\(\{/)
  assert.match(views.assets, /fetchWalletAccounts\(\)/)
  assert.match(views.assets, /fetchMarginWallets\(\)/)
  assert.match(views.assets, /await transferWalletFunds\(transferAsset\.value,\s*transferFrom\.value,\s*to,\s*transferValue\)/)
  assert.match(views.profile, /fetchUserProfile\(\)/)
  assert.match(views.profile, /fetchKycStatus\(\)/)
  assert.match(views.profile, /await updateUsername\(nameDraft\.value\)/)
  assert.match(views.profile, /<template v-if="!session\.isAuthenticated">/)
  assert.match(views.profile, /<template v-else>/)
  assert.doesNotMatch(views.home, /fallbackNews|usingFallbackNews/)
  assert.match(views.home, /:disabled="!briefNotice"/)
})

test('行情曲线、自选和加载失败状态保持真实语义与固定五行几何', () => {
  assert.match(views.markets, /import \{ fetchKlines \} from '@\/api\/market'/)
  assert.match(views.markets, /Promise\.allSettled\([\s\S]*?fetchKlines\(symbol, '15m', 24\)/)
  assert.match(views.markets, /const favoriteSymbols = ref\(new Set<string>\(\)\)/)
  assert.doesNotMatch(views.markets, /prototypeSparkPoints|new Set\(\['BTC\/USDT'/)
  assert.match(views.markets, /const neutralSparklinePoints = '0,17 76,17'/)
  assert.match(views.markets, /const hasTemperatureSample = computed\(\(\) => !marketRowsUnavailable\.value && rows\.value\.length > 0\)/)
  assert.match(views.markets, /hasTemperatureSample \? marketTemperature : '--'/)
  assert.match(views.markets, /hasTemperatureSample \? 'rootPrototype\.marketStrong' : 'rootPrototype\.marketNoSample'/)
  assert.match(views.markets, /:class="sparklineTone\(ticker\.symbol\)"/)
  assert.match(views.markets, /:points="sparklinePoints\(ticker\.symbol\)"/)
  assert.match(views.home, /if \(activeTab\.value === 'favorites'\) return \[\]/)

  for (const source of [views.home, views.markets]) {
    assert.match(source, /v-for="row in 5"/)
    assert.match(source, /class="root-market-reserved-state" role="alert"/)
    assert.match(source, /:aria-busy="marketRowsUnavailable && !marketStore\.error"/)
  }
  assert.match(views.markets, /class="market-picker-list"[\s\S]*?v-for="row in 5"[\s\S]*?class="market-picker-state" role="alert"/)
  assert.match(prototypeCss, /\.market-row\s*\{[\s\S]*?min-height:\s*72px/)
  assert.match(parityCss, /\.home-market-skeleton-row\s*\{[\s\S]*?min-height:\s*70px/)
  assert.match(parityCss, /\.root-market-reserved-state\s*\{[\s\S]*?inset:\s*0/)
  assert.match(parityCss, /\.market-picker-skeleton-row\s*\{[\s\S]*?min-height:\s*68px/)
  assert.match(parityCss, /\.market-picker-state\s*\{[\s\S]*?inset:\s*0/)
  assert.match(parityCss, /\.sparkline\.neutral\s*\{[\s\S]*?color:\s*var\(--muted\)/)
})

test('Seconds 是保留旧深链的二级面，不继承根头部与底栏', () => {
  assert.match(
    routerSource,
    /\{\s*path:\s*'\/seconds',\s*alias:\s*'\/products\/seconds',\s*name:\s*'seconds',[\s\S]*?meta:\s*\{\s*showBottomNav:\s*false,\s*depth:\s*1,\s*backFallback:\s*'\/products'\s*\}\s*\}/,
  )
  assert.match(appSource, /<RootHeader v-if="showBottomNav" \/>/)
  assert.match(appSource, /<AppBottomNav v-if="showBottomNav" \/>/)
})

test('根切片所有 i18n 键均同时存在于中文和英文资源', () => {
  const keys = new Set<string>()
  for (const source of [appSource, navSource, headerSource, ...Object.values(views)]) {
    for (const match of source.matchAll(/\bt\('([^']+)'/g)) keys.add(match[1])
  }

  for (const key of keys) {
    assert.notEqual(resolveMessage(zhCN, key), undefined, `zh-CN missing ${key}`)
    assert.notEqual(resolveMessage(en, key), undefined, `en missing ${key}`)
  }
})
