import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createI18n } from 'vue-i18n'
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
import { normalizeDecimalText } from '../src/core/decimal.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const walletApiSource = readFileSync(new URL('../src/api/wallet.ts', import.meta.url), 'utf8')
const walletCoreSource = readFileSync(new URL('../src/core/walletLedger.ts', import.meta.url), 'utf8')
const viewSource = readFileSync(new URL('../src/views/WalletLedgerView.vue', import.meta.url), 'utf8')
const transactionRecordsLayoutSource = readFileSync(new URL('../src/components/TransactionRecordsLayout.vue', import.meta.url), 'utf8')
const transactionRecordEmptySource = readFileSync(new URL('../src/components/TransactionRecordEmptyState.vue', import.meta.url), 'utf8')

test('账单适配器严格消费权威账户、分类、分页、金额、手续费和时间', () => {
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

  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ precision_scale: undefined })],
      page: pageFixture(),
    }),
    /precision_scale/,
  )
  for (const precisionScale of [null, -1, 19, 1.5, '8']) {
    assert.throws(
      () => mapWalletLedgerResponse({
        entries: [backendEntry({ precision_scale: precisionScale })],
        page: pageFixture(),
      }),
      /precision_scale/,
    )
  }
  assert.throws(
    () => mapWalletLedgerResponse({ entries: [], page: undefined }),
    (error) => isWalletLedgerContractError(error) && /page/.test(error.message),
  )
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ account_type: undefined })],
      page: pageFixture(),
    }),
    /account_type/,
  )
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ account_type: 'all' })],
      page: pageFixture(),
    }),
    /account_type/,
  )
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ category: 'trade' })],
      page: pageFixture(),
    }),
    /category/,
  )
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ amount: '1e3' })],
      page: pageFixture(),
    }),
    /amount/,
  )
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ fee: '-0.01' })],
      page: pageFixture(),
    }),
    /fee/,
  )
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry({ created_at: Number.MAX_SAFE_INTEGER })],
      page: pageFixture(),
    }),
    /created_at/,
  )
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [backendEntry(), backendEntry({ id: 2 })],
      page: { ...pageFixture(), size: 1 },
    }),
    /page entries/,
  )
  assert.throws(
    () => mapWalletLedgerResponse({
      entries: [],
      page: { ...pageFixture(), total_elements: 31, total_pages: 1 },
    }),
    /total_pages/,
  )
})

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

  const grouped = groupWalletLedgerEntries(merged, new Date(2026, 7, 10, 18, 0))
  assert.deepEqual(grouped[0].entries.map(walletLedgerEntryIdentity), ['margin:30', 'spot:30'])
})

test('加载更多偏移按服务端已消费行推进，重复 ID 和空页都能确定性收口', () => {
  const duplicatePage = {
    entries: [ledgerEntry(30, Date.now()), ledgerEntry(31, Date.now())],
    page: { number: 1, size: 30, totalElements: 91, totalPages: 4 },
  }
  assert.deepEqual(advanceWalletLedgerPagination(30, duplicatePage), {
    nextOffset: 32,
    exhausted: false,
  })

  const finalPage = {
    entries: [ledgerEntry(91, Date.now())],
    page: { number: 3, size: 30, totalElements: 91, totalPages: 4 },
  }
  assert.deepEqual(advanceWalletLedgerPagination(90, finalPage), {
    nextOffset: 91,
    exhausted: true,
  })

  const emptyPage = {
    entries: [],
    page: { number: 1, size: 30, totalElements: 91, totalPages: 4 },
  }
  assert.deepEqual(advanceWalletLedgerPagination(30, emptyPage), {
    nextOffset: 30,
    exhausted: true,
  })
})

test('本地日历分组按日期和组内时间倒序，并区分今天、昨天和本地日期', () => {
  const now = new Date(2026, 7, 10, 18, 30)
  const entries = [
    ledgerEntry(1, new Date(2026, 7, 9, 8, 0).getTime()),
    ledgerEntry(2, new Date(2026, 7, 10, 9, 0).getTime()),
    ledgerEntry(3, new Date(2026, 7, 8, 20, 0).getTime()),
    ledgerEntry(4, new Date(2026, 7, 10, 17, 0).getTime()),
  ]

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
  assert.equal(
    formatWalletLedgerGroupHeading(groups[0], 'en-US', { today: 'TODAY', yesterday: 'YESTERDAY' }),
    'TODAY',
  )
  assert.equal(
    formatWalletLedgerGroupHeading(groups[1], 'zh-CN', { today: '今天', yesterday: '昨天' }),
    '昨天',
  )
  assert.match(
    formatWalletLedgerGroupHeading(groups[2], 'en-US', { today: 'TODAY', yesterday: 'YESTERDAY' }),
    /2026/,
  )
  assert.match(formatWalletLedgerTime(entries[0].createdAt, 'en-US'), /08:00|8:00/)
})

test('方向与日期筛选、兼容分类以及全部已知变动类型均有双语文案', () => {
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
  assert.deepEqual(WALLET_LEDGER_ACCOUNT_FILTERS, ['all', 'spot', 'margin'])
  assert.deepEqual(WALLET_LEDGER_DIRECTIONS, ['all', 'credit', 'debit'])
  assert.deepEqual(WALLET_LEDGER_DATE_PRESETS, ['all', 'today', 'last7Days', 'last30Days'])
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
  for (const key of translationKeys) {
    assert.equal(typeof resolveMessage(zhCN, key), 'string', `zh-CN missing ${key}`)
    assert.equal(typeof resolveMessage(en, key), 'string', `en missing ${key}`)
  }

  const unknown = walletLedgerTypePresentation(' future_bonus_v2 ')
  assert.deepEqual(unknown, {
    translationKey: 'ledger.typeOther',
    source: 'future_bonus_v2',
  })
  assert.equal(walletLedgerTypePresentation('prediction_fee_refund').source, undefined)
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

  const testI18n = createI18n({
    legacy: false,
    locale: 'en',
    messages: { en },
  })
  assert.equal(testI18n.global.t('ledger.groupCount', 1), '1 record')
  assert.equal(testI18n.global.t('ledger.groupCount', 2), '2 records')
})

test('账单金额仅为正数添加加号，零值保持中性且不带加号', () => {
  assert.equal(walletLedgerAmountSign(normalizeDecimalText('8')), '+')
  assert.equal(walletLedgerAmountSign(normalizeDecimalText('0')), '')
  assert.equal(walletLedgerAmountSign(normalizeDecimalText('-8')), '')
})

test('交易方向来自真实金额符号且非零手续费以 DecimalText 扣除值展示', () => {
  assert.equal(walletLedgerDirectionForAmount(normalizeDecimalText('8')), 'credit')
  assert.equal(walletLedgerDirectionForAmount(normalizeDecimalText('-8')), 'debit')
  assert.equal(walletLedgerDirectionForAmount(normalizeDecimalText('0')), null)
  assert.equal(walletLedgerFeeDebitAmount(normalizeDecimalText('0.25')), '-0.25')
  assert.equal(walletLedgerFeeDebitAmount(normalizeDecimalText('0')), '0')
  assert.equal(walletLedgerFeeDebitAmount(normalizeDecimalText('-0')), '0')
})

test('账单保留权威资产精度，但可见文本使用独立精度并进行十进制舍入', () => {
  assert.equal(WALLET_LEDGER_MAX_FRACTION_DIGITS, 18)
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('0.00125'), 'en-US'), '0.00125')
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('0.000000000000000001'), 'en-US'), '<0.00000001')
  assert.equal(
    formatWalletLedgerDecimal(normalizeDecimalText('12.123456789012345678'), 'en-US'),
    '12.12345679',
  )
  assert.equal(
    formatWalletLedgerDecimal(normalizeDecimalText('9007199254740993.123456789012345678'), 'en-US'),
    '9,007,199,254,740,993.12345679',
  )
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('-0'), 'en-US'), '0')
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('12.340000000000000000'), 'en-US', 2), '12.34')
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('12.999999999999999999'), 'en-US', 2), '13')
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('0.000000000000000001'), 'en-US', 2), '<0.01')
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('1134.331253942506787192'), 'en-US', 18, 'USDT'), '1,134.33')
})

test('本地日期预设冻结为服务端可用的 UTC 区间', () => {
  const now = new Date(2026, 8, 1, 15, 30, 45, 123)
  assert.deepEqual(walletLedgerDateRange('all', now), {})

  const today = walletLedgerDateRange('today', now)
  assert.equal(today.endTime, now.toISOString())
  const todayStart = new Date(today.startTime || '')
  assert.deepEqual(
    [todayStart.getFullYear(), todayStart.getMonth(), todayStart.getDate(), todayStart.getHours()],
    [2026, 8, 1, 0],
  )

  const last7Start = new Date(walletLedgerDateRange('last7Days', now).startTime || '')
  const last30Start = new Date(walletLedgerDateRange('last30Days', now).startTime || '')
  assert.deepEqual([last7Start.getMonth(), last7Start.getDate(), last7Start.getHours()], [7, 26, 0])
  assert.deepEqual([last30Start.getMonth(), last30Start.getDate(), last30Start.getHours()], [7, 3, 0])
})

test('账单查询把带时区日期规范为 MySQL 安全 UTC 文本并拒绝坏边界', () => {
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
  assert.equal(createWalletLedgerRequestParams({ category: 'margin' }).category, 'margin')

  for (const options of [
    { startTime: '2026-02-30T00:00:00Z' },
    { startTime: '2026-09-01 00:00:00' },
    { startTime: '2026-09-02T00:00:00Z', endTime: '2026-09-01T23:59:59Z' },
  ]) {
    assert.throws(() => createWalletLedgerRequestParams(options), /wallet ledger/)
  }
})

test('账单业务分类直接传给服务端并隔离过期分类响应', async () => {
  let selectedCategory: WalletLedgerFilter = 'funding'
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

  const funding = lifecycle.load(0, 30)
  assert.equal(requests[0].options.category, 'funding')
  selectedCategory = 'spot'
  requests[0].deferred.resolve(pageResult(1, { category: 'funding' }))
  assert.deepEqual(await funding, { state: 'stale' })

  const spot = lifecycle.load(0, 30)
  assert.equal(requests[1].options.category, 'spot')
  requests[1].deferred.resolve(pageResult(2, { category: 'spot' }))
  assert.equal((await spot).state, 'loaded')

  const mismatched = lifecycle.load(0, 30)
  requests[2].deferred.resolve(pageResult(3, { category: 'margin' }))
  const result = await mismatched
  assert.equal(result.state, 'error')
  assert.ok(result.state === 'error' && isWalletLedgerContractError(result.error))
})

test('账单请求生命周期阻止旧资产、旧方向、旧日期、旧会话和卸载响应写回', async () => {
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

  assert.deepEqual(await lifecycle.load(0, 30), { state: 'guest' })
  assert.equal(requests.length, 0)

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

  selectedAssetSymbol = 'BTC'
  const btcCredit = lifecycle.load(0, 30)
  requests[0].deferred.resolve(pageResult(1, { symbol: 'USDT', amount: '1', createdAt: Date.parse('2026-09-01T12:00:00Z') }))
  assert.deepEqual(await usdtCredit, { state: 'stale' })
  requests[1].deferred.resolve(pageResult(2, { symbol: 'BTC', amount: '0.1', createdAt: Date.parse('2026-09-01T12:00:00Z') }))
  assert.equal((await btcCredit).state, 'loaded')

  selectedDirection = 'debit'
  const mismatchedDirection = lifecycle.load(0, 30)
  requests[2].deferred.resolve(pageResult(3, { symbol: 'BTC', amount: '1', createdAt: Date.parse('2026-09-01T12:00:00Z') }))
  const directionMismatchResult = await mismatchedDirection
  assert.equal(directionMismatchResult.state, 'error')
  assert.ok(
    directionMismatchResult.state === 'error'
      && isWalletLedgerContractError(directionMismatchResult.error),
  )

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

  sessionKey = 'TOKEN_A'
  sessionGeneration = 3
  const sameTokenOldGeneration = lifecycle.load(0, 30)
  sessionGeneration = 4
  requests[4].deferred.resolve(pageResult(5, { symbol: 'BTC', amount: '-1', createdAt: Date.parse('2026-08-30T12:00:00Z') }))
  assert.deepEqual(await sameTokenOldGeneration, { state: 'stale' })

  const beforeUnmount = lifecycle.load(0, 30)
  lifecycle.stop()
  requests[5].deferred.resolve(pageResult(6, { symbol: 'BTC', amount: '-1', createdAt: Date.parse('2026-08-30T12:00:00Z') }))
  assert.deepEqual(await beforeUnmount, { state: 'stale' })
  assert.deepEqual(await lifecycle.load(0, 30), { state: 'stale' })
})

test('分页控制器隔离初始错误与追加错误并按原偏移重试且保留既有行', async () => {
  const requests: Array<{
    options: WalletLedgerFetchOptions
    deferred: ReturnType<typeof deferred<WalletLedgerPage>>
  }> = []
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

  const initialFailure = new Error('initial failed')
  const first = controller.loadInitial()
  assert.equal(controller.snapshot().loading, true)
  requests[0].deferred.reject(initialFailure)
  assert.equal(await first, 'error')
  assert.equal(controller.snapshot().initialError, initialFailure)

  const retryInitial = controller.loadInitial()
  requests[1].deferred.resolve(ledgerPage([1, 2], 0, 4, 2))
  assert.equal(await retryInitial, 'loaded')
  assert.deepEqual(controller.snapshot().entries.map((entry) => entry.id), [1, 2])
  assert.equal(controller.snapshot().nextOffset, 2)

  const appendFailure = new Error('append failed')
  const append = controller.loadMore()
  assert.equal(await controller.loadMore(), 'ignored')
  assert.equal(requests[2].options.offset, 2)
  requests[2].deferred.reject(appendFailure)
  assert.equal(await append, 'error')
  assert.deepEqual(controller.snapshot().entries.map((entry) => entry.id), [1, 2])
  assert.equal(controller.snapshot().nextOffset, 2)
  assert.equal(controller.snapshot().appendError, appendFailure)

  const retryAppend = controller.retryLoadMore()
  assert.equal(requests[3].options.offset, 2)
  requests[3].deferred.resolve(ledgerPage([2, 3], 1, 4, 2))
  assert.equal(await retryAppend, 'loaded')
  assert.deepEqual(controller.snapshot().entries.map((entry) => entry.id), [1, 2, 3])
  assert.equal(controller.snapshot().appendError, null)
  assert.equal(controller.snapshot().exhausted, true)
})

test('资产目录请求隔离乱序响应、会话代际、退出登录与卸载状态', async () => {
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

  const priorGeneration = lifecycle.load()
  sessionGeneration = 2
  requests[2].resolve([{ symbol: 'ETH' }])
  assert.deepEqual(await priorGeneration, { state: 'stale' })

  const expectedError = new Error('directory failed')
  const failed = lifecycle.load()
  requests[3].reject(expectedError)
  assert.deepEqual(await failed, { state: 'error', error: expectedError })

  sessionKey = ''
  assert.deepEqual(await lifecycle.load(), { state: 'guest' })
  assert.equal(requests.length, 4)

  sessionKey = 'TOKEN_B'
  sessionGeneration = 3
  const stopped = lifecycle.load()
  lifecycle.stop()
  requests[4].resolve([{ symbol: 'SOL' }])
  assert.deepEqual(await stopped, { state: 'stale' })
  assert.deepEqual(await lifecycle.load(), { state: 'stale' })
})

test('页面和 API 源码实施固定四栏导航、三筛选、通栏行与精确小数合同', () => {
  assert.match(walletApiSource, /client\.get<BackendWalletLedgerResponse>\(requestUrl\('\/wallet\/ledger'\)/)
  assert.match(walletApiSource, /const params = createWalletLedgerRequestParams\(options\)/)
  assert.match(walletApiSource, /return mapWalletLedgerResponse\(response\.data\)/)

  assert.match(viewSource, /fetchWalletAccounts\(\)/)
  assert.match(viewSource, /selectedAssetSymbol: \(\) => activeAssetSymbol\.value/)
  assert.match(viewSource, /selectedCategory: \(\) => activeCategory\.value/)
  assert.match(viewSource, /selectedDirection: \(\) => activeDirection\.value/)
  assert.match(viewSource, /selectedDatePreset: \(\) => activeDatePreset\.value/)
  assert.match(viewSource, /createWalletLedgerPaginationController\(\{/)
  assert.match(walletCoreSource, /nextOffset >= result\.page\.totalElements/)
  assert.match(walletCoreSource, /result\.page\.number \+ 1 >= result\.page\.totalPages/)
  assert.match(viewSource, /isWalletLedgerContractError\(reason\)/)
  assert.match(viewSource, /v-if="error && !entries\.length"/)
  assert.match(viewSource, /v-else-if="loading && !entries\.length"/)
  assert.match(viewSource, /v-else-if="entries\.length"/)
  assert.match(viewSource, /v-if="error && entries\.length"/)
  assert.match(viewSource, /@click="load\(false\)"/)

  assert.match(viewSource, /<TransactionRecordsLayout/)
  assert.match(viewSource, /active-tab="ledger"/)
  assert.match(viewSource, /:back-fallback="\{ name: 'assets' \}"/)
  assert.match(viewSource, /data-pencil-source="kcP5D A85if"/)
  assert.match(transactionRecordsLayoutSource, /return \['position-history', 'ledger', 'current-strategy', 'strategy-history'\]/)
  assert.match(transactionRecordsLayoutSource, /grid-template-columns: repeat\(4, minmax\(0, 1fr\)\)/)
  assert.match(transactionRecordsLayoutSource, /\.records-tab i \{[\s\S]*?height: 3px;[\s\S]*?width: 100%;/)
  assert.doesNotMatch(viewSource, /<PageHeader|<header class="ledger-header">/)
  assert.match(viewSource, /ref="assetTrigger"[\s\S]*?openFilterSheet\('asset'\)/)
  assert.match(viewSource, /ref="categoryTrigger"[\s\S]*?categoryLabel\(activeCategory\)[\s\S]*?openFilterSheet\('category'\)/)
  assert.match(viewSource, /ref="moreTrigger"[\s\S]*?<ListFilter :size="24"/)
  assert.match(viewSource, /v-for="category in WALLET_LEDGER_FILTERS"/)
  assert.match(viewSource, /v-else class="ledger-more-filters"[\s\S]*?v-for="direction in WALLET_LEDGER_DIRECTIONS"[\s\S]*?v-for="preset in WALLET_LEDGER_DATE_PRESETS"/)
  assert.doesNotMatch(viewSource, /ref="directionTrigger"|openFilterSheet\('direction'\)/)
  assert.match(walletCoreSource, /\.\.\.\(category !== 'all' \? \{ category \} : \{\}\)/)
  assert.match(walletCoreSource, /input\.selectedCategory\?\.\(\) \?\? 'all'/)

  assert.match(viewSource, /useModalDialog\(\s*filterSheetOpen,\s*filterDialog/)
  assert.match(viewSource, /<Teleport to="body">/)
  assert.match(viewSource, /role="dialog"/)
  assert.match(viewSource, /aria-modal="true"/)
  assert.match(viewSource, /<ListFilter :size="24"/)
  assert.match(viewSource, /createWalletLedgerAssetDirectoryRequestLifecycle\(\{/)
  assert.match(viewSource, /walletAssetLogoUrls\.value = result\.value\.logoUrls/)
  assert.match(viewSource, /<AssetMark :symbol="entry\.symbol" :src="entryLogoUrl\(entry\)" :size="30"/)

  assert.match(viewSource, /v-for="entry in entries"/)
  assert.match(viewSource, /:key="walletLedgerEntryIdentity\(entry\)"/)
  assert.match(viewSource, /walletLedgerAmountSign\(entry\.amount\)/)
  assert.match(viewSource, /ledgerDecimal\(entry\.amount, entry\.precisionScale, entry\.symbol\)/)
  assert.match(viewSource, /ledgerDecimal\(entry\.balanceAfter, entry\.precisionScale, entry\.symbol\)/)
  assert.match(viewSource, /:title="exactAmountTitle\(entry\)"/)
  assert.match(viewSource, /entryAccessibleDetails\(entry\)/)
  assert.doesNotMatch(viewSource, /formatAmount\(entry\.(?:amount|balanceAfter|fee)/)
  assert.doesNotMatch(walletCoreSource, /(?:Number|parseFloat)\([^\n]*(?:amount|fee|balanceAfter)/)
  assert.doesNotMatch(walletCoreSource, /(?:amount|fee|balanceAfter)[^\n]*\.toFixed\(/)

  assert.match(viewSource, /function quantity\(entry: WalletLedgerEntry\): string \{[\s\S]*?void entry[\s\S]*?return '--'/)
  assert.match(viewSource, /function exactQuantityTitle\(entry: WalletLedgerEntry\): string \{[\s\S]*?void entry[\s\S]*?return '--'/)
  assert.match(viewSource, /const direction = walletLedgerDirectionForAmount\(entry\.amount\)/)
  assert.match(viewSource, /return direction \? directionLabel\(direction\) : '--'/)
  assert.match(viewSource, /feeIsKnown\(entry\)[\s\S]*?walletLedgerFeeDebitAmount\(entry\.fee\)/)
  assert.match(viewSource, /:title="exactFeeAmount\(entry\)"/)
  assert.match(viewSource, /function entryPair\(entry: WalletLedgerEntry\): string \{\s*return entryLabel\(entry\)\s*\}/)

  assert.match(viewSource, /<article[\s\S]*?class="ledger-row"[\s\S]*?role="listitem"/)
  assert.match(viewSource, /<header class="ledger-row__header">/)
  assert.match(viewSource, /<div class="ledger-row__details">/)
  assert.match(viewSource, /<footer class="ledger-row__footer">/)
  assert.match(viewSource, /\.ledger-list \{[\s\S]*?display: block;[\s\S]*?padding: 0;/)
  assert.match(viewSource, /\.ledger-row \{[\s\S]*?border-bottom: 1px solid var\(--wallet-record-row-line\);[\s\S]*?border-radius: 0;[\s\S]*?box-shadow: none;[\s\S]*?gap: 9px;[\s\S]*?min-height: 190px;[\s\S]*?padding: 12px 18px;[\s\S]*?width: 100%;/)
  assert.match(viewSource, /\.ledger-row__details \{[\s\S]*?display: grid;[\s\S]*?gap: 8px 16px;[\s\S]*?grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);/)
  assert.match(viewSource, /\.ledger-row__footer \{[\s\S]*?display: grid;[\s\S]*?grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);/)
  assert.match(viewSource, /--wallet-record-canvas: #ffffff;[\s\S]*?--wallet-record-card: #ffffff;/)
  assert.match(viewSource, /:global\(html\[data-theme='dark'\] \.wallet-ledger-pencil\)[\s\S]*?--wallet-record-canvas: #000000;[\s\S]*?--wallet-record-card: #000000;/)
  assert.doesNotMatch(viewSource, /--wallet-record-canvas: #f4f6f5|border-radius: 16px/)

  assert.match(transactionRecordEmptySource, /import \{ ReceiptText \}/)
  assert.match(transactionRecordEmptySource, /<ReceiptText :size="30"/)
  assert.match(transactionRecordEmptySource, /height: 64px;[\s\S]*?width: 64px;/)
  assert.match(transactionRecordEmptySource, /\.records-empty strong \{[\s\S]*?font-size: 18px;[\s\S]*?font-weight: 400;/)
  assert.match(transactionRecordEmptySource, /span:not\(\.records-empty__plate\) \{[\s\S]*?font-size: 13px;[\s\S]*?font-weight: 400;/)
  assert.doesNotMatch(viewSource, /ledger-group__header|groupWalletLedgerEntries\(|WALLET_LEDGER_ACCOUNT_FILTERS/)
})

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

function pageFixture() {
  return {
    number: 0,
    size: 30,
    total_elements: 1,
    total_pages: 1,
  }
}

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

function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}

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
