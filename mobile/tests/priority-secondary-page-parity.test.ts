import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const sources = {
  pageHeader: readFileSync(new URL('../src/components/PageHeader.vue', import.meta.url), 'utf8'),
  seconds: readFileSync(new URL('../src/views/SecondsView.vue', import.meta.url), 'utf8'),
  message: readFileSync(new URL('../src/views/MessageCenterView.vue', import.meta.url), 'utf8'),
  loan: readFileSync(new URL('../src/views/LoanView.vue', import.meta.url), 'utf8'),
  security: readFileSync(new URL('../src/views/SecurityView.vue', import.meta.url), 'utf8'),
  home: readFileSync(new URL('../src/views/HomeView.vue', import.meta.url), 'utf8'),
  profile: readFileSync(new URL('../src/views/ProfileView.vue', import.meta.url), 'utf8'),
}
const prototypeCss = readFileSync(new URL('../src/styles/prototype-base.css', import.meta.url), 'utf8')
const parityCss = readFileSync(new URL('../src/styles/prototype-parity.css', import.meta.url), 'utf8')
const selectedCss = readFileSync(new URL('../src/styles/pencil-selected-pages.css', import.meta.url), 'utf8')

function templateOf(source: string): string {
  const start = source.indexOf('<template>')
  const end = source.indexOf('<style scoped>')
  return start >= 0 && end > start ? source.slice(start + '<template>'.length, end) : ''
}

function assertOrdered(source: string, classNames: string[]): void {
  let cursor = -1
  for (const className of classNames) {
    const next = source.indexOf(className, cursor + 1)
    assert.ok(next > cursor, `${className} should follow the previous prototype section`)
    cursor = next
  }
}

test('共享 PageHeader 同时支持旧二级壳与 Pencil 60px 选中稿', () => {
  assert.match(sources.pageHeader, /pencil \? 'pencil-page-header' : 'secondary-header'/)
  assert.match(sources.pageHeader, /class="icon-button page-header__back"/)
  assert.match(sources.pageHeader, /class="secondary-scene page-header__eyebrow"/)
  assert.match(sources.pageHeader, /<strong class="page-header__title">\{\{ title \}\}<\/strong>/)
  assert.match(sources.pageHeader, /<small>\{\{ subtitle \|\| '' \}\}<\/small>/)
  assert.match(sources.pageHeader, /class="secondary-header-action page-header__actions"/)
  assert.match(sources.pageHeader, /:data-empty="hasActions \? 'false' : 'true'"/)
  assert.match(sources.pageHeader, /class="secondary-header-rail"/)
  assert.match(
    sources.pageHeader,
    /goBackOr\([\s\S]*?router,[\s\S]*?props\.fallback \|\| route\.meta\.backFallback \|\| '\/',[\s\S]*?\{ preferFallback: props\.preferFallback \},[\s\S]*?\)/,
  )

  assert.match(
    prototypeCss,
    /Signal Theatre final secondary-surface contract[\s\S]*?\.secondary-header\s*\{[\s\S]*?min-height:\s*76px;[\s\S]*?grid-template-columns:\s*44px minmax\(0, 1fr\) 44px;/,
  )
  assert.match(
    prototypeCss,
    /Signal Theatre final secondary-surface contract[\s\S]*?\.secondary-content\s*\{[\s\S]*?padding:\s*20px 18px calc\(36px \+ env\(safe-area-inset-bottom\)\);/,
  )
  assert.match(parityCss, /\.secondary-view\.page\s*\{[\s\S]*?padding-block:\s*0;/)
  assert.match(sources.pageHeader, /\.pencil-page-header\s*\{[\s\S]*?height:\s*60px;[\s\S]*?position:\s*sticky;[\s\S]*?z-index: var\(--layer-sticky-header\)/)

  assert.match(sources.seconds, /class="page page--plain seconds-page"/)
  assert.match(sources.seconds, /<PageHeader[\s\S]*?:pencil="true"/)
  assert.match(sources.seconds, /class="page-content seconds-content"/)
  assert.match(sources.message, /class="page page--plain pencil-page message-center-page"/)
  assert.match(sources.message, /class="message-root-header"/)
  assert.doesNotMatch(sources.message, /<PageHeader/)
  assert.match(sources.security, /class="page page--plain pencil-page security-view"/)
  assert.match(sources.security, /<PageHeader[\s\S]*?:pencil="true"/)
  for (const source of [sources.seconds, sources.message, sources.security]) {
    assert.doesNotMatch(source, /secondary-view|secondary-content|page--prototype-grid/)
  }
  assert.match(sources.loan, /class="page page--plain pencil-page loan-pencil"/)
  assert.match(sources.loan, /<PageHeader :back="true" :pencil="true"/)
})

test('四个优先页面精确使用当前 Pencil Header 与独立消息根头部', () => {
  assert.match(sources.seconds, /:pencil="true"[\s\S]*?:title="selected\?\.symbol \|\| t\('seconds\.title'\)"/)
  assert.match(sources.message, /<header class="message-root-header">[\s\S]*?t\('messageCenter\.title'\)[\s\S]*?t\('messageCenter\.markAllReadShort'\)/)
  assert.match(sources.loan, /:pencil="true" :title="t\('loan\.title'\)"/)
  assert.match(sources.security, /:pencil="true"[\s\S]*?:title="t\('security\.title'\)"/)

  assert.deepEqual({
    seconds: zhCN.seconds.title,
    message: zhCN.messageCenter.title,
    loan: zhCN.loan.title,
    security: zhCN.security.title,
    markAllRead: zhCN.messageCenter.markAllReadShort,
  }, {
    seconds: '秒合约',
    message: '消息中心',
    loan: '借贷',
    security: '安全中心',
    markAllRead: '全部已读',
  })

  assert.deepEqual({
    seconds: en.seconds.title,
    message: en.messageCenter.title,
    loan: en.loan.title,
    security: en.security.title,
    markAllRead: en.messageCenter.markAllReadShort,
  }, {
    seconds: 'Seconds contract',
    message: 'Message center',
    loan: 'Loans',
    security: 'Account & security',
    markAllRead: 'Read all',
  })
})

test('贷款与安全中心标题精确对齐公开路由且保留根页面入口文案', () => {
  assert.match(sources.loan, /:title="t\('loan\.title'\)"/)
  assert.match(sources.security, /:title="t\('security\.title'\)"/)
  assert.equal(zhCN.loan.title, '借贷')
  assert.equal(zhCN.security.title, '安全中心')

  assert.match(sources.home, /t\('products\.loan'\)/)
  assert.match(sources.profile, /t\('profile\.securityCenter'\)/)
  assert.equal(zhCN.products.loan, '借贷')
  assert.equal(zhCN.rootPrototype.accountAndSecurity, '账户与安全')
})

test('秒合约工作台保持选中稿操作区与订单区顺序、真实接口与登录回跳', () => {
  const template = templateOf(sources.seconds)
  assertOrdered(template, [
    'seconds-pair-field',
    'seconds-content',
    'seconds-workspace',
    'class="seconds-trading-operation"',
    'class="seconds-market-status"',
    'class="seconds-price-panel"',
    'class="seconds-micro-chart"',
    'class="instrument-plate seconds-order-console"',
    'class="seconds-duration-scroll"',
    'class="seconds-cycle-limit"',
    'class="seconds-amount-field"',
    'class="seconds-direction-grid"',
    'class="button button--primary button--full seconds-submit"',
    'class="seconds-orders-workspace"',
    'class="seconds-orders-heading"',
    'class="seconds-order-filters"',
    'class="seconds-feedback"',
    'class="seconds-active-order-list"',
    'class="seconds-active-order"',
  ])
  assert.equal(zhCN.seconds.estimatedProfit, '预计收益')
  assert.equal(en.seconds.estimatedProfit, 'Estimated profit')
  assert.match(sources.seconds, /orderReview\.value\?\.estimatedProfit \?\? null/)
  assert.match(sources.seconds, /estimatedProfit: orderProfit/)
  assert.match(sources.seconds, /moneyText\(orderMoney\(order\)\.entryPrice\)/)
  assert.match(sources.seconds, /openedOrder = await openSecondsOrder\(\{/)
  assert.match(sources.seconds, /orders\.value = upsertSecondsOrder\(orders\.value, openedOrder\)/)
  assert.match(sources.seconds, /fetchSecondsProducts\(\)/)
  assert.match(sources.seconds, /fetchSecondsOrders\(100\)/)
  assert.match(sources.seconds, /fetchWalletAccounts\(\)/)
  assert.match(sources.seconds, /await openSecondsOrder\(\{[\s\S]*productId: review\.productId,[\s\S]*durationSeconds: review\.durationSeconds,[\s\S]*direction: review\.direction,[\s\S]*stakeAmount: review\.stakeAmountText,[\s\S]*idempotencyKey: review\.idempotencyKey,/)
  assert.match(sources.seconds, /router\.push\(\{ name: 'login', query: \{ redirect: '\/seconds' \} \}\)/)
  assert.match(sources.seconds, /class="confirmation-layer seconds-mask"/)
  assert.match(sources.seconds, /role="dialog"/)
  assert.match(sources.seconds, /router\.push\(\{ name: 'seconds-history' \}\)/)
  assert.doesNotMatch(sources.seconds, /seconds-session-records|ordersSection|scrollToOrders/)
  assert.doesNotMatch(sources.seconds, /LoginRequiredState/)
})

test('秒合约行情摘要只使用权威订单倒计时或就绪态且保留国际化合同', () => {
  assert.match(selectedCss, /\.app-stage \.mobile-canvas \.seconds-page\s*\{[\s\S]*?--seconds-signal: #43efa9;/)
  assert.match(sources.seconds, /const nearestSelectedActiveOrder = computed/)
  assert.match(sources.seconds, /const roundStatusLabel = computed\(\(\) => \{[\s\S]*?id: order\.id,[\s\S]*?countdown: orderCountdown\(order\)[\s\S]*?t\('seconds\.readyState'\)/)
  assert.doesNotMatch(sources.seconds, /01842|t\('seconds\.currentRound'\)/)
  assert.equal(zhCN.seconds.readyState, '等待下单')
  assert.equal(en.seconds.readyState, 'Ready to trade')
  assert.equal(zhCN.seconds.activeRoundStatus, '订单 #{id} · 剩余 {countdown}')
  assert.equal(en.seconds.activeRoundStatus, 'Order #{id} · {countdown} left')
})

test('消息中心使用公告真实源并保持 FkZ6j 四分类连续列表结构', () => {
  assertOrdered(templateOf(sources.message), [
    'message-center-page',
    'message-root-header',
    'message-filter-bar',
    'message-list',
  ])
  assert.match(sources.message, /messages\.value = await fetchNews\(40\)/)
  assert.match(
    sources.message,
    /\{ value: 'all'[\s\S]*\{ value: 'account'[\s\S]*\{ value: 'funds'[\s\S]*\{ value: 'trade'/,
  )
  assert.equal((sources.message.match(/\{ value: '(?:all|account|funds|trade)'/g) || []).length, 4)
  assert.doesNotMatch(sources.message, /value: 'announcement'|const unreadOnly|categoryHasNewsSource/)
  assert.match(sources.message, /const visibleMessages = computed\(\(\) => activeCategory\.value === 'all' \? messages\.value : \[\]\)/)
  assert.match(sources.message, /\.message-filter-bar\s*\{[\s\S]*?display: flex;[\s\S]*?height: 38px;/)
  assert.match(sources.message, /\.message-row,[\s\S]*?grid-template-columns: 40px minmax\(0, 1fr\) auto;[\s\S]*?min-height: 64px;/)
  assert.match(sources.message, /globalThis\.localStorage\?\.getItem\(READ_IDS_STORAGE_KEY\)/)
  assert.match(sources.message, /globalThis\.localStorage\?\.setItem\(READ_IDS_STORAGE_KEY, JSON\.stringify\(values\)\)/)
  assert.match(sources.message, /router\.push\(\{ name: 'news-detail', params: \{ id: String\(message\.id\) \} \}\)/)
  assert.doesNotMatch(sources.message, /fetch(?:Orders|Trades|Wallet|Account)/)
})

test('借贷页保持原型申请与生命周期结构且沿用真实订单接口', () => {
  assertOrdered(templateOf(sources.loan), [
    'loan-hero-pencil',
    'loan-login-cta',
    'loan-categories',
    'loan-products-pencil',
    'loan-application-pencil',
    'loan-presets',
    'loan-estimate-pencil',
    'pencil-primary pencil-primary--full',
    'loan-orders-pencil',
    'loan-risk-note',
  ])
  assert.match(sources.loan, /fetchLoanProducts\(\)/)
  assert.match(sources.loan, /fetchLoanOrders\(\)/)
  assert.match(sources.loan, /await applyLoan\(\{[\s\S]*productId:[\s\S]*amount:[\s\S]*collateralAssetId:[\s\S]*collateralAmount:/)
  assert.match(sources.loan, /await cancelLoanOrder\(order\.id\)/)
  assert.match(sources.loan, /await repayLoanOrder\(order\.id\)/)
  assert.match(sources.loan, /router\.push\(\{ name: 'login', query: \{ redirect: '\/products\/loan' \} \}\)/)
  assert.match(sources.loan, /class="confirmation-layer loan-mask"/)
  assert.match(sources.loan, /role="dialog"/)
  assert.match(sources.loan, /return decimalMultiply\(amountText\.value, decimalTextFromFiniteNumber\(product\.interestRate\)\)/)
  assert.doesNotMatch(sources.loan, /product\.interestRate \* product\.termDays \/ 365/)
  assert.match(sources.loan, /function statusLabel\(status: string\)/)
  assert.doesNotMatch(sources.loan, /loan-access-pencil/)
})

test('安全页保持原型防护任务结构并只暴露后端支持的真实动作', () => {
  assertOrdered(templateOf(sources.security), [
    'security-hero',
    'security-feedback',
    'security-methods',
    'data-security-task="password"',
    'data-security-task="funds"',
    'data-security-task="two-factor"',
    'security-method--policy',
    'security-recovery',
    'data-security-task="recovery"',
  ])
  assert.match(sources.security, /Promise\.all\(\[fetchUserProfile\(\), fetchTwoFactorStatus\(\)\]\)/)
  assert.match(sources.security, /const securityReady = ref\(false\)/)
  assert.match(sources.security, /!session\.isAuthenticated \|\| !securityReady \|\| loading/)
  assert.match(sources.security, /securityReady \? protectionPercent : '--'/)
  assert.match(sources.security, /await changeLoginPassword\(loginOldPassword\.value, loginNewPassword\.value\)/)
  assert.match(sources.security, /await changeFundPassword\(fundOldPassword\.value, fundNewPassword\.value\)/)
  assert.match(sources.security, /await setFundPassword\(fundLoginPassword\.value, fundNewPassword\.value\)/)
  assert.match(sources.security, /await updateLoginTwoFactor\(enabled\)/)
  assert.match(sources.security, /sendUserTwoFactorResetCode\(\)/)
  assert.match(sources.security, /sendFundPasswordResetCode\(\)/)
  assert.doesNotMatch(sources.security, /device-section|device-list|data-device-state/)
  assert.doesNotMatch(sources.security, /(?:remove|revoke|delete)(?:Device|Session)/)
  assert.doesNotMatch(sources.security, /LoginRequiredState/)
})

test('四个页面模板只通过 i18n 输出文案并使用 Lucide 组件图标', () => {
  for (const source of [sources.seconds, sources.message, sources.loan, sources.security]) {
    const template = templateOf(source)
    assert.doesNotMatch(template, /[\u3400-\u9fff]/)
    assert.doesNotMatch(template, /<svg\b/i)
    assert.doesNotMatch(template, /\p{Extended_Pictographic}/u)
    assert.match(source, /from 'lucide-vue-next'/)
  }
  assert.match(prototypeCss, /\.secondary-content\s*\{[\s\S]*?env\(safe-area-inset-bottom\)/)
})
