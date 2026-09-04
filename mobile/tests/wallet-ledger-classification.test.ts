// ============================================================================
// 资金账单核心层与页面合同测试
// 覆盖三类合同：
//   1) 纯函数行为：响应映射、条目合并、分页推进、日期分组、格式化、
//      请求参数构造、请求生命周期（账单/分页/资产目录）
//   2) 页面源码合同：用正则锁定 WalletLedgerView.vue 的关键实现，防实现漂移
//   3) 样式编译合同：scoped 样式可编译，三态方向色按顺序覆盖，明暗主题 token 完整
// 运行：node --test --experimental-strip-types tests/wallet-ledger-classification.test.ts
// ============================================================================

// node:test / node:assert：测试框架与严格断言
import assert from 'node:assert/strict'
// readFileSync：读取相关源码原文，供正则合同断言使用
import { readFileSync } from 'node:fs'
import test from 'node:test'
// TypeScript 编译器：把页面脚本按 AST 解析，提取 directionTone 函数真身执行
import ts from 'typescript'
// vue-i18n：构造 i18n 实例校验复数文案
import { createI18n } from 'vue-i18n'
// vue SFC 编译器：解析页面单文件组件、编译 scoped 样式
import { compileStyle, parse as parseSfc } from 'vue/compiler-sfc'
// 被测核心层（src/core/walletLedger.ts）的全部导出：
//   advanceWalletLedgerPagination：分页推进（nextOffset / exhausted）
//   createWalletLedgerAssetDirectoryRequestLifecycle：资产目录请求生命周期
//   createWalletLedgerPaginationController：分页控制器
//   createWalletLedgerRequestParams：查询参数构造
//   createWalletLedgerRequestLifecycle：账单请求生命周期
//   formatWalletLedgerDecimal：十进制格式化
//   formatWalletLedgerGroupHeading / formatWalletLedgerTime：分组标题与时间
//   groupWalletLedgerEntries / mergeWalletLedgerEntries：分组与合并
//   isWalletLedgerContractError：契约错误判定
//   mapWalletLedgerResponse：后端响应 → 前端模型映射
//   WALLET_LEDGER_*：枚举常量；walletLedger*：辅助函数；type WalletLedger*：类型
import {
  advanceWalletLedgerPagination,
  createWalletLedgerAssetDirectoryRequestLifecycle,
  createWalletLedgerPaginationController,
  createWalletLedgerRequestParams,
  createWalletLedgerRequestLifecycle,
  formatWalletLedgerDecimal,
  formatWalletLedgerGroupHeading,
  formatWalletLedgerTime,
  groupWalletLedgerEntries,
  isWalletLedgerContractError,
  mapWalletLedgerResponse,
  mergeWalletLedgerEntries,
  WALLET_LEDGER_ACCOUNT_FILTERS,
  WALLET_LEDGER_DATE_PRESETS,
  WALLET_LEDGER_DIRECTIONS,
  WALLET_LEDGER_FILTERS,
  WALLET_LEDGER_KNOWN_CHANGE_TYPES,
  WALLET_LEDGER_MAX_FRACTION_DIGITS,
  walletLedgerAmountSign,
  walletLedgerAccountTranslationKey,
  walletLedgerCategoryTranslationKey,
  walletLedgerDatePresetTranslationKey,
  walletLedgerDateRange,
  walletLedgerDirectionForAmount,
  walletLedgerDirectionTranslationKey,
  walletLedgerEntryIdentity,
  walletLedgerFeeDebitAmount,
  walletLedgerTypePresentation,
  type WalletLedgerAccountFilter,
  type WalletLedgerAccountType,
  type WalletLedgerCategory,
  type WalletLedgerDatePreset,
  type WalletLedgerDirection,
  type WalletLedgerEntry,
  type WalletLedgerFetchOptions,
  type WalletLedgerFilter,
  type WalletLedgerPage,
} from '../src/core/walletLedger.ts'
// 十进制规范化工具（构造测试输入用）
import { normalizeDecimalText } from '../src/core/decimal.ts'
// 双语文案表（文案完整性校验用）
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

// 读取相关源码原文：API 层、核心层、账单页面与两个交易记录组件，
// 供"源码正则合同"断言使用（不执行，只做文本匹配）
const walletApiSource = readFileSync(new URL('../src/api/wallet.ts', import.meta.url), 'utf8')
const walletCoreSource = readFileSync(new URL('../src/core/walletLedger.ts', import.meta.url), 'utf8')
const viewSource = readFileSync(new URL('../src/views/WalletLedgerView.vue', import.meta.url), 'utf8')
const transactionRecordsLayoutSource = readFileSync(new URL('../src/components/TransactionRecordsLayout.vue', import.meta.url), 'utf8')
const transactionRecordEmptySource = readFileSync(new URL('../src/components/TransactionRecordEmptyState.vue', import.meta.url), 'utf8')
// 解析账单页面 SFC：模板 / 脚本 / scoped 样式三段分别供合同断言与编译校验使用
const parsedView = parseSfc(viewSource, { filename: 'WalletLedgerView.vue' })
const viewTemplateSource = parsedView.descriptor.template?.content ?? ''
const viewScriptSource = parsedView.descriptor.scriptSetup?.content ?? ''
const viewStyleSource = parsedView.descriptor.styles.find((style) => style.scoped)?.content ?? ''

// 从编译产物中截取的一条 CSS 规则：声明体文本 + 在编译产物中的起始位置
type CssRule = {
  body: string
  start: number
}

// 页面 directionTone 函数的签名（提取执行用）
type DirectionTone = (entry: WalletLedgerEntry) => 'is-buy' | 'is-sell' | 'is-ink'

// 按选择器从 CSS 文本中截取一条完整规则（大括号配对），
// 找不到规则或括号不闭合都直接断言失败
function cssRule(css: string, selector: string): CssRule {
  const marker = `${selector} {`
  const start = css.indexOf(marker)
  assert.notEqual(start, -1, `missing CSS rule ${selector}`)
  const openingBrace = start + marker.length - 1
  let depth = 1
  for (let index = openingBrace + 1; index < css.length; index += 1) {
    if (css[index] === '{') depth += 1
    if (css[index] !== '}') continue
    depth -= 1
    if (depth === 0) {
      return { body: css.slice(openingBrace + 1, index), start }
    }
  }
  assert.fail(`unterminated CSS rule ${selector}`)
}

// 在规则声明体里取指定属性的值（正则匹配"行首属性: 值;"）
function cssDeclaration(body: string, property: string): string {
  const escapedProperty = property.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = new RegExp(`(?:^|\\n)\\s*${escapedProperty}:\\s*([^;]+);`).exec(body)
  assert.ok(match, `missing CSS declaration ${property}`)
  return match[1].trim()
}

// 提取规则体里的全部 CSS 自定义属性（--xxx: value）成键值对
function cssCustomProperties(body: string): Record<string, string> {
  return Object.fromEntries(
    [...body.matchAll(/(?:^|\n)\s*(--[\w-]+):\s*([^;]+);/g)]
      .map((match) => [match[1], match[2].trim()]),
  )
}

// 从模板文本中截出 class="..." 所在的完整开标签（< 到 >），
// 用于断言某个元素上的具体属性绑定
function openingTagWithClass(template: string, classValue: string): string {
  const classMarker = `class="${classValue}"`
  const classIndex = template.indexOf(classMarker)
  assert.notEqual(classIndex, -1, `missing template class ${classValue}`)
  const start = template.lastIndexOf('<', classIndex)
  const end = template.indexOf('>', classIndex)
  assert.ok(start >= 0 && end > classIndex, `invalid opening tag for ${classValue}`)
  return template.slice(start, end + 1)
}

// 从页面脚本 AST 中找到 directionTone 函数声明，单独转译后用真实的
// walletLedgerDirectionForAmount 注入执行——保证测的是生产函数本身，
// 而不是测试里复刻的副本；同时返回函数源码文本供合同断言
function loadDirectionToneFromView(): { call: DirectionTone, source: string } {
  const sourceFile = ts.createSourceFile(
    'WalletLedgerView.script.ts',
    viewScriptSource,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  )
  const declaration = sourceFile.statements.find((statement) => (
    ts.isFunctionDeclaration(statement) && statement.name?.text === 'directionTone'
  ))
  assert.ok(declaration && ts.isFunctionDeclaration(declaration), 'missing directionTone implementation')
  const source = declaration.getText(sourceFile)
  const transpiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.None,
      target: ts.ScriptTarget.ES2022,
    },
    reportDiagnostics: true,
  })
  assert.equal(transpiled.diagnostics?.length ?? 0, 0)
  const createDirectionTone = new Function(
    'walletLedgerDirectionForAmount',
    `${transpiled.outputText}\nreturn directionTone;`,
  ) as (classify: typeof walletLedgerDirectionForAmount) => DirectionTone
  return {
    call: createDirectionTone(walletLedgerDirectionForAmount),
    source,
  }
}

// ---------------------------------------------------------------------------
// 合同 1：后端响应 → 前端模型的映射
// ---------------------------------------------------------------------------
// 账单适配器严格消费权威账户、分类、分页、金额、手续费和时间：
// 蛇形转驼峰、金额/手续费/余额去多余零、币种大写、秒级时间戳转毫秒、
// 精度缺省值为 18、分页字段重命名；
// 并逐项验证非法载荷必须抛契约错误（错误信息里包含违规字段名）
test('账单适配器严格消费权威账户、分类、分页、金额、手续费和时间', () => {
  // 正常载荷：验证整条映射链路（含负金额、非零手续费、秒级时间戳）
  const mapped = mapWalletLedgerResponse({
    entries: [
      backendEntry({
        id: 7,
        account_type: 'spot',
        symbol: 'usdt',
        change_type: 'withdrawal_confirm',
        category: 'funding',
        amount: '-12.500000000000000000',
        fee: '0.250000000000000000',
        balance_after: '88.250000000000000000',
        created_at: 1_786_307_400,
      }),
    ],
    page: {
      number: 2,
      size: 30,
      total_elements: 91,
      total_pages: 4,
    },
  })

  // 期望：字段重命名 + 数值规范化 + 精度补默认 18 + 时间戳 ×1000
  assert.deepEqual(mapped, {
    entries: [{
      id: 7,
      accountType: 'spot',
      symbol: 'USDT',
      changeType: 'withdrawal_confirm',
      category: 'funding',
      amount: '-12.5',
      fee: '0.25',
      balanceAfter: '88.25',
      precisionScale: 18,
      createdAt: 1_786_307_400_000,
    }],
    page: {
      number: 2,
      size: 30,
      totalElements: 91,
      totalPages: 4,
    },
  })

  // 精度缺失必须抛错（错误信息含 precision_scale）
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ precision_scale: undefined })],
      page: pageFixture(),
    }),
    /precision_scale/,
  )
  // 精度为 null / 负数 / 超 18 / 小数 / 字符串，全部视为非法
  for (const precisionScale of [null, -1, 19, 1.5, '8']) {
    assert.throws(
      () => mapWalletLedgerResponse({
        entries: [backendEntry({ precision_scale: precisionScale })],
        page: pageFixture(),
      }),
      /precision_scale/,
    )
  }
  // 分页元数据缺失必须抛错
  assert.throws(
    () => mapWalletLedgerResponse({ entries: [], page: undefined }),
    (error) => isWalletLedgerContractError(error) && /page/.test(error.message),
  )
  // 账户类型缺失必须抛错
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ account_type: undefined })],
      page: pageFixture(),
    }),
    /account_type/,
  )
  // 账户类型取到筛选值（all）必须抛错：all 是前端筛选语义，不是账户
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ account_type: 'all' })],
      page: pageFixture(),
    }),
    /account_type/,
  )
  // 分类不在白名单必须抛错
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ category: 'trade' })],
      page: pageFixture(),
    }),
    /category/,
  )
  // 金额使用科学计数法必须抛错（契约要求十进制文本）
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ amount: '1e3' })],
      page: pageFixture(),
    }),
    /amount/,
  )
  // 负手续费必须抛错（手续费恒为非负扣减口径）
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ fee: '-0.01' })],
      page: pageFixture(),
    }),
    /fee/,
  )
  // 超出安全整数范围的时间戳必须抛错（防止毫秒换算丢精度）
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ created_at: Number.MAX_SAFE_INTEGER })],
      page: pageFixture(),
    }),
    /created_at/,
  )
  // page.size 与 entries 条数不符必须抛错（行数必须与页大小一致）
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry(), backendEntry({ id: 2 })],
      page: { ...pageFixture(), size: 1 },
    }),
    /page entries/,
  )
  // total_pages 与 total_elements 矛盾必须抛错（1 条 31 个元素不可能 1 页）
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [],
      page: { ...pageFixture(), total_elements: 31, total_pages: 1 },
    }),
    /total_pages/,
  )
})

// ---------------------------------------------------------------------------
// 合同 2：条目身份、合并与分组
// ---------------------------------------------------------------------------
// 现货与杠杆的相同数字 ID 使用 accountType:id 复合身份合并：
// 两条 id=30 的记录互不覆盖；同 id 的旧现货记录被更新侧（amount=999）覆盖；
// 合并后身份键为 "accountType:id"；分组输出按该身份排序
test('现货与杠杆的相同数字 ID 使用 accountType:id 复合身份合并', () => {
  const timestamp = new Date(2026, 7, 10, 12, 0).getTime()
  const spot = ledgerEntry(30, timestamp, 'spot')
  const margin = ledgerEntry(30, timestamp, 'margin')
  const merged = mergeWalletLedgerEntries(
    [spot],
    [margin, { ...spot, amount: normalizeDecimalText('999') }],
  )

  assert.deepEqual(merged.map(walletLedgerEntryIdentity).sort(), ['margin:30', 'spot:30'])
  assert.equal(merged.length, 2)
  assert.equal(merged.find((entry) => entry.accountType === 'spot')?.amount, '999')

  // 分组：同组的条目按复合身份稳定排序
  const grouped = groupWalletLedgerEntries(merged, new Date(2026, 7, 10, 18, 0))
  assert.deepEqual(grouped[0].entries.map(walletLedgerEntryIdentity), ['margin:30', 'spot:30'])
})

// 加载更多偏移按服务端已消费行推进，重复 ID 和空页都能确定性收口：
// nextOffset = offset + 本页行数（即使行 id 与之前重复）；
// 服务端报告消费完则 exhausted=true；空页直接收口
test('加载更多偏移按服务端已消费行推进，重复 ID 和空页都能确定性收口', () => {
  // 首页（offset=0，30 行）取回 2 条重复 id 的行：偏移推进 30+2=32，未收口
  const duplicatePage = {
    entries: [ledgerEntry(30, Date.now()), ledgerEntry(31, Date.now())],
    page: { number: 1, size: 30, totalElements: 91, totalPages: 4 },
  }
  assert.deepEqual(advanceWalletLedgerPagination(30, duplicatePage), {
    nextOffset: 32,
    exhausted: false,
  })

  // 第 3 页（offset=90）取回最后 1 条：90+1=91 已达总数，收口
  const finalPage = {
    entries: [ledgerEntry(91, Date.now())],
    page: { number: 3, size: 30, totalElements: 91, totalPages: 4 },
  }
  assert.deepEqual(advanceWalletLedgerPagination(90, finalPage), {
    nextOffset: 91,
    exhausted: true,
  })

  // 空页：偏移不变，直接收口
  const emptyPage = {
    entries: [],
    page: { number: 1, size: 30, totalElements: 91, totalPages: 4 },
  }
  assert.deepEqual(advanceWalletLedgerPagination(30, emptyPage), {
    nextOffset: 30,
    exhausted: true,
  })
})

// 本地日历分组按日期和组内时间倒序，并区分今天、昨天和本地日期：
// 按本地日历日聚合；组间日期倒序、组内时间倒序；
// relation 标记今天/昨天/具体日期；组标题分别用对应文案或本地化日期
test('本地日历分组按日期和组内时间倒序，并区分今天、昨天和本地日期', () => {
  // 固定"现在"为 2026-08-10 18:30（本地时区）
  const now = new Date(2026, 7, 10, 18, 30)
  const entries = [
    ledgerEntry(1, new Date(2026, 7, 9, 8, 0).getTime()),
    ledgerEntry(2, new Date(2026, 7, 10, 9, 0).getTime()),
    ledgerEntry(3, new Date(2026, 7, 8, 20, 0).getTime()),
    ledgerEntry(4, new Date(2026, 7, 10, 17, 0).getTime()),
  ]

  // 期望：今天组（4、2 倒序）、昨天组（1）、前天组（3）
  const groups = groupWalletLedgerEntries(entries, now)
  assert.deepEqual(groups.map((group) => ({
    key: group.key,
    relation: group.relation,
    ids: group.entries.map((entry) => entry.id),
    count: group.entries.length,
  })), [
    { key: '2026-08-10', relation: 'today', ids: [4, 2], count: 2 },
    { key: '2026-08-09', relation: 'yesterday', ids: [1], count: 1 },
    { key: '2026-08-08', relation: 'date', ids: [3], count: 1 },
  ])
  // 今天组标题：英文用传入的 TODAY
  assert.equal(
    formatWalletLedgerGroupHeading(groups[0], 'en-US', { today: 'TODAY', yesterday: 'YESTERDAY' }),
    'TODAY',
  )
  // 昨天组标题：中文用传入的"昨天"
  assert.equal(
    formatWalletLedgerGroupHeading(groups[1], 'zh-CN', { today: '今天', yesterday: '昨天' }),
    '昨天',
  )
  // 更早的组标题：本地化完整日期（含年份）
  assert.match(
    formatWalletLedgerGroupHeading(groups[2], 'en-US', { today: 'TODAY', yesterday: 'YESTERDAY' }),
    /2026/,
  )
  // 时间格式化：本地 08:00（兼容 12/24 小时制区域差异）
  assert.match(formatWalletLedgerTime(entries[0].createdAt, 'en-US'), /08:00|8:00/)
})

// ---------------------------------------------------------------------------
// 合同 3：筛选枚举与双语文案
// ---------------------------------------------------------------------------
// 方向与日期筛选、兼容分类以及全部已知变动类型均有双语文案：
// 枚举全集逐项锁定（顺序也是契约）；全部 i18n key 在中英文里都必须存在；
// 未知变动类型回退 typeOther 并保留原始值；关键文案与复数规则逐条核对
test('方向与日期筛选、兼容分类以及全部已知变动类型均有双语文案', () => {
  // 交易类型筛选全集（顺序固定）
  assert.deepEqual(WALLET_LEDGER_FILTERS, [
    'all',
    'funding',
    'spot',
    'margin',
    'seconds',
    'convert',
    'earn',
    'new_coin',
    'loan',
    'prediction',
    'other',
  ])
  // 账户筛选全集
  assert.deepEqual(WALLET_LEDGER_ACCOUNT_FILTERS, ['all', 'spot', 'margin'])
  // 方向筛选全集
  assert.deepEqual(WALLET_LEDGER_DIRECTIONS, ['all', 'credit', 'debit'])
  // 日期预设全集
  assert.deepEqual(WALLET_LEDGER_DATE_PRESETS, ['all', 'today', 'last7Days', 'last30Days'])
  // 已知变动类型全集（服务端会出现的所有 change_type）
  assert.deepEqual([...WALLET_LEDGER_KNOWN_CHANGE_TYPES].sort(), [
    'admin_recharge',
    'agent_commission_payout',
    'convert_settlement',
    'deposit',
    'deposit_confirm',
    'deposit_credit',
    'deposit_reorg_reverse',
    'earn_redeem',
    'earn_subscribe',
    'loan_collateral_freeze',
    'loan_collateral_release',
    'loan_disbursement',
    'loan_repayment',
    'margin_cross_account_liquidate',
    'margin_cross_position_close',
    'margin_position_cancel',
    'margin_position_close',
    'margin_position_liquidate',
    'margin_position_open',
    'margin_transfer_in',
    'margin_transfer_out',
    'new_coin_distribution_lock',
    'new_coin_purchase_lock',
    'new_coin_purchase_payment',
    'new_coin_subscription_lock',
    'new_coin_subscription_payment',
    'new_coin_unlock_release',
    'prediction_fee',
    'prediction_fee_refund',
    'prediction_payout',
    'prediction_settle_loss',
    'prediction_settle_win',
    'prediction_stake_freeze',
    'prediction_stake_refund',
    'quick_recharge',
    'seconds_contract_open',
    'seconds_contract_settle_win',
    'spot_fill',
    'spot_freeze',
    'spot_price_improvement_release',
    'spot_trade_settlement',
    'spot_unfreeze',
    'withdrawal_confirm',
    'withdrawal_release',
    'withdrawal_reserve',
  ].sort())

  // 所有相关 i18n key：筛选、账户、方向、日期、变动类型、页面与组件文案
  const translationKeys = new Set<string>([
    ...WALLET_LEDGER_FILTERS.map(walletLedgerCategoryTranslationKey),
    ...WALLET_LEDGER_ACCOUNT_FILTERS.map(walletLedgerAccountTranslationKey),
    ...WALLET_LEDGER_DIRECTIONS.map(walletLedgerDirectionTranslationKey),
    ...WALLET_LEDGER_DATE_PRESETS.map(walletLedgerDatePresetTranslationKey),
    ...WALLET_LEDGER_KNOWN_CHANGE_TYPES.map((type) => (
      walletLedgerTypePresentation(type).translationKey
    )),
    'ledger.today',
    'ledger.yesterday',
    'ledger.groupCount',
    'ledger.fee',
    'ledger.sourceType',
    'ledger.typeOther',
    'ledger.accountFilterLabel',
    'ledger.assetFilterLabel',
    'ledger.directionFilterLabel',
    'ledger.dateFilterLabel',
    'ledger.filterBarLabel',
    'ledger.filterClose',
    'ledger.recordTabsLabel',
    'ledger.positionHistoryTab',
    'ledger.transactionLedgerTab',
    'ledger.currentStrategyTab',
    'ledger.strategyHistoryTab',
    'ledger.currencyFilterTrigger',
    'ledger.transactionTypeFilterTrigger',
    'ledger.categoryPickerTitle',
    'ledger.morePickerTitle',
    'ledger.moreFilterSummary',
    'ledger.filterSelectionLabel',
    'ledger.quantity',
    'ledger.feeLabel',
    'ledger.accountBalance',
    'ledger.amountExact',
    'ledger.entryDetails',
    'routeAccessibility.titles.walletLedger',
    'assets.ledger',
    'assets.quickLedger',
    'assets.fundLedger',
  ])
  // 每个 key 在中英文文案表里都必须是字符串
  for (const key of translationKeys) {
    assert.equal(typeof resolveMessage(zhCN, key), 'string', `zh-CN missing ${key}`)
    assert.equal(typeof resolveMessage(en, key), 'string', `en missing ${key}`)
  }

  // 未知变动类型：文案回退 typeOther，原始值保留为来源小字
  const unknown = walletLedgerTypePresentation(' future_bonus_v2 ')
  assert.deepEqual(unknown, {
    translationKey: 'ledger.typeOther',
    source: 'future_bonus_v2',
  })
  // 已知类型没有来源小字（直接用标准文案）
  assert.equal(walletLedgerTypePresentation('prediction_fee_refund').source, undefined)
  // 关键文案逐条核对中英文
  assert.equal(resolveMessage(zhCN, 'ledger.categoryFunding'), '充提')
  assert.equal(resolveMessage(en, 'ledger.categoryFunding'), 'Deposits & withdrawals')
  assert.equal(resolveMessage(zhCN, 'ledger.categoryMargin'), '杠杆')
  assert.equal(resolveMessage(en, 'ledger.categoryMargin'), 'Margin')
  assert.equal(resolveMessage(zhCN, 'ledger.accountSpot'), '现货')
  assert.equal(resolveMessage(en, 'ledger.accountSpot'), 'Spot')
  assert.equal(resolveMessage(zhCN, 'ledger.accountMargin'), '杠杆')
  assert.equal(resolveMessage(en, 'ledger.accountMargin'), 'Margin')
  assert.equal(resolveMessage(zhCN, 'ledger.title'), '交易记录')
  assert.equal(resolveMessage(en, 'ledger.title'), 'Transaction Records')
  assert.equal(resolveMessage(zhCN, 'routeAccessibility.titles.walletLedger'), '交易记录')
  assert.equal(resolveMessage(en, 'routeAccessibility.titles.walletLedger'), 'Transaction Records')
  assert.equal(resolveMessage(zhCN, 'assets.ledger'), '交易记录')
  assert.equal(resolveMessage(en, 'assets.ledger'), 'Transaction Records')

  // 复数文案：1 条单数、2 条复数
  const testI18n = createI18n({
    legacy: false,
    locale: 'en',
    messages: { en },
  })
  assert.equal(testI18n.global.t('ledger.groupCount', 1), '1 record')
  assert.equal(testI18n.global.t('ledger.groupCount', 2), '2 records')
})

// ---------------------------------------------------------------------------
// 合同 4：金额符号、方向推导与手续费
// ---------------------------------------------------------------------------
// 账单金额仅为正数添加加号，零值保持中性且不带加号：
// 正数 '+'；零 ''；负数 ''（负号由数字自身携带）
test('账单金额仅为正数添加加号，零值保持中性且不带加号', () => {
  assert.equal(walletLedgerAmountSign(normalizeDecimalText('8')), '+')
  assert.equal(walletLedgerAmountSign(normalizeDecimalText('0')), '')
  assert.equal(walletLedgerAmountSign(normalizeDecimalText('-8')), '')
})

// 交易方向来自真实金额符号且非零手续费以 DecimalText 扣除值展示：
// 方向由金额符号推导（正=credit，负=debit，零=null）；
// 手续费统一转扣减口径（0.25 → -0.25；0 与 -0 归一为 '0'）
test('交易方向来自真实金额符号且非零手续费以 DecimalText 扣除值展示', () => {
  assert.equal(walletLedgerDirectionForAmount(normalizeDecimalText('8')), 'credit')
  assert.equal(walletLedgerDirectionForAmount(normalizeDecimalText('-8')), 'debit')
  assert.equal(walletLedgerDirectionForAmount(normalizeDecimalText('0')), null)
  assert.equal(walletLedgerFeeDebitAmount(normalizeDecimalText('0.25')), '-0.25')
  assert.equal(walletLedgerFeeDebitAmount(normalizeDecimalText('0')), '0')
  assert.equal(walletLedgerFeeDebitAmount(normalizeDecimalText('-0')), '0')
})

// ---------------------------------------------------------------------------
// 合同 5：页面总额方向色绑定（本轮改动核心）
// ---------------------------------------------------------------------------
// 账单总额真实绑定权威金额的三态方向类：
// 总额元素必须动态绑定 directionTone(entry) 并保留原始精确金额 title；
// 提取页面里的 directionTone 函数真身执行：正/负/零金额分别得到
// is-buy / is-sell / is-ink，且函数不得依赖变动类型（只看金额符号）
test('账单总额真实绑定权威金额的三态方向类', () => {
  // SFC 解析必须无错误（页面本身合法）
  assert.deepEqual(parsedView.errors, [])
  // 总额开标签：<strong> 且绑定 directionTone 与精确金额 title
  const totalTag = openingTagWithClass(viewTemplateSource, 'ledger-row__total numeric')
  assert.ok(totalTag.startsWith('<strong '))
  assert.ok(totalTag.includes(':class="directionTone(entry)"'))
  assert.ok(totalTag.includes(':title="exactAmountTitle(entry)"'))

  // 提取生产函数真身执行：正/负/零三种金额映射三种语义类
  const directionTone = loadDirectionToneFromView()
  assert.ok(directionTone.source.includes('walletLedgerDirectionForAmount(entry.amount)'))
  assert.equal(directionTone.source.includes('changeType'), false)
  const timestamp = Date.parse('2026-09-04T00:00:00Z')
  const entries = [
    { ...ledgerEntry(1, timestamp), changeType: 'withdrawal_confirm', amount: normalizeDecimalText('8') },
    { ...ledgerEntry(2, timestamp), changeType: 'deposit_confirm', amount: normalizeDecimalText('-8') },
    { ...ledgerEntry(3, timestamp), changeType: 'withdrawal_confirm', amount: normalizeDecimalText('0') },
  ]
  assert.deepEqual(entries.map(directionTone.call), ['is-buy', 'is-sell', 'is-ink'])
})

// 账单总额的动态语义色覆盖默认 ink，且明暗主题 token 完整：
// scoped 样式必须可编译；总额默认墨色且不用 !important；
// 三条语义色规则必须位于默认色规则之后（同优先级下后者生效）；
// 明暗两套主题的买入/卖出/墨色 token 值逐项锁定
test('账单总额的动态语义色覆盖默认 ink，且明暗主题 token 完整', () => {
  // scoped 样式编译必须无错误
  const compiled = compileStyle({
    source: viewStyleSource,
    filename: 'WalletLedgerView.vue',
    id: 'data-v-wallet-ledger',
    scoped: true,
  })
  assert.deepEqual(compiled.errors, [])

  // 总额规则：默认墨色、禁用 !important
  const totalRule = cssRule(compiled.code, '.ledger-row__total[data-v-wallet-ledger]')
  assert.equal(cssDeclaration(totalRule.body, 'color'), 'var(--wallet-record-ink)')
  assert.equal(totalRule.body.includes('!important'), false)
  // 三条语义色规则：在总额规则之后声明、颜色指向对应主题变量
  for (const [selector, token] of [
    ['.is-buy[data-v-wallet-ledger]', '--wallet-record-buy'],
    ['.is-sell[data-v-wallet-ledger]', '--wallet-record-sell'],
    ['.is-ink[data-v-wallet-ledger]', '--wallet-record-ink'],
  ] as const) {
    const semanticRule = cssRule(compiled.code, selector)
    assert.ok(semanticRule.start > totalRule.start, `${selector} must follow the equal-specificity total rule`)
    assert.equal(cssDeclaration(semanticRule.body, 'color'), `var(${token})`)
  }

  // 明暗主题：暗色规则必须在亮色规则之后；token 集合并集逐值锁定
  const lightRule = cssRule(compiled.code, '.wallet-ledger-pencil[data-v-wallet-ledger]')
  const darkRule = cssRule(compiled.code, "html[data-theme='dark'] .wallet-ledger-pencil")
  assert.ok(darkRule.start > lightRule.start)
  const light = cssCustomProperties(lightRule.body)
  const dark = { ...light, ...cssCustomProperties(darkRule.body) }
  assert.deepEqual(
    [light, dark].map((theme) => ({
      buy: theme['--wallet-record-buy'],
      sell: theme['--wallet-record-sell'],
      ink: theme['--wallet-record-ink'],
    })),
    [
      { buy: '#0dbe7b', sell: '#ff5878', ink: '#111714' },
      { buy: '#45efae', sell: '#ff5878', ink: '#f3f7f5' },
    ],
  )
})

// ---------------------------------------------------------------------------
// 合同 6：格式化与日期区间
// ---------------------------------------------------------------------------
// 账单保留权威资产精度，但可见文本使用独立精度并进行十进制舍入：
// 最大小数 18 位；默认 8 位显示、超长十进制舍入；极小值显示 "<0.00000001"；
// 千分位；-0 归一；传入 precisionScale 时按资产精度显示
test('账单保留权威资产精度，但可见文本使用独立精度并进行十进制舍入', () => {
  // 最大小数位数常量锁定为 18
  assert.equal(WALLET_LEDGER_MAX_FRACTION_DIGITS, 18)
  // 默认 8 位：原本就短的值原样显示
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('0.00125'), 'en-US'), '0.00125')
  // 低于 8 位最小单位的极小值：显示 "<0.00000001"
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('0.000000000000000001'), 'en-US'), '<0.00000001')
  // 18 位小数：舍入到 8 位
  assert.equal(
    formatWalletLedgerDecimal(normalizeDecimalText('12.123456789012345678'), 'en-US'),
    '12.12345679',
  )
  // 大数：千分位 + 舍入（且不丢整数部分精度）
  assert.equal(
    formatWalletLedgerDecimal(normalizeDecimalText('9007199254740993.123456789012345678'), 'en-US'),
    '9,007,199,254,740,993.12345679',
  )
  // -0 归一为 0
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('-0'), 'en-US'), '0')
  // 传入资产精度 2：全零小数被裁剪
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('12.340000000000000000'), 'en-US', 2), '12.34')
  // 传入资产精度 2：十进制进位（12.999…→13）
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('12.999999999999999999'), 'en-US', 2), '13')
  // 传入资产精度 2：极小值显示 "<0.01"
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('0.000000000000000001'), 'en-US', 2), '<0.01')
  // 传 symbol（USDT）：使用该资产的权威精度 2
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('1134.331253942506787192'), 'en-US', 18, 'USDT'), '1,134.33')
})

// 本地日期预设冻结为服务端可用的 UTC 区间：
// all 无区间；today 起点是本地当日 0 点、终点是当前时刻（UTC ISO）；
// 近 7 天 / 近 30 天起点为本地 0 点；用固定 now 验证
test('本地日期预设冻结为服务端可用的 UTC 区间', () => {
  // 固定"现在"：2026-09-01 15:30:45.123 本地时区
  const now = new Date(2026, 8, 1, 15, 30, 45, 123)
  // all：无起止
  assert.deepEqual(walletLedgerDateRange('all', now), {})

  // today：终点 = 当前时刻；起点 = 本地当日 0 点
  const today = walletLedgerDateRange('today', now)
  assert.equal(today.endTime, now.toISOString())
  const todayStart = new Date(today.startTime || '')
  assert.deepEqual(
    [todayStart.getFullYear(), todayStart.getMonth(), todayStart.getDate(), todayStart.getHours()],
    [2026, 8, 1, 0],
  )

  // 近 7 天起点 = 本地 8 月 26 日 0 点；近 30 天起点 = 本地 8 月 3 日 0 点
  const last7Start = new Date(walletLedgerDateRange('last7Days', now).startTime || '')
  const last30Start = new Date(walletLedgerDateRange('last30Days', now).startTime || '')
  assert.deepEqual([last7Start.getMonth(), last7Start.getDate(), last7Start.getHours()], [7, 26, 0])
  assert.deepEqual([last30Start.getMonth(), last30Start.getDate(), last30Start.getHours()], [7, 3, 0])
})

// ---------------------------------------------------------------------------
// 合同 7：请求参数与请求生命周期
// ---------------------------------------------------------------------------
// 账单查询把带时区日期规范为 MySQL 安全 UTC 文本并拒绝坏边界：
// 资产去空白并大写、方向透传、分类仅非 all 时携带、
// 带时区时间转 UTC "YYYY-MM-DD HH:mm:ss.SSS"；
// 非法日期、缺时区文本、起止倒置必须抛错
test('账单查询把带时区日期规范为 MySQL 安全 UTC 文本并拒绝坏边界', () => {
  // 正常构造：东八区时间换算为 UTC 文本（带毫秒）
  assert.deepEqual(createWalletLedgerRequestParams({
    limit: 30,
    offset: 60,
    assetSymbol: ' usdt ',
    direction: 'credit',
    startTime: '2026-09-01T08:00:00.123+08:00',
    endTime: '2026-09-02T07:59:59.999+08:00',
  }), {
    limit: 30,
    offset: 60,
    category: undefined,
    account_type: 'all',
    change_type: undefined,
    asset_symbol: 'USDT',
    direction: 'credit',
    start_time: '2026-09-01 00:00:00.123',
    end_time: '2026-09-01 23:59:59.999',
  })
  // 分类非 all 时直接透传给服务端
  assert.equal(createWalletLedgerRequestParams({ category: 'margin' }).category, 'margin')

  // 非法边界：不存在的日期、缺时区的空格分隔文本、起止倒置
  for (const options of [
    { startTime: '2026-02-30T00:00:00Z' },
    { startTime: '2026-09-01 00:00:00' },
    { startTime: '2026-09-02T00:00:00Z', endTime: '2026-09-01T23:59:59Z' },
  ]) {
    assert.throws(() => createWalletLedgerRequestParams(options), /wallet ledger/)
  }
})

// 账单业务分类直接传给服务端并隔离过期分类响应：
// 请求发出时携带"当时"的分类；响应回来时分类已变 → stale 不写回；
// 响应携带的分类与请求分类不符 → 契约错误
test('账单业务分类直接传给服务端并隔离过期分类响应', async () => {
  // 可变分类（模拟用户在请求飞行中切换筛选）
  let selectedCategory: WalletLedgerFilter = 'funding'
  // 记录每次发出的请求与手工 Promise，逐个控制响应时机
  const requests: Array<{
    options: WalletLedgerFetchOptions
    deferred: ReturnType<typeof deferred<WalletLedgerPage>>
  }> = []
  const lifecycle = createWalletLedgerRequestLifecycle({
    sessionKey: () => 'TOKEN',
    sessionGeneration: () => 1,
    selectedAssetSymbol: () => undefined,
    selectedCategory: () => selectedCategory,
    selectedDirection: () => 'all',
    selectedDatePreset: () => 'all',
    selectedDateRange: () => ({}),
    fetchPage: (options) => {
      const pending = deferred<WalletLedgerPage>()
      requests.push({ options, deferred: pending })
      return pending.promise
    },
  })

  // 第一发：携带 funding；发出后切换为 spot，再让响应回来 → stale
  const funding = lifecycle.load(0, 30)
  assert.equal(requests[0].options.category, 'funding')
  selectedCategory = 'spot'
  requests[0].deferred.resolve(pageResult(1, { category: 'funding' }))
  assert.deepEqual(await funding, { state: 'stale' })

  // 第二发：携带 spot；分类未变 → 正常 loaded
  const spot = lifecycle.load(0, 30)
  assert.equal(requests[1].options.category, 'spot')
  requests[1].deferred.resolve(pageResult(2, { category: 'spot' }))
  assert.equal((await spot).state, 'loaded')

  // 第三发：响应携带的分类（margin）与请求分类不符 → 契约错误
  const mismatched = lifecycle.load(0, 30)
  requests[2].deferred.resolve(pageResult(3, { category: 'margin' }))
  const result = await mismatched
  assert.equal(result.state, 'error')
  assert.ok(result.state === 'error' && isWalletLedgerContractError(result.error))
})

// 账单请求生命周期阻止旧资产、旧方向、旧日期、旧会话和卸载响应写回：
// 未登录直接 guest 且不发请求；请求参数携带当时的筛选；
// 资产变更后旧响应 → stale；方向不符 → 契约错误；
// 会话 token/代际变化后的旧响应 → stale；stop() 之后一律 stale 且不再发新请求
test('账单请求生命周期阻止旧资产、旧方向、旧日期、旧会话和卸载响应写回', async () => {
  // 可变外部状态（模拟会话与筛选的实时变化）
  let sessionKey = ''
  let sessionGeneration = 0
  let selectedAssetSymbol: string | undefined
  let selectedDirection: WalletLedgerDirection = 'all'
  let selectedDatePreset: WalletLedgerDatePreset = 'all'
  let selectedDateRange = walletLedgerDateRange('all')
  const requests: Array<{
    options: WalletLedgerFetchOptions
    deferred: ReturnType<typeof deferred<WalletLedgerPage>>
  }> = []
  const lifecycle = createWalletLedgerRequestLifecycle({
    sessionKey: () => sessionKey,
    sessionGeneration: () => sessionGeneration,
    selectedAssetSymbol: () => selectedAssetSymbol,
    selectedDirection: () => selectedDirection,
    selectedDatePreset: () => selectedDatePreset,
    selectedDateRange: () => selectedDateRange,
    fetchPage: (options) => {
      const pending = deferred<WalletLedgerPage>()
      requests.push({ options, deferred: pending })
      return pending.promise
    },
  })

  // 未登录：直接 guest，且根本没有发出请求
  assert.deepEqual(await lifecycle.load(0, 30), { state: 'guest' })
  assert.equal(requests.length, 0)

  // 登录并设置筛选（USDT + 收入 + 今天）：请求参数必须携带当时的全部筛选
  sessionKey = 'TOKEN_A'
  sessionGeneration = 1
  selectedAssetSymbol = 'USDT'
  selectedDirection = 'credit'
  selectedDatePreset = 'today'
  selectedDateRange = {
    startTime: '2026-09-01T00:00:00.000Z',
    endTime: '2026-09-01T23:59:59.999Z',
  }
  const usdtCredit = lifecycle.load(0, 30)
  assert.deepEqual(requests[0].options, {
    limit: 30,
    offset: 0,
    assetSymbol: 'USDT',
    direction: 'credit',
    startTime: '2026-09-01T00:00:00.000Z',
    endTime: '2026-09-01T23:59:59.999Z',
  })

  // 飞行中把资产切到 BTC：旧 USDT 响应回来 → stale；新 BTC 响应 → loaded
  selectedAssetSymbol = 'BTC'
  const btcCredit = lifecycle.load(0, 30)
  requests[0].deferred.resolve(pageResult(1, { symbol: 'USDT', amount: '1', createdAt: Date.parse('2026-09-01T12:00:00Z') }))
  assert.deepEqual(await usdtCredit, { state: 'stale' })
  requests[1].deferred.resolve(pageResult(2, { symbol: 'BTC', amount: '0.1', createdAt: Date.parse('2026-09-01T12:00:00Z') }))
  assert.equal((await btcCredit).state, 'loaded')

  // 方向切到支出后再发起：响应携带的方向不符 → 契约错误
  selectedDirection = 'debit'
  const mismatchedDirection = lifecycle.load(0, 30)
  requests[2].deferred.resolve(pageResult(3, { symbol: 'BTC', amount: '1', createdAt: Date.parse('2026-09-01T12:00:00Z') }))
  const directionMismatchResult = await mismatchedDirection
  assert.equal(directionMismatchResult.state, 'error')
  assert.ok(
    directionMismatchResult.state === 'error'
      && isWalletLedgerContractError(directionMismatchResult.error),
  )

  // 切到近 7 天并发起第 2 页；随后切换会话（换 token + 代际）：
  // 旧会话的响应回来 → stale
  selectedDirection = 'all'
  selectedDatePreset = 'last7Days'
  selectedDateRange = {
    startTime: '2026-08-26T00:00:00.000Z',
    endTime: '2026-09-01T23:59:59.999Z',
  }
  const dated = lifecycle.load(30, 30)
  assert.deepEqual(requests[3].options, {
    limit: 30,
    offset: 30,
    assetSymbol: 'BTC',
    direction: 'all',
    startTime: '2026-08-26T00:00:00.000Z',
    endTime: '2026-09-01T23:59:59.999Z',
  })
  sessionKey = 'TOKEN_B'
  sessionGeneration = 2
  requests[3].deferred.resolve(pageResult(4, { symbol: 'BTC', amount: '-1', createdAt: Date.parse('2026-08-30T12:00:00Z') }))
  assert.deepEqual(await dated, { state: 'stale' })

  // 同 token 但代际更旧：代际推进后响应回来 → stale
  sessionKey = 'TOKEN_A'
  sessionGeneration = 3
  const sameTokenOldGeneration = lifecycle.load(0, 30)
  sessionGeneration = 4
  requests[4].deferred.resolve(pageResult(5, { symbol: 'BTC', amount: '-1', createdAt: Date.parse('2026-08-30T12:00:00Z') }))
  assert.deepEqual(await sameTokenOldGeneration, { state: 'stale' })

  // 卸载（stop）：在飞响应 → stale；stop 之后再 load 也直接 stale，不再发请求
  const beforeUnmount = lifecycle.load(0, 30)
  lifecycle.stop()
  requests[5].deferred.resolve(pageResult(6, { symbol: 'BTC', amount: '-1', createdAt: Date.parse('2026-08-30T12:00:00Z') }))
  assert.deepEqual(await beforeUnmount, { state: 'stale' })
  assert.deepEqual(await lifecycle.load(0, 30), { state: 'stale' })
})

// 分页控制器隔离初始错误与追加错误并按原偏移重试且保留既有行：
// 初始失败记录 initialError；重试成功写入行；加载更多进行中再点 → ignored；
// 追加失败保留已有行与原偏移并记录 appendError；重试按原偏移；
// 重复 id 不重复插入；成功后清错误并按总数收口
test('分页控制器隔离初始错误与追加错误并按原偏移重试且保留既有行', async () => {
  const requests: Array<{
    options: WalletLedgerFetchOptions
    deferred: ReturnType<typeof deferred<WalletLedgerPage>>
  }> = []
  // 页大小 2，便于小数据集验证分页
  const controller = createWalletLedgerPaginationController({
    sessionKey: () => 'TOKEN',
    sessionGeneration: () => 1,
    selectedAssetSymbol: () => undefined,
    selectedDirection: () => 'all',
    selectedDatePreset: () => 'all',
    selectedDateRange: () => ({}),
    pageSize: 2,
    fetchPage: (options) => {
      const pending = deferred<WalletLedgerPage>()
      requests.push({ options, deferred: pending })
      return pending.promise
    },
  })

  // 初始加载失败：状态置 error 并记录 initialError
  const initialFailure = new Error('initial failed')
  const first = controller.loadInitial()
  assert.equal(controller.snapshot().loading, true)
  requests[0].deferred.reject(initialFailure)
  assert.equal(await first, 'error')
  assert.equal(controller.snapshot().initialError, initialFailure)

  // 初始重试成功：写入前两行，偏移推进到 2
  const retryInitial = controller.loadInitial()
  requests[1].deferred.resolve(ledgerPage([1, 2], 0, 4, 2))
  assert.equal(await retryInitial, 'loaded')
  assert.deepEqual(controller.snapshot().entries.map((entry) => entry.id), [1, 2])
  assert.equal(controller.snapshot().nextOffset, 2)

  // 加载更多：进行中再点一次 → ignored（防重复请求）；
  // 本轮追加失败：已有行保留、偏移不变、记录 appendError
  const appendFailure = new Error('append failed')
  const append = controller.loadMore()
  assert.equal(await controller.loadMore(), 'ignored')
  assert.equal(requests[2].options.offset, 2)
  requests[2].deferred.reject(appendFailure)
  assert.equal(await append, 'error')
  assert.deepEqual(controller.snapshot().entries.map((entry) => entry.id), [1, 2])
  assert.equal(controller.snapshot().nextOffset, 2)
  assert.equal(controller.snapshot().appendError, appendFailure)

  // 追加重试：仍按原偏移 2 发请求；返回 [2,3] 中 2 是重复 id 不重复插入；
  // 成功后清空 appendError，且 1+2+3 达到总数 4？未达（总数 4）但服务端报告
  // 第 2 页（页号 1）且 ceil(4/2)=2 页已到尾 → exhausted
  const retryAppend = controller.retryLoadMore()
  assert.equal(requests[3].options.offset, 2)
  requests[3].deferred.resolve(ledgerPage([2, 3], 1, 4, 2))
  assert.equal(await retryAppend, 'loaded')
  assert.deepEqual(controller.snapshot().entries.map((entry) => entry.id), [1, 2, 3])
  assert.equal(controller.snapshot().appendError, null)
  assert.equal(controller.snapshot().exhausted, true)
})

// 资产目录请求隔离乱序响应、会话代际、退出登录与卸载状态：
// 并发请求只认最后一次（旧响应 → stale）；符号去空白、大写、去重并排序；
// 图标按符号映射（重复符号保留先到者）；代际过期 → stale；
// 失败原样回传 error；未登录 → guest 且不发请求；stop() 后一律 stale
test('资产目录请求隔离乱序响应、会话代际、退出登录与卸载状态', async () => {
  // 资产目录条目类型（与 fetchWalletAccounts 返回一致的最小形状）
  type DirectoryItem = { symbol: string; logoUrl?: string }
  let sessionKey = 'TOKEN_A'
  let sessionGeneration = 1
  const requests: Array<ReturnType<typeof deferred<DirectoryItem[]>>> = []
  const lifecycle = createWalletLedgerAssetDirectoryRequestLifecycle({
    sessionKey: () => sessionKey,
    sessionGeneration: () => sessionGeneration,
    fetchDirectory: () => {
      const pending = deferred<DirectoryItem[]>()
      requests.push(pending)
      return pending.promise
    },
  })

  // 并发两次：旧请求先回来 → stale；新请求回来 → loaded，
  // 且符号规范化（去空白、大写、去重、排序），图标按符号映射
  const older = lifecycle.load()
  const latest = lifecycle.load()
  requests[0].resolve([{ symbol: 'OLD', logoUrl: 'https://cdn.example/old.png' }])
  assert.deepEqual(await older, { state: 'stale' })
  requests[1].resolve([
    { symbol: 'usdt', logoUrl: ' https://cdn.example/usdt.png ' },
    { symbol: 'btc', logoUrl: 'https://cdn.example/btc.png' },
    { symbol: 'BTC', logoUrl: 'https://cdn.example/duplicate.png' },
  ])
  assert.deepEqual(await latest, {
    state: 'loaded',
    value: {
      symbols: ['BTC', 'USDT'],
      logoUrls: {
        BTC: 'https://cdn.example/btc.png',
        USDT: 'https://cdn.example/usdt.png',
      },
    },
  })

  // 会话代际推进后的旧响应 → stale
  const priorGeneration = lifecycle.load()
  sessionGeneration = 2
  requests[2].resolve([{ symbol: 'ETH' }])
  assert.deepEqual(await priorGeneration, { state: 'stale' })

  // 请求失败：原样回传错误对象
  const expectedError = new Error('directory failed')
  const failed = lifecycle.load()
  requests[3].reject(expectedError)
  assert.deepEqual(await failed, { state: 'error', error: expectedError })

  // 退出登录：直接 guest，且没有发出新请求（总数仍是 4）
  sessionKey = ''
  assert.deepEqual(await lifecycle.load(), { state: 'guest' })
  assert.equal(requests.length, 4)

  // 重新登录后 stop：在飞响应 → stale；stop 后再 load 也直接 stale
  sessionKey = 'TOKEN_B'
  sessionGeneration = 3
  const stopped = lifecycle.load()
  lifecycle.stop()
  requests[4].resolve([{ symbol: 'SOL' }])
  assert.deepEqual(await stopped, { state: 'stale' })
  assert.deepEqual(await lifecycle.load(), { state: 'stale' })
})

// ---------------------------------------------------------------------------
// 合同 8：页面与 API 源码正则合同（防实现漂移）
// ---------------------------------------------------------------------------
// 页面和 API 源码实施固定四栏导航、三筛选、通栏行与精确小数合同：
// API 层固定走分页账单接口（构造参数 + 映射响应）；
// 页面固定接四类筛选 getter 与分页控制器、固定错误/加载/列表分支次序；
// 页面骨架、三个筛选触发器与弹层、资产目录生命周期；
// 金额/余额用精度格式化（核心层禁止 Number/parseFloat/toFixed 处理金额）；
// 数量固定占位、方向由金额推导、手续费已知判断；行结构与通栏样式；
// 空状态组件规格；页面不得引入分组头或账户筛选
test('页面和 API 源码实施固定四栏导航、三筛选、通栏行与精确小数合同', () => {
  // API 层：GET /wallet/ledger + 参数构造 + 响应映射，三步缺一不可
  assert.match(walletApiSource, /client\.get<BackendWalletLedgerResponse>\(requestUrl\('\/wallet\/ledger'\)/)
  assert.match(walletApiSource, /const params = createWalletLedgerRequestParams\(options\)/)
  assert.match(walletApiSource, /return mapWalletLedgerResponse\(response\.data\)/)

  // 页面：资产目录来源 + 四类筛选 getter 全部接到响应式状态 + 分页控制器
  assert.match(viewSource, /fetchWalletAccounts\(\)/)
  assert.match(viewSource, /selectedAssetSymbol: \(\) => activeAssetSymbol\.value/)
  assert.match(viewSource, /selectedCategory: \(\) => activeCategory\.value/)
  assert.match(viewSource, /selectedDirection: \(\) => activeDirection\.value/)
  assert.match(viewSource, /selectedDatePreset: \(\) => activeDatePreset\.value/)
  assert.match(viewSource, /createWalletLedgerPaginationController\(\{/)
  // 核心层：按服务端已消费行推进偏移与页号收口的两条判定
  assert.match(walletCoreSource, /nextOffset >= result\.page\.totalElements/)
  assert.match(walletCoreSource, /result\.page\.number \+ 1 >= result\.page\.totalPages/)
  // 页面：契约错误识别 + 四个互斥分支（错误/加载/列表，含内联错误）+ 重试加载更多
  assert.match(viewSource, /isWalletLedgerContractError\(reason\)/)
  assert.match(viewSource, /v-if="error && !entries\.length"/)
  assert.match(viewSource, /v-else-if="loading && !entries\.length"/)
  assert.match(viewSource, /v-else-if="entries\.length"/)
  assert.match(viewSource, /v-if="error && entries\.length"/)
  assert.match(viewSource, /@click="load\(false\)"/)

  // 页面骨架：交易记录框架 + 账单 Tab + 返回回退 + Pencil 来源标记
  assert.match(viewSource, /<TransactionRecordsLayout/)
  assert.match(viewSource, /active-tab="ledger"/)
  assert.match(viewSource, /:back-fallback="\{ name: 'assets' \}"/)
  assert.match(viewSource, /data-pencil-source="kcP5D A85if"/)
  // 框架：四栏 Tab 固定顺序 + 四等分列 + 3px 下划线指示条
  assert.match(transactionRecordsLayoutSource, /return \['position-history', 'ledger', 'current-strategy', 'strategy-history'\]/)
  assert.match(transactionRecordsLayoutSource, /grid-template-columns: repeat\(4, minmax\(0, 1fr\)\)/)
  assert.match(transactionRecordsLayoutSource, /\.records-tab i \{[\s\S]*?height: 3px;[\s\S]*?width: 100%;/)
  // 页面不得自带页头（页头由框架提供）
  assert.doesNotMatch(viewSource, /<PageHeader|<header class="ledger-header">/)
  // 三个筛选触发器：ref 与打开动作一一对应（更多触发器用 24px 图标）
  assert.match(viewSource, /ref="assetTrigger"[\s\S]*?openFilterSheet\('asset'\)/)
  assert.match(viewSource, /ref="categoryTrigger"[\s\S]*?categoryLabel\(activeCategory\)[\s\S]*?openFilterSheet\('category'\)/)
  assert.match(viewSource, /ref="moreTrigger"[\s\S]*?<ListFilter :size="24"/)
  // 分类选项来自固定枚举；更多弹层固定"方向组 + 日期组"次序
  assert.match(viewSource, /v-for="category in WALLET_LEDGER_FILTERS"/)
  assert.match(viewSource, /v-else class="ledger-more-filters"[\s\S]*?v-for="direction in WALLET_LEDGER_DIRECTIONS"[\s\S]*?v-for="preset in WALLET_LEDGER_DATE_PRESETS"/)
  // 方向不做独立触发器（收在更多里）
  assert.doesNotMatch(viewSource, /ref="directionTrigger"|openFilterSheet\('direction'\)/)
  // 核心层：分类透传（非 all 才带）+ 默认 all 的兜底读取
  assert.match(walletCoreSource, /\.\.\.\(category !== 'all' \? \{ category \} : \{\}\)/)
  assert.match(walletCoreSource, /input\.selectedCategory\?\.\(\) \?\? 'all'/)

  // 弹层：全局模态工具 + Teleport + 对话框语义 + 24px 更多图标
  assert.match(viewSource, /useModalDialog\(\s*filterSheetOpen,\s*filterDialog/)
  assert.match(viewSource, /<Teleport to="body">/)
  assert.match(viewSource, /role="dialog"/)
  assert.match(viewSource, /aria-modal="true"/)
  assert.match(viewSource, /<ListFilter :size="24"/)
  // 资产目录生命周期 + 图标映射写回 + 行内 30px 资产图标
  assert.match(viewSource, /createWalletLedgerAssetDirectoryRequestLifecycle\(\{/)
  assert.match(viewSource, /walletAssetLogoUrls\.value = result\.value\.logoUrls/)
  assert.match(viewSource, /<AssetMark :symbol="entry\.symbol" :src="entryLogoUrl\(entry\)" :size="30"/)

  // 列表渲染：复合身份作 key；金额带符号 + 精度格式化；余额精度格式化
  assert.match(viewSource, /v-for="entry in entries"/)
  assert.match(viewSource, /:key="walletLedgerEntryIdentity\(entry\)"/)
  assert.match(viewSource, /walletLedgerAmountSign\(entry\.amount\)/)
  assert.match(viewSource, /ledgerDecimal\(entry\.amount, entry\.precisionScale, entry\.symbol\)/)
  assert.match(viewSource, /ledgerDecimal\(entry\.balanceAfter, entry\.precisionScale, entry\.symbol\)/)
  // 总额精确 title + 整行无障碍描述
  assert.match(viewSource, /:title="exactAmountTitle\(entry\)"/)
  assert.match(viewSource, /entryAccessibleDetails\(entry\)/)
  // 页面不得出现旧的 formatAmount 金额路径
  assert.doesNotMatch(viewSource, /formatAmount\(entry\.(?:amount|balanceAfter|fee)/)
  // 核心层禁止用浮点 API 处理金额（必须走十进制文本）
  assert.doesNotMatch(walletCoreSource, /(?:Number|parseFloat)\([^\n]*(?:amount|fee|balanceAfter)/)
  assert.doesNotMatch(walletCoreSource, /(?:amount|fee|balanceAfter)[^\n]*\.toFixed\(/)

  // 数量固定占位（含 title）；方向由金额符号推导；手续费按已知判断
  assert.match(viewSource, /function quantity\(entry: WalletLedgerEntry\): string \{[\s\S]*?void entry[\s\S]*?return '--'/)
  assert.match(viewSource, /function exactQuantityTitle\(entry: WalletLedgerEntry\): string \{[\s\S]*?void entry[\s\S]*?return '--'/)
  assert.match(viewSource, /const direction = walletLedgerDirectionForAmount\(entry\.amount\)/)
  assert.match(viewSource, /return direction \? directionLabel\(direction\) : '--'/)
  assert.match(viewSource, /feeIsKnown\(entry\)[\s\S]*?walletLedgerFeeDebitAmount\(entry\.fee\)/)
  assert.match(viewSource, /:title="exactFeeAmount\(entry\)"/)
  // 交易对位置函数必须保持"只有 return"的形状
  assert.match(viewSource, /function entryPair\(entry: WalletLedgerEntry\): string \{\s*return entryLabel\(entry\)\s*\}/)

  // 行结构：article 列表项 + 头/明细/脚三段
  assert.match(viewSource, /<article[\s\S]*?class="ledger-row"[\s\S]*?role="listitem"/)
  assert.match(viewSource, /<header class="ledger-row__header">/)
  assert.match(viewSource, /<div class="ledger-row__details">/)
  assert.match(viewSource, /<footer class="ledger-row__footer">/)
  // 通栏外框几何：块级列表 + 行的边框/圆角/阴影/间距/最小高/内边距
  assert.match(viewSource, /\.ledger-list \{[\s\S]*?display: block;[\s\S]*?padding: 0;/)
  assert.match(viewSource, /\.ledger-row \{[\s\S]*?border-bottom: 1px solid var\(--wallet-record-row-line\);[\s\S]*?border-radius: 0;[\s\S]*?box-shadow: none;[\s\S]*?gap: 9px;[\s\S]*?min-height: 190px;[\s\S]*?padding: 12px 18px;[\s\S]*?width: 100%;/)
  // 明细与行脚的两栏等宽网格
  assert.match(viewSource, /\.ledger-row__details \{[\s\S]*?display: grid;[\s\S]*?gap: 8px 16px;[\s\S]*?grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);/)
  assert.match(viewSource, /\.ledger-row__footer \{[\s\S]*?display: grid;[\s\S]*?grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);/)
  // 亮色画布与卡片全白；暗色画布与卡片纯黑
  assert.match(viewSource, /--wallet-record-canvas: #ffffff;[\s\S]*?--wallet-record-card: #ffffff;/)
  assert.match(viewSource, /:global\(html\[data-theme='dark'\] \.wallet-ledger-pencil\)[\s\S]*?--wallet-record-canvas: #000000;[\s\S]*?--wallet-record-card: #000000;/)
  // 不得回退到灰白画布或卡片圆角（保持 Pencil 选稿的通栏方角）
  assert.doesNotMatch(viewSource, /--wallet-record-canvas: #f4f6f5|border-radius: 16px/)

  // 空状态组件：30px 收据图标 + 64px 圆盘 + 18px/13px 文案层级
  assert.match(transactionRecordEmptySource, /import \{ ReceiptText \}/)
  assert.match(transactionRecordEmptySource, /<ReceiptText :size="30"/)
  assert.match(transactionRecordEmptySource, /height: 64px;[\s\S]*?width: 64px;/)
  assert.match(transactionRecordEmptySource, /\.records-empty strong \{[\s\S]*?font-size: 18px;[\s\S]*?font-weight: 400;/)
  assert.match(transactionRecordEmptySource, /span:not\(\.records-empty__plate\) \{[\s\S]*?font-size: 13px;[\s\S]*?font-weight: 400;/)
  // 页面不得引入分组头/账户筛选（与 Pencil 选稿保持一致）
  assert.doesNotMatch(viewSource, /ledger-group__header|groupWalletLedgerEntries\(|WALLET_LEDGER_ACCOUNT_FILTERS/)
})

// ---------------------------------------------------------------------------
// 测试夹具（fixtures）
// ---------------------------------------------------------------------------

// 后端蛇形命名的账单条目构造器：给出最小合法载荷，允许按字段覆盖
function backendEntry(overrides: Record<string, unknown> = {}) {
  return {
    id: 1,
    account_type: 'spot',
    symbol: 'USDT',
    change_type: 'deposit_confirm',
    category: 'funding',
    amount: '10.000000000000000000',
    fee: '0.000000000000000000',
    balance_after: '110.000000000000000000',
    precision_scale: 18,
    created_at: 1_786_307_400_000,
    ...overrides,
  }
}

// 最小合法分页元数据（单页单条）
function pageFixture() {
  return {
    number: 0,
    size: 30,
    total_elements: 1,
    total_pages: 1,
  }
}

// 前端账单条目构造器：金额/余额随 id 生成，默认现货账户
function ledgerEntry(
  id: number,
  createdAt: number,
  accountType: WalletLedgerAccountType = 'spot',
): WalletLedgerEntry {
  return {
    id,
    accountType,
    symbol: 'USDT',
    changeType: 'deposit_confirm',
    category: 'funding',
    amount: normalizeDecimalText(String(id)),
    fee: normalizeDecimalText('0'),
    balanceAfter: normalizeDecimalText(String(100 + id)),
    precisionScale: 18,
    createdAt,
  }
}

// 单条结果的页响应构造器：允许覆盖币种/金额/时间/分类
function pageResult(id: number, overrides: {
  symbol?: string
  amount?: string
  createdAt?: number
  category?: WalletLedgerCategory
} = {}): WalletLedgerPage {
  return {
    entries: [{
      ...ledgerEntry(id, overrides.createdAt ?? new Date(2026, 7, 10, id).getTime()),
      symbol: overrides.symbol ?? 'USDT',
      amount: normalizeDecimalText(overrides.amount ?? String(id)),
      category: overrides.category ?? 'funding',
    }],
    page: {
      number: 0,
      size: 30,
      totalElements: 1,
      totalPages: 1,
    },
  }
}

// 多条结果的页响应构造器：总页数由元素总数与页大小推算
function ledgerPage(
  ids: number[],
  number: number,
  totalElements: number,
  size: number,
): WalletLedgerPage {
  return {
    entries: ids.map((id) => ledgerEntry(id, Date.parse('2026-09-01T12:00:00Z'))),
    page: {
      number,
      size,
      totalElements,
      totalPages: Math.max(1, Math.ceil(totalElements / size)),
    },
  }
}

// 按点路径从文案表取值（i18n key 存在性检查用），不存在返回 undefined
function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}

// 手工 Promise 工厂：把 resolve/reject 暴露出来，
// 让测试能精确控制每个请求的响应时机（构造乱序/过期场景）
function deferred<T>(): {
  promise: Promise<T>
  resolve: (value: T) => void
  reject: (error: unknown) => void
} {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
