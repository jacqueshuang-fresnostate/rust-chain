import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  THEME_META_COLORS,
  normalizeAppTheme,
  resolveAppTheme,
} from '../src/stores/theme.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const baseCss = readFileSync(new URL('../src/styles/base.css', import.meta.url), 'utf8')
const parityCss = readFileSync(new URL('../src/styles/prototype-parity.css', import.meta.url), 'utf8')
const indexHtml = readFileSync(new URL('../index.html', import.meta.url), 'utf8')
const viteConfigSource = readFileSync(new URL('../vite.config.ts', import.meta.url), 'utf8')
const rootHeaderSource = readFileSync(new URL('../src/components/RootHeader.vue', import.meta.url), 'utf8')
const assetsViewSource = readFileSync(new URL('../src/views/AssetsView.vue', import.meta.url), 'utf8')
const profileViewSource = readFileSync(new URL('../src/views/ProfileView.vue', import.meta.url), 'utf8')
const orderBookSource = readFileSync(new URL('../src/components/OrderBookPanel.vue', import.meta.url), 'utf8')
const selectedPageCss = readFileSync(new URL('../src/styles/pencil-selected-pages.css', import.meta.url), 'utf8')
const themeStoreSource = readFileSync(new URL('../src/stores/theme.ts', import.meta.url), 'utf8')

function tokenHex(source: string, name: string): string {
  const match = source.match(new RegExp(`${name}:\\s*(#[0-9a-f]{6})`, 'i'))
  assert.ok(match, `missing hexadecimal token: ${name}`)
  return match[1]
}

function contrastRatio(left: string, right: string): number {
  const luminance = (color: string): number => {
    const channels = color.slice(1).match(/../g)?.map((value) => Number.parseInt(value, 16) / 255) || []
    const linear = channels.map((value) => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4)
    return .2126 * linear[0] + .7152 * linear[1] + .0722 * linear[2]
  }
  const values = [luminance(left), luminance(right)].sort((a, b) => b - a)
  return (values[0] + .05) / (values[1] + .05)
}

test('主题偏好只接受受支持值并以系统偏好兜底', () => {
  assert.equal(normalizeAppTheme('light'), 'light')
  assert.equal(normalizeAppTheme('dark'), 'dark')
  assert.equal(normalizeAppTheme('system'), null)
  assert.equal(resolveAppTheme(null, false), 'light')
  assert.equal(resolveAppTheme(undefined, true), 'dark')
  assert.equal(resolveAppTheme('light', true), 'light')
})

test('明暗主题提供高对比共享令牌与稳定主题色', () => {
  assert.equal(THEME_META_COLORS.light, '#f7faf8')
  assert.equal(THEME_META_COLORS.dark, '#070908')
  assert.match(indexHtml, /name="theme-color" content="#f7faf8" media="\(prefers-color-scheme: light\)"/)
  assert.match(indexHtml, /name="theme-color" content="#070908" media="\(prefers-color-scheme: dark\)"/)
  assert.match(viteConfigSource, /theme_color: '#f7faf8'/)
  assert.match(viteConfigSource, /background_color: '#f7faf8'/)
  assert.match(baseCss, /:root\[data-theme='dark'\]/)
  for (const token of [
    '--background',
    '--surface',
    '--surface-elevated',
    '--ink',
    '--muted',
    '--line',
    '--positive',
    '--negative',
    '--accent',
    '--focus',
    '--signal-green',
  ]) {
    assert.match(baseCss, new RegExp(`${token}:`))
  }
  assert.doesNotMatch(baseCss, /#0b1811|rgba\(\s*11\s*,\s*24\s*,\s*17/i)
})

test('两套主题的正文、辅助文字和语义色达到正文对比阈值', () => {
  const darkCss = baseCss.slice(baseCss.indexOf(":root[data-theme='dark']"))
  for (const source of [baseCss, darkCss]) {
    const surface = tokenHex(source, '--surface')
    assert.ok(contrastRatio(tokenHex(source, '--ink'), surface) >= 7)
    for (const token of ['--muted', '--positive', '--negative', '--accent', '--focus']) {
      assert.ok(contrastRatio(tokenHex(source, token), surface) >= 4.5, `${token} contrast is too low`)
    }
    const signalGreen = tokenHex(source, '--signal-green')
    assert.ok(contrastRatio(tokenHex(source, '--on-positive'), signalGreen) >= 4.5)
  }
  assert.notEqual(tokenHex(baseCss, '--positive'), tokenHex(baseCss, '--signal-green'))
})

test('生产原型覆盖层的中性色板在明暗主题下保持可访问对比', () => {
  const darkRule = parityCss.match(/\.app-stage\s*\{([^}]*)\}/)
  const lightRule = parityCss.match(/\.app-stage\.theme-light\s*\{([^}]*)\}/)
  assert.ok(darkRule)
  assert.ok(lightRule)

  for (const source of [darkRule[1], lightRule[1]]) {
    const surface = tokenHex(source, '--surface')
    assert.ok(contrastRatio(tokenHex(source, '--text'), surface) >= 7)
    for (const token of ['--muted', '--green', '--coral', '--cyan', '--yellow']) {
      assert.ok(contrastRatio(tokenHex(source, token), surface) >= 4.5, `${token} contrast is too low`)
    }
  }
})

test('深色主题的旧版分隔线使用石墨令牌而不是白色边框', () => {
  assert.match(
    parityCss,
    /\.app-stage\.theme-dark\s*\{[\s\S]*?--line:\s*rgb\(184 204 194 \/ 9%\);[\s\S]*?--line-strong:\s*rgb\(184 204 194 \/ 14%\);[\s\S]*?--header-border:\s*rgb\(184 204 194 \/ 12%\);/,
  )
  assert.match(
    parityCss,
    /\.app-stage\.theme-dark \.mobile-canvas :is\([\s\S]*?\.portfolio-overview\s*\)\s*\{[\s\S]*?border-color:\s*var\(--line\);/,
  )
  assert.match(
    parityCss,
    /\.app-stage\.theme-dark \.mobile-canvas \.portfolio-periods button\.active\s*\{[\s\S]*?border-color:\s*var\(--line-strong\);/,
  )
  assert.match(
    parityCss,
    /\.app-stage\.theme-dark \.mobile-canvas \.hero-grid\s*\{[\s\S]*?linear-gradient\(var\(--grid-line\) 1px, transparent 1px\)/,
  )
  assert.match(
    parityCss,
    /\.app-stage\.theme-dark \.mobile-canvas > \.topbar\.topbar,[\s\S]*?0 1px 0 rgb\(184 204 194 \/ 7%\)/,
  )
  assert.match(
    parityCss,
    /@media \(max-width: 820px\)\s*\{\s*\.app-stage\.theme-dark \.mobile-canvas\s*\{\s*box-shadow:\s*none;/,
  )
})

test('深色主题关键阴影兼容旧版 Android WebView', () => {
  for (const source of [parityCss, assetsViewSource, profileViewSource, orderBookSource, selectedPageCss]) {
    assert.doesNotMatch(source, /box-shadow:[^;]*color-mix\([^;]*;/s)
  }
  assert.match(
    selectedPageCss,
    /\.pencil-hero\s*\{[\s\S]*?box-shadow:\s*none;/,
  )
  assert.match(selectedPageCss, /\.pencil-field__shell:focus-within[\s\S]*?box-shadow:\s*0 0 0 2px var\(--focus-ring\);/)
  assert.match(
    profileViewSource,
    /\.profile-register-action\s*\{[\s\S]*?background:\s*var\(--ink\);[\s\S]*?color:\s*var\(--surface\);/,
  )
  assert.match(
    orderBookSource,
    /\.order-book--split header\s*\{[\s\S]*?box-shadow:\s*inset 0 1px 0 var\(--line\);/,
  )
})

test('壳层层级确保转场低于导航和头部，浮层与启动层仍在最上方', () => {
  const readLayer = (name: string): number => {
    const match = baseCss.match(new RegExp(`--layer-${name}:\\s*(\\d+)`))
    assert.ok(match, `missing layer token: ${name}`)
    return Number(match[1])
  }

  const layers = [
    readLayer('content'),
    readLayer('route-transition'),
    readLayer('navigation'),
    readLayer('sticky-header'),
    readLayer('overlay'),
    readLayer('launch'),
  ]
  assert.deepEqual([...layers].sort((left, right) => left - right), layers)
  assert.equal(new Set(layers).size, layers.length)
  assert.match(parityCss, /\.route-forward-enter-active,[\s\S]*?z-index: var\(--layer-route-transition\)/)
  assert.match(parityCss, /\.bottom-nav\s*\{[\s\S]*?pointer-events: auto;[\s\S]*?z-index: var\(--layer-navigation\)/)
  assert.match(parityCss, /> \.root-header\.root-header\s*\{[\s\S]*?z-index: var\(--layer-sticky-header\)/)
})

test('共享根头部主题按钮复用持久化主题 store 并提供双语可访问标签', () => {
  assert.match(themeStoreSource, /localStorage\?\.setItem\(THEME_STORAGE_KEY, nextTheme\)/)
  assert.match(rootHeaderSource, /const theme = useThemeStore\(\)/)
  assert.match(rootHeaderSource, /@click="theme\.toggleTheme"/)
  assert.match(rootHeaderSource, /<Sun v-if="theme\.isDark"[\s\S]*?<Moon v-else/)
  assert.match(rootHeaderSource, /:aria-pressed="theme\.isDark"/)

  for (const messages of [zhCN, en]) {
    assert.ok(messages.home.switchToLightTheme)
    assert.ok(messages.home.switchToDarkTheme)
    assert.ok(messages.home.openMessageCenter)
  }
})
