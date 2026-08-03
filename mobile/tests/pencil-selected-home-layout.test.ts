import assert from 'node:assert/strict'
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const homeSource = source('../src/views/HomeView.vue')
const headerSource = source('../src/components/RootHeader.vue')
const navSource = source('../src/components/AppBottomNav.vue')
const detailSource = source('../src/views/MarketDetailView.vue')
const productSource = source('../src/views/ProductHubView.vue')
const routerSource = source('../src/router/index.ts')
const parityCss = source('../src/styles/prototype-parity.css')

test('Root Header 使用 1000x250 横版品牌素材并精确渲染为 136x34', () => {
  assert.match(headerSource, /import landscapeLogo from '@\/assets\/brand\/hippo-logo-landscape\.png'/)
  assert.match(headerSource, /:src="landscapeLogo"/)
  assert.doesNotMatch(headerSource, /hippo-logo-compact/)

  const logo = readFileSync(new URL('../src/assets/brand/hippo-logo-landscape.png', import.meta.url))
  assert.equal(logo.readUInt32BE(16), 1000)
  assert.equal(logo.readUInt32BE(20), 250)
  assert.match(parityCss, /\.root-header \.brand-logo\s*\{[\s\S]*?height:\s*34px;[\s\S]*?object-fit:\s*contain;[\s\S]*?width:\s*136px;/)
  assert.match(parityCss, /\.root-header__brand\s*\{[\s\S]*?flex:\s*0 0 136px;[\s\S]*?width:\s*136px;/)
  assert.match(parityCss, /\.root-header\.root-header\s*\{[\s\S]*?height:\s*calc\(56px \+ var\(--root-header-top-inset\)\)/)
  assert.match(parityCss, /\.root-header__control\.root-header__control,[\s\S]*?background:\s*transparent;[\s\S]*?border:\s*0;/)
})

test('访客主页使用 tracked 明暗 Hero，登录后才展示真实资产与观测曲线', () => {
  assert.match(homeSource, /import guestHeroDark from '@\/assets\/home\/market-hero-dark\.jpg'/)
  assert.match(homeSource, /import guestHeroLight from '@\/assets\/home\/market-hero-light\.jpg'/)
  assert.doesNotMatch(homeSource, /mobile\/pencil|@\/\.\.\/pencil|generated-178568/)
  assert.ok(existsSync(new URL('../src/assets/home/market-hero-light.jpg', import.meta.url)))
  assert.ok(existsSync(new URL('../src/assets/home/market-hero-dark.jpg', import.meta.url)))

  assert.match(homeSource, /v-if="!session\.isAuthenticated" class="home-portfolio home-portfolio--guest"/)
  assert.match(homeSource, /v-else[\s\S]*class="portfolio-overview home-portfolio home-portfolio--member"/)
  assert.match(homeSource, /class="home-guest-hero__image home-guest-hero__image--light" :src="guestHeroLight"/)
  assert.match(homeSource, /class="home-guest-hero__image home-guest-hero__image--dark" :src="guestHeroDark"/)
  assert.match(homeSource, /function openLogin\(\)[\s\S]*name: 'login'[\s\S]*redirect: '\/'/)
  assert.doesNotMatch(homeSource.match(/home-portfolio--guest[\s\S]*?<\/section>/)?.[0] ?? '', /displayedAssetAmount|portfolio-chart|todayReturn/)

  assert.match(homeSource, /fetchWalletAccounts\(\)/)
  assert.match(homeSource, /fetchMarginWallets\(\)/)
  assert.match(homeSource, /const portfolioSamples = ref<number\[\]>\(\[\]\)/)
  assert.match(homeSource, /if \(!ready \|\| !complete \|\| !Number\.isFinite\(value\)\) return/)
  assert.match(homeSource, /v-if="portfolioGeometry"[\s\S]*:d="portfolioGeometry\.path"/)
  assert.match(homeSource, /rootPrototype\.todayReturn[\s\S]*<strong class="numeric">--<\/strong>/)
  assert.doesNotMatch(homeSource, /M0 67 C28 62|fallbackReturn|mockPortfolio|demo(?:Data|Portfolio)/i)
})

test('首页 390px 块级几何与三行真实行情按选中画板固定', () => {
  assert.match(parityCss, /\.home-view \.home-utility-row\s*\{[\s\S]*?height:\s*56px;/)
  assert.match(parityCss, /\.home-view \.home-portfolio--guest\s*\{[\s\S]*?padding:\s*16px;/)
  assert.match(parityCss, /\.home-view \.home-guest-hero\s*\{[\s\S]*?height:\s*270px;/)
  assert.match(parityCss, /\.home-portfolio--member\.portfolio-overview\s*\{[\s\S]*?grid-template-rows:\s*82px 153px 43px;[\s\S]*?min-height:\s*302px;/)
  assert.match(parityCss, /\.home-view \.funding-actions\s*\{[\s\S]*?height:\s*64px;/)
  assert.match(parityCss, /\.home-view \.shortcut-section\s*\{[\s\S]*?height:\s*176px;/)
  assert.match(parityCss, /\.home-view \.market-brief\s*\{[\s\S]*?height:\s*64px;[\s\S]*?margin:\s*8px 16px;/)
  assert.match(parityCss, /\.theme-light \.mobile-canvas \.home-view \.market-brief:disabled\s*\{[\s\S]*?color:\s*#f2f7f4;[\s\S]*?opacity:\s*1;[\s\S]*?-webkit-text-fill-color:\s*#f2f7f4;/)
  assert.match(parityCss, /\.theme-light \.mobile-canvas \.home-view \.market-brief:disabled small\s*\{[\s\S]*?color:\s*var\(--pencil-mint\);/)
  assert.match(parityCss, /\.theme-light \.mobile-canvas \.home-view \.market-brief:disabled strong\s*\{[\s\S]*?color:\s*#f2f7f4;/)
  assert.match(parityCss, /\.theme-light \.mobile-canvas \.home-view \.market-brief:disabled em,[\s\S]*?color:\s*#95a19a;/)
  assert.match(parityCss, /\.home-view \.home-market-section\s*\{[\s\S]*?height:\s*290px;/)
  assert.equal((homeSource.match(/v-for="row in 3"/g) || []).length, 1)
  assert.match(homeSource, /v-for="ticker in visibleTickers\.slice\(0, 3\)"/)
  assert.match(homeSource, /marketRowsUnavailable[\s\S]*class="root-market-reserved-state" role="alert"/)
  assert.match(parityCss, /@media \(max-width: 360px\)/)
  assert.match(parityCss, /@media \(max-width: 340px\)/)
})

test('根 Dock 只保留五入口与 56px 中央交易，独立产品路由仍可达', () => {
  const keys = [...navSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'trade', 'assets', 'profile'])
  assert.match(navSource, /class="bottom-nav__dock"/)
  assert.match(navSource, /'trade-nav-action': item\.primary/)
  assert.match(navSource, /params: \{ symbol: navigation\.lastTradeSymbol \}/)
  assert.match(navSource, /navigation\.lastTradeMode === 'contract'/)
  assert.match(parityCss, /\.bottom-nav__dock\s*\{[\s\S]*?grid-template-columns:\s*repeat\(5, minmax\(0, 1fr\)\);[\s\S]*?height:\s*68px;/)
  assert.match(parityCss, /\.trade-nav-action \.bottom-nav__icon\s*\{[\s\S]*?background:\s*var\(--pencil-mint\);[\s\S]*?height:\s*56px;[\s\S]*?width:\s*56px;/)
  assert.match(parityCss, /\.bottom-nav__item\.active:not\(\.seconds-nav-action\):not\(\.trade-nav-action\) \.bottom-nav__icon/)
  assert.doesNotMatch(parityCss, /\.bottom-nav__item\.active:not\(\.seconds-nav-action\) \.bottom-nav__icon/)
  assert.match(homeSource, /openTrade\('spot'\)/)
  assert.match(homeSource, /openTrade\('contract'\)/)
  assert.match(homeSource, /data-home-shortcut="seconds"[\s\S]*?name: 'seconds'[\s\S]*?<Zap :size="19"[\s\S]*?home\.secondsShortcut/)
  assert.doesNotMatch(homeSource.match(/<section class="shortcut-section"[\s\S]*?<\/section>/)?.[0] ?? '', /name: 'prediction'|home\.predictionShortcut|<Gauge/)
  assert.match(homeSource, /name: 'products'/)
  assert.match(detailSource, /openTrade\('spot'\)/)
  assert.match(detailSource, /openTrade\('contract'\)/)
  assert.match(productSource, /name: 'prediction'/)
  assert.match(productSource, /name: 'news'/)
  assert.match(routerSource, /path: '\/seconds',[\s\S]*name: 'seconds'/)
  assert.doesNotMatch(navSource, /\p{Extended_Pictographic}/u)
})

test('选中首页文案在中英资源中对称', () => {
  for (const key of [
    'guestHeroLine1',
    'guestHeroLine2',
    'guestHeroDescription',
    'guestHeroLogin',
    'mainstream',
  ] as const) {
    assert.equal(typeof zhCN.home[key], 'string')
    assert.equal(typeof en.home[key], 'string')
  }
})

test('生产运行时源码与构建入口不依赖 Pencil 设计目录', () => {
  const runtimeSources = [
    ...readTree(new URL('../src/', import.meta.url)),
    source('../index.html'),
    source('../vite.config.ts'),
    source('../package.json'),
  ].join('\n')

  assert.doesNotMatch(runtimeSources, /mobile\/pencil|hippo-mobile-uiux\.pen|(?:^|["'(@])(?:\.\.\/)*pencil\//m)
})

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

function readTree(root: URL): string[] {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const target = new URL(entry.name + (entry.isDirectory() ? '/' : ''), root)
    if (entry.isDirectory()) return readTree(target)
    if (!/\.(?:css|html|ts|vue)$/.test(entry.name)) return []
    return [readFileSync(target, 'utf8')]
  })
}
