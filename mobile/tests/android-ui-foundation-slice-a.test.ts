import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const baseCss = read('../src/styles/base.css')
const prototypeCss = read('../src/styles/prototype-base.css')
const parityCss = read('../src/styles/prototype-parity.css')
const bottomNavSource = read('../src/components/AppBottomNav.vue')
const rootHeaderSource = read('../src/components/RootHeader.vue')
const homeSource = read('../src/views/HomeView.vue')
const marketsSource = read('../src/views/MarketsView.vue')
const assetsSource = read('../src/views/AssetsView.vue')
const profileSource = read('../src/views/ProfileView.vue')
const productHubSource = read('../src/views/ProductHubView.vue')

test('共享基础使用淡网格纸面、薄荷信号和 84px 五入口浮动 Dock', () => {
  assert.match(baseCss, /--bottom-nav-height:\s*84px/)
  assert.match(parityCss, /\.app-stage\.theme-light\s*\{[\s\S]*?--page:\s*#f8faf8/)
  assert.match(parityCss, /\.app-stage\s*\{[\s\S]*?--page:\s*#080a09/)
  assert.match(baseCss, /\.page--prototype-grid\s*\{[\s\S]*?background-image:/)
  assert.match(parityCss, /\.app-stage \.mobile-canvas\s*\{[\s\S]*?background-image:/)
  assert.match(prototypeCss, /--signal-green:\s*#55f7a5/)
  assert.match(prototypeCss, /--signal-coral:\s*#ff5c4d/)
  assert.match(parityCss, /@import '\.\/prototype-base\.css';/)

  const keys = [...bottomNavSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'trade', 'assets', 'profile'])
  assert.match(prototypeCss, /grid-template-columns:\s*repeat\(7,\s*minmax\(44px,\s*1fr\)\)/)
  assert.match(parityCss, /\.bottom-nav__dock\s*\{[\s\S]*?grid-template-columns:\s*repeat\(5, minmax\(0, 1fr\)\);[\s\S]*?height:\s*68px/)
  assert.match(parityCss, /\.trade-nav-action \.bottom-nav__icon\s*\{[\s\S]*?background:\s*var\(--pencil-mint\);[\s\S]*?height:\s*56px;[\s\S]*?width:\s*56px/)
  assert.match(parityCss, /\.bottom-nav__item\.active:not\(\.trade-nav-action\) \.bottom-nav__icon\s*\{[\s\S]*?var\(--pencil-positive\)/)
  assert.match(bottomNavSource, /data-nav-key="item\.key"/)
  assert.match(bottomNavSource, /aria-label="item\.label"/)
})

test('根头部与首页对齐选中品牌 Hero、成员资产图、双资金动作和八宫格', () => {
  assert.match(rootHeaderSource, /class="topbar root-header"/)
  assert.match(rootHeaderSource, /class="brand-logo"/)
  assert.match(rootHeaderSource, /class="icon-button has-dot root-header__control root-header__message"/)
  assert.match(homeSource, /class="home-utility-row"/)
  assert.match(homeSource, /home-portfolio--guest/)
  assert.match(homeSource, /home-guest-hero__image--light/)
  assert.match(homeSource, /home-guest-hero__image--dark/)
  assert.match(homeSource, /class="portfolio-overview home-portfolio home-portfolio--member"/)
  assert.match(homeSource, /class="portfolio-chart"/)
  assert.match(homeSource, /v-for="period in portfolioPeriods"/)
  assert.match(homeSource, /t\('home\.periodDays', \{ days \}\)/)
  assert.match(homeSource, /rootPrototype\.todayReturn/)
  assert.match(homeSource, /class="funding-actions"/)
  assert.doesNotMatch(homeSource, /portfolio-kicker|home-auth-primary|portfolio-retry/)

  const shortcutSection = homeSource.slice(
    homeSource.indexOf('<section class="shortcut-section"'),
    homeSource.indexOf('</section>', homeSource.indexOf('<section class="shortcut-section"')),
  )
  assert.equal((shortcutSection.match(/<button/g) || []).length, 8)
  for (const routeName of ['swap', 'earn', 'loan', 'new-coins', 'seconds', 'products']) {
    assert.match(shortcutSection, new RegExp(`name: '${routeName}'`))
  }
  assert.doesNotMatch(shortcutSection, /name: 'prediction'/)
})

test('首页保留真实数据链路并呈现日报与三行行情', () => {
  for (const contract of [
    /fetchWalletAccounts\(\)/,
    /fetchMarginWallets\(\)/,
    /marketStore\.startLiveUpdates\(\)/,
    /fetchNews\(\)/,
    /visibleTickers\.slice\(0,\s*3\)/,
    /name: 'news-detail'/,
  ]) assert.match(homeSource, contract)

  assert.match(homeSource, /type HomeTab = 'favorites' \| 'mainstream' \| 'popular' \| 'gainers' \| 'newCoins'/)
  assert.match(homeSource, /class="market-brief"/)
  assert.match(homeSource, /class="home-market-section"/)
  assert.doesNotMatch(homeSource, /\b128(?:,\d{3})?\b/)
})

test('资产、我的与产品中心保持真实数据摘要和对应工作台', () => {
  assert.match(assetsSource, /class="page pencil-page pencil-root-page assets-pencil"/)
  assert.match(assetsSource, /data-assets-workspace="live"/)
  assert.match(assetsSource, /new Intl\.NumberFormat\(locale\.value === 'en' \? 'en-US' : 'zh-CN'/)
  assert.match(assetsSource, /class="pencil-hero assets-hero assets-hero--guest"/)
  assert.match(assetsSource, /class="pencil-hero assets-hero assets-hero--member"/)
  assert.match(assetsSource, /class="assets-holdings__list"/)
  assert.match(profileSource, /data-profile-workspace="live"/)
  assert.match(profileSource, /class="profile-identity-pencil"/)
  assert.match(profileSource, /v-if="!session\.isAuthenticated" class="profile-auth-actions"/)
  assert.match(productHubSource, /data-product-workspace="live"/)
  assert.match(productHubSource, /data-product-count="2"/)
  assert.equal((productHubSource.match(/class="product-card product-card--secondary product-hub__row"/g) || []).length, 2)
  assert.match(productHubSource, /function openPrediction\(\): void[\s\S]*?name: 'prediction'/)
  assert.match(productHubSource, /function openNews\(\): void[\s\S]*?name: 'news'/)
  assert.doesNotMatch(productHubSource, /featuredProducts|secondaryProducts|v-for="product in/)
})

test('行情页提供五分类、真实温度概览和可进入详情的行情行', () => {
  assert.match(marketsSource, /type MarketCategory = 'popular' \| 'favorites' \| 'spot' \| 'contract' \| 'gainers'/)
  assert.match(marketsSource, /class="page-intro markets-hero"/)
  assert.match(marketsSource, /class="market-controls"/)
  assert.match(marketsSource, /marketTemperature = computed/)
  assert.match(marketsSource, /class="market-index"/)
  assert.match(marketsSource, /<strong v-if="hasTemperatureSample" class="numeric">/)
  assert.doesNotMatch(marketsSource, /hasTemperatureSample \? marketTemperature : '--'/)
  assert.match(marketsSource, /class="market-list"/)
  assert.match(marketsSource, /router\.push\(\{ name: 'market-detail'/)
  assert.match(marketsSource, /route\.query\.purpose === 'trade'/)
})
