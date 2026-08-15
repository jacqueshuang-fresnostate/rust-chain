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

test('首页市场简报只使用实时 ticker 生成市场广度和真实价格', () => {
  assert.equal(zhCN.home.marketBriefTitle, '市场脉搏')
  assert.equal(zhCN.home.marketBriefAdvancing, '上涨占比')
  assert.equal(en.home.marketBriefTitle, 'Market pulse')
  assert.equal(en.home.marketBriefAdvancing, 'Advancing')
  assert.match(homeSource, /import \{ buildHomeMarketBrief \} from '@\/core\/homeMarketBrief'/)
  assert.match(homeSource, /const marketBrief = computed\(\(\) => buildHomeMarketBrief\(marketStore\.tickers\)\)/)
  assert.match(homeSource, /marketBrief\.advancingPercent/)
  assert.match(homeSource, /marketBrief\.focusTicker\.lastPrice/)
  assert.match(homeSource, /marketBrief\.topMover\.changePercent/)
  assert.match(homeSource, /marketBrief\.rising,[\s\S]*?marketBrief\.falling,[\s\S]*?marketBrief\.unchanged/)
  assert.doesNotMatch(homeSource, /fetchNews|briefNotice|briefMessage|announcementState|NewsItem/)
})

test('市场简报加载失败可原位重试，加载成功进入完整行情而不是新闻详情', () => {
  assert.match(homeSource, /function openMarketBrief\(\): void \{\s*if \(!marketBrief\.value\) \{\s*void refreshMarkets\(true\)/)
  assert.match(homeSource, /router\.replace\(\{ name: 'markets' \}\)/)
  assert.match(homeSource, /:role="marketStore\.error \? 'alert' : 'status'"/)
  assert.match(homeSource, /home\.marketBriefUnavailable[\s\S]*?home\.marketBriefTapToRetry/)
  assert.doesNotMatch(homeSource, /name: 'news-detail'|:disabled="!briefNotice"/)
})
