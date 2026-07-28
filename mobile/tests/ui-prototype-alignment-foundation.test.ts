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
const rootViewSources = [
  read('../src/views/HomeView.vue'),
  read('../src/views/MarketsView.vue'),
  read('../src/views/TradeView.vue'),
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
  assert.match(prototypeCss, /\.app-stage\.theme-light\s*\{[\s\S]*?--page:\s*#fbfcfa/)
})

test('共享输入把可见焦点提升到容器且清除嵌套输入内框', () => {
  assert.match(baseCss, /:where\([^)]*\.field-shell[^)]*\):focus-within/)
  assert.match(baseCss, /:is\(input,\s*select,\s*textarea\):focus-visible/)
  assert.match(baseCss, /box-shadow:\s*none/)
  assert.match(baseCss, /outline:\s*0/)
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

test('七栏根导航保留独立入口、44px 焦点与抬升秒合约', () => {
  const keys = [...bottomNavSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'spot', 'seconds', 'contract', 'assets', 'profile'])
  assert.match(prototypeCss, /clip-path:\s*polygon\(/)
  assert.match(prototypeCss, /grid-template-columns:\s*repeat\(7,\s*minmax\(44px,\s*1fr\)\)/)
  assert.match(prototypeCss, /\.bottom-nav button\s*\{[\s\S]*?min-width:\s*44px/)
  assert.match(prototypeCss, /\.bottom-nav \.seconds-nav-action\s*\{[\s\S]*?translateY\(-16px\)/)
  assert.match(prototypeCss, /\.bottom-nav \.seconds-nav-action span\s*\{[\s\S]*?height:\s*48px/)
  assert.doesNotMatch(bottomNavSource, /<style scoped/)
})

test('登录与 PWA 状态保留原有真实状态带', () => {
  assert.match(loginStateSource, /grid-template-columns:\s*44px minmax\(0,\s*1fr\) auto/)
  assert.match(loginStateSource, /border-left:\s*3px solid var\(--positive\)/)
  assert.match(pwaStatusSource, /max-width:\s*var\(--app-max-width,\s*448px\)/)
  assert.match(pwaStatusSource, /border-radius:\s*0/)
  assert.match(pwaStatusSource, /box-shadow:\s*none/)
})

test('根页面使用共享窄屏合同，不引入 scoped 几何、手绘图标或表情', () => {
  assert.match(parityCss, /@import '\.\/prototype-base\.css';/)
  assert.match(prototypeCss, /@media \(max-width: 350px\)/)
  assert.match(prototypeCss, /@media \(max-width: 820px\)/)
  for (const source of rootViewSources) {
    assert.doesNotMatch(source, /<style scoped/)
    assert.doesNotMatch(source, /font-size:\s*(?:clamp|min|max|calc)\(/)
    assert.doesNotMatch(source, /letter-spacing:\s*-\d/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
  }
  for (const source of rootViewSources.slice(2)) assert.doesNotMatch(source, /<svg/)
})
