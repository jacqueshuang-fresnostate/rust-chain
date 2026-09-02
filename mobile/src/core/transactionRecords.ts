import {
  decimalAdd,
  decimalDivide,
  decimalMultiply,
  decimalSign,
  formatDecimalText,
  normalizeDecimalText,
  type DecimalText,
} from './decimal.ts'
import type {
  MarginCrossAccount,
  MarginPosition,
  MarginPositionExecution,
  MarginWalletAccount,
  SpotOrder,
} from '../api/trading.ts'

export const TRANSACTION_RECORD_TABS = [
  'current',
  'history',
  'positions',
  'position-history',
  'ledger',
  'current-strategy',
  'strategy-history',
] as const

export type TransactionRecordTab = typeof TRANSACTION_RECORD_TABS[number]
export type TransactionOrderFilter = 'all' | 'spot' | 'margin'

export type TransactionOrderRow =
  | { kind: 'spot'; id: string; occurredAt: number; order: SpotOrder }
  | { kind: 'margin'; id: string; occurredAt: number; position: MarginPosition }

const ZERO = normalizeDecimalText('0')
const HUNDRED = normalizeDecimalText('100')
const RECORD_DIVISION_SCALE = 18
const TERMINAL_MARGIN_STATUSES = new Set(['closed', 'liquidated'])

export interface MarginPositionExposure {
  originalNotionalText: DecimalText
  closedNotionalText: DecimalText
  executionNotionalText: DecimalText
  originalMarginText: DecimalText
  closedMarginText: DecimalText
  executionMarginText: DecimalText
  hasFullyClosedExecution: boolean
}

export interface MarginWalletAssetAmounts {
  balanceText: DecimalText
  equityText: DecimalText | null
  occupiedText: DecimalText | null
}

export function normalizeTransactionRecordTab(value: unknown): TransactionRecordTab {
  const tab = String(value || '').trim()
  if ((TRANSACTION_RECORD_TABS as readonly string[]).includes(tab)) {
    return tab as TransactionRecordTab
  }
  if (tab === 'margin') return 'current'
  return 'current'
}

export function positionOccurredAt(position: MarginPosition, history = false): number {
  return (history ? position.closedAt : undefined)
    || position.createdAt
    || position.openedAt
    || 0
}

export function mergeTransactionOrders(
  spotOrders: readonly SpotOrder[],
  marginPositions: readonly MarginPosition[],
  history = false,
): TransactionOrderRow[] {
  return [
    ...spotOrders.map((order): TransactionOrderRow => ({
      kind: 'spot',
      id: `spot-${order.id}`,
      occurredAt: order.createdAt || 0,
      order,
    })),
    ...marginPositions.map((position): TransactionOrderRow => ({
      kind: 'margin',
      id: `margin-${position.id}`,
      occurredAt: positionOccurredAt(position, history),
      position,
    })),
  ].sort((left, right) => right.occurredAt - left.occurredAt || left.id.localeCompare(right.id))
}

export function filterTransactionOrders(
  rows: readonly TransactionOrderRow[],
  filter: TransactionOrderFilter,
): TransactionOrderRow[] {
  return filter === 'all' ? [...rows] : rows.filter((row) => row.kind === filter)
}

export function sumDecimalText(values: readonly (DecimalText | null | undefined)[]): DecimalText {
  return values.reduce<DecimalText>((sum, value) => value ? decimalAdd(sum, value) : sum, ZERO)
}

/** Keeps wallet buckets authoritative without relabelling balance/locked as undefined portfolio facts. */
export function marginWalletAssetAmounts(
  wallet: Pick<MarginWalletAccount, 'availableText' | 'frozenText' | 'lockedText'>,
  crossAccount?: Pick<MarginCrossAccount, 'equityText'> | null,
): MarginWalletAssetAmounts {
  const balanceText = sumDecimalText([wallet.availableText, wallet.frozenText, wallet.lockedText])
  return {
    balanceText,
    equityText: crossAccount?.equityText ?? null,
    occupiedText: null,
  }
}

export function formatMarginContractTitle(symbol: string, perpetual: string): string {
  const rawSymbol = symbol.trim().replace(/\s*(?:永续|perpetual)\s*$/iu, '')
  if (!rawSymbol || /^[-/_\s]+$/.test(rawSymbol)) return '--'
  const compactSymbol = rawSymbol.replace(/[\s/_-]+/g, '').toUpperCase()
  const suffix = perpetual.trim()
  return suffix ? `${compactSymbol} ${suffix}` : compactSymbol
}

/**
 * Rebuilds the immutable opening exposure without counting a terminal position row twice.
 *
 * A fully-closed execution owns the terminal slice, so all execution slices already equal the
 * original notional. Legacy/body-less closes have no fully-closed execution and retain their
 * final slice on the position row; in that case the row is added to the explicit slices.
 */
export function reconstructMarginPositionExposure(
  position: MarginPosition,
  executions: readonly MarginPositionExecution[],
): MarginPositionExposure {
  const executionNotionalText = sumDecimalText(
    executions.map((execution) => execution.closeNotionalAmountText),
  )
  const executionMarginText = sumDecimalText(
    executions.map((execution) => execution.closeMarginAmountText),
  )
  const hasFullyClosedExecution = executions.some((execution) => execution.fullyClosed)
  const originalNotionalText = hasFullyClosedExecution
    ? executionNotionalText
    : decimalAdd(position.notionalAmountText, executionNotionalText)
  const originalMarginText = hasFullyClosedExecution
    ? executionMarginText
    : decimalAdd(position.marginAmountText, executionMarginText)
  const isTerminal = isTerminalMarginPosition(position)
  return {
    originalNotionalText,
    closedNotionalText: hasFullyClosedExecution || isTerminal
      ? originalNotionalText
      : executionNotionalText,
    executionNotionalText,
    originalMarginText,
    closedMarginText: hasFullyClosedExecution || isTerminal
      ? originalMarginText
      : executionMarginText,
    executionMarginText,
    hasFullyClosedExecution,
  }
}

export function isTerminalMarginPosition(position: Pick<MarginPosition, 'status'>): boolean {
  return TERMINAL_MARGIN_STATUSES.has(position.status.trim().toLowerCase())
}

export function marginPositionOriginalQuantity(
  position: MarginPosition,
  executions: readonly MarginPositionExecution[] = [],
): DecimalText | null {
  const price = position.entryPriceText || position.limitPriceText
  if (!price) return null
  const notional = executions.length
    ? reconstructMarginPositionExposure(position, executions).originalNotionalText
    : position.notionalAmountText
  return divideRecordDecimal(notional, price)
}

export function marginPositionClosedQuantity(
  position: MarginPosition,
  executions: readonly MarginPositionExecution[],
): DecimalText | null {
  if (!position.entryPriceText) return null
  const exposure = reconstructMarginPositionExposure(position, executions)
  return divideRecordDecimal(exposure.closedNotionalText, position.entryPriceText)
}

export function marginExecutionQuantity(
  execution: MarginPositionExecution,
  entryPrice: DecimalText | null | undefined,
): DecimalText | null {
  return entryPrice
    ? divideRecordDecimal(execution.closeNotionalAmountText, entryPrice)
    : null
}

export function marginPositionRealizedReturn(
  position: MarginPosition,
  executions: readonly MarginPositionExecution[] = [],
): DecimalText | null {
  const originalMarginText = executions.length
    ? reconstructMarginPositionExposure(position, executions).originalMarginText
    : position.marginAmountText
  return position.realizedPnlText
    ? divideRecordDecimal(position.realizedPnlText, originalMarginText)
    : null
}

/**
 * Weights every immutable close slice by its original notional exposure.
 *
 * A legacy/body-less terminal close has no fully-closed execution, so the terminal row owns the
 * remaining close slice and its `exit_price`. An explicit fully-closed execution already owns that
 * final slice and the unchanged terminal row must not be counted again.
 */
export function marginPositionAverageExitPrice(
  position: MarginPosition,
  executions: readonly MarginPositionExecution[],
): DecimalText | null {
  const exposure = reconstructMarginPositionExposure(position, executions)
  let weightedExitText = ZERO
  let closedNotionalText = ZERO
  for (const execution of executions) {
    weightedExitText = decimalAdd(
      weightedExitText,
      decimalMultiply(execution.closeNotionalAmountText, execution.exitPriceText),
    )
    closedNotionalText = decimalAdd(closedNotionalText, execution.closeNotionalAmountText)
  }
  if (isTerminalMarginPosition(position)
    && !exposure.hasFullyClosedExecution
    && position.exitPriceText
    && decimalSign(position.notionalAmountText) > 0) {
    weightedExitText = decimalAdd(
      weightedExitText,
      decimalMultiply(position.notionalAmountText, position.exitPriceText),
    )
    closedNotionalText = decimalAdd(closedNotionalText, position.notionalAmountText)
  }
  return decimalSign(closedNotionalText) > 0
    ? divideRecordDecimal(weightedExitText, closedNotionalText)
    : null
}

/** Includes the terminal row's legacy residual interest without duplicating an explicit final slice. */
export function marginPositionClosedInterest(
  position: MarginPosition,
  executions: readonly MarginPositionExecution[],
): DecimalText {
  const executionInterestText = executionInterest(executions)
  const hasFullyClosedExecution = executions.some((execution) => execution.fullyClosed)
  return isTerminalMarginPosition(position) && !hasFullyClosedExecution
    ? decimalAdd(executionInterestText, position.interestAmountText)
    : executionInterestText
}

/** Prefers the position's cumulative realized PnL and falls back to explicit execution slices. */
export function marginPositionClosedRealizedPnl(
  position: MarginPosition,
  executions: readonly MarginPositionExecution[],
): DecimalText | null {
  return position.realizedPnlText
    ?? (executions.length ? executionRealizedPnl(executions) : null)
}

/** Produces an opaque, stable display number while route actions keep using the internal ID. */
export function formatTransactionRecordDisplayNo(
  prefix: string,
  sourceId: string,
  occurredAt?: number,
  existing?: string | null,
): string {
  const backendNumber = existing?.trim()
  if (backendNumber) return backendNumber
  const safePrefix = prefix.trim().toUpperCase().replace(/[^A-Z0-9]/g, '') || 'TR'
  const date = transactionRecordDateToken(occurredAt)
  const source = sourceId.trim()
  const token = source ? opaqueRecordToken(`${safePrefix}:${date}:${source}`) : '00000000'
  return `${safePrefix}${date}${token}`
}

export function formatRecordDecimal(
  value: DecimalText | null | undefined,
  locale: string,
  maximumFractionDigits = 8,
): string {
  if (!value) return '--'
  return formatDecimalText(value, locale, {
    maximumFractionDigits,
    preserveNonZero: true,
    useGrouping: true,
  })
}

export function formatRecordSignedDecimal(
  value: DecimalText | null | undefined,
  locale: string,
  maximumFractionDigits = 8,
): string {
  if (!value) return '--'
  const formatted = formatRecordDecimal(value, locale, maximumFractionDigits)
  return decimalSign(value) > 0 ? `+${formatted}` : formatted
}

export function formatRecordPercent(
  ratio: DecimalText | null | undefined,
  locale: string,
  maximumFractionDigits = 2,
): string {
  if (!ratio) return '--'
  return `${formatRecordSignedDecimal(decimalMultiply(ratio, HUNDRED), locale, maximumFractionDigits)}%`
}

export function executionRealizedPnl(executions: readonly MarginPositionExecution[]): DecimalText {
  return sumDecimalText(executions.map((execution) => execution.realizedPnlText))
}

export function executionInterest(executions: readonly MarginPositionExecution[]): DecimalText {
  return sumDecimalText(executions.map((execution) => execution.closeInterestAmountText))
}

export function latestExecutionTime(executions: readonly MarginPositionExecution[]): number | undefined {
  return [...executions].sort((left, right) => right.createdAt - left.createdAt || right.id.localeCompare(left.id))[0]?.createdAt
}

function divideRecordDecimal(dividend: DecimalText, divisor: DecimalText): DecimalText | null {
  if (decimalSign(divisor) <= 0) return null
  try {
    return decimalDivide(dividend, divisor, RECORD_DIVISION_SCALE)
  } catch {
    return null
  }
}

function transactionRecordDateToken(timestamp?: number): string {
  if (!timestamp || !Number.isSafeInteger(timestamp) || timestamp <= 0) return '00000000'
  const date = new Date(timestamp)
  if (Number.isNaN(date.getTime())) return '00000000'
  return [
    String(date.getUTCFullYear()).padStart(4, '0'),
    String(date.getUTCMonth() + 1).padStart(2, '0'),
    String(date.getUTCDate()).padStart(2, '0'),
  ].join('')
}

function opaqueRecordToken(value: string): string {
  let hash = 0xcbf29ce484222325n
  const prime = 0x100000001b3n
  const mask = 0xffffffffffffffffn
  for (const character of value) {
    hash ^= BigInt(character.codePointAt(0) || 0)
    hash = (hash * prime) & mask
  }
  return hash.toString(36).toUpperCase().padStart(8, '0').slice(-8)
}
