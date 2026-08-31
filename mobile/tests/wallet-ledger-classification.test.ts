import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createI18n } from 'vue-i18n'
import {
  advanceWalletLedgerPagination,
  createWalletLedgerRequestLifecycle,
  formatWalletLedgerDecimal,
  formatWalletLedgerGroupHeading,
  formatWalletLedgerTime,
  groupWalletLedgerEntries,
  isWalletLedgerContractError,
  mapWalletLedgerResponse,
  mergeWalletLedgerEntries,
  WALLET_LEDGER_ACCOUNT_FILTERS,
  WALLET_LEDGER_FILTERS,
  WALLET_LEDGER_KNOWN_CHANGE_TYPES,
  WALLET_LEDGER_MAX_FRACTION_DIGITS,
  walletLedgerAmountSign,
  walletLedgerAccountTranslationKey,
  walletLedgerCategoryTranslationKey,
  walletLedgerEntryIdentity,
  walletLedgerTypePresentation,
  type WalletLedgerAccountFilter,
  type WalletLedgerAccountType,
  type WalletLedgerEntry,
  type WalletLedgerFilter,
  type WalletLedgerPage,
} from '../src/core/walletLedger.ts'
import { normalizeDecimalText } from '../src/core/decimal.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const walletApiSource = readFileSync(new URL('../src/api/wallet.ts', import.meta.url), 'utf8')
const walletCoreSource = readFileSync(new URL('../src/core/walletLedger.ts', import.meta.url), 'utf8')
const viewSource = readFileSync(new URL('../src/views/WalletLedgerView.vue', import.meta.url), 'utf8')

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

test('业务与账户筛选项、全部已知变动类型在中英文中都有对称文案', () => {
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

test('账单金额、余额和手续费保留最多 18 位精度且不经过 IEEE-754', () => {
  assert.equal(WALLET_LEDGER_MAX_FRACTION_DIGITS, 18)
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('0.00125'), 'en-US'), '0.00125')
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('0.000000000000000001'), 'en-US'), '0.000000000000000001')
  assert.equal(
    formatWalletLedgerDecimal(normalizeDecimalText('12.123456789012345678'), 'en-US'),
    '12.123456789012345678',
  )
  assert.equal(
    formatWalletLedgerDecimal(normalizeDecimalText('9007199254740993.123456789012345678'), 'en-US'),
    '9,007,199,254,740,993.123456789012345678',
  )
  assert.equal(formatWalletLedgerDecimal(normalizeDecimalText('-0'), 'en-US'), '0')
})

test('账单请求生命周期阻止旧账户、旧分类、旧会话和卸载后的响应写回', async () => {
  let sessionKey = ''
  let selectedFilter: WalletLedgerFilter = 'all'
  let selectedAccountType: WalletLedgerAccountFilter = 'all'
  const requests: Array<{
    options: { limit: number; offset: number; category?: string; accountType: WalletLedgerAccountFilter }
    deferred: ReturnType<typeof deferred<WalletLedgerPage>>
  }> = []
  const lifecycle = createWalletLedgerRequestLifecycle({
    sessionKey: () => sessionKey,
    selectedFilter: () => selectedFilter,
    selectedAccountType: () => selectedAccountType,
    fetchPage: (options) => {
      const pending = deferred<WalletLedgerPage>()
      requests.push({ options, deferred: pending })
      return pending.promise
    },
  })

  assert.deepEqual(await lifecycle.load(0, 30), { state: 'guest' })
  assert.equal(requests.length, 0)

  sessionKey = 'TOKEN_A'
  selectedFilter = 'funding'
  const funding = lifecycle.load(0, 30)
  assert.deepEqual(requests[0].options, {
    limit: 30,
    offset: 0,
    category: 'funding',
    accountType: 'all',
  })

  selectedFilter = 'spot'
  const spot = lifecycle.load(0, 30)
  requests[0].deferred.resolve(pageResult(1, 'funding'))
  assert.deepEqual(await funding, { state: 'stale' })
  requests[1].deferred.resolve(pageResult(2, 'spot'))
  assert.equal((await spot).state, 'loaded')

  selectedAccountType = 'spot'
  const spotAccount = lifecycle.load(0, 30)
  selectedAccountType = 'margin'
  const marginAccount = lifecycle.load(0, 30)
  requests[2].deferred.resolve(pageResult(3, 'spot', 'spot'))
  assert.deepEqual(await spotAccount, { state: 'stale' })
  requests[3].deferred.resolve(pageResult(4, 'spot', 'margin'))
  assert.equal((await marginAccount).state, 'loaded')

  selectedFilter = 'funding'
  const mismatched = lifecycle.load(0, 30)
  requests[4].deferred.resolve(pageResult(5, 'spot', 'margin'))
  const mismatchResult = await mismatched
  assert.equal(mismatchResult.state, 'error')
  assert.ok(mismatchResult.state === 'error' && isWalletLedgerContractError(mismatchResult.error))

  selectedFilter = 'all'
  selectedAccountType = 'spot'
  const mismatchedAccount = lifecycle.load(0, 30)
  requests[5].deferred.resolve(pageResult(6, 'other', 'margin'))
  const accountMismatchResult = await mismatchedAccount
  assert.equal(accountMismatchResult.state, 'error')
  assert.ok(
    accountMismatchResult.state === 'error'
      && isWalletLedgerContractError(accountMismatchResult.error),
  )

  selectedFilter = 'all'
  selectedAccountType = 'all'
  const all = lifecycle.load(30, 30)
  assert.deepEqual(requests[6].options, {
    limit: 30,
    offset: 30,
    category: undefined,
    accountType: 'all',
  })
  sessionKey = 'TOKEN_B'
  requests[6].deferred.resolve(pageResult(7, 'other'))
  assert.deepEqual(await all, { state: 'stale' })

  const beforeUnmount = lifecycle.load(0, 30)
  lifecycle.stop()
  requests[7].deferred.resolve(pageResult(8, 'other'))
  assert.deepEqual(await beforeUnmount, { state: 'stale' })
  assert.deepEqual(await lifecycle.load(0, 30), { state: 'stale' })
})

test('页面和 API 源码保留分页、状态分支、手续费、原始未知类型与窄屏合同', () => {
  assert.match(walletApiSource, /client\.get<BackendWalletLedgerResponse>\(requestUrl\('\/wallet\/ledger'\)/)
  assert.match(walletApiSource, /category: options\.category/)
  assert.match(walletApiSource, /account_type: accountType/)
  assert.match(walletApiSource, /change_type: changeType \|\| undefined/)
  assert.match(walletApiSource, /return mapWalletLedgerResponse\(response\.data\)/)

  assert.match(viewSource, /v-for="filter in WALLET_LEDGER_FILTERS"/)
  assert.match(viewSource, /v-for="accountType in WALLET_LEDGER_ACCOUNT_FILTERS"/)
  assert.match(viewSource, /selectedAccountType: \(\) => activeAccountType\.value/)
  assert.match(viewSource, /groupWalletLedgerEntries\(entries\.value\)/)
  assert.match(viewSource, /t\('ledger\.groupCount', group\.entries\.length\)/)
  assert.match(viewSource, /categoryLabel\(entry\.category\)/)
  assert.match(viewSource, /accountLabel\(entry\.accountType\)/)
  assert.match(viewSource, /:key="walletLedgerEntryIdentity\(entry\)"/)
  assert.match(viewSource, /mergeWalletLedgerEntries\(entries\.value, result\.value\.entries\)/)
  assert.match(viewSource, /v-if="decimalSign\(entry\.fee\) > 0"/)
  assert.match(viewSource, /t\('ledger\.sourceType', \{ type: entrySource\(entry\) \}\)/)
  assert.match(viewSource, /advanceWalletLedgerPagination\(offset, result\.value\)/)
  assert.match(walletCoreSource, /nextOffset >= result\.page\.totalElements/)
  assert.match(walletCoreSource, /result\.page\.number \+ 1 >= result\.page\.totalPages/)
  assert.match(viewSource, /isWalletLedgerContractError\(result\.error\)/)
  assert.match(viewSource, /walletLedgerAmountSign\(entry\.amount\)/)
  assert.match(viewSource, /ledgerDecimal\(entry\.amount, entry\.precisionScale\)/)
  assert.match(viewSource, /ledgerDecimal\(entry\.balanceAfter, entry\.precisionScale\)/)
  assert.match(viewSource, /ledgerDecimal\(entry\.fee, entry\.precisionScale\)/)
  assert.doesNotMatch(viewSource, /formatAmount\(entry\.(?:amount|balanceAfter|fee)/)
  assert.match(viewSource, /v-if="error && !entries\.length"/)
  assert.match(viewSource, /v-else-if="loading && !entries\.length"/)
  assert.match(viewSource, /v-else-if="groupedEntries\.length"/)
  assert.match(viewSource, /v-if="error && entries\.length"/)
  assert.match(viewSource, /@click="load\(false\)"/)
  assert.match(viewSource, /\.ledger-filter button \{[\s\S]*?height: 44px;[\s\S]*?min-height: 44px;/)
  assert.match(viewSource, /\.ledger-filter \{[\s\S]*?overflow-x: auto;/)
  assert.match(viewSource, /\.ledger-page \{[\s\S]*?min-width: 0;[\s\S]*?overflow-x: hidden;/)
  assert.match(viewSource, /@media \(max-width: 340px\)/)
  assert.doesNotMatch(viewSource, /spot_trade_settlement.*activeFilter|margin_position_open.*activeFilter/)
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

function pageResult(
  id: number,
  category: WalletLedgerEntry['category'],
  accountType: WalletLedgerAccountType = 'spot',
): WalletLedgerPage {
  return {
    entries: [{ ...ledgerEntry(id, new Date(2026, 7, 10, id).getTime(), accountType), category }],
    page: {
      number: 0,
      size: 30,
      totalElements: 1,
      totalPages: 1,
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
