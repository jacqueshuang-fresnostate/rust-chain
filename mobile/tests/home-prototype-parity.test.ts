import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const homeSource = readFileSync(new URL('../src/views/HomeView.vue', import.meta.url), 'utf8')

test('首页搜索与快捷入口使用原型短文案且不缩短产品页标签', () => {
  assert.equal(zhCN.home.searchPlaceholder, '搜索币种、产品或功能')
  assert.equal(zhCN.home.newCoinsShortcut, '新币')
  assert.equal(zhCN.home.secondsShortcut, '秒合约')
  assert.equal(zhCN.products.newCoins, '新币认购')
  assert.equal(zhCN.products.prediction, '预测市场')

  assert.equal(en.home.searchPlaceholder, 'Search coins, products, or features')
  assert.equal(en.home.newCoinsShortcut, 'New coins')
  assert.equal(en.home.secondsShortcut, 'Seconds')
  assert.equal(en.products.newCoins, 'New coins')
  assert.equal(en.products.prediction, 'Prediction markets')

  const shortcutSection = homeSource.slice(
    homeSource.indexOf('<section class="shortcut-section"'),
    homeSource.indexOf('</section>', homeSource.indexOf('<section class="shortcut-section"')),
  )
  assert.match(shortcutSection, /t\('home\.newCoinsShortcut'\)/)
  assert.match(shortcutSection, /data-home-shortcut="seconds"/)
  assert.match(shortcutSection, /router\.push\(\{ name: 'seconds' \}\)/)
  assert.match(shortcutSection, /<Zap :size="19"/)
  assert.match(shortcutSection, /t\('home\.secondsShortcut'\)/)
  assert.doesNotMatch(shortcutSection, /name: 'prediction'|<Gauge|predictionShortcut/)
  assert.doesNotMatch(shortcutSection, /t\('products\.(?:newCoins|prediction)'\)/)
})

test('首页行情日报固定文案精确对齐原型并仅展示真实公告或诚实状态', () => {
  assert.equal(zhCN.rootPrototype.aiMarketBrief, 'AI 行情日报')
  assert.equal(zhCN.rootPrototype.aiMarketBriefTitle, '三分钟读懂今日市场')
  assert.match(homeSource, /<small>\{\{ t\('rootPrototype\.aiMarketBrief'\) \}\}<\/small>/)
  assert.match(homeSource, /<strong>\{\{ t\('rootPrototype\.aiMarketBriefTitle'\) \}\}<\/strong>/)
  assert.match(homeSource, /<em>\{\{ briefMessage \}\}<\/em>/)
  assert.match(homeSource, /const briefNotice = computed<NewsItem \| null>\(\(\) => announcements\.value\[0\] \|\| null\)/)
  assert.match(homeSource, /if \(briefNotice\.value\) return briefNotice\.value\.title/)
  assert.match(homeSource, /announcementState\.value === 'loading'[\s\S]*?announcementState\.value === 'error'/)
  assert.doesNotMatch(homeSource, /BTC 资金回流，主流币波动率正在抬升|fallbackNews|usingFallbackNews/)
})

test('无真实公告时日报保持原色禁用且不会产生详情导航', () => {
  assert.match(homeSource, /:disabled="!briefNotice"/)
  assert.match(homeSource, /function openBriefNotice\(\): void \{\s*if \(!briefNotice\.value\) return/)
  assert.match(homeSource, /\.home-view \.market-brief:disabled\s*\{\s*opacity:\s*1;\s*\}/)
})
