import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const appSource = read('../src/App.vue')
const bottomNavSource = read('../src/components/AppBottomNav.vue')
const rootHeaderSource = read('../src/components/RootHeader.vue')
const pageHeaderSource = read('../src/components/PageHeader.vue')
const secondsSource = read('../src/views/SecondsView.vue')
const productHubSource = read('../src/views/ProductHubView.vue')
const routerSource = read('../src/router/index.ts')
const baseStyles = read('../src/styles/base.css')
const prototypeStyles = read('../src/styles/prototype-base.css')
const parityStyles = read('../src/styles/prototype-parity.css')

test('根导航保持五个有序目的地与单一交易入口', () => {
  const keys = [...bottomNavSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'trade', 'assets', 'profile'])
  assert.match(bottomNavSource, /t\('nav\.trade'\)/)
  assert.match(bottomNavSource, /params:\s*\{\s*symbol:\s*navigation\.lastTradeSymbol\s*\}/)
  assert.match(bottomNavSource, /navigation\.lastTradeMode === 'contract' \? \{ mode: 'contract' \} : undefined/)
  assert.match(bottomNavSource, /function selectRoot\(to: RouteLocationRaw\)/)
  assert.match(bottomNavSource, /router\.replace\(to\)/)
  assert.match(bottomNavSource, /key:\s*'trade'[\s\S]*?primary:\s*true/)
})

test('秒合约保留旧深链但作为无根导航的二级面', () => {
  assert.match(
    routerSource,
    /\{\s*path:\s*'\/seconds',\s*alias:\s*'\/products\/seconds',\s*name:\s*'seconds',[\s\S]*?meta:\s*\{\s*showBottomNav:\s*false,\s*depth:\s*1,\s*backFallback:\s*'\/'\s*\}\s*\}/,
  )
  assert.match(
    routerSource,
    /\{\s*path:\s*'\/messages',\s*name:\s*'message-center',[\s\S]*?meta:\s*\{\s*depth:\s*1,\s*showBottomNav:\s*false,\s*backFallback:\s*\{\s*name:\s*'home'\s*\}\s*\}\s*\}/,
  )
  assert.match(appSource, /const showRootHeader = computed\(\(\) => \([\s\S]*?\['home', 'markets'\]/)
  assert.match(appSource, /<RootHeader v-if="showRootHeader" \/>/)
  assert.match(appSource, /<AppBottomNav v-if="showBottomNav" \/>/)
  assert.match(secondsSource, /createBottomNavSecondsFallbackTarget\(\)/)
  assert.match(secondsSource, /isBottomNavigationSecondsEntry\(router\.options\.history\.state\)/)
  assert.match(secondsSource, /:fallback="homeFallback"/)
  assert.match(secondsSource, /:prefer-fallback="preferHomeFallback"/)
  assert.match(productHubSource, /router\.push\(\{ name: 'prediction' \}\)/)
  assert.match(productHubSource, /router\.push\(\{ name: 'news' \}\)/)
  assert.match(productHubSource, /router\.push\(\{ name: 'news', query: \{ category: 'product' \} \}\)/)
  assert.match(pageHeaderSource, /\{\s*preferFallback:\s*props\.preferFallback\s*\}/)
})

test('导航触控和头部层级由受检原型 CSS 统一提供', () => {
  assert.match(parityStyles, /\.bottom-nav__dock\s*\{[\s\S]*?grid-template-columns:\s*repeat\(5,\s*minmax\(0,\s*1fr\)\)/)
  assert.match(parityStyles, /\.bottom-nav__item\s*\{[\s\S]*?min-width:\s*44px/)
  assert.match(baseStyles, /--bottom-nav-height:\s*84px/)
  assert.match(parityStyles, /\.bottom-nav__item:focus-visible \.bottom-nav__icon\s*\{[\s\S]*?box-shadow:\s*0 0 0 3px var\(--focus-ring\)/)
  assert.match(baseStyles, /html\s*\{[\s\S]*scrollbar-width:\s*none/)
  assert.match(rootHeaderSource, /class="topbar root-header"/)
  assert.match(rootHeaderSource, /:aria-label="t\('nav\.home'\)"/)
  assert.match(rootHeaderSource, /router\.replace\(\{ name: 'home' \}\)/)
  assert.match(prototypeStyles, /\.topbar,[\s\S]*?\.secondary-header\s*\{[\s\S]*?z-index:\s*70/)
  assert.match(pageHeaderSource, /pencil \? 'pencil-page-header' : 'secondary-header'/)
  assert.match(prototypeStyles, /Signal Theatre final secondary-surface contract[\s\S]*?\.secondary-header\s*\{[\s\S]*?min-height:\s*76px/)
})
