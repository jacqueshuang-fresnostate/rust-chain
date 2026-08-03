import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const baseCss = read('../src/styles/base.css')
const parityCss = read('../src/styles/prototype-parity.css')
const headerSource = read('../src/components/RootHeader.vue')
const navSource = read('../src/components/AppBottomNav.vue')
const homeSource = read('../src/views/HomeView.vue')
const marketsSource = read('../src/views/MarketsView.vue')

function assertOrdered(source: string, selectors: string[]): void {
  let previous = -1
  for (const selector of selectors) {
    const current = source.indexOf(selector, previous + 1)
    assert.ok(current > previous, `${selector} must follow the previous home block`)
    previous = current
  }
}

test('Instrument Editorial 全局材质收敛为淡网格画布、连续面板与薄荷主动作', () => {
  assert.match(baseCss, /--background:\s*#f7faf8/)
  assert.match(baseCss, /:root\[data-theme='dark'\]\s*\{[\s\S]*?--background:\s*#070908/)
  assert.match(
    baseCss,
    /\.page--prototype-grid\s*\{[\s\S]*?background-image:\s*[\s\S]*?linear-gradient\(var\(--grid-line\)[\s\S]*?background-size:\s*48px 48px/,
  )
  assert.match(parityCss, /\.app-stage\s*\{[\s\S]*?--page:\s*#080a09;[\s\S]*?--green:\s*#52e2a1/)
  assert.match(parityCss, /--accent:\s*var\(--green\)/)
  assert.match(parityCss, /\.app-stage\.theme-light\s*\{[\s\S]*?--page:\s*#f8faf8;[\s\S]*?--surface:\s*#ffffff;[\s\S]*?--green:\s*#007a4d/)
  assert.match(parityCss, /\.markets-view \.markets-hero\s*\{[\s\S]*?var\(--signal-green\)/)
  assert.doesNotMatch(`${baseCss}\n${parityCss}`, /#0b1811|rgba\(11,\s*24,\s*17/i)
})

test('Root Header 保留真实首页、主题与消息行为并稳定在 44px 和 sticky token 层', () => {
  assert.match(headerSource, /const isHome = computed\(\(\) => route\.name === 'home'\)/)
  assert.match(headerSource, /:aria-current="isHome \? 'page' : undefined"/)
  assert.match(headerSource, /router\.replace\(\{ name: 'home' \}\)/)
  assert.match(headerSource, /@click="theme\.toggleTheme"/)
  assert.match(headerSource, /router\.push\(\{ name: 'message-center' \}\)/)
  assert.equal((headerSource.match(/\broot-header__control\b/g) || []).length, 2)
  assert.match(parityCss, /> \.root-header\.root-header\s*\{[\s\S]*?height:\s*calc\(56px \+ var\(--root-header-top-inset\)\);[\s\S]*?z-index:\s*var\(--layer-sticky-header\)/)
  assert.match(parityCss, /\.topbar-actions > \.icon-button\.icon-button,[\s\S]*?height:\s*44px;[\s\S]*?width:\s*44px/)
  assert.match(headerSource, /hippo-logo-landscape\.png/)
  assert.match(parityCss, /\.root-header \.brand-logo\s*\{[\s\S]*?height:\s*34px;[\s\S]*?object-fit:\s*contain;[\s\S]*?width:\s*136px/)
})

test('五项导航保留真实路由、当前项与键盘焦点，中央交易为 56px 主动作', () => {
  const keys = [...navSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'trade', 'assets', 'profile'])
  assert.match(navSource, /class="bottom-nav__item"/)
  assert.match(navSource, /class="bottom-nav__icon"/)
  assert.match(navSource, /:aria-current="item\.active \? 'page' : undefined"/)
  assert.match(navSource, /params: \{ symbol: navigation\.lastTradeSymbol \}/)
  assert.match(navSource, /navigation\.lastTradeMode === 'contract'/)
  assert.match(navSource, /router\.replace\(to\)/)
  assert.doesNotMatch(navSource, /<svg|\p{Extended_Pictographic}/u)

  assert.match(parityCss, /\.bottom-nav\s*\{[\s\S]*?env\(safe-area-inset-bottom\)/)
  assert.match(parityCss, /\.bottom-nav__dock\s*\{[\s\S]*?grid-template-columns:\s*repeat\(5, minmax\(0, 1fr\)\);[\s\S]*?height:\s*68px/)
  assert.match(parityCss, /\.bottom-nav__item\s*\{[\s\S]*?min-height:\s*56px;[\s\S]*?min-width:\s*44px/)
  assert.match(parityCss, /\.trade-nav-action \.bottom-nav__icon\s*\{[\s\S]*?background:\s*var\(--pencil-mint\);[\s\S]*?height:\s*56px/)
  assert.match(parityCss, /\.bottom-nav__item:focus-visible \.bottom-nav__icon\s*\{[\s\S]*?box-shadow:\s*0 0 0 3px var\(--focus-ring\)/)
  assert.match(parityCss, /\.bottom-nav\s*\{[\s\S]*?pointer-events:\s*none;[\s\S]*?z-index:\s*var\(--layer-navigation\)/)
  assert.match(parityCss, /\.bottom-nav__dock\s*\{[\s\S]*?pointer-events:\s*auto/)

  const readLayer = (name: string): number => {
    const match = baseCss.match(new RegExp(`--layer-${name}:\\s*(\\d+)`))
    assert.ok(match, `missing layer token: ${name}`)
    return Number(match[1])
  }
  assert.ok(readLayer('content') < readLayer('route-transition'))
  assert.ok(readLayer('route-transition') < readLayer('navigation'))
  assert.ok(readLayer('navigation') < readLayer('sticky-header'))
  assert.ok(readLayer('sticky-header') < readLayer('overlay'))
  assert.ok(readLayer('overlay') < readLayer('launch'))
})

test('首页按选中访客 Hero，登录后使用真实资产观测曲线', () => {
  assert.equal((homeSource.match(/class="portfolio-overview\s+home-portfolio\s+home-portfolio--member"/g) || []).length, 1)
  assert.match(homeSource, /const portfolioPeriods = computed/)
  assert.match(homeSource, /t\('home\.periodDays', \{ days \}\)/)
  assert.match(homeSource, /const assetEstimateReady = ref\(false\)/)
  assert.match(homeSource, /home-portfolio--guest/)
  assert.match(homeSource, /guestHeroLight/)
  assert.match(homeSource, /guestHeroDark/)
  assert.match(homeSource, /class="portfolio-chart"/)
  assert.match(homeSource, /<svg viewBox="0 0 358 153"[\s\S]*?v-if="portfolioGeometry"[\s\S]*?:d="portfolioGeometry\.path"/)
  assert.match(homeSource, /rootPrototype\.todayReturn/)
  assert.match(homeSource, /v-for="period in portfolioPeriods"/)
  assert.match(homeSource, /name: 'quick-recharge'/)
  assert.match(homeSource, /name: 'deposit-asset'/)
  assert.doesNotMatch(homeSource, /portfolio-kicker|home-auth-primary|portfolio-retry|assetEstimateState|hasAssetEstimate/)
  assertOrdered(homeSource, [
    'class="home-utility-row"',
    'class="home-portfolio home-portfolio--guest"',
    'class="portfolio-overview home-portfolio home-portfolio--member"',
    'class="funding-actions"',
    'class="shortcut-section"',
    'class="market-brief"',
    'class="home-market-section"',
  ])
  assert.match(parityCss, /\.home-portfolio--member\.portfolio-overview\s*\{[\s\S]*?min-height:\s*302px/)
  assert.match(parityCss, /\.home-view \.home-guest-hero\s*\{[\s\S]*?height:\s*270px/)
  assert.match(parityCss, /\.home-view \.market-brief\s*\{[\s\S]*?height:\s*64px/)
})

test('行情页压缩 Hero 并把搜索、五分类、真实广度和连续列表组成单一层级', () => {
  assert.match(marketsSource, /class="page-intro markets-hero"/)
  assert.match(marketsSource, /<section class="market-controls"[\s\S]*?class="search-field"[\s\S]*?class="filter-rail"/)
  assert.equal((marketsSource.match(/<section class="market-index"/g) || []).length, 1)
  assert.match(marketsSource, /<section class="market-index"[\s\S]*?<\/section>\s*<div class="market-table-head"/)
  assert.match(marketsSource, /const marketTemperature = computed\(\(\) => Math\.round\(positiveRate\.value\)\)/)
  assert.match(marketsSource, /<strong v-if="hasTemperatureSample" class="numeric">/)
  assert.doesNotMatch(marketsSource, /hasTemperatureSample \? marketTemperature : '--'/)
  assert.match(marketsSource, /class="market-list"/)
  assert.match(marketsSource, /fetchKlines\(symbol, '15m', 24\)/)
  assert.match(marketsSource, /router\.push\(\{ name: 'market-detail'/)

  assert.match(parityCss, /\.markets-view \.markets-hero\s*\{[\s\S]*?min-height:\s*128px/)
  assert.match(parityCss, /\.markets-view \.filter-rail\s*\{[\s\S]*?grid-template-columns:\s*repeat\(5, minmax\(0, 1fr\)\)/)
  assert.match(parityCss, /\.markets-view \.market-index\s*\{[\s\S]*?background:\s*transparent;[\s\S]*?border-bottom:\s*1px solid var\(--line\)/)
  assert.match(parityCss, /@media \(max-width: 340px\)[\s\S]*?\.markets-view \.sparkline\s*\{[\s\S]*?display:\s*none/)
})

test('行情搜索框输入本体保留 44px，并由外层提供单一聚焦光环', () => {
  const inputRule = parityCss.match(/\.app-stage \.mobile-canvas \.markets-view \.search-field input\s*\{([^}]*)\}/)
  assert.ok(inputRule)
  assert.match(inputRule[1]!, /box-sizing:\s*border-box/)
  assert.match(inputRule[1]!, /min-height:\s*44px/)
  assert.match(inputRule[1]!, /min-width:\s*0/)
  assert.match(inputRule[1]!, /border:\s*0/)
  assert.match(inputRule[1]!, /box-shadow:\s*none/)
  assert.match(inputRule[1]!, /outline:\s*0/)

  assert.match(parityCss, /\.markets-view \.search-field:focus-within\s*\{[\s\S]*?border-color:\s*var\(--focus\);[\s\S]*?box-shadow:\s*0 0 0 3px var\(--focus-ring\)/)
  assert.match(parityCss, /\.market-picker-search input\s*\{[\s\S]*?min-height:\s*44px/)
})

test('320–448px、安全区与低动态合同由共享样式和双语资源覆盖', () => {
  assert.match(baseCss, /html,\s*body\s*\{[\s\S]*?overscroll-behavior:\s*none/)
  assert.match(baseCss, /body\s*\{[\s\S]*?overflow-x:\s*hidden/)
  assert.match(parityCss, /@media \(max-width: 360px\)/)
  assert.match(parityCss, /@media \(max-width: 340px\)/)
  assert.match(parityCss, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.markets-view \.markets-hero/)
  assert.match(parityCss, /\.bottom-nav \.trade-nav-action,[\s\S]*?transform:\s*none/)
  assert.equal(zhCN.home.assetEstimateUnavailable, '当前无法完整估算总资产')
  assert.equal(en.home.assetEstimateUnavailable, 'A complete asset estimate is currently unavailable')
})
