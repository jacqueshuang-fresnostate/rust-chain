import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const homeSource = readFileSync(new URL('../src/views/HomeView.vue', import.meta.url), 'utf8')
const marketsSource = readFileSync(new URL('../src/views/MarketsView.vue', import.meta.url), 'utf8')
const productHubSource = readFileSync(new URL('../src/views/ProductHubView.vue', import.meta.url), 'utf8')

test('首页保留真实数据链路并提供三个独立交易入口', () => {
  assert.match(homeSource, /import logo from '@\/assets\/logo\.png'/)
  assert.match(homeSource, /<img :src="logo" class="home-header__logo" alt="HIPPO" \/>/)
  assert.match(homeSource, /fetchWalletAccounts\(\)/)
  assert.match(homeSource, /fetchMarginWallets\(\)/)
  assert.match(homeSource, /formatFiat\(totalAssetEstimate\.value\)/)
  assert.match(homeSource, /openTrade\('spot'\)/)
  assert.match(homeSource, /openTrade\('contract'\)/)
  assert.match(homeSource, /router\.push\(\{ name: 'seconds' \}\)/)
  assert.match(homeSource, /navigation\.rememberTradeMode\(mode\)/)
  assert.match(homeSource, /query: mode === 'contract' \? \{ mode \} : undefined/)
  assert.match(homeSource, /marketStore\.startLiveUpdates\(\)/)
  assert.match(homeSource, /fetchNews\(\)/)
})

test('首页通知按钮打开真实消息中心命名路由', () => {
  const headerSource = homeSource.slice(
    homeSource.indexOf('<header class="home-header">'),
    homeSource.indexOf('</header>'),
  )

  assert.equal((headerSource.match(/name: 'message-center'/g) || []).length, 1)
  assert.match(headerSource, /<Bell[\s\S]*?name: 'message-center'|name: 'message-center'[\s\S]*?<Bell/)
  assert.match(headerSource, /@click="theme\.toggleTheme"/)
  assert.doesNotMatch(headerSource, /name: 'news'|Newspaper/)
  assert.match(homeSource, /class="announcement-more"[\s\S]*?name: 'news'/)
  assert.match(homeSource, /class="announcement-row"[\s\S]*?name: 'news-detail'/)
  assert.match(
    homeSource,
    /\.home-header\s*\{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\)\s*auto/,
  )
  assert.doesNotMatch(homeSource, /grid-template-columns:\s*minmax\(88px,\s*1fr\)/)
})

test('行情页保留交易对选择器的交易模式、查询参数和历史语义', () => {
  assert.match(marketsSource, /route\.query\.purpose === 'trade'/)
  assert.match(marketsSource, /route\.query\.mode === 'contract'/)
  assert.match(marketsSource, /navigation\.rememberTradeSymbol\(routeSymbol\)/)
  assert.match(marketsSource, /navigation\.rememberTradeMode\(mode\)/)
  assert.match(marketsSource, /router\.replace\(\{[\s\S]*?name: 'trade'/)
  assert.match(marketsSource, /router\.push\(\{ name: 'market-detail'/)
  assert.equal((marketsSource.match(/data-market-destination=/g) || []).length, 3)
})

test('产品中心维持两项精选与三项次级产品的真实路由矩阵', () => {
  assert.equal((productHubSource.match(/tier: 'featured'/g) || []).length, 2)
  assert.equal((productHubSource.match(/tier: 'secondary'/g) || []).length, 3)
  for (const routeName of ['earn', 'loan', 'new-coins', 'prediction', 'seconds']) {
    assert.match(productHubSource, new RegExp(`name: '${routeName}'`))
  }
  assert.match(productHubSource, /router\.push\(\{ name \}\)/)
  assert.match(productHubSource, /featuredProducts/)
  assert.match(productHubSource, /secondaryProducts/)
})

test('核心发现视图遵守窄屏、触控与图标契约', () => {
  for (const source of [homeSource, marketsSource, productHubSource]) {
    assert.match(source, /@media \(max-width: 340px\)/)
    assert.doesNotMatch(source, /<svg/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
    assert.doesNotMatch(source, /[\u3400-\u9fff]/)
    assert.doesNotMatch(source, /#[0-9a-f]{3,8}/i)
  }
  assert.match(homeSource, /min-height: 44px/)
  assert.match(marketsSource, /min-height: 44px/)
  assert.match(productHubSource, /height: 44px/)
})
