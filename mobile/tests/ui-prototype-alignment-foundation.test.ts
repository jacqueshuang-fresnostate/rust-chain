import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const baseCss = read('../src/styles/base.css')
const prototypeCss = read('../src/styles/prototype-base.css')
const parityCss = read('../src/styles/prototype-parity.css')
const appSource = read('../src/App.vue')
const bottomNavSource = read('../src/components/AppBottomNav.vue')
const pageHeaderSource = read('../src/components/PageHeader.vue')
const loginStateSource = read('../src/components/LoginRequiredState.vue')
const pwaStatusSource = read('../src/components/PwaStatus.vue')
const tradeSource = read('../src/views/TradeView.vue')
const rootViewSources = [
  read('../src/views/HomeView.vue'),
  read('../src/views/MarketsView.vue'),
  tradeSource,
  read('../src/views/AssetsView.vue'),
  read('../src/views/ProfileView.vue'),
]

test('共享视觉令牌固定为 448px 冷中性双主题与低圆角', () => {
  assert.match(baseCss, /--app-max-width:\s*448px/)
  assert.match(baseCss, /--radius:\s*[0-8]px/)
  assert.match(baseCss, /--signal-green:/)
  assert.match(baseCss, /--signal-coral:/)
  assert.match(baseCss, /--data-font:/)
  assert.match(baseCss, /:root\[data-theme='dark'\]/)
  assert.match(parityCss, /\.app-stage\.theme-light\s*\{[\s\S]*?--page:\s*#f8faf8/)
})

test('共享输入把可见焦点提升到单一完整容器光环且清除嵌套输入内框', () => {
  assert.match(baseCss, /:where\([^)]*\.field-shell[^)]*\):focus-within/)
  assert.match(baseCss, /:is\(input,\s*select,\s*textarea\):focus-visible/)
  assert.match(baseCss, /box-shadow:\s*none/)
  assert.match(baseCss, /outline:\s*0/)

  const sharedFocusRule = parityCss.match(
    /\.app-stage \.mobile-canvas :is\(\s*\.input-stack label:focus-within,[\s\S]*?\.security-field:focus-within\s*\)\s*\{([^}]*)\}/,
  )
  assert.ok(sharedFocusRule)
  assert.match(sharedFocusRule[1]!, /background:\s*color-mix\(in srgb,\s*var\(--focus\)\s*5%,\s*var\(--surface\)\)/)
  assert.match(sharedFocusRule[1]!, /border-color:\s*var\(--focus\)/)
  assert.match(sharedFocusRule[1]!, /box-shadow:\s*0 0 0 3px var\(--focus-ring\);/)
  assert.doesNotMatch(sharedFocusRule[1]!, /\binset\b/)

  assert.match(tradeSource, /\.input-stack \.field-shell input\s*\{[\s\S]*?border:\s*0;[\s\S]*?outline:\s*0;/)
  assert.match(
    tradeSource,
    /\.input-stack \.field-shell input:focus,[\s\S]*?\.input-stack \.field-shell input:focus-visible\s*\{[\s\S]*?border:\s*0;[\s\S]*?outline:\s*0;/,
  )
})

test('二级页分组标题不再继承旧页面的背景型 soft 令牌', () => {
  assert.match(
    parityCss,
    /\.app-stage \.mobile-canvas \.group-title\s*\{[\s\S]*?color:\s*var\(--text\)/,
  )
})

test('路由转场被内容栈隔离，根头部与异形导航保持独立层级', () => {
  assert.match(appSource, /class="app-route-host"/)
  assert.doesNotMatch(appSource, /class="app-route-host view-stack"/)
  assert.match(appSource, /'app-route-layer',[\s\S]*?'view-stack',[\s\S]*?\.\.\.routeMotionClasses/)
  assert.match(parityCss, /route-forward-leave-active[\s\S]*?z-index:\s*var\(--layer-content\)/)
  assert.match(pageHeaderSource, /eyebrow\?:\s*string/)
  assert.match(pageHeaderSource, /subtitle\?:\s*string/)
  assert.match(pageHeaderSource, /compact\?:\s*boolean/)
  assert.match(prototypeCss, /\.topbar,[\s\S]*?\.secondary-header\s*\{[\s\S]*?z-index:\s*70/)
  assert.match(prototypeCss, /\.bottom-nav\s*\{[\s\S]*?z-index:\s*40/)
})

test('五栏根导航保留 44px 焦点与抬升交易入口', () => {
  const keys = [...bottomNavSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'trade', 'assets', 'profile'])
  assert.match(parityCss, /\.bottom-nav__dock\s*\{[\s\S]*?grid-template-columns:\s*repeat\(5,\s*minmax\(0,\s*1fr\)\)/)
  assert.match(parityCss, /\.bottom-nav__item\s*\{[\s\S]*?min-width:\s*44px/)
  assert.match(parityCss, /\.bottom-nav \.trade-nav-action \.bottom-nav__icon\s*\{[\s\S]*?height:\s*56px;[\s\S]*?top:\s*-18px/)
  assert.doesNotMatch(bottomNavSource, /<style scoped/)
})

test('登录状态带保持真实，PWA 状态升级为非阻塞沉浸式系统浮岛', () => {
  assert.match(loginStateSource, /grid-template-columns:\s*44px minmax\(0,\s*1fr\) auto/)
  assert.match(loginStateSource, /border-left:\s*3px solid var\(--positive\)/)
  assert.match(pwaStatusSource, /max-width:\s*var\(--app-max-width,\s*448px\)/)
  assert.match(pwaStatusSource, /<Transition name="pwa-status-reveal">/)
  assert.match(pwaStatusSource, /class="pwa-status__panel"/)
  assert.match(pwaStatusSource, /backdrop-filter:\s*blur\(22px\) saturate\(145%\)/)
  assert.match(pwaStatusSource, /border-radius:\s*var\(--pwa-card-radius\)/)
  assert.match(pwaStatusSource, /0 18px 48px color-mix/)
  assert.doesNotMatch(pwaStatusSource, /\.pwa-status__card\s*\{[^}]*border-radius:\s*0/)
})

test('根页面使用共享窄屏合同，Pencil 资产与我的允许局部几何且不引入表情', () => {
  assert.match(parityCss, /@import '\.\/prototype-base\.css';/)
  assert.match(prototypeCss, /@media \(max-width: 350px\)/)
  assert.match(prototypeCss, /@media \(max-width: 820px\)/)
  for (const source of rootViewSources.slice(0, 3)) {
    assert.doesNotMatch(source, /<style scoped/)
    assert.doesNotMatch(source, /font-size:\s*(?:clamp|min|max|calc)\(/)
    assert.doesNotMatch(source, /letter-spacing:\s*-\d/)
  }
  for (const source of rootViewSources) assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
  for (const source of rootViewSources.slice(3)) {
    assert.match(source, /<style scoped>/)
    assert.match(source, /data-pencil-source=/)
  }
  for (const source of rootViewSources.slice(2)) assert.doesNotMatch(source, /<svg/)
})
