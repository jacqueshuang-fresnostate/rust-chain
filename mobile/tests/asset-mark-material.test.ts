import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'

const assetMarkSource = readFileSync(new URL('../src/components/AssetMark.vue', import.meta.url), 'utf8')
const tradeSource = readFileSync(new URL('../src/views/TradeView.vue', import.meta.url), 'utf8')
const assetMarkCss = scopedStyle(assetMarkSource)
const tradeCss = scopedStyle(tradeSource)

function cssRule(source: string, selector: string): string {
  const start = source.indexOf(`${selector} {`)
  assert.notEqual(start, -1, `missing CSS selector: ${selector}`)
  const openingBrace = source.indexOf('{', start)
  let depth = 0

  for (let index = openingBrace; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1
    if (source[index] === '}') depth -= 1
    if (depth === 0) return source.slice(start, index + 1)
  }

  assert.fail(`unterminated CSS selector: ${selector}`)
}

test('AssetMark 将后台圆形图片态与确定性字母回退态分离', () => {
  assert.match(assetMarkSource, /imageSource \? 'asset-mark--image' : 'asset-mark--fallback'/)
  assert.match(assetMarkSource, /buildAssetMarkImageSources\(props\.src, props\.fallbackSrc\)/)
  assert.match(assetMarkSource, /@error="imageIndex \+= 1"/)
  assert.match(assetMarkSource, /<b v-else aria-hidden="true">\{\{ initial \}\}<\/b>/)

  const imageRule = cssRule(assetMarkCss, '.asset-mark--image')
  assert.match(imageRule, /background: transparent/)
  assert.match(imageRule, /box-shadow: none/)
  assert.match(imageRule, /padding: 0/)
  assert.doesNotMatch(imageRule, /var\(--asset-color\)/)
  assert.match(cssRule(assetMarkCss, '.asset-mark'), /border-radius: 50%/)
  assert.match(cssRule(assetMarkCss, '.asset-mark'), /overflow: hidden/)
  assert.match(cssRule(assetMarkCss, '.asset-mark img'), /border-radius: inherit/)
  assert.match(cssRule(assetMarkCss, '.asset-mark img'), /object-fit: cover/)

  const fallbackRule = cssRule(assetMarkCss, '.asset-mark--fallback')
  assert.match(fallbackRule, /background: color-mix/)
  assert.match(fallbackRule, /var\(--asset-color\)/)
  assert.match(fallbackRule, /box-shadow: none/)
  assert.doesNotMatch(assetMarkSource, /linear-gradient|radial-gradient|asset-mark--image::after|text-shadow/)
})

test('tone-2 仅控制无图回退色且字号随组件尺寸缩放', () => {
  const toneRule = cssRule(assetMarkCss, '.asset-mark--tone-2')

  assert.match(toneRule, /--asset-color: var\(--accent\)/)
  assert.match(toneRule, /--asset-ink: var\(--accent\)/)
  assert.doesNotMatch(assetMarkSource, /#[0-9a-f]{3,8}/i)
  assert.match(assetMarkSource, /--asset-mark-font-size': `\$\{Math\.min\(21, Math\.max\(11, props\.size \* 0\.4\)\)\}px`/)
  assert.match(assetMarkSource, /font-size: var\(--asset-mark-font-size\)/)
  assert.match(assetMarkSource, /height: var\(--asset-mark-size\)/)
  assert.match(assetMarkSource, /width: var\(--asset-mark-size\)/)
})

test('杠杆交易头部沿用共享资产材质而不覆盖绿色描边和无阴影样式', () => {
  const tradeRule = cssRule(tradeCss, '.contract-pair-selector :deep(.asset-mark)')

  assert.match(tradeRule, /flex: 0 0 auto/)
  assert.doesNotMatch(tradeRule, /contract-accent|border|box-shadow/)
  assert.match(tradeSource, /<AssetMark :symbol="baseAsset" :src="selectedProduct\?\.logoUrl \|\| ticker\?\.iconUrl" :fallback-src="ticker\?\.baseIconUrl" :size="28"/)
})

test('杠杆持仓标签使用专用类且不覆盖 AssetMark 圆形根节点', () => {
  const badgeSelector = '.contract-position-identity > div > .contract-position-badge'
  const badgeRule = cssRule(tradeCss, badgeSelector)
  const badgeGroupRule = cssRule(tradeCss, '.contract-position-identity > div')
  const positionIdentityTemplate = sliceBetween(
    tradeSource,
    '<div class="contract-position-identity">',
    '<div class="contract-position-pnl"',
  )
  const assetMarkTag = positionIdentityTemplate.match(/<AssetMark\b[\s\S]*?\/>/)?.[0]
  const badges = [...positionIdentityTemplate.matchAll(/<span\b([^>]*)>([\s\S]*?)<\/span>/g)].map((match) => ({
    attributes: match[1] || '',
    content: match[2] || '',
  }))

  assert.ok(assetMarkTag)
  assert.doesNotMatch(assetMarkTag, /contract-position-badge/)
  assert.equal(badges.length, 3)
  for (const badge of badges) {
    assert.ok(staticClassTokens(badge.attributes).includes('contract-position-badge'))
  }

  const [directionBadge, marginModeBadge, leverageBadge] = badges
  assert.ok(directionBadge && marginModeBadge && leverageBadge)
  assert.match(directionBadge.attributes, /:class="position\.direction === 'long' \? 'positive' : 'negative'"/)
  assert.match(directionBadge.content, /t\(position\.direction === 'long' \? 'orders\.long' : 'orders\.short'\)/)
  assert.match(marginModeBadge.content, /t\(position\.marginMode === 'cross' \? 'trade\.cross' : 'trade\.isolated'\)/)
  assert.ok(staticClassTokens(leverageBadge.attributes).includes('numeric'))
  assert.match(leverageBadge.content, /\{\{\s*position\.leverage\s*\}\}x/)

  const compiled = compileStyle({
    source: tradeCss,
    filename: 'TradeView.vue',
    id: 'data-v-margin-position-logo',
    scoped: true,
  })
  assert.deepEqual(compiled.errors, [])
  const compiledCss = compiled.code.replace(/\/\*[\s\S]*?\*\//g, '')
  const broadPositionSpanSelector = /(?:^|,)\s*\.contract-position-identity(?:\[data-v-margin-position-logo\])?(?:\s+|\s*>\s*)span\[data-v-margin-position-logo\]\s*(?=,|\{)/m
  assert.match(
    compiledCss,
    /\.contract-position-identity\s*>\s*div\s*>\s*\.contract-position-badge\[data-v-margin-position-logo\]\s*\{/,
  )
  assert.doesNotMatch(compiledCss, broadPositionSpanSelector)

  const broadSelectorFixture = compileStyle({
    source: tradeCss.replace(`${badgeSelector} {`, '.contract-position-identity span {'),
    filename: 'TradeView.vue',
    id: 'data-v-margin-position-logo',
    scoped: true,
  })
  assert.deepEqual(broadSelectorFixture.errors, [])
  assert.match(
    broadSelectorFixture.code.replace(/\/\*[\s\S]*?\*\//g, ''),
    broadPositionSpanSelector,
    '回归守卫必须能识别 Vue scoped CSS 编译后的旧宽泛选择器',
  )

  assert.match(badgeRule, /background: var\(--contract-surface-soft\)/)
  assert.match(badgeRule, /border-radius: 3px/)
  assert.match(badgeRule, /color: var\(--contract-muted\)/)
  assert.match(badgeRule, /font-size: 8px/)
  assert.match(badgeRule, /line-height: 16px/)
  assert.match(badgeRule, /padding: 0 5px/)
  assert.match(badgeGroupRule, /gap: 4px/)
})

function scopedStyle(source: string): string {
  const match = source.match(/<style\b[^>]*\bscoped\b[^>]*>([\s\S]*?)<\/style>/)
  assert.ok(match, 'missing scoped style block')
  return match[1] || ''
}

function sliceBetween(source: string, startToken: string, endToken: string): string {
  const start = source.indexOf(startToken)
  assert.notEqual(start, -1, `missing start token: ${startToken}`)
  const end = source.indexOf(endToken, start + startToken.length)
  assert.notEqual(end, -1, `missing end token: ${endToken}`)
  return source.slice(start, end)
}

function staticClassTokens(attributes: string): string[] {
  const match = attributes.match(/(?:^|\s)class="([^"]*)"/)
  assert.ok(match, 'missing static class attribute')
  return (match[1] || '').split(/\s+/).filter(Boolean)
}
