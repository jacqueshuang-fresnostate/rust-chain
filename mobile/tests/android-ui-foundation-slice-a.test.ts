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

test('v16 共享基础使用原型纸面、信号色和 84px 七栏导航', () => {
  assert.match(baseCss, /--bottom-nav-height:\s*84px/)
  assert.match(parityCss, /\.app-stage\.theme-light\s*\{[\s\S]*?--page:\s*#f6f8fb/)
  assert.match(prototypeCss, /--signal-green:\s*#55f7a5/)
  assert.match(prototypeCss, /--signal-coral:\s*#ff5c4d/)
  assert.match(parityCss, /@import '\.\/prototype-base\.css';/)

  const keys = [...bottomNavSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'spot', 'seconds', 'contract', 'assets', 'profile'])
  assert.match(prototypeCss, /grid-template-columns:\s*repeat\(7,\s*minmax\(44px,\s*1fr\)\)/)
  assert.match(prototypeCss, /\.bottom-nav \.seconds-nav-action span\s*\{[\s\S]*?width:\s*48px/)
})

test('根头部与首页对齐原型工具栏、资产图、双资金动作和八宫格', () => {
  assert.match(rootHeaderSource, /class="topbar root-header"/)
  assert.match(rootHeaderSource, /class="brand-logo"/)
  assert.match(rootHeaderSource, /class="icon-button has-dot root-header__control root-header__message"/)
  assert.match(homeSource, /class="home-utility-row"/)
  assert.match(homeSource, /class="portfolio-overview"/)
  assert.match(homeSource, /class="portfolio-chart"/)
  assert.match(homeSource, /v-for="period in portfolioPeriods"/)
  assert.match(homeSource, /t\('home\.periodDays', \{ days \}\)/)
  assert.match(homeSource, /class="funding-actions"/)

  const shortcutSection = homeSource.slice(
    homeSource.indexOf('<section class="shortcut-section"'),
    homeSource.indexOf('</section>', homeSource.indexOf('<section class="shortcut-section"')),
  )
  assert.equal((shortcutSection.match(/<button/g) || []).length, 8)
  for (const routeName of ['swap', 'earn', 'loan', 'new-coins', 'prediction', 'products']) {
    assert.match(shortcutSection, new RegExp(`name: '${routeName}'`))
  }
})

test('首页保留真实数据链路并呈现日报、五行行情和安全入口', () => {
  for (const contract of [
    /fetchWalletAccounts\(\)/,
    /fetchMarginWallets\(\)/,
    /marketStore\.startLiveUpdates\(\)/,
    /fetchNews\(\)/,
    /visibleTickers\.slice\(0,\s*5\)/,
    /name: 'news-detail'/,
    /name: 'kyc'/,
    /name: 'security'/,
  ]) assert.match(homeSource, contract)

  assert.match(homeSource, /type HomeTab = 'favorites' \| 'mainstream' \| 'popular' \| 'gainers' \| 'newCoins'/)
  assert.match(homeSource, /class="market-brief"/)
  assert.match(homeSource, /class="home-market-section"/)
  assert.doesNotMatch(homeSource, /\b128(?:,\d{3})?\b/)
})

test('资产、我的与产品中心保持真实数据摘要和对应工作台', () => {
  assert.match(assetsSource, /class="view assets-view prototype-root-view"/)
  assert.match(assetsSource, /data-assets-workspace="live"/)
  assert.match(assetsSource, /formatFiat\(spotEstimate\)/)
  assert.match(assetsSource, /formatFiat\(marginEstimate\)/)
  assert.match(assetsSource, /class="asset-hero"/)
  assert.match(profileSource, /data-profile-workspace="live"/)
  assert.match(profileSource, /class="profile-metrics"/)
  assert.match(profileSource, /<template v-if="!session\.isAuthenticated">/)
  assert.match(productHubSource, /data-product-workspace="live"/)
  assert.match(productHubSource, /featuredProducts/)
  assert.match(productHubSource, /secondaryProducts/)
})

test('行情页提供五分类、真实温度概览和可进入详情的行情行', () => {
  assert.match(marketsSource, /type MarketCategory = 'popular' \| 'favorites' \| 'spot' \| 'contract' \| 'gainers'/)
  assert.match(marketsSource, /class="page-intro"/)
  assert.match(marketsSource, /marketTemperature = computed/)
  assert.match(marketsSource, /class="market-index"/)
  assert.match(marketsSource, /class="market-list"/)
  assert.match(marketsSource, /router\.push\(\{ name: 'market-detail'/)
  assert.match(marketsSource, /route\.query\.purpose === 'trade'/)
})
