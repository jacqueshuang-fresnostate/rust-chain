import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const viewRoot = new URL('../src/views/', import.meta.url)

const sources = {
  app: read('../src/App.vue'),
  baseCss: read('../src/styles/prototype-base.css'),
  css: read('../src/styles/prototype-parity.css'),
  pageHeader: read('../src/components/PageHeader.vue'),
  rootHeader: read('../src/components/RootHeader.vue'),
  login: read('../src/views/LoginView.vue'),
  register: read('../src/views/RegisterView.vue'),
  marketDetail: read('../src/views/MarketDetailView.vue'),
}

const headerBranches = [
  '.topbar-actions > .icon-button.icon-button',
  '.secondary-header > .icon-button.icon-button',
  '.secondary-header-action > .icon-button.icon-button',
  '.auth-topbar > .icon-button.icon-button',
  '.market-detail__header > .icon-button.icon-button',
] as const

const headerSelector = `.app-stage .mobile-canvas :is(${headerBranches.join(',')})`

const viewSources = readdirSync(viewRoot, { withFileTypes: true })
  .filter((entry) => entry.isFile() && entry.name.endsWith('.vue'))
  .map((entry) => ({
    name: entry.name,
    source: readFileSync(new URL(entry.name, viewRoot), 'utf8'),
  }))

function normalizeSelector(selector: string): string {
  return selector
    .replace(/\s+/g, ' ')
    .replace(/\s*([(),>+~])\s*/g, '$1')
    .trim()
}

function rulesFor(source: string, selector: string): string[] {
  const normalizedSelector = normalizeSelector(selector)
  const sourceWithoutComments = source.replace(/\/\*[\s\S]*?\*\//g, '')
  const rules: string[] = []
  const rulePattern = /([^{}]+)\{([^{}]*)\}/g

  for (const match of sourceWithoutComments.matchAll(rulePattern)) {
    if (normalizeSelector(match[1]) === normalizedSelector) rules.push(match[2])
  }

  assert.ok(rules.length > 0, `missing exact CSS selector: ${selector}`)
  return rules
}

function ruleFor(source: string, selector: string): string {
  const rules = rulesFor(source, selector)
  assert.equal(rules.length, 1, `expected one CSS rule for: ${selector}`)
  return rules[0]
}

function declarationValue(rule: string, property: string): string {
  const escapedProperty = property.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = rule.match(new RegExp(`${escapedProperty}:\\s*([^;]+);`))
  assert.ok(match, `missing declaration: ${property}`)
  return match[1].replace(/\s+/g, ' ').trim()
}

test('共享头部控件只覆盖五类头部轨道并保留 44px 圆形精密仪器结构', () => {
  const rules = rulesFor(sources.css, headerSelector)
  const rule = rules.find((candidate) => /width:\s*44px;/.test(candidate))
  assert.ok(rule, 'missing normal-state header-control rule')
  assert.equal(rules.filter((candidate) => /width:\s*44px;/.test(candidate)).length, 1)

  assert.match(rule, /display:\s*grid;/)
  assert.match(rule, /align-items:\s*center;/)
  assert.match(rule, /justify-items:\s*center;/)
  assert.match(rule, /width:\s*44px;/)
  assert.match(rule, /height:\s*44px;/)
  assert.match(rule, /min-width:\s*44px;/)
  assert.match(rule, /min-height:\s*44px;/)
  assert.match(rule, /padding:\s*0;/)
  assert.match(rule, /border-radius:\s*50%;/)
  assert.match(rule, /linear-gradient\(/)
  assert.match(rule, /padding-box/)
  assert.match(rule, /border-box/)
  assert.match(rule, /inset 0 1px 0 var\(--header-control-inner-highlight\)/)
  assert.match(rule, /inset 0 -2px 0 var\(--header-control-inner-depth\)/)
  assert.match(rule, /var\(--header-control-elevation\)/)
})

test('明暗主题分别定义冷中性面板与金属边框且不恢复退役绿色边框族', () => {
  const darkTokens = ruleFor(sources.css, '.app-stage')
  const lightTokens = ruleFor(sources.css, '.app-stage.theme-light')

  for (const token of [
    '--header-control-face-top',
    '--header-control-face-mid',
    '--header-control-face-bottom',
    '--header-control-bezel-highlight',
    '--header-control-bezel-mid',
    '--header-control-bezel-shadow',
    '--header-control-inner-highlight',
    '--header-control-inner-depth',
    '--header-control-elevation',
    '--header-control-pressed-shadow',
  ]) {
    const darkValue = declarationValue(darkTokens, token)
    const lightValue = declarationValue(lightTokens, token)
    assert.notEqual(darkValue, lightValue, `${token} must differ between themes`)
  }

  assert.match(sources.app, /class="app-stage"/)
  assert.match(sources.app, /:class="theme\.isDark \? 'theme-dark' : 'theme-light'"/)
  assert.doesNotMatch(
    `${sources.baseCss}\n${sources.css}`,
    /#0b1811|rgba\(11,\s*24,\s*17\s*,/i,
  )
})

test('按压、键盘焦点、禁用与减弱动效状态有独立且无布局漂移的合同', () => {
  const activeRules = rulesFor(sources.css, `${headerSelector}:not(:disabled):active`)
  const activeRule = activeRules.find((candidate) => /translateY\(1px\)/.test(candidate))
  const reducedMotionActive = activeRules.find((candidate) => /transform:\s*none;/.test(candidate))
  const focusRule = ruleFor(sources.css, `${headerSelector}:focus-visible`)
  const disabledRule = ruleFor(sources.css, `${headerSelector}:disabled`)
  const reducedMotionRule = rulesFor(sources.css, headerSelector)
    .find((candidate) => /transition:\s*none;/.test(candidate))

  assert.ok(activeRule, 'missing tactile active-state rule')
  assert.ok(reducedMotionActive, 'missing reduced-motion active-state override')
  assert.ok(reducedMotionRule, 'missing reduced-motion transition override')
  assert.match(activeRule, /transform:\s*translateY\(1px\);/)
  assert.match(activeRule, /var\(--header-control-pressed-shadow\)/)
  assert.doesNotMatch(activeRule, /width:|height:|margin:/)
  assert.match(focusRule, /outline:\s*2px solid var\(--focus\);/)
  assert.match(focusRule, /outline-offset:\s*3px;/)
  assert.match(disabledRule, /opacity:\s*0\.48;/)
  assert.match(disabledRule, /cursor:\s*not-allowed;/)
  assert.match(disabledRule, /animation:\s*none;/)
  assert.match(disabledRule, /transition:\s*none;/)
  assert.match(disabledRule, /transform:\s*none;/)
  assert.match(reducedMotionRule, /transition:\s*none;/)
  assert.match(reducedMotionActive, /transform:\s*none;/)
})

test('Lucide SVG 显式双轴居中，消息珊瑚点附着在金属边框内', () => {
  const svgRule = ruleFor(sources.css, `${headerSelector} > svg`)
  const dotRule = ruleFor(
    sources.css,
    '.app-stage .mobile-canvas .topbar-actions .has-dot.has-dot::after',
  )

  assert.match(svgRule, /display:\s*block;/)
  assert.match(svgRule, /grid-area:\s*1 \/ 1;/)
  assert.match(svgRule, /margin:\s*auto;/)
  assert.match(svgRule, /place-self:\s*center;/)
  assert.match(svgRule, /pointer-events:\s*none;/)
  assert.match(dotRule, /background:\s*var\(--coral\);/)
  assert.match(dotRule, /right:\s*1px;/)
  assert.match(dotRule, /top:\s*2px;/)
  assert.match(dotRule, /width:\s*8px;/)
  assert.match(dotRule, /height:\s*8px;/)
})

test('PageHeader action 包装层完全透明且仅实际按钮呈现 44px 控件', () => {
  const wrapperRule = ruleFor(
    sources.css,
    '.app-stage .mobile-canvas .secondary-header-action.secondary-header-action',
  )

  assert.match(wrapperRule, /background:\s*transparent;/)
  assert.match(wrapperRule, /border:\s*0;/)
  assert.match(wrapperRule, /box-shadow:\s*none;/)
  assert.match(wrapperRule, /display:\s*grid;/)
  assert.match(wrapperRule, /place-items:\s*center;/)
  assert.match(wrapperRule, /width:\s*44px;/)
  assert.match(wrapperRule, /height:\s*44px;/)
  assert.match(sources.pageHeader, /class="secondary-header-action page-header__actions"/)
  assert.match(sources.pageHeader, /<slot name="actions" \/>/)

  const actionConsumers = viewSources.flatMap(({ name, source }) =>
    [...source.matchAll(/<template #actions>([\s\S]*?)<\/template>/g)]
      .map((match) => ({ name, action: match[1].trim() })),
  )
  assert.ok(actionConsumers.length > 0, 'expected PageHeader action consumers')
  for (const { name, action } of actionConsumers) {
    assert.match(
      action,
      /^<button\b[^>]*\bclass="[^"]*\bicon-button\b[^"]*"/,
      `${name} must project a direct icon-button into PageHeader actions`,
    )
  }
})

test('所有目标 Header 保留 Lucide 图标、导航与动作处理器', () => {
  assert.match(sources.rootHeader, /import \{ Bell, Moon, Sun \} from 'lucide-vue-next'/)
  assert.match(sources.rootHeader, /@click="theme\.toggleTheme"/)
  assert.match(sources.rootHeader, /router\.push\(\{ name: 'message-center' \}\)/)

  assert.match(sources.pageHeader, /import \{ ArrowLeft \} from 'lucide-vue-next'/)
  assert.match(
    sources.pageHeader,
    /goBackOr\(router, props\.fallback \|\| route\.meta\.backFallback \|\| '\/'\)/,
  )

  for (const source of [sources.login, sources.register]) {
    assert.match(source, /class="auth-topbar"/)
    assert.match(source, /class="icon-button"/)
    assert.match(source, /<Languages :size="21" \/>/)
    assert.match(source, /@click="router\.push\(\{ name: 'language' \}\)"/)
  }
  assert.match(sources.login, /@click="handleBack"/)
  assert.match(
    sources.login,
    /function handleBack\(\): void \{[\s\S]*?if \(step\.value === 2\)[\s\S]*?goBackOr\(router, '\/'\)/,
  )
  assert.match(sources.register, /@click="handleBack"/)
  assert.match(
    sources.register,
    /function handleBack\(\): void \{[\s\S]*?if \(step\.value === 2\)[\s\S]*?goBackOr\(router, '\/login'\)/,
  )

  assert.match(sources.marketDetail, /class="market-detail__header"/)
  assert.match(sources.marketDetail, /@click="goBack"/)
  assert.match(sources.marketDetail, /@click="shareMarket"/)
  assert.match(sources.marketDetail, /<Share2 :size="20" \/>/)
  assert.match(
    sources.marketDetail,
    /function goBack\(\): void \{[\s\S]*?goBackOr\(router, \{ name: 'markets' \}\)/,
  )
  assert.match(sources.marketDetail, /await navigator\.share\(\{ title: pairSymbol\.value, url \}\)/)
  assert.match(sources.marketDetail, /navigator\.clipboard\?\.writeText\(url\)/)
})
