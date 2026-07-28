import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')

const baseCss = read('../src/styles/base.css')
const appSource = read('../src/App.vue')
const bottomNavSource = read('../src/components/AppBottomNav.vue')
const pageHeaderSource = read('../src/components/PageHeader.vue')
const loginStateSource = read('../src/components/LoginRequiredState.vue')
const pwaStatusSource = read('../src/components/PwaStatus.vue')
const homeSource = read('../src/views/HomeView.vue')
const rootViewSources = [
  homeSource,
  read('../src/views/MarketsView.vue'),
  read('../src/views/AssetsView.vue'),
  read('../src/views/ProfileView.vue'),
  read('../src/views/ProductHubView.vue'),
]

const themeTokenBlock = (theme: 'light' | 'dark'): string => {
  const selector = theme === 'light'
    ? /:root,\s*:root\[data-theme='light'\]\s*\{([\s\S]*?)\n\}/
    : /:root\[data-theme='dark'\]\s*\{([\s\S]*?)\n\}/

  return baseCss.match(selector)?.[1] || ''
}

const readHexToken = (block: string, token: string): string => {
  return block.match(new RegExp(`${token}:\\s*(#[\\da-f]{6})`, 'i'))?.[1] || ''
}

const relativeLuminance = (hex: string): number => {
  const channels = hex.slice(1).match(/.{2}/g)?.map((channel) => {
    const normalized = Number.parseInt(channel, 16) / 255
    return normalized <= 0.04045
      ? normalized / 12.92
      : ((normalized + 0.055) / 1.055) ** 2.4
  }) || []

  return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
}

test('共享视觉令牌固定为 448px 冷中性双主题与低圆角', () => {
  assert.match(baseCss, /--app-max-width:\s*448px/)
  assert.match(baseCss, /--radius:\s*[0-8]px/)
  assert.match(baseCss, /--signal-green:/)
  assert.match(baseCss, /--signal-coral:/)
  assert.match(baseCss, /--data-font:/)
  assert.match(baseCss, /:root\[data-theme='dark'\]/)
  assert.doesNotMatch(baseCss, /#0b1811|rgba\(\s*11\s*,\s*24\s*,\s*17/i)
})

test('首页资产 Hero 使用明亮浅色面并保留独立深色覆盖', () => {
  for (const theme of ['light', 'dark'] as const) {
    const onDarkSurface = readHexToken(themeTokenBlock(theme), '--on-dark-surface')
    assert.match(onDarkSurface, /^#[\da-f]{6}$/i)
    assert.ok(relativeLuminance(onDarkSurface) >= 0.8)
  }

  const assetGlanceRules = [...homeSource.matchAll(/\.asset-glance\s*\{([\s\S]*?)\}/g)]
    .map((match) => match[1] || '')
    .find((rules) => rules.includes('color: var(--ink)')) || ''
  assert.match(assetGlanceRules, /var\(--surface\)/)
  assert.match(assetGlanceRules, /color:\s*var\(--ink\)/)
  assert.doesNotMatch(assetGlanceRules, /background:\s*var\(--dark-surface\)/)
  assert.match(homeSource, /:global\(:root\[data-theme='dark'\]\) \.asset-glance[\s\S]*?var\(--dark-surface\)/)
  assert.match(homeSource, /color-mix\(in srgb,\s*var\(--on-dark-surface\)/)
  assert.doesNotMatch(homeSource, /--home-contrast-ink/)
})

test('共享输入把可见焦点提升到容器且清除嵌套输入内框', () => {
  assert.match(baseCss, /:where\([^)]*\.field-shell[^)]*\):focus-within/)
  assert.match(baseCss, /:is\(input,\s*select,\s*textarea\):focus-visible/)
  assert.match(baseCss, /box-shadow:\s*none/)
  assert.match(baseCss, /outline:\s*0/)
})

test('路由转场被内容栈隔离，粘性头部和异形导航保持独立层级', () => {
  assert.match(appSource, /class="app-route-host"/)
  const routeHostRules = appSource.match(/\.app-route-host\s*\{([\s\S]*?)\}/)?.[1] || ''
  assert.doesNotMatch(routeHostRules, /\bisolation\s*:/)
  assert.doesNotMatch(routeHostRules, /\bz-index\s*:/)
  assert.match(appSource, /route-forward-leave-active[\s\S]*?z-index:\s*var\(--layer-content\)/)
  assert.match(pageHeaderSource, /eyebrow\?:\s*string/)
  assert.match(pageHeaderSource, /subtitle\?:\s*string/)
  assert.match(pageHeaderSource, /compact\?:\s*boolean/)
  assert.match(pageHeaderSource, /position:\s*sticky/)
  assert.match(pageHeaderSource, /z-index:\s*var\(--layer-sticky-header\)/)
})

test('七栏根导航保留独立入口、44px 图标焦点与抬升秒合约', () => {
  const keys = [...bottomNavSource.matchAll(/\bkey:\s*'([^']+)'/g)].map((match) => match[1])
  assert.deepEqual(keys, ['home', 'markets', 'spot', 'seconds', 'contract', 'assets', 'profile'])
  assert.match(bottomNavSource, /clip-path:\s*polygon\(/)
  assert.match(bottomNavSource, /grid-template-columns:\s*repeat\(7,\s*minmax\(44px,\s*1fr\)\)/)
  assert.match(bottomNavSource, /min-height:\s*66px/)
  assert.match(bottomNavSource, /flex:\s*0 0 44px/)
  assert.match(bottomNavSource, /\.is-primary[\s\S]*?margin-top:\s*-24px/)
  assert.match(bottomNavSource, /:focus-visible \.bottom-nav__icon/)
  assert.doesNotMatch(bottomNavSource, /border-radius:\s*(?:9|[1-9]\d+)px/)
})

test('登录与 PWA 状态使用硬边界全宽状态带', () => {
  assert.match(loginStateSource, /grid-template-columns:\s*44px minmax\(0,\s*1fr\) auto/)
  assert.match(loginStateSource, /border-left:\s*3px solid var\(--positive\)/)
  assert.match(pwaStatusSource, /max-width:\s*var\(--app-max-width,\s*448px\)/)
  assert.match(pwaStatusSource, /border-radius:\s*0/)
  assert.match(pwaStatusSource, /box-shadow:\s*none/)
})

test('一级页面保持窄屏合同且不引入视口字号、负字距、手绘图标或表情', () => {
  for (const source of rootViewSources) {
    assert.match(source, /@media \(max-width: 340px\)/)
    assert.doesNotMatch(source, /font-size:\s*(?:clamp|min|max|calc)\(/)
    assert.doesNotMatch(source, /letter-spacing:\s*-\d/)
    assert.doesNotMatch(source, /<svg/)
    assert.doesNotMatch(source, /\p{Extended_Pictographic}/u)
  }
  for (const source of rootViewSources.slice(2)) {
    assert.match(source, /page--prototype-grid/)
  }
})
