export type MarginLiquidationRiskScope = 'position' | 'account'

const MARGIN_RISK_DECIMAL_PATTERN = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/

export type MarginCrossAccountPriceAssumption = 'reference_pair_only_other_marks_static'

/** Display-only account snapshot; converted DECIMAL values must never enter a mutation payload. */
export interface MarginCrossAccountRisk {
  marginAssetId: number
  referencePairId: number
  priceAssumption: MarginCrossAccountPriceAssumption
  equity: number
  maintenanceMargin: number
  liquidationBuffer: number
  marginRatio: number | null
  unrealizedPnl: number
  interestAmount: number
  shouldLiquidate: boolean
  netQuantity: number
  grossQuantity: number
  estimateStatus: string
  conditionalLiquidationPrice: number | null
  conditionalLiquidationDistanceRate: number | null
  marksObservedAtMin: number
  marksObservedAtMax: number
}

export interface IsolatedLiquidationPriceInput {
  direction: unknown
  entryPrice: unknown
  notionalAmount: unknown
  marginAmount: unknown
  interestAmount: unknown
  maintenanceMarginRate: unknown
}

export interface MarginPositionRiskMetricsInput extends Omit<IsolatedLiquidationPriceInput, 'maintenanceMarginRate'> {
  marginMode: unknown
  serverMaintenanceMarginRate?: unknown
  productMaintenanceMarginRate?: unknown
  serverEstimatedLiquidationPrice?: unknown
  serverLiquidationDistanceRate?: unknown
  crossAccountRisk?: {
    estimateStatus?: unknown
    conditionalLiquidationPrice?: unknown
    conditionalLiquidationDistanceRate?: unknown
  } | null
}

export type MarginCrossAccountEstimateState = 'legacy' | 'estimated' | 'unavailable'

export interface MarginPositionRiskMetrics {
  maintenanceMarginRate: number | null
  estimatedLiquidationPrice: number | null
  liquidationDistanceRate: number | null
  liquidationRiskScope: MarginLiquidationRiskScope
  crossAccountEstimateState: MarginCrossAccountEstimateState | null
}

/** Parses only finite JSON numbers or backend DECIMAL strings used by risk display inputs. */
export function parseMarginRiskNumber(value: unknown): number | null {
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value === 0 ? 0 : value : null
  }
  if (typeof value !== 'string') return null
  const normalized = value.trim()
  if (!normalized || !MARGIN_RISK_DECIMAL_PATTERN.test(normalized)) return null
  const parsed = Number(normalized)
  return Number.isFinite(parsed) ? parsed === 0 ? 0 : parsed : null
}

/** Strictly maps the optional backend account object; null/missing alone means an older backend. */
export function mapMarginCrossAccountRisk(value: unknown): MarginCrossAccountRisk | undefined {
  if (value === null || value === undefined) return undefined
  if (!isRecord(value)) throw new TypeError('invalid cross account risk object')

  const priceAssumption = requiredMarginRiskString(value.price_assumption, 'price_assumption')
  if (priceAssumption !== 'reference_pair_only_other_marks_static') {
    throw new TypeError('invalid cross account risk price_assumption')
  }
  const marksObservedAtMin = requiredMarginRiskTimestamp(
    value.marks_observed_at_min,
    'marks_observed_at_min',
  )
  const marksObservedAtMax = requiredMarginRiskTimestamp(
    value.marks_observed_at_max,
    'marks_observed_at_max',
  )
  if (marksObservedAtMin > marksObservedAtMax) {
    throw new TypeError('invalid cross account risk mark observation range')
  }

  return {
    marginAssetId: requiredMarginRiskId(value.margin_asset, 'margin_asset'),
    referencePairId: requiredMarginRiskId(value.reference_pair_id, 'reference_pair_id'),
    priceAssumption,
    equity: strictMarginRiskNumber(value.equity, 'equity'),
    maintenanceMargin: strictMarginRiskNumber(value.maintenance_margin, 'maintenance_margin'),
    liquidationBuffer: strictMarginRiskNumber(value.liquidation_buffer, 'liquidation_buffer'),
    marginRatio: strictNullableMarginRiskNumber(value.margin_ratio, 'margin_ratio'),
    unrealizedPnl: strictMarginRiskNumber(value.unrealized_pnl, 'unrealized_pnl'),
    interestAmount: strictMarginRiskNumber(value.interest_amount, 'interest_amount'),
    shouldLiquidate: requiredMarginRiskBoolean(value.should_liquidate, 'should_liquidate'),
    netQuantity: strictMarginRiskNumber(value.net_quantity, 'net_quantity'),
    grossQuantity: strictMarginRiskNumber(value.gross_quantity, 'gross_quantity'),
    estimateStatus: requiredMarginRiskString(value.estimate_status, 'estimate_status'),
    conditionalLiquidationPrice: strictNullableMarginRiskNumber(
      value.conditional_liquidation_price,
      'conditional_liquidation_price',
    ),
    conditionalLiquidationDistanceRate: strictNullableMarginRiskNumber(
      value.conditional_liquidation_distance_rate,
      'conditional_liquidation_distance_rate',
    ),
    marksObservedAtMin,
    marksObservedAtMax,
  }
}

export function resolveMaintenanceMarginRate(
  serverValue: unknown,
  productValue: unknown,
): number | null {
  return finiteNonNegativeNumber(serverValue) ?? finiteNonNegativeNumber(productValue)
}

export function estimateIsolatedLiquidationPrice(
  input: IsolatedLiquidationPriceInput,
): number | null {
  const entryPrice = finitePositiveNumber(input.entryPrice)
  const notionalAmount = finitePositiveNumber(input.notionalAmount)
  const marginAmount = finitePositiveNumber(input.marginAmount)
  const interestAmount = finiteNonNegativeNumber(input.interestAmount)
  const maintenanceMarginRate = finiteNonNegativeNumber(input.maintenanceMarginRate)
  if (
    entryPrice === null
    || notionalAmount === null
    || marginAmount === null
    || interestAmount === null
    || maintenanceMarginRate === null
    || (input.direction !== 'long' && input.direction !== 'short')
  ) {
    return null
  }

  const maintenance = notionalAmount * maintenanceMarginRate
  const adjustment = (maintenance - marginAmount + interestAmount) / notionalAmount
  const multiplier = input.direction === 'long' ? 1 + adjustment : 1 - adjustment
  return finitePositiveNumber(entryPrice * multiplier)
}

export function resolveMarginPositionRiskMetrics(
  input: MarginPositionRiskMetricsInput,
): MarginPositionRiskMetrics {
  const maintenanceMarginRate = resolveMaintenanceMarginRate(
    input.serverMaintenanceMarginRate,
    input.productMaintenanceMarginRate,
  )

  if (input.marginMode === 'cross') {
    if (!input.crossAccountRisk) {
      return {
        maintenanceMarginRate,
        estimatedLiquidationPrice: null,
        liquidationDistanceRate: null,
        liquidationRiskScope: 'account',
        crossAccountEstimateState: 'legacy',
      }
    }

    const conditionalLiquidationPrice = finitePositiveNumber(
      input.crossAccountRisk.conditionalLiquidationPrice,
    )
    const hasStableEstimate = input.crossAccountRisk.estimateStatus === 'estimated'
      && conditionalLiquidationPrice !== null
    return {
      maintenanceMarginRate,
      estimatedLiquidationPrice: hasStableEstimate ? conditionalLiquidationPrice : null,
      liquidationDistanceRate: hasStableEstimate
        ? finiteNonNegativeNumber(input.crossAccountRisk.conditionalLiquidationDistanceRate)
        : null,
      liquidationRiskScope: 'account',
      crossAccountEstimateState: hasStableEstimate ? 'estimated' : 'unavailable',
    }
  }

  const serverEstimatedLiquidationPrice = finitePositiveNumber(
    input.serverEstimatedLiquidationPrice,
  )
  const estimatedLiquidationPrice = input.marginMode === 'isolated'
    ? serverEstimatedLiquidationPrice ?? estimateIsolatedLiquidationPrice({
      direction: input.direction,
      entryPrice: input.entryPrice,
      notionalAmount: input.notionalAmount,
      marginAmount: input.marginAmount,
      interestAmount: input.interestAmount,
      maintenanceMarginRate,
    })
    : null

  return {
    maintenanceMarginRate,
    estimatedLiquidationPrice,
    liquidationDistanceRate: finiteNonNegativeNumber(input.serverLiquidationDistanceRate),
    liquidationRiskScope: 'position',
    crossAccountEstimateState: null,
  }
}

function finiteNonNegativeNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
    ? value === 0 ? 0 : value
    : null
}

function finitePositiveNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : null
}

function strictMarginRiskNumber(value: unknown, field: string): number {
  const parsed = parseMarginRiskNumber(value)
  if (parsed === null) throw new TypeError(`invalid cross account risk ${field}`)
  return parsed
}

function strictNullableMarginRiskNumber(value: unknown, field: string): number | null {
  if (value === null) return null
  return strictMarginRiskNumber(value, field)
}

function requiredMarginRiskId(value: unknown, field: string): number {
  const parsed = strictMarginRiskNumber(value, field)
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new TypeError(`invalid cross account risk ${field}`)
  }
  return parsed
}

function requiredMarginRiskTimestamp(value: unknown, field: string): number {
  const parsed = strictMarginRiskNumber(value, field)
  if (!Number.isSafeInteger(parsed) || parsed < 0) {
    throw new TypeError(`invalid cross account risk ${field}`)
  }
  return parsed
}

function requiredMarginRiskString(value: unknown, field: string): string {
  if (typeof value !== 'string' || !value.trim()) {
    throw new TypeError(`invalid cross account risk ${field}`)
  }
  return value.trim()
}

function requiredMarginRiskBoolean(value: unknown, field: string): boolean {
  if (typeof value !== 'boolean') throw new TypeError(`invalid cross account risk ${field}`)
  return value
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
