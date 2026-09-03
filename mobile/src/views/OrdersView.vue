<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import { Check, ChevronDown, CircleAlert, Eye, EyeOff, ListFilter, LoaderCircle, RefreshCw, X } from 'lucide-vue-next'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import MarginAssetRecord from '@/components/MarginAssetRecord.vue'
import MarginCloseSheet from '@/components/MarginCloseSheet.vue'
import MarginHistoryPositionRecord from '@/components/MarginHistoryPositionRecord.vue'
import MarginPositionRecord from '@/components/MarginPositionRecord.vue'
import TransactionOrderRecord from '@/components/TransactionOrderRecord.vue'
import TransactionRecordEmptyState from '@/components/TransactionRecordEmptyState.vue'
import TransactionRecordsLayout from '@/components/TransactionRecordsLayout.vue'
import { apiErrorMessage } from '@/api/client'
import {
  cancelAllMarginPositions,
  cancelAllSpotOrders,
  cancelMarginPosition,
  cancelSpotOrder,
  closeAllMarginPositions,
  closeMarginPosition,
  createMarginCloseIdempotencyKey,
  type MarginPosition,
  type MarginWalletAccount,
} from '@/api/trading'
import { useTransactionRecords } from '@/composables/useTransactionRecords'
import {
  decimalMultiply,
  decimalSign,
  normalizeDecimalText,
  type DecimalText,
} from '@/core/decimal'
import { orderStatusPresentation } from '@/core/financialEnumPresentation'
import { resolveMarginPositionLiveProjection } from '@/core/marginRiskMetrics'
import { useModalDialog } from '@/core/modalDialog'
import {
  filterTransactionOrders,
  formatMarginContractTitle,
  formatRecordDecimal,
  formatRecordSignedDecimal,
  isTerminalMarginPosition,
  marginWalletAssetAmounts,
  marginPositionAverageExitPrice,
  marginPositionClosedQuantity,
  marginPositionOriginalQuantity,
  marginPositionRealizedReturn,
  mergeTransactionOrders,
  normalizeTransactionRecordTab,
  positionOccurredAt,
  sumDecimalText,
  type TransactionOrderFilter,
  type TransactionOrderRow,
} from '@/core/transactionRecords'
import { currentIntlLocale } from '@/i18n'
import { useMarketStore } from '@/stores/market'
import { useSessionStore } from '@/stores/session'

type PendingAction =
  | { kind: 'spot'; id: string }
  | { kind: 'margin'; id: string }
  | { kind: 'spot-all' }
  | { kind: 'margin-cancel-all' }
  | { kind: 'margin-close-all' }

const HUNDRED = normalizeDecimalText('100')
const ZERO = normalizeDecimalText('0')
const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const market = useMarketStore()
const { t } = useI18n()
const records = useTransactionRecords()
const activeTab = computed(() => normalizeTransactionRecordTab(route.query.tab))
const orderFilter = ref<TransactionOrderFilter>('all')
const typeFilter = ref<'all' | 'standard'>('all')
const valuesHidden = ref(false)
const filterOpen = ref(false)
const filterDialog = ref<HTMLElement | null>(null)
const filterTrigger = ref<HTMLElement | null>(null)
const pendingAction = ref<PendingAction | null>(null)
const confirmDialog = ref<HTMLElement | null>(null)
const actionId = ref('')
const feedback = ref('')
const actionError = ref('')
const closePositionId = ref<string | null>(null)
const closeReturnFocus = ref<HTMLElement | null>(null)
const closeError = ref('')
const closeAttempt = ref<{ positionId: string; percentage: number; idempotencyKey: string } | null>(null)
const shareFeedback = ref('')
let shareFeedbackTimer: ReturnType<typeof setTimeout> | undefined

const filterSheetVisible = computed(() => filterOpen.value)
const confirmVisible = computed(() => pendingAction.value !== null)
const { trapFocus: trapFilterFocus, setReturnFocus: setFilterReturnFocus } = useModalDialog(
  filterSheetVisible,
  filterDialog,
  '[data-dialog-initial]',
)
const { trapFocus: trapConfirmFocus, setReturnFocus: setConfirmReturnFocus } = useModalDialog(
  confirmVisible,
  confirmDialog,
  '[data-dialog-initial]',
)

const orderTypeTabs = computed(() => [
  { value: 'all' as const, label: t('orders.typeAll'), disabled: false },
  { value: 'standard' as const, label: t('orders.typeStandard'), disabled: false },
  { value: 'advanced' as const, label: t('orders.typeAdvanced'), disabled: true },
  { value: 'tp-sl' as const, label: t('orders.typeTpSl'), disabled: true },
])

const currentRows = computed(() => filterTransactionOrders(
  mergeTransactionOrders(records.currentSpot.value, records.pendingMargin.value),
  orderFilter.value,
))
const historyRows = computed(() => filterTransactionOrders(
  mergeTransactionOrders(records.historySpot.value, records.historyMargin.value),
  orderFilter.value,
))
const historyPositions = computed(() => [...records.historyMargin.value]
  .filter(isTerminalMarginPosition)
  .sort((left, right) => positionOccurredAt(right, true) - positionOccurredAt(left, true)))
const closePosition = computed(() => records.openPositions.value.find((position) => position.id === closePositionId.value) || null)
const closeRisk = computed(() => closePosition.value ? records.risks.value.get(closePosition.value.id) : undefined)
const loadingError = computed(() => records.error.value
  ? apiErrorMessage(records.error.value, t('orders.loadFailed'))
  : '')
const visibleError = computed(() => actionError.value || loadingError.value)

function productFor(position: MarginPosition) {
  return records.products.value.find((product) => product.id === position.productId || product.pairId === position.pairId)
}

function symbolFor(position: MarginPosition): string {
  return productFor(position)?.symbol
    || records.pairs.value.find((pair) => pair.id === position.pairId)?.symbol
    || t('orders.contractNumber', { id: position.productId })
}

function displaySpotSymbol(symbol: string): string {
  const trimmed = symbol.trim()
  if (/^\d+$/.test(trimmed)) {
    return records.pairs.value.find((pair) => String(pair.id) === trimmed)?.symbol || trimmed
  }
  return trimmed.replace(/[_-]/g, '/')
}

function splitPair(symbol: string): { base: string; quote: string } {
  const [base = '', quote = ''] = symbol.replace(/[_-]/g, '/').split('/')
  return { base, quote }
}

function dateTime(timestamp?: number, compact = false): string {
  if (!timestamp) return '--'
  const options: Intl.DateTimeFormatOptions = {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }
  if (!compact) options.year = 'numeric'
  return new Intl.DateTimeFormat(currentIntlLocale(), options).format(timestamp)
}

function decimal(value: DecimalText | null | undefined, digits = 8): string {
  return formatRecordDecimal(value, currentIntlLocale(), digits)
}

function signed(value: DecimalText | null | undefined, digits = 8): string {
  return formatRecordSignedDecimal(value, currentIntlLocale(), digits)
}

function percent(value: DecimalText | null | undefined, signedValue = true): string {
  if (!value) return '--'
  const rate = decimalMultiply(value, HUNDRED)
  return `${signedValue ? signed(rate, 2) : decimal(rate, 2)}%`
}

function metricLabel(label: string, asset?: string): string {
  const unit = asset?.trim()
  return unit ? `${label} (${unit})` : label
}

function statusLabel(status: string): string {
  const presentation = orderStatusPresentation(status)
  return t(presentation.translationKey, { source: presentation.source || status || '--' })
}

function statusTone(status: string): 'negative' | 'warning' | 'muted' {
  const normalized = status.toLowerCase()
  if (['cancelled', 'canceled', 'rejected', 'liquidated'].includes(normalized)) return 'negative'
  if (['submitted', 'pending', 'open', 'opened', 'partially_filled', 'trading'].includes(normalized)) return 'warning'
  return 'muted'
}

function pnlTone(value: DecimalText | null | undefined): 'positive' | 'negative' | 'muted' {
  if (!value) return 'muted'
  const sign = decimalSign(value)
  return sign > 0 ? 'positive' : sign < 0 ? 'negative' : 'muted'
}

function orderRecord(row: TransactionOrderRow, history: boolean) {
  if (row.kind === 'spot') {
    const order = row.order
    const symbol = displaySpotSymbol(order.symbol)
    const pair = splitPair(symbol)
    const sideTone = order.side === 'buy' ? 'positive' as const : 'negative' as const
    return {
      id: row.id,
      market: 'spot' as const,
      variant: history ? 'history' as const : 'current' as const,
      symbol,
      status: history ? statusLabel(order.status) : t('orders.waitingFill'),
      statusTone: history ? statusTone(order.status) : 'warning' as const,
      chips: [
        { label: order.orderType === 'market' ? t('trade.marketOrderShort') : t('trade.limitOrderShort'), tone: sideTone },
        { label: order.side === 'buy' ? t('orders.buy') : t('orders.sell'), tone: sideTone },
      ],
      time: dateTime(order.createdAt, true),
      metrics: history
        ? [
            { label: metricLabel(t('orders.orderQuantity'), pair.base), value: decimal(order.quantityText) },
            { label: metricLabel(t('orders.filled'), pair.base), value: decimal(order.filledQuantityText) },
            { label: t('orders.averagePrice'), value: decimal(order.averagePriceText) },
          ]
        : [
            { label: t('orders.orderPrice'), value: order.orderType === 'market' ? t('orders.marketPrice') : decimal(order.priceText) },
            { label: metricLabel(t('orders.orderQuantity'), pair.base), value: decimal(order.quantityText) },
            { label: metricLabel(t('orders.filled'), pair.base), value: decimal(order.filledQuantityText) },
          ],
    }
  }

  const position = row.position
  const symbol = symbolFor(position)
  const pair = splitPair(symbol)
  const executionHistoryAvailable = records.executions.value.has(position.id)
  const executions = records.executions.value.get(position.id) || []
  const normalizedStatus = position.status.trim().toLowerCase()
  const closingOrder = history && ['closed', 'liquidated'].includes(normalizedStatus)
  const operationTone = position.direction === 'long'
    ? closingOrder ? 'negative' as const : 'positive' as const
    : closingOrder ? 'positive' as const : 'negative' as const
  const originalQuantity = closingOrder && !executionHistoryAvailable
    ? null
    : marginPositionOriginalQuantity(position, executions)
  const filledQuantity = closingOrder
    ? executionHistoryAvailable ? marginPositionClosedQuantity(position, executions) : null
    : ZERO
  const realizedReturn = closingOrder && !executionHistoryAvailable
    ? null
    : marginPositionRealizedReturn(position, executions)
  return {
    id: row.id,
    market: 'margin' as const,
    variant: history ? 'history' as const : 'current' as const,
    symbol: formatMarginContractTitle(symbol, t('orders.perpetual')),
    status: history ? statusLabel(position.status) : t('orders.waitingFill'),
    statusTone: history ? statusTone(position.status) : 'warning' as const,
    chips: [
      { label: position.orderType === 'market' ? t('trade.marketOrderShort') : t('trade.limitOrderShort'), tone: operationTone },
      {
        label: t(closingOrder
          ? position.direction === 'long' ? 'associated.closeLong' : 'associated.closeShort'
          : position.direction === 'long' ? 'associated.openLong' : 'associated.openShort'),
        tone: operationTone,
      },
      { label: t(position.marginMode === 'cross' ? 'orders.cross' : 'orders.isolated') },
      { label: `${position.leverage}x` },
    ],
    time: dateTime(position.createdAt || position.openedAt, true),
    metrics: history
      ? [
          { label: metricLabel(t('orders.orderQuantity'), pair.base), value: decimal(originalQuantity) },
          { label: metricLabel(t('orders.filled'), pair.base), value: decimal(filledQuantity) },
          { label: t('orders.averagePrice'), value: decimal(position.entryPriceText) },
        ]
      : [
          { label: t('orders.orderPrice'), value: position.orderType === 'market' ? t('orders.marketPrice') : decimal(position.limitPriceText) },
          { label: metricLabel(t('orders.orderQuantity'), pair.base), value: decimal(originalQuantity) },
          { label: metricLabel(t('orders.filled'), pair.base), value: decimal(ZERO) },
        ],
    secondaryMetrics: history && closingOrder ? [
      {
        label: metricLabel(t('orders.closeProfit'), productFor(position)?.marginAssetSymbol || pair.quote),
        value: signed(position.realizedPnlText),
        tone: pnlTone(position.realizedPnlText),
      },
      {
        label: t('orders.closeProfitRate'),
        value: percent(realizedReturn),
        tone: pnlTone(realizedReturn),
      },
    ] : undefined,
  }
}

function positionRecord(position: MarginPosition) {
  const risk = records.risks.value.get(position.id)
  const symbol = symbolFor(position)
  const pair = splitPair(symbol)
  const marginAsset = productFor(position)?.marginAssetSymbol || pair.quote
  const live = resolveMarginPositionLiveProjection(position, market.tickerFor(symbol), risk)
  return {
    id: position.id,
    contractTitle: formatMarginContractTitle(symbol, t('orders.perpetual')),
    pnlLabel: metricLabel(t('orders.pnlAmount'), marginAsset),
    pnl: signed(live.unrealizedPnlText),
    returnRate: percent(live.returnRateText),
    pnlTone: pnlTone(live.unrealizedPnlText),
    chips: [
      { label: t(position.direction === 'long' ? 'orders.longShort' : 'orders.shortShort'), tone: position.direction === 'long' ? 'positive' as const : 'negative' as const },
      { label: t(position.marginMode === 'cross' ? 'orders.cross' : 'orders.isolated') },
      { label: `${position.leverage}x` },
    ],
    metrics: [
      { label: metricLabel(t('orders.positionQuantity'), pair.base), value: decimal(risk?.positionQuantityText) },
      { label: metricLabel(t('orders.margin'), marginAsset), value: decimal(position.marginAmountText) },
      { label: t('orders.maintenanceMarginRate'), value: percent(risk?.maintenanceMarginRateText, false) },
      { label: t('orders.entryPrice'), value: decimal(position.entryPriceText) },
      { label: t('orders.markPrice'), value: decimal(live.markPriceText) },
      { label: t('orders.liquidationPrice'), value: decimal(risk?.estimatedLiquidationPriceText) },
    ],
  }
}

function walletPnl(wallet: MarginWalletAccount): DecimalText | null {
  const cross = records.crossAccounts.value.find((account) => account.marginAssetId === wallet.assetId)
  if (cross) return cross.unrealizedPnlText
  const positions = records.openPositions.value.filter((position) => position.marginAssetId === wallet.assetId)
  if (!positions.length) return ZERO
  const snapshots = positions.map((position) => records.risks.value.get(position.id)?.unrealizedPnlText)
  return snapshots.every((value): value is DecimalText => Boolean(value))
    ? sumDecimalText(snapshots)
    : null
}

function assetLatestPrice(wallet: MarginWalletAccount): DecimalText | null {
  const ticker = market.tickers.find((item) => item.base.toUpperCase() === wallet.symbol.toUpperCase())
  return ticker?.lastPriceText || null
}

function isQuoteAsset(wallet: MarginWalletAccount): boolean {
  const symbol = wallet.symbol.toUpperCase()
  return records.products.value.some((product) => (
    product.marginAssetId === wallet.assetId || product.marginAssetSymbol.toUpperCase() === symbol
  ))
}

function assetRecord(wallet: MarginWalletAccount) {
  const cross = records.crossAccounts.value.find((account) => account.marginAssetId === wallet.assetId)
  const { balanceText: balance, equityText: equity, occupiedText: occupied } = marginWalletAssetAmounts(wallet, cross)
  const pnl = walletPnl(wallet)
  const latest = assetLatestPrice(wallet)
  const common = {
    symbol: wallet.symbol,
    logoUrl: wallet.logoUrl,
  }
  if (isQuoteAsset(wallet)) {
    return {
      ...common,
      metrics: [
        { label: t('orders.currencyEquity'), value: decimal(equity) },
        { label: t('orders.occupied'), value: decimal(occupied) },
        { label: t('orders.available'), value: decimal(wallet.availableText) },
        { label: t('orders.floatingPnl'), value: signed(pnl), tone: pnlTone(pnl) },
        { label: t('orders.balance'), value: decimal(balance) },
        { label: t('orders.frozen'), value: decimal(wallet.frozenText) },
      ],
    }
  }
  return {
    ...common,
    metrics: [
      { label: t('orders.currencyEquity'), value: decimal(equity) },
      { label: t('orders.costPrice'), value: '--' },
      { label: t('orders.latestPrice'), value: decimal(latest) },
      { label: t('orders.balance'), value: decimal(balance) },
      { label: t('orders.floatingPnl'), value: signed(pnl), tone: pnlTone(pnl) },
      { label: t('orders.available'), value: decimal(wallet.availableText) },
    ],
  }
}

function historyPositionRecord(position: MarginPosition) {
  const executionHistoryAvailable = records.executions.value.has(position.id)
  const executions = records.executions.value.get(position.id) || []
  const symbol = symbolFor(position)
  const pair = splitPair(symbol)
  const marginAsset = productFor(position)?.marginAssetSymbol || pair.quote
  const originalQuantity = executionHistoryAvailable
    ? marginPositionOriginalQuantity(position, executions)
    : null
  const closedQuantity = executionHistoryAvailable
    ? marginPositionClosedQuantity(position, executions)
    : null
  const realizedReturn = executionHistoryAvailable
    ? marginPositionRealizedReturn(position, executions)
    : null
  const averageExitPrice = executionHistoryAvailable
    ? marginPositionAverageExitPrice(position, executions)
    : null
  return {
    contractTitle: formatMarginContractTitle(symbol, t('orders.perpetual')),
    status: position.status.trim().toLowerCase() === 'closed'
      ? t('orders.statusFullyClosed')
      : statusLabel(position.status),
    statusTone: statusTone(position.status) === 'negative' ? 'negative' as const : 'muted' as const,
    chips: [
      { label: t(position.direction === 'long' ? 'orders.longShort' : 'orders.shortShort'), tone: position.direction === 'long' ? 'positive' as const : 'negative' as const },
      { label: t(position.marginMode === 'cross' ? 'orders.cross' : 'orders.isolated') },
      { label: `${position.leverage}x` },
    ],
    metrics: [
      { label: t('orders.entryPrice'), value: decimal(position.entryPriceText) },
      { label: metricLabel(t('orders.realizedPnl'), marginAsset), value: signed(position.realizedPnlText), tone: pnlTone(position.realizedPnlText) },
      { label: metricLabel(t('orders.maximumPosition'), pair.base), value: decimal(originalQuantity) },
      { label: t('orders.exitPrice'), value: decimal(averageExitPrice) },
      { label: t('orders.realizedReturn'), value: percent(realizedReturn), tone: pnlTone(realizedReturn) },
      { label: metricLabel(t('orders.closedQuantity'), pair.base), value: decimal(closedQuantity) },
    ],
    openedAt: dateTime(position.openedAt || position.createdAt),
    closedAt: dateTime(position.closedAt || executions.at(-1)?.createdAt),
  }
}

function openFilters(event: Event): void {
  setFilterReturnFocus(event.currentTarget instanceof HTMLElement ? event.currentTarget : filterTrigger.value)
  filterOpen.value = true
}

function selectOrderFilter(value: TransactionOrderFilter): void {
  orderFilter.value = value
  filterOpen.value = false
}

function requestAction(action: PendingAction, event?: Event): void {
  filterOpen.value = false
  setConfirmReturnFocus(event?.currentTarget instanceof HTMLElement ? event.currentTarget : null)
  pendingAction.value = action
}

function closeConfirm(): void {
  if (!actionId.value) pendingAction.value = null
}

function pendingActionLabel(): string {
  if (pendingAction.value?.kind === 'margin-close-all') return t('orders.closeAll')
  return pendingAction.value?.kind.endsWith('-all') ? t('orders.cancelAll') : t('orders.cancel')
}

async function confirmAction(): Promise<void> {
  const action = pendingAction.value
  if (!action || actionId.value) return
  const generation = session.generation
  actionId.value = `${action.kind}-${'id' in action ? action.id : 'all'}`
  actionError.value = ''
  try {
    let batchFailures = 0
    if (action.kind === 'spot') await cancelSpotOrder(action.id)
    else if (action.kind === 'margin') await cancelMarginPosition(action.id)
    else if (action.kind === 'spot-all') batchFailures = (await cancelAllSpotOrders()).failures.length
    else if (action.kind === 'margin-cancel-all') batchFailures = (await cancelAllMarginPositions()).failures.length
    else batchFailures = (await closeAllMarginPositions()).failures.length
    if (!session.isAuthenticated || session.generation !== generation) return
    pendingAction.value = null
    await records.load(activeTab.value)
    if (batchFailures) actionError.value = t('orders.batchActionPartial', { failed: batchFailures })
    else feedback.value = action.kind === 'margin-close-all' ? t('orders.allCloseSubmitted') : t('orders.cancelSucceeded')
  } catch (reason) {
    if (session.generation === generation) actionError.value = apiErrorMessage(reason, t('orders.actionFailed'))
  } finally {
    if (session.generation === generation) actionId.value = ''
  }
}

function openClose(position: MarginPosition, event: Event): void {
  closeReturnFocus.value = event.currentTarget instanceof HTMLElement ? event.currentTarget : null
  closeError.value = ''
  closeAttempt.value = null
  closePositionId.value = position.id
}

function closeCloseSheet(): void {
  if (!actionId.value.startsWith('close-')) closePositionId.value = null
}

async function confirmClose(percentage: number): Promise<void> {
  const position = closePosition.value
  if (!position || actionId.value) return
  const generation = session.generation
  const prior = closeAttempt.value
  const attempt = prior && prior.positionId === position.id && prior.percentage === percentage
    ? prior
    : { positionId: position.id, percentage, idempotencyKey: createMarginCloseIdempotencyKey() }
  closeAttempt.value = attempt
  actionId.value = `close-${position.id}`
  closeError.value = ''
  try {
    await closeMarginPosition(position.id, {
      percentage: attempt.percentage,
      idempotencyKey: attempt.idempotencyKey,
    })
    if (!session.isAuthenticated || session.generation !== generation) return
    closePositionId.value = null
    feedback.value = t(percentage === 100 ? 'orders.closeSubmitted' : 'orders.partialCloseSubmitted', { percentage })
    await records.load(activeTab.value)
  } catch (reason) {
    if (session.generation === generation) closeError.value = apiErrorMessage(reason, t('orders.closeFailed'))
  } finally {
    if (session.generation === generation) actionId.value = ''
  }
}

function openAssociated(position: MarginPosition): void {
  void router.push({ name: 'position-associated-orders', params: { id: position.id } })
}

function announceShare(message: string): void {
  if (shareFeedbackTimer) clearTimeout(shareFeedbackTimer)
  shareFeedback.value = message
  shareFeedbackTimer = setTimeout(() => {
    shareFeedback.value = ''
    shareFeedbackTimer = undefined
  }, 3_000)
}

async function shareRecord(title: string, href: string): Promise<void> {
  try {
    if (typeof navigator === 'undefined') throw new Error('share unavailable')
    const url = typeof window === 'undefined' ? href : new URL(href, window.location.href).href
    if (typeof navigator.share === 'function') await navigator.share({ title, text: title, url })
    else if (navigator.clipboard?.writeText) await navigator.clipboard.writeText(`${title}\n${url}`)
    else throw new Error('clipboard unavailable')
    announceShare(t('orders.recordShared'))
  } catch {
    announceShare(t('orders.recordShareFailed'))
  }
}

function shareHistoryOrder(row: TransactionOrderRow): void {
  if (row.kind !== 'margin') return
  const title = formatMarginContractTitle(symbolFor(row.position), t('orders.perpetual'))
  void shareRecord(title, router.resolve({ name: 'orders', query: { tab: 'history' } }).href)
}

function shareHistoryPosition(position: MarginPosition): void {
  const title = formatMarginContractTitle(symbolFor(position), t('orders.perpetual'))
  void shareRecord(title, router.resolve({ name: 'orders', query: { tab: 'position-history' } }).href)
}

async function load(): Promise<void> {
  feedback.value = ''
  actionError.value = ''
  await records.load(activeTab.value)
}

watch(() => route.query.tab, (tab) => {
  const normalized = normalizeTransactionRecordTab(tab)
  if (tab === 'spot') orderFilter.value = 'spot'
  else if (tab === 'margin') orderFilter.value = 'margin'
  else orderFilter.value = 'all'
  if (tab === 'ledger') {
    const symbol = typeof route.query.symbol === 'string' && route.query.symbol.trim()
      ? route.query.symbol
      : undefined
    void router.replace({ name: 'wallet-ledger', query: symbol ? { symbol } : undefined })
    return
  }
  void records.load(normalized)
}, { immediate: true })

watch(() => session.generation, () => {
  records.invalidate()
  pendingAction.value = null
  closePositionId.value = null
  if (!session.isAuthenticated) records.clear()
  else void records.load(activeTab.value)
}, { flush: 'sync' })

onMounted(() => {
  market.startLiveUpdates('transaction-records-assets')
  void market.refresh()
})

onBeforeUnmount(() => {
  market.stopLiveUpdates('transaction-records-assets')
  records.stop()
  if (shareFeedbackTimer) clearTimeout(shareFeedbackTimer)
})
</script>

<template>
  <TransactionRecordsLayout
    :active-tab="activeTab"
    :back-fallback="{ name: 'home' }"
    data-orders-workspace="live"
    data-pencil-source="kcP5D A85if n6oGO t2GTW4 e5Qs1 hxe8l"
  >
    <nav v-if="activeTab === 'current' || activeTab === 'history'" class="orders-type-tabs" :aria-label="t('orders.orderTypeCategory')">
      <button
        v-for="tab in orderTypeTabs"
        :key="tab.value"
        type="button"
        :disabled="tab.disabled"
        :aria-pressed="typeFilter === tab.value"
        @click="!tab.disabled && (typeFilter = tab.value as 'all' | 'standard')"
      >{{ tab.label }}</button>
    </nav>

    <nav class="orders-filter-bar" :class="{ 'orders-filter-bar--history-position': activeTab === 'position-history' }" :aria-label="t('orders.filterBarLabel')">
      <button ref="filterTrigger" class="orders-filter-trigger" type="button" :disabled="!session.isAuthenticated" aria-haspopup="dialog" :aria-expanded="filterOpen" @click="openFilters">
        <span>{{ orderFilter === 'all' ? t('orders.allTransactionTypes') : t(orderFilter === 'spot' ? 'orders.spot' : 'orders.marginMarket') }}</span>
        <ChevronDown :size="16" aria-hidden="true" />
      </button>
      <button v-if="activeTab === 'history'" class="orders-history-range" type="button" disabled>
        <span>{{ t('orders.pastYear') }}</span><ChevronDown :size="16" aria-hidden="true" />
      </button>
      <span class="orders-filter-spacer" aria-hidden="true" />
      <button v-if="activeTab === 'positions'" class="orders-filter-icon" type="button" :aria-label="t(valuesHidden ? 'orders.showValues' : 'orders.hideValues')" :aria-pressed="valuesHidden" @click="valuesHidden = !valuesHidden">
        <EyeOff v-if="valuesHidden" :size="23" aria-hidden="true" /><Eye v-else :size="23" aria-hidden="true" />
      </button>
      <button class="orders-filter-icon" type="button" :disabled="!session.isAuthenticated" :aria-label="t('orders.openFilters')" aria-haspopup="dialog" :aria-expanded="filterOpen" @click="openFilters">
        <ListFilter :size="24" aria-hidden="true" />
      </button>
    </nav>

    <span class="orders-share-feedback" role="status" aria-live="polite" aria-atomic="true">{{ shareFeedback }}</span>

    <LoginRequiredState v-if="!session.isAuthenticated" class="orders-login-state" :description="t('orders.loginDescription')" />
    <template v-else>
      <div v-if="visibleError" class="orders-message orders-message--error" role="alert">
        <CircleAlert :size="18" aria-hidden="true" /><span>{{ visibleError }}</span>
        <button type="button" :aria-label="t('orders.refresh')" @click="load"><RefreshCw :size="18" /></button>
      </div>
      <div v-if="feedback" class="orders-message orders-message--success" role="status">{{ feedback }}</div>
      <div v-if="records.loading.value" class="orders-loading" role="status"><LoaderCircle :size="24" class="spin" /><span>{{ t('orders.loading') }}</span></div>

      <template v-else-if="activeTab === 'current' && currentRows.length">
        <div class="orders-record-list" role="list">
          <TransactionOrderRecord
            v-for="row in currentRows"
            :key="row.id"
            :record="orderRecord(row, false)"
            :modify-label="t('orders.modify')"
            :cancel-label="t('orders.cancel')"
            :processing="actionId.endsWith(row.kind === 'spot' ? row.order.id : row.position.id)"
            @cancel="requestAction(row.kind === 'spot' ? { kind: 'spot', id: row.order.id } : { kind: 'margin', id: row.position.id }, $event)"
          />
        </div>
        <p class="orders-current-note">{{ t('orders.limitOrderExecutionNote') }}</p>
      </template>
      <TransactionRecordEmptyState v-else-if="activeTab === 'current'" :title="t('orders.emptyCurrent')" :description="t('orders.emptyDescription')" />

      <div v-else-if="activeTab === 'history' && historyRows.length" class="orders-record-list" role="list">
        <TransactionOrderRecord
          v-for="row in historyRows"
          :key="row.id"
          :record="orderRecord(row, true)"
          :share-label="t('orders.shareHistoryOrder')"
          @share="shareHistoryOrder(row)"
        />
      </div>
      <TransactionRecordEmptyState v-else-if="activeTab === 'history'" :title="t('orders.emptyHistory')" :description="t('orders.emptyDescription')" />

      <div v-else-if="activeTab === 'positions' && (records.openPositions.value.length || records.wallets.value.length)" class="orders-record-list" role="list">
        <MarginPositionRecord
          v-for="position in records.openPositions.value"
          :key="position.id"
          v-bind="positionRecord(position)"
          :tp-sl-label="t('orders.takeProfitStopLoss')"
          :close-label="t('orders.close')"
          :close-all-label="t('orders.marketCloseAll')"
          :processing="actionId === `close-${position.id}`"
          :values-hidden="valuesHidden"
          @close="openClose(position, $event)"
          @close-all="openClose(position, $event)"
        />
        <MarginAssetRecord v-for="wallet in records.wallets.value" :key="wallet.assetId" v-bind="assetRecord(wallet)" :values-hidden="valuesHidden" />
      </div>
      <TransactionRecordEmptyState v-else-if="activeTab === 'positions'" :title="t('orders.emptyPositionsAssets')" :description="t('orders.emptyDescription')" />

      <div v-else-if="activeTab === 'position-history' && historyPositions.length" class="orders-record-list" role="list">
        <MarginHistoryPositionRecord
          v-for="position in historyPositions"
          :key="position.id"
          v-bind="historyPositionRecord(position)"
          :opened-label="t('orders.openedAt')"
          :closed-label="t('orders.closedAt')"
          :associated-label="t('orders.associatedOrders')"
          :share-label="t('orders.shareHistoryPosition')"
          @associated="openAssociated(position)"
          @share="shareHistoryPosition(position)"
        />
        <p class="orders-history-scope">{{ t('orders.historyScope') }}</p>
      </div>
      <TransactionRecordEmptyState v-else-if="activeTab === 'position-history'" :title="t('orders.emptyPositionHistory')" :description="t('orders.emptyDescription')" />

      <TransactionRecordEmptyState v-else :title="t('orders.strategyUnavailable')" :description="t('orders.strategyUnavailableDescription')" />
    </template>

    <Teleport to="body">
      <div v-if="filterOpen" class="pencil-sheet-mask orders-filter-mask" @click.self="filterOpen = false">
        <section ref="filterDialog" class="pencil-sheet orders-filter-sheet" role="dialog" aria-modal="true" aria-labelledby="orders-filter-title" tabindex="-1" @keydown="trapFilterFocus($event, () => { filterOpen = false })">
          <div class="pencil-sheet__handle" aria-hidden="true" />
          <header><h2 id="orders-filter-title">{{ t('orders.transactionTypeFilter') }}</h2><button type="button" :aria-label="t('common.close')" @click="filterOpen = false"><X :size="20" /></button></header>
          <div class="orders-filter-options">
            <button v-for="value in (['all', 'spot', 'margin'] as const)" :key="value" type="button" :aria-pressed="orderFilter === value" :data-dialog-initial="orderFilter === value ? '' : undefined" @click="selectOrderFilter(value)">
              <span>{{ value === 'all' ? t('orders.allTransactionTypes') : t(value === 'spot' ? 'orders.spot' : 'orders.marginMarket') }}</span><Check v-if="orderFilter === value" :size="18" />
            </button>
          </div>
          <div v-if="activeTab === 'current' || activeTab === 'positions'" class="orders-filter-batch">
            <button v-if="activeTab === 'current' && records.currentSpot.value.length" type="button" @click="requestAction({ kind: 'spot-all' }, $event)">{{ t('orders.cancelAllSpot') }}</button>
            <button v-if="activeTab === 'current' && records.pendingMargin.value.length" type="button" @click="requestAction({ kind: 'margin-cancel-all' }, $event)">{{ t('orders.cancelAllMargin') }}</button>
            <button v-if="activeTab === 'positions' && records.openPositions.value.length" type="button" @click="requestAction({ kind: 'margin-close-all' }, $event)">{{ t('orders.closeAll') }}</button>
          </div>
        </section>
      </div>

      <div v-if="pendingAction" class="orders-confirm-mask" @click.self="closeConfirm">
        <section ref="confirmDialog" class="orders-confirm" role="alertdialog" aria-modal="true" aria-labelledby="orders-confirm-title" aria-describedby="orders-confirm-description" tabindex="-1" @keydown="trapConfirmFocus($event, closeConfirm)">
          <h2 id="orders-confirm-title">{{ t('orders.confirmAction', { action: pendingActionLabel() }) }}</h2>
          <p id="orders-confirm-description">{{ t('orders.confirmDescription') }}</p>
          <div><button data-dialog-initial type="button" :disabled="Boolean(actionId)" @click="closeConfirm">{{ t('common.cancel') }}</button><button class="is-danger" type="button" :disabled="Boolean(actionId)" @click="confirmAction">{{ pendingActionLabel() }}</button></div>
        </section>
      </div>
    </Teleport>

    <MarginCloseSheet
      :open="Boolean(closePosition)"
      :saving="actionId.startsWith('close-')"
      :return-focus="closeReturnFocus"
      :symbol="closePosition ? symbolFor(closePosition) : '--/--'"
      :direction="closePosition?.direction || 'long'"
      :margin-mode="closePosition?.marginMode || 'isolated'"
      :leverage="closePosition?.leverage || 1"
      :base-asset="closePosition ? splitPair(symbolFor(closePosition)).base : '--'"
      :quote-asset="closePosition ? splitPair(symbolFor(closePosition)).quote : '--'"
      :mark-price="closeRisk?.markPrice || null"
      :position-quantity="closeRisk?.positionQuantity || null"
      :estimated-pnl="closeRisk?.unrealizedPnl || null"
      :error="closeError"
      @close="closeCloseSheet"
      @confirm="confirmClose"
    />
  </TransactionRecordsLayout>
</template>

<style scoped>
.orders-type-tabs {
  align-items: center;
  background: var(--records-canvas);
  box-sizing: border-box;
  display: flex;
  gap: 18px;
  height: 44px;
  min-height: 44px;
  overflow-x: auto;
  padding: 4px 16px;
  scrollbar-width: none;
}
.orders-type-tabs::-webkit-scrollbar { display: none; }
.orders-type-tabs button { background: transparent; border: 0; border-radius: 18px; color: var(--records-muted); flex: 0 0 auto; font-size: 14px; font-weight: 500; line-height: 20px; min-height: 36px; padding: 7px 11px; white-space: nowrap; }
.orders-type-tabs button[aria-pressed='true'] { background: var(--records-chip); color: var(--records-ink); font-weight: 700; }
.orders-type-tabs button:disabled { color: var(--records-muted); cursor: default; opacity: 1; }

.orders-filter-bar { align-items: center; background: var(--records-canvas); box-sizing: border-box; display: flex; gap: 16px; height: 52px; min-height: 52px; padding: 0 16px; }
.orders-filter-bar--history-position { height: 58px; min-height: 58px; }
.orders-filter-trigger, .orders-history-range, .orders-filter-icon { background: transparent; border: 0; color: var(--records-ink); min-height: 44px; }
.orders-filter-trigger { align-items: center; display: flex; font-size: 16px; font-weight: 600; gap: 8px; padding: 0; }
.orders-history-range { align-items: center; color: var(--records-ink); display: flex; font-size: 16px; font-weight: 600; gap: 8px; padding: 0; }
.orders-history-range:disabled { color: var(--records-ink); cursor: default; opacity: 1; -webkit-text-fill-color: currentColor; }
.orders-filter-spacer { flex: 1; }
.orders-filter-icon { display: grid; flex: 0 0 44px; padding: 0; place-items: center; width: 44px; }
.orders-record-list { background: var(--records-canvas); min-width: 0; }
.orders-current-note { border-top: 0; color: var(--records-muted); font-size: 13px; font-weight: 400; line-height: 18px; margin: 0; padding: 20px 18px; text-align: center; }
.orders-history-scope { color: var(--records-muted); font-size: 11px; line-height: 17px; margin: 0; padding: 16px 18px calc(28px + env(safe-area-inset-bottom)); text-align: center; }
.orders-share-feedback { clip: rect(0 0 0 0); clip-path: inset(50%); height: 1px; overflow: hidden; position: absolute; white-space: nowrap; width: 1px; }
.orders-loading { align-items: center; color: var(--records-muted); display: flex; font-size: 13px; gap: 9px; justify-content: center; min-height: 260px; }
.spin { animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

.orders-message { align-items: center; border-bottom: 1px solid var(--records-divider); display: grid; font-size: 12px; gap: 8px; grid-template-columns: auto minmax(0, 1fr) auto; min-height: 44px; padding: 0 18px; }
.orders-message--error { color: var(--records-negative); }
.orders-message--success { color: var(--records-positive); display: flex; }
.orders-message button { background: transparent; border: 0; color: inherit; display: grid; height: 44px; padding: 0; place-items: center; width: 44px; }
.orders-login-state { border: 0; border-radius: 0; margin: 0; }

.orders-filter-mask { justify-items: center; }
.orders-filter-sheet { color: var(--ink); max-width: 448px; }
.orders-filter-sheet > header { align-items: center; display: flex; justify-content: space-between; }
.orders-filter-sheet h2 { font-size: 18px; margin: 0; }
.orders-filter-sheet header button { background: transparent; border: 0; color: var(--ink); display: grid; height: 44px; place-items: center; width: 44px; }
.orders-filter-options { display: grid; }
.orders-filter-options button { align-items: center; background: transparent; border: 0; border-bottom: 1px solid var(--hairline); color: var(--ink); display: grid; font-size: 14px; font-weight: 600; grid-template-columns: 1fr 20px; min-height: 56px; padding: 0 4px; text-align: left; }
.orders-filter-options button[aria-pressed='true'] { color: var(--positive); }
.orders-filter-batch { display: grid; gap: 10px; grid-template-columns: repeat(2, minmax(0, 1fr)); padding-top: 14px; }
.orders-filter-batch button { background: var(--surface-soft); border: 0; border-radius: 12px; color: var(--negative); font-size: 13px; min-height: 44px; }

.orders-confirm-mask { align-items: center; background: var(--overlay); display: grid; inset: 0; padding: 20px; place-items: center; position: fixed; z-index: var(--layer-modal); }
.orders-confirm { background: var(--surface-elevated); border-radius: 18px; color: var(--ink); display: grid; gap: 14px; max-width: 340px; padding: 20px; width: 100%; }
.orders-confirm h2 { font-size: 18px; margin: 0; }
.orders-confirm p { color: var(--muted); font-size: 13px; line-height: 20px; margin: 0; }
.orders-confirm > div { display: grid; gap: 10px; grid-template-columns: repeat(2, 1fr); }
.orders-confirm button { background: var(--surface-soft); border: 0; border-radius: 12px; color: var(--ink); font-size: 14px; min-height: 44px; }
.orders-confirm button.is-danger { background: var(--negative); color: var(--on-negative); }
.orders-type-tabs button:focus-visible, .orders-filter-bar button:focus-visible, .orders-filter-sheet button:focus-visible, .orders-confirm button:focus-visible { box-shadow: 0 0 0 2px var(--focus-ring); outline: 0; }

@media (max-width: 340px) {
  .orders-filter-bar { gap: 8px; padding-inline: 14px; }
  .orders-type-tabs { gap: 12px; padding-inline: 14px; }
}
@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
