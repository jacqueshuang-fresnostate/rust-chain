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

export type TodayReturnLoadResult =
  | { state: 'guest' }
  | { state: 'loaded'; value: TodayReturn }
  | { state: 'error'; error: unknown }
  | { state: 'stale' }

export interface TodayReturnRequestLifecycle {
  load: () => Promise<TodayReturnLoadResult>
  invalidate: () => void
  stop: () => void
}

export function mapTodayReturn(payload: BackendTodayReturn): TodayReturn {
  if (payload.scope !== 'realized') throw new Error('invalid today return scope')
  if (typeof payload.reporting_asset !== 'string'
    || payload.reporting_asset.trim().toUpperCase() !== 'USDT') {
    throw new Error('invalid today return reporting asset')
  }
  if (payload.status !== 'complete' && payload.status !== 'partial') {
    throw new Error('invalid today return status')
  }
  if (!Array.isArray(payload.missing_price_assets)) {
    throw new Error('invalid today return missing price assets')
  }
  const missingPriceAssets = [...new Set(payload.missing_price_assets.map((asset) => {
    if (typeof asset !== 'string') throw new Error('invalid today return missing price asset')
    const normalized = asset.trim().toUpperCase()
    if (!/^[A-Z0-9]{1,32}$/.test(normalized)) {
      throw new Error('invalid today return missing price asset')
    }
    return normalized
  }))]
  if (payload.status === 'complete' && missingPriceAssets.length) {
    throw new Error('invalid complete today return missing price assets')
  }

  const amount = requiredFiniteTodayReturnNumber(payload.amount, 'amount')
  const basisAmount = requiredFiniteTodayReturnNumber(payload.basis_amount, 'basis_amount')
  const rate = requiredFiniteTodayReturnNumber(payload.rate, 'rate')
  const periodStartAt = normalizeTodayReturnTimestamp(payload.period_start_at, 'period_start_at')
  const calculatedAt = normalizeTodayReturnTimestamp(payload.calculated_at, 'calculated_at')
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
  let requestVersion = 0
  let active = true

  return {
    async load(): Promise<TodayReturnLoadResult> {
      const version = ++requestVersion
      if (!active) return { state: 'stale' }
      const sessionKey = input.sessionKey()
      if (!sessionKey) return { state: 'guest' }

      try {
        const value = await input.fetchTodayReturn()
        if (!active || version !== requestVersion || input.sessionKey() !== sessionKey) {
          return { state: 'stale' }
        }
        return { state: 'loaded', value }
      } catch (error) {
        if (!active || version !== requestVersion || input.sessionKey() !== sessionKey) {
          return { state: 'stale' }
        }
        return { state: 'error', error }
      }
    },
    invalidate(): void {
      requestVersion += 1
    },
    stop(): void {
      active = false
      requestVersion += 1
    },
  }
}

function requiredFiniteTodayReturnNumber(value: unknown, field: string): number {
  if (typeof value !== 'number' && typeof value !== 'string') {
    throw new Error(`invalid today return ${field}`)
  }
  if (typeof value === 'string' && !value.trim()) {
    throw new Error(`invalid today return ${field}`)
  }
  if (typeof value === 'string' && !/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/.test(value.trim())) {
    throw new Error(`invalid today return ${field}`)
  }
  const parsed = typeof value === 'number' ? value : Number(value.trim())
  if (!Number.isFinite(parsed)) throw new Error(`invalid today return ${field}`)
  return parsed
}

function normalizeTodayReturnTimestamp(value: unknown, field: string): number {
  const parsed = requiredFiniteTodayReturnNumber(value, field)
  if (parsed <= 0 || !Number.isSafeInteger(parsed)) throw new Error(`invalid today return ${field}`)
  const normalized = parsed < 1_000_000_000_000 ? parsed * 1000 : parsed
  if (!Number.isSafeInteger(normalized)) throw new Error(`invalid today return ${field}`)
  return normalized
}
