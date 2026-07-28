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

test('共享二级壳复用原型 PageShell DOM 与 76/20px 几何合同', () => {
  assert.match(sources.pageHeader, /class="secondary-header page-header"/)
  assert.match(sources.pageHeader, /class="icon-button page-header__back"/)
  assert.match(sources.pageHeader, /class="secondary-scene page-header__eyebrow"/)
  assert.match(sources.pageHeader, /<strong class="page-header__title">\{\{ title \}\}<\/strong>/)
  assert.match(sources.pageHeader, /<small>\{\{ subtitle \|\| '' \}\}<\/small>/)
  assert.match(sources.pageHeader, /class="secondary-header-action page-header__actions"/)
  assert.match(sources.pageHeader, /:data-empty="hasActions \? 'false' : 'true'"/)
  assert.match(sources.pageHeader, /class="secondary-header-rail"/)
  assert.match(sources.pageHeader, /goBackOr\(router, props\.fallback \|\| route\.meta\.backFallback \|\| '\/'\)/)

  assert.match(
    prototypeCss,
    /Signal Theatre final secondary-surface contract[\s\S]*?\.secondary-header\s*\{[\s\S]*?min-height:\s*76px;[\s\S]*?grid-template-columns:\s*44px minmax\(0, 1fr\) 44px;/,
  )
  assert.match(
    prototypeCss,
    /Signal Theatre final secondary-surface contract[\s\S]*?\.secondary-content\s*\{[\s\S]*?padding:\s*20px 18px calc\(36px \+ env\(safe-area-inset-bottom\)\);/,
  )
  assert.match(parityCss, /\.secondary-view\.page\s*\{[\s\S]*?padding-block:\s*0;/)

  for (const source of [sources.seconds, sources.message, sources.loan, sources.security]) {
    const template = templateOf(source)
    assert.match(template, /class="secondary-view page /)
    assert.match(template, /class="secondary-content page-content /)
    assert.doesNotMatch(source, /\.(?:seconds|message-center|loan|security)-content\s*\{[\s\S]*?padding-top:/)
  }
})

test('四个优先页面精确使用公开原型的 PageShell 场景与上下文文案', () => {
  assert.match(sources.seconds, /:eyebrow="t\('seconds\.scene'\)"/)
  assert.match(sources.seconds, /:subtitle="t\('seconds\.context'\)"/)
  assert.match(sources.message, /:eyebrow="t\('messageCenter\.scene'\)"/)
  assert.match(sources.message, /:subtitle="t\('messageCenter\.context'\)"/)
  assert.match(sources.loan, /:eyebrow="t\('loan\.scene'\)"/)
  assert.match(sources.loan, /:subtitle="t\('loan\.context'\)"/)
  assert.match(sources.security, /:eyebrow="t\('security\.scene'\)"/)
  assert.match(sources.security, /:subtitle="t\('security\.context'\)"/)

  assert.deepEqual({
    seconds: [zhCN.seconds.scene, zhCN.seconds.context],
    message: [zhCN.messageCenter.scene, zhCN.messageCenter.context],
    loan: [zhCN.loan.scene, zhCN.loan.context],
    security: [zhCN.security.scene, zhCN.security.context],
  }, {
    seconds: ['产品与服务', '短周期方向交易'],
    message: ['消息与提醒', '账户、资金与交易动态'],
    loan: ['借贷工作台', '额度、成本与订单周期'],
    security: ['安全中心', '账户、资金与设备保护'],
  })

  assert.deepEqual({
    seconds: [en.seconds.scene, en.seconds.context],
    message: [en.messageCenter.scene, en.messageCenter.context],
    loan: [en.loan.scene, en.loan.context],
    security: [en.security.scene, en.security.context],
  }, {
    seconds: ['Products & services', 'Short-cycle directional trading'],
    message: ['Messages & alerts', 'Account, funds & trading activity'],
    loan: ['Lending workbench', 'Limits, costs & order lifecycle'],
    security: ['Security center', 'Account, funds & device protection'],
  })
})

test('贷款与安全中心标题精确对齐公开路由且保留根页面入口文案', () => {
  assert.match(sources.loan, /:title="t\('loan\.title'\)"/)
  assert.match(sources.security, /:title="t\('security\.title'\)"/)
  assert.equal(zhCN.loan.title, '贷款')
  assert.equal(zhCN.security.title, '安全中心')

  assert.match(sources.home, /t\('products\.loan'\)/)
  assert.match(sources.profile, /t\('rootPrototype\.accountAndSecurity'\)/)
  assert.equal(zhCN.products.loan, '借贷')
  assert.equal(zhCN.rootPrototype.accountAndSecurity, '账户与安全')
})

test('秒合约工作台保持原型顺序、真实接口与登录回跳', () => {
  const template = templateOf(sources.seconds)
  assertOrdered(template, [
    'seconds-workspace',
    'seconds-market-board',
    'seconds-pair-field',
    'seconds-direction-grid',
    'seconds-duration-grid',
    'seconds-amount-field',
    'seconds-amount-presets',
    'seconds-order-summary',
    'seconds-feedback',
    'seconds-submit',
    'seconds-session-records',
  ])
  assertOrdered(template, [
    "t('seconds.workbenchTitle')",
    'selected?.symbol',
    "t('seconds.referencePrice')",
    'selected.stakeAssetSymbol',
    "t('seconds.currentRound')",
    "t('seconds.settlementWindow')",
    "t('seconds.payoutCoefficient')",
    "t('seconds.estimatedPayout')",
    "t('seconds.availableBalance')",
    "t('seconds.localResult')",
  ])
  assert.match(
    template,
    /<dt>\{\{ t\('seconds\.currentRound'\) \}\}<\/dt>\s*<dd>--<\/dd>/,
  )
  assert.ok(template.includes('`${payoutCoefficient.toFixed(2)}x`'))
  assert.match(sources.seconds, /const payoutCoefficient = computed\(\(\) => 1 \+ payoutRate\.value\)/)
  assert.match(sources.seconds, /amountNumber\.value \* \(1 \+ payoutRate\.value\)/)
  assert.match(template, /t\('seconds\.directionHelper'\)/)
  assert.match(template, /t\('seconds\.durationHelper'\)/)
  assert.deepEqual([
    zhCN.seconds.workbenchTitle,
    zhCN.seconds.referencePrice,
    zhCN.seconds.currentRound,
    zhCN.seconds.settlementWindow,
    zhCN.seconds.payoutCoefficient,
    zhCN.seconds.estimatedPayout,
    zhCN.seconds.availableBalance,
    zhCN.seconds.localResult,
    zhCN.seconds.directionHelper,
    zhCN.seconds.durationHelper,
  ], [
    '短周期交易工作台',
    '实时参考价',
    '当前轮次',
    '结算窗口',
    '派彩系数',
    '预计派彩',
    '可用余额',
    '本地结果',
    '按轮次结束时的参考价方向判定',
    '选择本地判定周期',
  ])
  assert.match(sources.seconds, /fetchSecondsProducts\(\)/)
  assert.match(sources.seconds, /fetchSecondsOrders\(\)/)
  assert.match(sources.seconds, /fetchWalletAccounts\(\)/)
  assert.match(sources.seconds, /await openSecondsOrder\(\{[\s\S]*productId:[\s\S]*durationSeconds:[\s\S]*direction:[\s\S]*stakeAmount:/)
  assert.match(sources.seconds, /router\.push\(\{ name: 'login', query: \{ redirect: '\/seconds' \} \}\)/)
  assert.match(sources.seconds, /class="confirmation-layer seconds-mask"/)
  assert.match(sources.seconds, /role="dialog"/)
  assert.doesNotMatch(sources.seconds, /LoginRequiredState/)
})

test('消息中心使用公告真实源并保持原型时间线结构', () => {
  assertOrdered(templateOf(sources.message), [
    'message-center',
    'inbox-summary',
    'message-filter-bar',
    'message-tools',
    'inbox-all-read',
    'message-timeline',
    'message-time-group',
    'message-list',
  ])
  assert.match(sources.message, /messages\.value = await fetchNews\(40\)/)
  assert.match(
    sources.message,
    /\{ value: 'all'[\s\S]*\{ value: 'account'[\s\S]*\{ value: 'funds'[\s\S]*\{ value: 'trade'[\s\S]*\{ value: 'announcement'/,
  )
  assert.match(sources.message, /categoryHasNewsSource = computed\(\(\) => activeCategory\.value === 'all' \|\| activeCategory\.value === 'announcement'\)/)
  assert.match(sources.message, /categoryMessages = computed\(\(\) => categoryHasNewsSource\.value \? messages\.value : \[\]\)/)
  assert.match(sources.message, /const unreadOnly = ref\(false\)/)
  assert.match(sources.message, /\.message-filter-bar\s*\{[\s\S]*?grid-template-columns: repeat\(5, minmax\(0, 1fr\)\)/)
  assert.match(sources.message, /globalThis\.localStorage\?\.getItem\(READ_IDS_STORAGE_KEY\)/)
  assert.match(sources.message, /globalThis\.localStorage\?\.setItem\(READ_IDS_STORAGE_KEY, JSON\.stringify\(values\)\)/)
  assert.match(sources.message, /router\.push\(\{ name: 'news-detail', params: \{ id: String\(message\.id\) \} \}\)/)
  assert.doesNotMatch(sources.message, /fetch(?:Orders|Trades|Wallet|Account)/)
})

test('借贷页保持原型申请与生命周期结构且沿用真实订单接口', () => {
  assertOrdered(templateOf(sources.loan), [
    'borrowing-overview',
    'product-choice-grid',
    'loan-disclosures',
    'loan-requirement',
    'loan-application',
    'amount-presets',
    'loan-estimate',
    'loan-feedback',
    'loan-submit',
    'loan-order-columns',
  ])
  assert.match(sources.loan, /fetchLoanProducts\(\)/)
  assert.match(sources.loan, /fetchLoanOrders\(\)/)
  assert.match(sources.loan, /await applyLoan\(\{[\s\S]*productId:[\s\S]*amount:[\s\S]*collateralAssetId:[\s\S]*collateralAmount:/)
  assert.match(sources.loan, /await cancelLoanOrder\(order\.id\)/)
  assert.match(sources.loan, /await repayLoanOrder\(order\.id\)/)
  assert.match(sources.loan, /router\.push\(\{ name: 'login', query: \{ redirect: '\/products\/loan' \} \}\)/)
  assert.match(sources.loan, /class="confirmation-layer loan-mask"/)
  assert.match(sources.loan, /role="dialog"/)
  assert.match(sources.loan, /return amountNumber\.value \* product\.interestRate/)
  assert.doesNotMatch(sources.loan, /product\.interestRate \* product\.termDays \/ 365/)
  assert.match(sources.loan, /function statusLabel\(status: string\)/)
})

test('安全页保持原型防护任务结构并只暴露后端支持的真实动作', () => {
  assertOrdered(templateOf(sources.security), [
    'protection-overview',
    'protection-score',
    'security-checklist',
    'security-feedback-slot',
    'data-security-task="two-factor"',
    'data-security-task="password"',
    'data-security-task="funds"',
    'device-section',
    'device-list',
  ])
  assert.match(sources.security, /Promise\.all\(\[fetchUserProfile\(\), fetchTwoFactorStatus\(\)\]\)/)
  assert.match(sources.security, /const securityReady = ref\(false\)/)
  assert.match(sources.security, /!session\.isAuthenticated \|\| !securityReady \|\| loading/)
  assert.match(sources.security, /securityReady \? protectionPercent : '--'/)
  assert.match(sources.security, /await changeLoginPassword\(loginOldPassword\.value, loginNewPassword\.value\)/)
  assert.match(sources.security, /await changeFundPassword\(fundOldPassword\.value, fundNewPassword\.value\)/)
  assert.match(sources.security, /await setFundPassword\(fundLoginPassword\.value, fundNewPassword\.value\)/)
  assert.match(sources.security, /await updateLoginTwoFactor\(enabled\)/)
  assert.match(sources.security, /data-device-state="unavailable"/)
  assert.match(sources.security, /<button type="button" disabled>\{\{ t\('security\.notSet'\) \}\}<\/button>/)
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
