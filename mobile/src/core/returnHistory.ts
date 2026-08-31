import {
  normalizeRealizedReturnAssetSymbol,
  normalizeRealizedReturnAssetSymbols,
  normalizeRealizedReturnTimestamp,
  nullableRealizedReturnDecimal,
} from './realizedReturn.ts'
import {
  decimalAdd,
  decimalCompare,
  decimalDivide,
  normalizeDecimalText,
  type DecimalText,
} from './decimal.ts'
import {
  createSessionRequestLifecycle,
  type SessionRequestLifecycle,
} from './sessionRequest.ts'

export const RETURN_HISTORY_PERIODS = [1, 7, 30, 180] as const
export const RETURN_HISTORY_DAY_MS = 86_400_000

export type ReturnHistoryPeriodDays = typeof RETURN_HISTORY_PERIODS[number]
export type ReturnHistoryStatus = 'complete' | 'partial'
export type ReturnHistoryViewState = 'idle' | 'loading' | ReturnHistoryStatus | 'error'

export interface BackendReturnHistorySummary {
  amount: unknown
  basis_amount: unknown
  rate: unknown
}

export interface BackendReturnHistoryMissingPrice {
  day_start_at: unknown
  asset_symbol: unknown
}

export interface BackendReturnHistoryPoint {
  day_start_at: unknown
  valued_at: unknown
  amount: unknown
  basis_amount: unknown
  rate: unknown
  cumulative_amount: unknown
  status: unknown
  missing_price_assets: unknown
}

export interface BackendReturnHistory {
  scope: unknown
  reporting_asset: unknown
  period_days: unknown
  period_start_at: unknown
  calculated_at: unknown
  status: unknown
  summary: unknown
  missing_prices: unknown
  points: unknown
}

export interface ReturnHistorySummary {
  amount: DecimalText | null
  basisAmount: DecimalText | null
  rate: DecimalText | null
}

export interface ReturnHistoryMissingPrice {
  dayStartAt: number
  assetSymbol: string
}

export interface ReturnHistoryPoint {
  dayStartAt: number
  valuedAt: number
  amount: DecimalText | null
  basisAmount: DecimalText | null
  rate: DecimalText | null
  cumulativeAmount: DecimalText | null
  status: ReturnHistoryStatus
  missingPriceAssets: string[]
}

export interface ReturnHistory {
  scope: 'realized'
  reportingAsset: 'USDT'
  periodDays: ReturnHistoryPeriodDays
  periodStartAt: number
  calculatedAt: number
  status: ReturnHistoryStatus
  summary: ReturnHistorySummary
  missingPrices: ReturnHistoryMissingPrice[]
  points: ReturnHistoryPoint[]
}

export type ReturnHistoryRequestLifecycle = SessionRequestLifecycle<ReturnHistory>

export function isReturnHistoryPeriod(value: unknown): value is ReturnHistoryPeriodDays {
  return typeof value === 'number'
    && RETURN_HISTORY_PERIODS.includes(value as ReturnHistoryPeriodDays)
}

export function mapReturnHistory(
  payload: BackendReturnHistory,
  expectedPeriodDays: ReturnHistoryPeriodDays,
): ReturnHistory {
  if (!isReturnHistoryPeriod(expectedPeriodDays)) throw new Error('invalid return history expected period')
  if (payload.scope !== 'realized') throw new Error('invalid return history scope')
  if (typeof payload.reporting_asset !== 'string'
    || payload.reporting_asset.trim().toUpperCase() !== 'USDT') {
    throw new Error('invalid return history reporting asset')
  }
  if (payload.period_days !== expectedPeriodDays) throw new Error('invalid return history period_days')
  const status = returnHistoryStatus(payload.status, 'status')
  const periodStartAt = normalizeRealizedReturnTimestamp(
    payload.period_start_at,
    'period_start_at',
    'return history',
  )
  const calculatedAt = normalizeRealizedReturnTimestamp(
    payload.calculated_at,
    'calculated_at',
    'return history',
  )
  const todayStartAt = Math.floor(calculatedAt / RETURN_HISTORY_DAY_MS) * RETURN_HISTORY_DAY_MS
  if (periodStartAt % RETURN_HISTORY_DAY_MS !== 0
    || calculatedAt < todayStartAt
    || calculatedAt >= todayStartAt + RETURN_HISTORY_DAY_MS
    || periodStartAt !== todayStartAt - (expectedPeriodDays - 1) * RETURN_HISTORY_DAY_MS) {
    throw new Error('invalid return history UTC period')
  }

  if (!Array.isArray(payload.points) || payload.points.length !== expectedPeriodDays) {
    throw new Error('invalid return history points length')
  }
  const zero = normalizeDecimalText('0')
  let cumulative = zero
  let basisTotal = zero
  let cumulativeKnown = true
  let hasPartialPoint = false
  const pointMissingKeys: string[] = []
  const points = payload.points.map((rawPoint, index) => {
    if (!isRecord(rawPoint)) throw new Error('invalid return history point')
    const point = rawPoint as unknown as BackendReturnHistoryPoint
    const dayStartAt = normalizeRealizedReturnTimestamp(
      point.day_start_at,
      `points[${index}].day_start_at`,
      'return history',
    )
    const expectedDayStartAt = periodStartAt + index * RETURN_HISTORY_DAY_MS
    if (dayStartAt !== expectedDayStartAt || dayStartAt % RETURN_HISTORY_DAY_MS !== 0) {
      throw new Error('invalid return history point UTC continuity')
    }
    const valuedAt = normalizeRealizedReturnTimestamp(
      point.valued_at,
      `points[${index}].valued_at`,
      'return history',
    )
    const expectedValuedAt = dayStartAt === todayStartAt
      ? calculatedAt
      : dayStartAt + RETURN_HISTORY_DAY_MS
    if (valuedAt !== expectedValuedAt) throw new Error('invalid return history point valued_at')

    const pointStatus = returnHistoryStatus(point.status, `points[${index}].status`)
    const missingPriceAssets = canonicalMissingAssets(
      point.missing_price_assets,
      `points[${index}].missing_price_assets`,
    )
    const amount = nullableRealizedReturnDecimal(
      point.amount,
      `points[${index}].amount`,
      'return history',
    )
    const basisAmount = nullableRealizedReturnDecimal(
      point.basis_amount,
      `points[${index}].basis_amount`,
      'return history',
    )
    const rate = nullableRealizedReturnDecimal(
      point.rate,
      `points[${index}].rate`,
      'return history',
    )
    const cumulativeAmount = nullableRealizedReturnDecimal(
      point.cumulative_amount,
      `points[${index}].cumulative_amount`,
      'return history',
    )

    if (pointStatus === 'partial') {
      if (amount !== null || basisAmount !== null || rate !== null || cumulativeAmount !== null
        || !missingPriceAssets.length) {
        throw new Error('invalid partial return history point')
      }
      hasPartialPoint = true
      cumulativeKnown = false
      for (const assetSymbol of missingPriceAssets) {
        pointMissingKeys.push(`${dayStartAt}:${assetSymbol}`)
      }
    } else {
      if (amount === null || basisAmount === null || rate === null || missingPriceAssets.length) {
        throw new Error('invalid complete return history point')
      }
      if (decimalCompare(basisAmount, zero) < 0) {
        throw new Error('invalid return history point basis_amount')
      }
      const expectedRate = decimalCompare(basisAmount, zero) > 0
        ? decimalDivide(amount, basisAmount, 18)
        : zero
      if (decimalCompare(rate, expectedRate) !== 0) {
        throw new Error('invalid return history point rate consistency')
      }
      basisTotal = decimalAdd(basisTotal, basisAmount)
      if (cumulativeKnown) {
        cumulative = decimalAdd(cumulative, amount)
        if (cumulativeAmount === null || decimalCompare(cumulativeAmount, cumulative) !== 0) {
          throw new Error('invalid return history cumulative consistency')
        }
      } else if (cumulativeAmount !== null) {
        throw new Error('invalid return history cumulative after partial')
      }
    }

    return {
      dayStartAt,
      valuedAt,
      amount,
      basisAmount,
      rate,
      cumulativeAmount,
      status: pointStatus,
      missingPriceAssets,
    }
  })

  if (points.at(-1)?.dayStartAt !== todayStartAt) {
    throw new Error('invalid return history final UTC day')
  }
  if ((status === 'partial') !== hasPartialPoint) {
    throw new Error('invalid return history status consistency')
  }

  const missingPrices = mapMissingPrices(payload.missing_prices, periodStartAt, todayStartAt)
  const topMissingKeys = missingPrices.map(({ dayStartAt, assetSymbol }) => (
    `${dayStartAt}:${assetSymbol}`
  ))
  if (!arraysEqual(topMissingKeys, pointMissingKeys)) {
    throw new Error('invalid return history missing price consistency')
  }

  const summary = mapSummary(payload.summary)
  if (status === 'partial') {
    if (summary.amount !== null || summary.basisAmount !== null || summary.rate !== null) {
      throw new Error('invalid partial return history summary')
    }
  } else {
    if (summary.amount === null || summary.basisAmount === null || summary.rate === null
      || missingPrices.length) {
      throw new Error('invalid complete return history summary')
    }
    const finalCumulative = points.at(-1)?.cumulativeAmount
    const expectedRate = decimalCompare(basisTotal, zero) > 0
      ? decimalDivide(summary.amount, basisTotal, 18)
      : zero
    if (finalCumulative === null || finalCumulative === undefined
      || decimalCompare(summary.amount, finalCumulative) !== 0
      || decimalCompare(summary.basisAmount, basisTotal) !== 0
      || decimalCompare(summary.rate, expectedRate) !== 0) {
      throw new Error('invalid return history summary consistency')
    }
  }

  return {
    scope: 'realized',
    reportingAsset: 'USDT',
    periodDays: expectedPeriodDays,
    periodStartAt,
    calculatedAt,
    status,
    summary,
    missingPrices,
    points,
  }
}

export function createReturnHistoryRequestLifecycle(input: {
  sessionKey: () => string
  fetchReturnHistory: () => Promise<ReturnHistory>
}): ReturnHistoryRequestLifecycle {
  return createSessionRequestLifecycle({
    sessionKey: input.sessionKey,
    request: input.fetchReturnHistory,
  })
}

function returnHistoryStatus(value: unknown, field: string): ReturnHistoryStatus {
  if (value !== 'complete' && value !== 'partial') {
    throw new Error(`invalid return history ${field}`)
  }
  return value
}

function canonicalMissingAssets(value: unknown, field: string): string[] {
  if (!Array.isArray(value)) throw new Error(`invalid return history ${field}`)
  const normalized = normalizeRealizedReturnAssetSymbols(value, field, 'return history')
  const sorted = [...normalized].sort()
  if (normalized.length !== value.length || !arraysEqual(normalized, sorted)) {
    throw new Error(`invalid return history ${field}`)
  }
  return normalized
}

function mapMissingPrices(
  value: unknown,
  periodStartAt: number,
  todayStartAt: number,
): ReturnHistoryMissingPrice[] {
  if (!Array.isArray(value)) throw new Error('invalid return history missing_prices')
  const rows = value.map((rawRow, index) => {
    if (!isRecord(rawRow)) throw new Error('invalid return history missing price')
    const row = rawRow as unknown as BackendReturnHistoryMissingPrice
    const dayStartAt = normalizeRealizedReturnTimestamp(
      row.day_start_at,
      `missing_prices[${index}].day_start_at`,
      'return history',
    )
    if (dayStartAt % RETURN_HISTORY_DAY_MS !== 0
      || dayStartAt < periodStartAt
      || dayStartAt > todayStartAt) {
      throw new Error('invalid return history missing price day')
    }
    return {
      dayStartAt,
      assetSymbol: normalizeRealizedReturnAssetSymbol(
        row.asset_symbol,
        `missing_prices[${index}].asset_symbol`,
        'return history',
      ),
    }
  })
  const keys = rows.map(({ dayStartAt, assetSymbol }) => `${dayStartAt}:${assetSymbol}`)
  if (new Set(keys).size !== keys.length || !arraysEqual(keys, [...keys].sort(compareMissingKey))) {
    throw new Error('invalid return history missing price order')
  }
  return rows
}

function mapSummary(value: unknown): ReturnHistorySummary {
  if (!isRecord(value)) throw new Error('invalid return history summary')
  const summary = value as unknown as BackendReturnHistorySummary
  const amount = nullableRealizedReturnDecimal(summary.amount, 'summary.amount', 'return history')
  const basisAmount = nullableRealizedReturnDecimal(
    summary.basis_amount,
    'summary.basis_amount',
    'return history',
  )
  const rate = nullableRealizedReturnDecimal(summary.rate, 'summary.rate', 'return history')
  if (basisAmount !== null && decimalCompare(basisAmount, normalizeDecimalText('0')) < 0) {
    throw new Error('invalid return history summary.basis_amount')
  }
  return { amount, basisAmount, rate }
}

function compareMissingKey(left: string, right: string): number {
  const [leftDay = '', leftAsset = ''] = left.split(':')
  const [rightDay = '', rightAsset = ''] = right.split(':')
  const dayOrder = Number(leftDay) - Number(rightDay)
  return dayOrder || leftAsset.localeCompare(rightAsset)
}

function arraysEqual<T>(left: T[], right: T[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index])
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
