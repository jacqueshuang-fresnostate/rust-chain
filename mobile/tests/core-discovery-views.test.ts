import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const homeSource = read('../src/views/HomeView.vue')
const marketsSource = read('../src/views/MarketsView.vue')
const productHubSource = read('../src/views/ProductHubView.vue')
const rootHeaderSource = read('../src/components/RootHeader.vue')
const prototypeCss = read('../src/styles/prototype-base.css')

test('首页保留真实数据链路并提供独立现货与合约入口', () => {
  assert.match(homeSource, /fetchWalletAccounts\(\)/)
  assert.match(homeSource, /fetchMarginWallets\(\)/)
  assert.match(homeSource, /formatAmount\(totalAssetEstimate\.value\)/)
  assert.match(homeSource, /openTrade\('spot'\)/)
  assert.match(homeSource, /openTrade\('contract'\)/)
  assert.match(homeSource, /navigation\.rememberTradeMode\(mode\)/)
  assert.match(homeSource, /query: mode === 'contract' \? \{ mode \} : undefined/)
  assert.match(homeSource, /marketStore\.startLiveUpdates\('home'\)/)
  assert.match(homeSource, /fetchNews\(\)/)
})

test('共享根头部打开真实消息中心并复用主题 store', () => {
  assert.equal((rootHeaderSource.match(/name: 'message-center'/g) || []).length, 1)
  assert.match(rootHeaderSource, /<Bell/)
  assert.match(rootHeaderSource, /const theme = useThemeStore\(\)/)
  assert.match(rootHeaderSource, /@click="theme\.toggleTheme"/)
  assert.match(rootHeaderSource, /<Sun v-if="theme\.isDark"[\s\S]*?<Moon v-else/)
  assert.match(rootHeaderSource, /class="topbar-actions action-cluster root-header__actions"/)
})

test('行情页保留交易对选择器的模式、查询参数和历史语义', () => {
  assert.match(marketsSource, /route\.query\.purpose === 'trade'/)
  assert.match(marketsSource, /route\.query\.mode === 'contract'/)
  assert.match(marketsSource, /navigation\.rememberTradeSymbol\(routeSymbol\)/)
  assert.match(marketsSource, /navigation\.rememberTradeMode\(mode\)/)
  assert.match(marketsSource, /router\.replace\(\{[\s\S]*?name: 'trade'/)
  assert.match(marketsSource, /router\.push\(\{ name: 'market-detail'/)
})

test('产品中心按 Z0B0N6 仅提供预测、新闻与产品说明真实路由', () => {
  assert.equal((productHubSource.match(/class="product-card product-card--secondary product-hub__row"/g) || []).length, 2)
  assert.match(productHubSource, /data-product="prediction"[\s\S]*?@click="openPrediction"/)
  assert.match(productHubSource, /data-product="news"[\s\S]*?@click="openNews"/)
  assert.match(productHubSource, /router\.push\(\{ name: 'prediction' \}\)/)
  assert.match(productHubSource, /router\.push\(\{ name: 'news' \}\)/)
  assert.match(productHubSource, /router\.push\(\{ name: 'news', query: \{ category: 'product' \} \}\)/)
  assert.doesNotMatch(productHubSource, /featuredProducts|secondaryProducts|tier: 'featured'|v-for="product in/)
})

test('核心发现视图遵守共享窄屏、触控与 Lucide 图标契约', () => {
  assert.match(prototypeCss, /@media \(max-width: 350px\)/)
  assert.match(prototypeCss, /\.home-search\s*\{[\s\S]*?min-height:\s*44px/)
  assert.match(prototypeCss, /\.market-main\s*\{[\s\S]*?min-height:\s*68px/)
  for (const source of [homeSource, marketsSource]) {
    assert.doesNotMatch(source, /<style scoped|\p{Extended_Pictographic}/u)
    assert.doesNotMatch(source, /[\u3400-\u9fff]/)
    assert.doesNotMatch(source, /#[0-9a-f]{3,8}/i)
  }
  assert.match(homeSource, /<svg viewBox="0 0 358 153"/)
  assert.match(homeSource, /v-if="portfolioGeometry"[\s\S]*:d="portfolioGeometry\.path"/)
  assert.match(marketsSource, /class="sparkline"/)
})
