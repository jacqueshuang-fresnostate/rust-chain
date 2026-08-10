import {
  createSessionRequestLifecycle,
  type SessionRequestLifecycle,
  type SessionRequestLoadResult,
} from './sessionRequest.ts'
import {
  normalizeRealizedReturnAssetSymbols,
  normalizeRealizedReturnTimestamp,
  requiredRealizedReturnNumber,
} from './realizedReturn.ts'

export type TodayReturnStatus = 'complete' | 'partial'

export interface TodayReturn {
  scope: 'realized'
  reportingAsset: 'USDT'
  amount: number
  basisAmount: number
  rate: number
  periodStartAt: number
  calculatedAt: number
  status: TodayReturnStatus
  missingPriceAssets: string[]
}

export interface BackendTodayReturn {
  scope: unknown
  reporting_asset: unknown
  amount: unknown
  basis_amount: unknown
  rate: unknown
  period_start_at: unknown
  calculated_at: unknown
  status: unknown
  missing_price_assets: unknown
}

export type TodayReturnLoadResult = SessionRequestLoadResult<TodayReturn>
export type TodayReturnRequestLifecycle = SessionRequestLifecycle<TodayReturn>

export function mapTodayReturn(payload: BackendTodayReturn): TodayReturn {
  if (payload.scope !== 'realized') throw new Error('invalid today return scope')
  if (typeof payload.reporting_asset !== 'string'
    || payload.reporting_asset.trim().toUpperCase() !== 'USDT') {
    throw new Error('invalid today return reporting asset')
  }
  if (payload.status !== 'complete' && payload.status !== 'partial') {
    throw new Error('invalid today return status')
  }
  const missingPriceAssets = normalizeRealizedReturnAssetSymbols(
    payload.missing_price_assets,
    'missing price assets',
    'today return',
  )
  if (payload.status === 'complete' && missingPriceAssets.length) {
    throw new Error('invalid complete today return missing price assets')
  }

  const amount = requiredRealizedReturnNumber(payload.amount, 'amount', 'today return')
  const basisAmount = requiredRealizedReturnNumber(payload.basis_amount, 'basis_amount', 'today return')
  const rate = requiredRealizedReturnNumber(payload.rate, 'rate', 'today return')
  const periodStartAt = normalizeRealizedReturnTimestamp(payload.period_start_at, 'period_start_at', 'today return')
  const calculatedAt = normalizeRealizedReturnTimestamp(payload.calculated_at, 'calculated_at', 'today return')
  if (basisAmount < 0) throw new Error('invalid today return basis_amount')
  if (periodStartAt % 86_400_000 !== 0
    || calculatedAt < periodStartAt
    || calculatedAt >= periodStartAt + 86_400_000) {
    throw new Error('invalid today return UTC period')
  }

  return {
    scope: 'realized',
    reportingAsset: 'USDT',
    amount,
    basisAmount,
    rate,
    periodStartAt,
    calculatedAt,
    status: payload.status,
    missingPriceAssets,
  }
}

export function isCompleteTodayReturn(value: TodayReturn | null | undefined): value is TodayReturn {
  return value?.status === 'complete'
}

export function createTodayReturnRequestLifecycle(input: {
  sessionKey: () => string
  fetchTodayReturn: () => Promise<TodayReturn>
}): TodayReturnRequestLifecycle {
  return createSessionRequestLifecycle({
    sessionKey: input.sessionKey,
    request: input.fetchTodayReturn,
  })
}
