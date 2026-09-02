import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  formatMarginContractTitle,
  formatTransactionRecordDisplayNo,
  isTerminalMarginPosition,
  marginWalletAssetAmounts,
  marginExecutionQuantity,
  marginPositionAverageExitPrice,
  marginPositionClosedInterest,
  marginPositionClosedRealizedPnl,
  marginPositionClosedQuantity,
  marginPositionOriginalQuantity,
  marginPositionRealizedReturn,
  mergeTransactionOrders,
  normalizeTransactionRecordTab,
  reconstructMarginPositionExposure,
} from '../src/core/transactionRecords.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const layout = read('../src/components/TransactionRecordsLayout.vue')
const empty = read('../src/components/TransactionRecordEmptyState.vue')
const orderRecord = read('../src/components/TransactionOrderRecord.vue')
const positionRecord = read('../src/components/MarginPositionRecord.vue')
const assetRecord = read('../src/components/MarginAssetRecord.vue')
const historyRecord = read('../src/components/MarginHistoryPositionRecord.vue')
const orders = read('../src/views/OrdersView.vue')
const ledger = read('../src/views/WalletLedgerView.vue')
const associated = read('../src/views/PositionAssociatedOrdersView.vue')
const router = read('../src/router/index.ts')
const tradingApi = read('../src/api/trading.ts')
const transactionCore = read('../src/core/transactionRecords.ts')
const transactionRecordsComposable = read('../src/composables/useTransactionRecords.ts')

function marginPosition(overrides: Record<string, unknown> = {}) {
  return {
    id: '91',
    productId: 3,
    pairId: 7,
    marginAssetId: 2,
    direction: 'long',
    marginMode: 'isolated',
    marginAmountText: '10.000000000000000001',
    notionalAmountText: '50.000000000000000005',
    borrowedAmountText: '40.000000000000000004',
    leverage: 5,
    orderType: 'limit',
    entryPriceText: '2.5',
    limitPriceText: '2.4',
    exitPriceText: null,
    realizedPnlText: null,
    interestAmountText: '0.000000000000000001',
    status: 'opened',
    ...overrides,
  } as never
}

function spotOrder(id: string, createdAt: number) {
  return {
    id,
    symbol: 'PAIR',
    side: 'buy',
    orderType: 'limit',
    price: 1,
    quantity: 1,
    filledQuantity: 0,
    priceText: '1' as never,
    averagePriceText: null,
    quantityText: '1' as never,
    filledQuantityText: '0' as never,
    status: 'open',
    createdAt,
  } as never
}

function marginExecution(overrides: Record<string, unknown> = {}) {
  return {
    id: 'execution-1',
    positionId: '91',
    idempotencyKey: 'internal-only',
    closePercentage: 50,
    closeMarginAmountText: '1.000000000000000001',
    closeNotionalAmountText: '10.000000000000000001',
    closeBorrowedAmountText: '9',
    closeInterestAmountText: '0.01',
    exitPriceText: '4',
    realizedPnlText: '0.5',
    settlementAmountText: '1.49',
    fullyClosed: false,
    createdAt: Date.UTC(2026, 7, 19, 16, 20, 52),
    ...overrides,
  } as never
}

test('七个 canonical key 保持稳定，正式页面按状态选择固定四栏窗口', () => {
  for (const key of ['current', 'history', 'positions', 'position-history', 'ledger', 'current-strategy', 'strategy-history']) {
    assert.equal(normalizeTransactionRecordTab(key), key)
  }
  assert.equal(normalizeTransactionRecordTab('spot'), 'current')
  assert.equal(normalizeTransactionRecordTab('margin'), 'current')
  assert.match(layout, /return \['current', 'history', 'positions', 'position-history'\]/)
  assert.match(layout, /return \['current', 'positions', 'position-history', 'ledger'\]/)
  assert.match(layout, /return \['position-history', 'ledger', 'current-strategy', 'strategy-history'\]/)
  assert.match(layout, /v-for="tab in visibleTabs"/)
  assert.doesNotMatch(layout, /v-for="tab in TRANSACTION_RECORD_TABS"/)
  assert.match(layout, /grid-template-columns: repeat\(4, minmax\(0, 1fr\)\)/)
  assert.match(layout, /\.records-tabs \{[\s\S]*?column-gap: 2px;[\s\S]*?padding: 0 8px;/)
  assert.match(layout, /\.records-tabs--ledger-window \{[\s\S]*?padding-inline: 10px;/)
  assert.match(layout, /\.records-tab \{[\s\S]*?font-size: 13px;/)
  assert.match(layout, /\.records-tab span \{[\s\S]*?padding-inline: 0;[\s\S]*?width: 100%;/)
  assert.doesNotMatch(layout, /\.records-tab span \{[\s\S]*?padding-inline: 5px;/)
  assert.match(layout, /\.records-tab span \{[\s\S]*?overflow: hidden;[\s\S]*?text-overflow: ellipsis;/)
  assert.match(layout, /\.records-workspace \{[\s\S]*?overflow-x: clip;/)
  assert.match(layout, /\.records-tab i \{[\s\S]*?height: 3px;[\s\S]*?width: 100%;/)
})

test('正式空态使用 30px ReceiptText、64px 圆底和 18/13 正常字重', () => {
  assert.match(empty, /import \{ ReceiptText \}/)
  assert.match(empty, /<ReceiptText :size="30"/)
  assert.match(empty, /\.records-empty__plate \{[\s\S]*?border-radius: 50%;[\s\S]*?height: 64px;[\s\S]*?width: 64px;/)
  assert.match(empty, /\.records-empty strong \{[\s\S]*?font-size: 18px;[\s\S]*?font-weight: 400;/)
  assert.match(empty, /\.records-empty > span:not\(\.records-empty__plate\) \{[\s\S]*?font-size: 13px;[\s\S]*?font-weight: 400;/)
  assert.doesNotMatch(empty, /ClipboardList|FileSearch/)
})

test('Pencil content-box 记录高度换算为真实可见外框 advance', () => {
  const visibleAdvance = (contentHeight: number, verticalPadding: number): number => (
    contentHeight + verticalPadding + 1 - 0.5
  )
  assert.deepEqual({
    currentOrder: visibleAdvance(209.5, 28),
    historicalSpotOrder: visibleAdvance(149.5, 24),
    historicalMarginOrder: visibleAdvance(189.5, 24),
    currentPosition: visibleAdvance(309.5, 24),
    asset: visibleAdvance(199.5, 28),
    historicalPosition: visibleAdvance(363.5, 34),
    ledger: visibleAdvance(165.5, 24),
    associatedExecution: visibleAdvance(189.5, 28),
  }, {
    currentOrder: 238,
    historicalSpotOrder: 174,
    historicalMarginOrder: 214,
    currentPosition: 334,
    asset: 228,
    historicalPosition: 398,
    ledger: 190,
    associatedExecution: 218,
  })
})

test('订单、仓位、资产、历史仓位和账单保持正式通栏外框几何', () => {
  assert.match(orderRecord, /--current \{[\s\S]*?gap: 12px;[\s\S]*?min-height: 238px;[\s\S]*?padding: 14px 18px;/)
  assert.match(orderRecord, /--history \{[\s\S]*?min-height: 174px;[\s\S]*?padding: 12px 18px;/)
  assert.match(orderRecord, /--history\.transaction-order-record--margin \{[\s\S]*?min-height: 214px;/)
  assert.match(positionRecord, /\.margin-position-record \{[\s\S]*?min-height: 334px;[\s\S]*?padding: 12px 18px;/)
  assert.match(assetRecord, /\.margin-asset-record \{[\s\S]*?min-height: 228px;[\s\S]*?padding: 14px 18px;/)
  assert.match(historyRecord, /\.margin-history-record \{[\s\S]*?min-height: 398px;[\s\S]*?padding: 10px 18px 24px;/)
  assert.match(associated, /\.associated-record \{[^}]*height: 218px;[^}]*min-height: 218px;[^}]*padding: 14px 18px;/)
  assert.match(ledger, /\.ledger-list \{[\s\S]*?display: block;[\s\S]*?padding: 0;/)
  assert.match(ledger, /\.ledger-row \{[\s\S]*?border-bottom: 1px solid var\(--wallet-record-row-line\);[\s\S]*?border-radius: 0;[\s\S]*?gap: 9px;[\s\S]*?min-height: 190px;[\s\S]*?padding: 12px 18px;/)
  assert.doesNotMatch(`${orderRecord}\n${positionRecord}\n${assetRecord}\n${historyRecord}\n${associated}\n${ledger}`, /box-sizing: content-box/)
  assert.doesNotMatch(ledger, /--wallet-record-canvas: #f4f6f5|border-radius: 16px/)
})

test('历史杠杆分享按钮保持 20px 视觉尺寸、44px 命中区和 214px 外框', () => {
  assert.match(orderRecord, /\.transaction-order-record__status-actions button \{[^}]*height: 20px;[^}]*min-height: 20px;[^}]*min-width: 20px;[^}]*width: 20px;/)
  assert.match(orderRecord, /\.transaction-order-record__status-actions button::before \{[^}]*inset: -12px;/)

  const headingHeight = Math.max(26, 20)
  const contentHeight = headingHeight + 26 + 44 + 44 + (3 * 12)
  const visibleOuterHeight = Math.max(214, contentHeight + 24 + 1)
  const shareHitArea = 20 + (2 * 12)

  assert.equal(headingHeight, 26)
  assert.equal(visibleOuterHeight, 214)
  assert.equal(shareHitArea, 44)
})

test('320px 历史杠杆指标标签单行省略且不撑高 214px 外框', () => {
  assert.match(orderRecord, /\.transaction-order-record dt \{[^}]*line-height: 18px;[^}]*min-width: 0;[^}]*overflow: hidden;[^}]*text-overflow: ellipsis;[^}]*white-space: nowrap;/)

  const contentWidth = 320 - (2 * 18)
  const metricCellWidth = (contentWidth - (2 * 12)) / 3
  const metricRowHeight = 18 + 4 + 22
  const contentHeight = 26 + 26 + metricRowHeight + metricRowHeight + (3 * 12)
  const visibleOuterHeight = Math.max(214, contentHeight + 24 + 1)

  assert.equal(metricCellWidth < 87, true)
  assert.equal(metricRowHeight, 44)
  assert.equal(visibleOuterHeight, 214)
})

test('320px 三列仓位与资产标签在本地单元内省略并保留完整 title', () => {
  for (const source of [orderRecord, positionRecord, assetRecord, historyRecord]) {
    assert.match(source, /<dt :title="metric\.label">\{\{ metric\.label \}\}<\/dt>/)
  }
  for (const [source, selector] of [
    [positionRecord, 'margin-position-record__metrics'],
    [assetRecord, 'margin-asset-record__metrics'],
    [historyRecord, 'margin-history-record'],
  ] as const) {
    assert.match(source, new RegExp(`\\.${selector} dt \\{[^}]*min-width: 0;[^}]*overflow: hidden;[^}]*text-overflow: ellipsis;[^}]*white-space: nowrap;`))
  }
})

test('委托类型栏、等待点、标签顺序和操作按钮与 390px Pencil 一致', () => {
  assert.match(orders, /\.orders-type-tabs \{[\s\S]*?gap: 18px;[\s\S]*?height: 44px;[\s\S]*?padding: 4px 16px;/)
  assert.match(orders, /\.orders-type-tabs button \{[\s\S]*?border-radius: 18px;[\s\S]*?font-size: 14px;[\s\S]*?padding: 7px 11px;/)
  assert.match(orders, /button\[aria-pressed='true'\] \{ background: var\(--records-chip\);/)
  assert.match(orders, /\.orders-type-tabs button:disabled \{[^}]*color: var\(--records-muted\);[^}]*opacity: 1;/)
  assert.doesNotMatch(orders, /\.orders-type-tabs button:disabled \{[^}]*opacity: \.42;/)
  assert.match(orders, /class="orders-history-range" type="button" disabled>[\s\S]*?orders\.pastYear[\s\S]*?<ChevronDown :size="16"/)
  assert.match(orders, /\.orders-history-range \{[^}]*color: var\(--records-ink\);[^}]*font-size: 16px;[^}]*font-weight: 600;/)
  assert.match(orders, /\.orders-history-range:disabled \{[^}]*color: var\(--records-ink\);[^}]*opacity: 1;/)
  assert.match(orders, /<button class="orders-filter-icon" type="button"[^>]*aria-haspopup="dialog"[^>]*@click="openFilters">\s*<ListFilter :size="24"/)
  assert.doesNotMatch(orders, /v-if="activeTab !== 'position-history'" class="orders-filter-icon"/)
  assert.match(orderRecord, /__status::before \{[\s\S]*?flex: 0 0 7px;[\s\S]*?height: 7px;[\s\S]*?width: 7px;/)
  assert.match(orderRecord, /__meta time \{[\s\S]*?margin-left: 0;/)
  assert.doesNotMatch(orderRecord, /__meta time \{[\s\S]*?margin-left: auto;/)
  assert.match(orderRecord, /button class="is-modify"/)
  assert.match(orderRecord, /button class="is-cancel"/)
  assert.match(orderRecord, /button\.is-cancel::before \{[\s\S]*?background: var\(--records-chip-negative\);/)
  assert.match(orders, /chips: \[\s*\{ label: order\.orderType[\s\S]*?\{ label: order\.side/)
  assert.match(orders, /chips: \[\s*\{ label: position\.orderType[\s\S]*?associated\.closeLong[\s\S]*?orders\.cross[\s\S]*?position\.leverage/)
  assert.match(orders, /class="orders-current-note">\{\{ t\('orders\.limitOrderExecutionNote'\) \}\}/)
  assert.match(orders, /\.orders-current-note \{[^}]*color: var\(--records-muted\);[^}]*font-size: 13px;[^}]*padding: 20px 18px;[^}]*text-align: center;/)
})

test('margin 时间与 execution DTO 严格保留毫秒和 DecimalText，旧代理缺字段仍可编译', () => {
  assert.match(tradingApi, /if \(value === null \|\| value === undefined \|\| value === ''\) return undefined/)
  assert.match(tradingApi, /timestamp < 1_000_000_000_000 \? timestamp \* 1000 : timestamp/)
  assert.match(tradingApi, /openedAt: normalizeTimestamp\(position\.opened_at, 'margin position opened_at'\)/)
  assert.match(tradingApi, /createdAt: normalizeTimestamp\(position\.created_at, 'margin position created_at'\)/)
  assert.match(tradingApi, /closedAt: normalizeTimestamp\(position\.closed_at, 'margin position closed_at'\)/)
  assert.match(tradingApi, /typeof execution\.fully_closed !== 'boolean'/)
  assert.match(tradingApi, /realizedPnlText: tradingDecimal\(execution\.realized_pnl/)
  assert.match(tradingApi, /createdAt: normalizeTimestamp\(execution\.created_at, 'margin execution created_at'\)!/)
  assert.doesNotMatch(tradingApi, /parseFloat\(|\.toFixed\(/)
})

test('DecimalText 精确重建杠杆数量、历史收益率与终态边界', () => {
  const position = marginPosition({
    status: 'closed',
    entryPriceText: '3',
    marginAmountText: '2',
    notionalAmountText: '20',
    realizedPnlText: '1.5000000000000000015',
  })
  const executions = [
    marginExecution(),
    marginExecution({
      id: 'execution-2',
      closeMarginAmountText: '2.000000000000000002',
      closeNotionalAmountText: '20.000000000000000002',
      fullyClosed: true,
    }),
  ]
  const exposure = reconstructMarginPositionExposure(position, executions)
  assert.equal(exposure.originalNotionalText, '30.000000000000000003')
  assert.equal(exposure.originalMarginText, '3.000000000000000003')
  assert.equal(marginPositionOriginalQuantity(position, executions), '10.000000000000000001')
  assert.equal(marginPositionClosedQuantity(position, executions), '10.000000000000000001')
  assert.equal(marginExecutionQuantity(executions[0]!, '3' as never), '3.333333333333333333')
  assert.equal(marginPositionRealizedReturn(position, executions), '0.5')
  assert.equal(
    marginPositionAverageExitPrice(position, executions),
    '4',
  )
  assert.equal(marginPositionClosedInterest(position, executions), '0.02')
  assert.equal(marginPositionClosedRealizedPnl(position, executions), '1.5000000000000000015')
  assert.equal(isTerminalMarginPosition(position), true)
  const canceled = marginPosition({ status: 'canceled', entryPriceText: '3', notionalAmountText: '20', marginAmountText: '2' })
  const canceledExposure = reconstructMarginPositionExposure(canceled, [])
  assert.equal(isTerminalMarginPosition(canceled), false)
  assert.equal(isTerminalMarginPosition(marginPosition({ status: 'cancelled' })), false)
  assert.equal(canceledExposure.closedNotionalText, '0')
  assert.equal(canceledExposure.closedMarginText, '0')
  assert.equal(marginPositionClosedQuantity(canceled, []), '0')
  assert.equal(isTerminalMarginPosition(marginPosition({ status: 'opened' })), false)
  const legacyTerminal = marginPosition({
    status: 'closed',
    notionalAmountText: '5',
    interestAmountText: '0.02',
    exitPriceText: '10',
    realizedPnlText: '2',
  })
  assert.equal(
    marginPositionAverageExitPrice(legacyTerminal, [marginExecution({
      closeNotionalAmountText: '5',
      exitPriceText: '4',
    })]),
    '7',
  )
  assert.equal(
    marginPositionClosedInterest(legacyTerminal, [marginExecution({ closeInterestAmountText: '0.01' })]),
    '0.03',
  )
  assert.match(transactionCore, /TERMINAL_MARGIN_STATUSES = new Set\(\['closed', 'liquidated'\]\)/)
  assert.match(transactionRecordsComposable, /fetchHistoryOrderPositions\(signal\)/)
  assert.doesNotMatch(transactionRecordsComposable, /fetchTerminalPositions/)
  assert.match(transactionRecordsComposable, /fetchMarginPositions\('canceled', 30, signal\)/)
  assert.match(transactionRecordsComposable, /historyOrderStatuses = new Set\(\['closed', 'liquidated', 'canceled', 'cancelled'\]\)/)
  assert.match(transactionRecordsComposable, /historyOrderStatuses\.has\(position\.status\.trim\(\)\.toLowerCase\(\)\)/)
  assert.doesNotMatch(`${orders}\n${associated}`, /parseFloat\(|\bNumber\(/)
  assert.match(transactionCore, /decimalDivide\(dividend, divisor, RECORD_DIVISION_SCALE\)/)
})

test('现货与杠杆按真实时间稳定混排，合约标题标准化且关联页消费真实接口', () => {
  const rows = mergeTransactionOrders(
    [spotOrder('a', 2_000), spotOrder('b', 1_000)],
    [marginPosition({ id: '7', createdAt: 3_000 })],
  )
  assert.deepEqual(rows.map((row) => row.id), ['margin-7', 'spot-a', 'spot-b'])
  assert.equal(formatMarginContractTitle('btc/usdt', '永续'), 'BTCUSDT 永续')
  assert.equal(formatMarginContractTitle('BTC_USDT Perpetual', 'Perpetual'), 'BTCUSDT Perpetual')
  assert.equal(formatMarginContractTitle('--/--', 'Perpetual'), '--')
  assert.match(orders, /mergeTransactionOrders\(records\.currentSpot\.value, records\.pendingMargin\.value\)/)
  assert.match(orders, /symbol: formatMarginContractTitle\(symbol, t\('orders\.perpetual'\)\)/)
  assert.match(orders, /contractTitle: formatMarginContractTitle\(symbol, t\('orders\.perpetual'\)\)/)
  assert.match(router, /path: '\/orders\/positions\/:id\/associated'/)
  assert.match(router, /showBottomNav: false, depth: 2/)
  assert.match(associated, /fetchMarginPosition\(positionId\.value, controller\.signal\)/)
  assert.match(associated, /fetchMarginPositionExecutions\(positionId\.value, controller\.signal\)/)
  assert.match(associated, /positionRealizedPnl = computed\(\(\) => position\.value\?\.realizedPnlText/)
  assert.match(associated, /closeProfit = computed\(\(\) => position\.value[\s\S]*?marginPositionClosedRealizedPnl/)
  assert.match(associated, /interest = computed\(\(\) => position\.value[\s\S]*?marginPositionClosedInterest/)
  assert.match(associated, /amount: amount\(exposure\.originalNotionalText, marginAsset\.value\)/)
  assert.match(associated, /marginPositionOriginalQuantity\(position\.value, executions\.value\)/)
  assert.match(associated, /marginExecutionQuantity\(execution, position\.value\?\.entryPriceText\)/)
  assert.match(associated, /formatTransactionRecordDisplayNo\('MO'/)
  assert.match(associated, /formatTransactionRecordDisplayNo\('MC'/)
  assert.match(associated, /const contractTitle = computed\(\(\) => position\.value[\s\S]*?formatMarginContractTitle/)
  assert.match(associated, /\.sort\(\(left, right\) => right\.occurredAt - left\.occurredAt[\s\S]*?return \[\.\.\.closing, opening\]/)
  assert.match(associated, /realizedPnlWithAsset[\s\S]*?signed\(positionRealizedPnl\)/)
  assert.match(associated, /closedQuantityWithAsset[\s\S]*?decimal\(closedQuantity\)/)
  assert.match(associated, /t\('associated\.tradingFee'\)[\s\S]*?<dd>--<\/dd>/)
  assert.match(associated, /associated-record__operation[\s\S]*?<time>\{\{ record\.time \}\}<\/time>[\s\S]*?associated-record__amount/)
  assert.match(associated, /associated\.filledQuantity[\s\S]*?associated\.fillPrice[\s\S]*?associated\.fee[\s\S]*?associated\.orderNumber/)
  assert.match(associated, /\.associated-record dl > div \{[^}]*display: flex;[^}]*justify-content: space-between;/)
  assert.doesNotMatch(associated, /\.associated-record dl \{[^}]*grid-template-columns:/)
  assert.match(associated, /class="associated-record__copy"[^>]*@click="copyDisplayId\(record\.displayId\)"[^>]*>\{\{ record\.displayId \}\}<\/button>/)
  assert.doesNotMatch(associated, /\bCopy\b|<Copy|associated-record code/)
  assert.doesNotMatch(associated, /associated-record__copy \{[^}]*;\s*width:|associated-record__copy \{[^}]*flex: 0 0/)
  assert.match(associated, /navigator\.share[\s\S]*?navigator\.clipboard\?\.writeText/)
  assert.match(associated, /role="status" aria-live="polite" aria-atomic="true">\{\{ shareFeedback \}\}/)
  assert.doesNotMatch(associated, /displayId: '--'|execution\.idempotencyKey|decimalDivide\(/)
  assert.match(orders, /\.filter\(isTerminalMarginPosition\)/)
  assert.match(orders, /executionHistoryAvailable = records\.executions\.value\.has\(position\.id\)/)
  assert.match(orders, /averageExitPrice = executionHistoryAvailable[\s\S]*?marginPositionAverageExitPrice/)
  assert.match(orders, /label: metricLabel\(t\('orders\.maximumPosition'\), pair\.base\), value: decimal\(originalQuantity\)/)
  assert.match(orders, /label: t\('orders\.realizedReturn'\), value: percent\(realizedReturn\)/)
  assert.match(orders, /label: metricLabel\(t\('orders\.closedQuantity'\), pair\.base\), value: decimal\(closedQuantity\)/)
  const displayNo = formatTransactionRecordDisplayNo('MC', 'raw-database-id-91', Date.UTC(2026, 7, 31))
  assert.equal(displayNo, formatTransactionRecordDisplayNo('MC', 'raw-database-id-91', Date.UTC(2026, 7, 31)))
  assert.equal(displayNo.startsWith('MC20260831'), true)
  assert.equal(displayNo.includes('raw-database-id-91'), false)
  assert.doesNotMatch(`${orders}\n${ledger}\n${associated}`, /09:41|BTC\/USDT|100\.00/)
})

test('仓位保证金率、方向色、灰色三按钮与资产 3×2 字段使用真实值', () => {
  const wallet = {
    availableText: '10.000000000000000001',
    frozenText: '2.000000000000000002',
    lockedText: '3.000000000000000003',
  } as never
  assert.deepEqual(marginWalletAssetAmounts(wallet), {
    balanceText: '15.000000000000000006',
    equityText: null,
    occupiedText: null,
  })
  assert.deepEqual(marginWalletAssetAmounts(wallet, { equityText: '99.000000000000000009' } as never), {
    balanceText: '15.000000000000000006',
    equityText: '99.000000000000000009',
    occupiedText: null,
  })
  assert.match(orders, /value: percent\(risk\?\.maintenanceMarginRateText, false\)/)
  assert.doesNotMatch(orders, /value: percent\(risk\?\.marginRatioText, false\)/)
  assert.match(positionRecord, /chips: MarginPositionRecordChip\[\]/)
  assert.match(positionRecord, /import \{ ChevronRight \}/)
  assert.match(positionRecord, /<strong :title="contractTitle">\{\{ contractTitle \}\}<\/strong>\s*<ChevronRight :size="20"/)
  assert.match(positionRecord, /\.margin-position-record__heading \{[^}]*align-items: flex-start;[^}]*display: flex;[^}]*justify-content: space-between;/)
  assert.doesNotMatch(positionRecord, /\.margin-position-record__heading \{[^}]*display: grid;/)
  assert.match(positionRecord, /\.margin-position-record__title \{[^}]*flex: 0 0 auto;[^}]*max-width: 70%;/)
  assert.match(positionRecord, /\.margin-position-record__pnl \{[^}]*flex: 1 1 0;/)
  assert.match(positionRecord, /margin-position-record__pnl[\s\S]*?<strong>[\s\S]*?<small v-if="!valuesHidden">\(\{\{ returnRate \}\}\)<\/small>/)
  assert.match(positionRecord, /span\.is-negative \{[\s\S]*?var\(--records-chip-negative\)/)
  assert.doesNotMatch(positionRecord, /is-primary/)
  assert.match(positionRecord, /__actions button \{[\s\S]*?background: var\(--records-button\);/)
  assert.match(assetRecord, /import \{ ChevronRight \}/)
  assert.match(assetRecord, /<ChevronRight :size="20"/)
  assert.doesNotMatch(assetRecord, /equityLabel|equity: string/)
  assert.match(orders, /if \(isQuoteAsset\(wallet\)\)[\s\S]*?currencyEquity[\s\S]*?occupied[\s\S]*?available[\s\S]*?floatingPnl[\s\S]*?balance[\s\S]*?frozen/)
  assert.match(orders, /currencyEquity[\s\S]*?costPrice[\s\S]*?latestPrice[\s\S]*?balance[\s\S]*?floatingPnl[\s\S]*?available/)
  assert.match(orders, /marginWalletAssetAmounts\(wallet, cross\)/)
  assert.doesNotMatch(orders, /function walletOccupied|function walletEquity|positionEquities/)
  assert.match(orders, /currencyEquity'\), value: decimal\(equity\)/)
  assert.match(orders, /occupied'\), value: decimal\(occupied\)/)
  assert.doesNotMatch(orders, /\bamount\(/)
})

test('标签切换与 ledger 兼容重定向保留调用方 symbol query', () => {
  assert.match(layout, /typeof route\.query\.symbol === 'string'/)
  assert.match(layout, /const query = symbol \? \{ tab, symbol \} : \{ tab \}/)
  assert.match(layout, /name: 'wallet-ledger', query: symbol \? \{ symbol \} : undefined/)
  assert.match(orders, /router\.replace\(\{ name: 'wallet-ledger', query: symbol \? \{ symbol \} : undefined \}\)/)
})

test('历史分享、短方向、动态单位和三列收益行遵守 Pencil 03/04/09/10/13/14', () => {
  assert.match(historyRecord, /import \{ ChevronRight, Share2 \}/)
  assert.match(historyRecord, /<strong :title="contractTitle">\{\{ contractTitle \}\}<\/strong>[\s\S]*?<ChevronRight :size="20"/)
  assert.match(historyRecord, /margin-history-record__actions[\s\S]*?\{\{ status \}\}[\s\S]*?<Share2 :size="20"/)
  assert.match(historyRecord, /\.margin-history-record__actions \{[^}]*gap: 12px;/)
  assert.doesNotMatch(historyRecord, /<i aria-hidden="true"|margin-history-record__actions > i/)
  assert.match(historyRecord, /margin-history-record__times dd \{[^}]*color: var\(--records-ink\);[^}]*font-family: var\(--font-geist-mono\), var\(--data-font\);[^}]*font-size: 14px;/)
  assert.match(orderRecord, /import \{ Share2 \}/)
  assert.match(orderRecord, /record\.variant === 'history' && record\.market === 'margin'[\s\S]*?<Share2 :size="20"/)
  assert.match(orderRecord, /transaction-order-record__secondary-metrics[\s\S]*?v-for="metric in record\.secondaryMetrics"[\s\S]*?class="is-placeholder"/)
  assert.match(orderRecord, /\.transaction-order-record__secondary-metrics \{[\s\S]*?grid-template-columns: repeat\(3, minmax\(0, 1fr\)\)/)
  assert.doesNotMatch(orderRecord, /transaction-order-record__profit/)
  assert.match(orders, /label: metricLabel\(t\('orders\.orderQuantity'\), pair\.base\), value: decimal/)
  assert.match(orders, /label: metricLabel\(t\('orders\.filled'\), pair\.base\), value: decimal/)
  assert.match(orders, /label: metricLabel\(t\('orders\.margin'\), marginAsset\), value: decimal\(position\.marginAmountText\)/)
  assert.match(orders, /label: metricLabel\(t\('orders\.realizedPnl'\), marginAsset\), value: signed/)
  assert.match(orders, /label: metricLabel\(t\('orders\.closeProfit'\), productFor\(position\)\?\.marginAssetSymbol \|\| pair\.quote\)/)
  assert.match(orders, /label: t\('orders\.closeProfitRate'\),[\s\S]*?value: percent\(realizedReturn\)/)
  assert.match(orders, /orders\.longShort' : 'orders\.shortShort/)
  assert.match(orders, /position\.status\.trim\(\)\.toLowerCase\(\) === 'closed'[\s\S]*?orders\.statusFullyClosed/)
  assert.match(orders, /function statusTone\(status: string\): 'negative' \| 'warning' \| 'muted'/)
  assert.doesNotMatch(orders, /\['filled', 'completed', 'closed'\][^\n]*return 'positive'/)
  assert.match(orders, /statusTone: statusTone\(position\.status\) === 'negative' \? 'negative' as const : 'muted' as const/)
  assert.equal(zhCN.orders.filled, '已成交量')
  assert.equal(en.orders.filled, 'Filled quantity')
  assert.equal(zhCN.orders.pastYear, '近 1 年')
  assert.equal(en.orders.pastYear, 'Past year')
  assert.equal(zhCN.orders.statusFullyClosed, '全部平仓')
  assert.equal(en.orders.statusFullyClosed, 'Fully closed')
  assert.equal(zhCN.orders.closedAt, '平仓时间')
  assert.match(orders, /navigator\.share[\s\S]*?navigator\.clipboard\?\.writeText/)
  assert.match(orders, /role="status" aria-live="polite" aria-atomic="true">\{\{ shareFeedback \}\}/)
  assert.match(orders, /@share="shareHistoryOrder\(row\)"/)
  assert.match(orders, /@share="shareHistoryPosition\(position\)"/)
})

test('交易记录新增固定文案保持中英文对称', () => {
  for (const key of [
    'positionsAssetsTab', 'typeAdvanced', 'emptyPositionHistory', 'associatedOrders',
    'strategyUnavailable', 'batchActionPartial', 'currencyEquity', 'occupied', 'frozen',
    'pastYear', 'longShort', 'shortShort', 'pnlAmount', 'realizedPnlWithAsset',
    'closedQuantityWithAsset', 'closeProfit', 'closeProfitRate', 'statusFullyClosed',
    'shareHistoryOrder', 'shareHistoryPosition', 'recordShared', 'recordShareFailed',
    'limitOrderExecutionNote',
  ]) {
    assert.equal(typeof zhCN.orders[key as keyof typeof zhCN.orders], 'string')
    assert.equal(typeof en.orders[key as keyof typeof en.orders], 'string')
  }
  for (const key of Object.keys(zhCN.associated)) {
    assert.equal(typeof en.associated[key as keyof typeof en.associated], 'string')
  }
})
