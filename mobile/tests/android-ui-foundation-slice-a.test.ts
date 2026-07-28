import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')

const baseCss = read('../src/styles/base.css')
const bottomNavSource = read('../src/components/AppBottomNav.vue')
const homeSource = read('../src/views/HomeView.vue')
const marketsSource = read('../src/views/MarketsView.vue')
const assetsSource = read('../src/views/AssetsView.vue')
const profileSource = read('../src/views/ProfileView.vue')
const productHubSource = read('../src/views/ProductHubView.vue')

test('v16 共享基础使用明亮纸面、信号色和 84px 七栏导航', () => {
  assert.match(baseCss, /--background:\s*#edf1ee/)
  assert.match(baseCss, /--surface:\s*#ffffff/)
  assert.match(baseCss, /--signal-green:\s*#55f7a5/)
  assert.match(baseCss, /--positive:\s*#007a4d/)
  assert.match(baseCss, /--signal-coral:\s*#ff5c4d/)
  assert.match(baseCss, /--bottom-nav-height:\s*84px/)
  assert.match(baseCss, /:root\[data-theme='dark'\][\s\S]*?--background:\s*#090b0a/)

  const keys = [...bottomNavSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'spot', 'seconds', 'contract', 'assets', 'profile'])
  assert.match(bottomNavSource, /grid-template-columns:\s*repeat\(7,\s*minmax\(44px,\s*1fr\)\)/)
  assert.match(bottomNavSource, /\.is-primary[\s\S]*?width:\s*48px/)
})

test('首页对齐 v16 的品牌工具栏、明亮资产图、双资金动作和八宫格', () => {
  assert.match(homeSource, /<img :src="logo" class="home-header__logo" alt="HIPPO" \/>/)
  assert.match(homeSource, /class="[^"]*home-header__notification[^"]*"/)
  assert.match(homeSource, /class="home-scan"/)
  assert.match(homeSource, /class="asset-glance__chart"/)
  assert.match(homeSource, /v-for="period in portfolioPeriods"/)
  assert.match(homeSource, /t\('home\.periodDays', \{ days \}\)/)
  assert.doesNotMatch(homeSource, /earn\.termDays/)
  assert.match(homeSource, /name: 'quick-recharge'/)
  assert.match(homeSource, /name: 'deposit-asset'/)

  const shortcutSection = homeSource.slice(
    homeSource.indexOf('<section class="shortcut-section"'),
    homeSource.indexOf('</section>', homeSource.indexOf('<section class="shortcut-section"')),
  )
  assert.equal((shortcutSection.match(/<button/g) || []).length, 8)
  for (const routeName of ['swap', 'earn', 'loan', 'new-coins', 'seconds', 'products']) {
    assert.match(shortcutSection, new RegExp(`name: '${routeName}'`))
  }
})

test('首页保留真实数据链路并呈现橙色简报、五栏行情和安全入口', () => {
  for (const contract of [
    /fetchWalletAccounts\(\)/,
    /fetchMarginWallets\(\)/,
    /marketStore\.startLiveUpdates\(\)/,
    /fetchNews\(\)/,
    /visibleTickers\.slice\(0,\s*5\)/,
    /name: 'news-detail'/,
    /name: 'kyc'/,
    /name: 'security'/,
  ]) {
    assert.match(homeSource, contract)
  }

  assert.match(homeSource, /type HomeTab = 'favorites' \| 'mainstream' \| 'popular' \| 'gainers' \| 'newCoins'/)
  assert.match(homeSource, /\.announcement-row\s*\{[\s\S]*?background:\s*var\(--accent\)/)
  assert.doesNotMatch(homeSource, /\b128(?:,\d{3})?\b/)
})

test('资产、我的与产品中心使用真实数据摘要和 v16 信号工作台', () => {
  for (const source of [assetsSource, profileSource, productHubSource]) {
    assert.match(source, /page--prototype-grid/)
  }
  assert.match(assetsSource, /data-assets-workspace="live"/)
  assert.match(assetsSource, /formatFiat\(spotEstimate\)/)
  assert.match(assetsSource, /formatFiat\(marginEstimate\)/)
  assert.match(assetsSource, /assets-summary__metrics/)
  assert.match(profileSource, /data-profile-workspace="live"/)
  assert.match(profileSource, /profile-metrics/)
  assert.match(productHubSource, /data-product-workspace="live"/)
  assert.match(productHubSource, /featuredProducts/)
  assert.match(productHubSource, /secondaryProducts/)
  assert.match(productHubSource, /products\.featuredServices/)
  assert.match(productHubSource, /products\.specializedServices/)
})

test('行情页提供五分类、真实温度概览和可进入详情的行情行', () => {
  assert.match(marketsSource, /type MarketCategory = 'popular' \| 'favorites' \| 'spot' \| 'contract' \| 'gainers'/)
  assert.match(marketsSource, /class="market-intro"/)
  assert.match(marketsSource, /marketTemperature = computed/)
  assert.match(marketsSource, /class="market-temperature"/)
  assert.match(marketsSource, /class="market-list__spark"/)
  assert.match(marketsSource, /router\.push\(\{ name: 'market-detail'/)
  assert.equal((marketsSource.match(/data-market-destination=/g) || []).length, 3)
})
