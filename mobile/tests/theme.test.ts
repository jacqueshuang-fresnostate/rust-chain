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
const rootHeaderSource = readFileSync(new URL('../src/components/RootHeader.vue', import.meta.url), 'utf8')
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
  assert.equal(THEME_META_COLORS.light, '#f1f4f8')
  assert.equal(THEME_META_COLORS.dark, '#07090c')
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

test('壳层层级遵循内容、导航、转场、粘性头部、浮层顺序', () => {
  const readLayer = (name: string): number => {
    const match = baseCss.match(new RegExp(`--layer-${name}:\\s*(\\d+)`))
    assert.ok(match, `missing layer token: ${name}`)
    return Number(match[1])
  }

  const layers = [
    readLayer('content'),
    readLayer('navigation'),
    readLayer('route-transition'),
    readLayer('sticky-header'),
    readLayer('overlay'),
  ]
  assert.deepEqual([...layers].sort((left, right) => left - right), layers)
  assert.equal(new Set(layers).size, layers.length)
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
