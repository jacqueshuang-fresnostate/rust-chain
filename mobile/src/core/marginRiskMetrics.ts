export type MarginLiquidationRiskScope = 'position' | 'account'

const MARGIN_RISK_DECIMAL_PATTERN = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/

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
}

export interface MarginPositionRiskMetrics {
  maintenanceMarginRate: number | null
  estimatedLiquidationPrice: number | null
  liquidationRiskScope: MarginLiquidationRiskScope
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
    return {
      maintenanceMarginRate,
      estimatedLiquidationPrice: null,
      liquidationRiskScope: 'account',
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
    liquidationRiskScope: 'position',
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
