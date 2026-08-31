export type MarginLeverageDirection = 'long' | 'short'
export type MarginMode = 'cross' | 'isolated'

export interface MarginLeveragePreviewInput {
  availableBalance: number
  referencePrice: number
  marginAmount: number
  leverage: number
  maintenanceMarginRate?: number | null
  marginMode: MarginMode
  direction: MarginLeverageDirection
}

export interface MarginLeveragePreview {
  maximumOpenQuantity: number | null
  requiredMargin: number | null
  estimatedLiquidationPrice: number | null
}

export interface MarginUserLeverageSetting {
  leverage: number | null
  longLeverage: number | null
  shortLeverage: number | null
}

/**
 * 严格解析设置响应中的正数倍数，并只对确实缺少方向字段的旧响应回落 legacy 值。
 *
 * 新响应若显式给出 null、零、负数或非法文本，保留为未设置，避免把损坏的方向字段
 * 静默伪装成兼容值；旧服务完全没有方向字段时才将 legacy 值复制到两边。
 */
export function mapMarginUserLeverageSetting(input: {
  leverage?: unknown
  long_leverage?: unknown
  short_leverage?: unknown
}): MarginUserLeverageSetting {
  const leverage = positiveNumericValue(input.leverage)
  const hasLongLeverage = Object.prototype.hasOwnProperty.call(input, 'long_leverage')
  const hasShortLeverage = Object.prototype.hasOwnProperty.call(input, 'short_leverage')
  return {
    leverage,
    longLeverage: hasLongLeverage ? positiveNumericValue(input.long_leverage) : leverage,
    shortLeverage: hasShortLeverage ? positiveNumericValue(input.short_leverage) : leverage,
  }
}

/**
 * 将后台产品档位收敛为唯一、正数且升序的集合。
 *
 * 弹窗的所有加减与快捷入口都必须从这个集合派生，避免界面生成后台不接受的倍数。
 */
export function normalizeMarginLeverageLevels(levels: readonly number[]): number[] {
  return [...new Set(levels.filter((level) => Number.isFinite(level) && level > 0))]
    .sort((left, right) => left - right)
}

/** 返回当前档位相邻的真实产品档位；到达边界时保持当前值。 */
export function stepMarginLeverage(
  levels: readonly number[],
  current: number,
  direction: -1 | 1,
): number {
  const normalized = normalizeMarginLeverageLevels(levels)
  if (!normalized.length) return current
  const currentIndex = normalized.indexOf(current)
  const fallbackIndex = currentIndex >= 0 ? currentIndex : 0
  const nextIndex = Math.max(0, Math.min(fallbackIndex + direction, normalized.length - 1))
  return normalized[nextIndex] ?? current
}

/**
 * 生成 Pencil 胶囊轨使用的六档窗口。
 *
 * 当前档位尽量落在第 4 个位置；低档位从首项开始，高档位自动右移，正好覆盖选稿中的
 * `[1,2,3,5,10,20]` 与 `[5,10,20,30,50,75]` 两种状态。
 */
export function marginLeverageWindowStart(
  levels: readonly number[],
  current: number,
  size = 6,
): number {
  const normalized = normalizeMarginLeverageLevels(levels)
  if (normalized.length <= size) return 0
  const index = Math.max(0, normalized.indexOf(current))
  return Math.max(0, Math.min(index - 3, normalized.length - size))
}

export function marginLeverageWindow(
  levels: readonly number[],
  start: number,
  size = 6,
): number[] {
  const normalized = normalizeMarginLeverageLevels(levels)
  const maximumStart = Math.max(0, normalized.length - size)
  const safeStart = Math.max(0, Math.min(Math.trunc(start), maximumStart))
  return normalized.slice(safeStart, safeStart + size)
}

/** 更多入口向右移动一个完整窗口；最后一页再次点击会回到首个窗口。 */
export function nextMarginLeverageWindowStart(
  levels: readonly number[],
  currentStart: number,
  size = 6,
): number {
  const normalized = normalizeMarginLeverageLevels(levels)
  const maximumStart = Math.max(0, normalized.length - size)
  if (maximumStart === 0) return 0
  return currentStart >= maximumStart ? 0 : Math.min(currentStart + size, maximumStart)
}

/**
 * 用当前真实余额、行情和产品维持保证金率构造只读预览。
 *
 * 全仓强平是账户级条件，不能用单仓公式伪造，因此只在逐仓时返回预估强平价。
 */
export function createMarginLeveragePreview(input: MarginLeveragePreviewInput): MarginLeveragePreview {
  const balance = positiveFinite(input.availableBalance)
  const price = positiveFinite(input.referencePrice)
  const leverage = positiveFinite(input.leverage)
  const marginAmount = nonNegativeFinite(input.marginAmount)
  const maintenanceRate = nonNegativeFinite(input.maintenanceMarginRate)

  const maximumOpenQuantity = balance !== null && price !== null && leverage !== null
    ? finiteOrNull((balance * leverage) / price)
    : null
  const requiredMargin = marginAmount

  if (
    input.marginMode !== 'isolated'
    || price === null
    || leverage === null
    || maintenanceRate === null
  ) {
    return { maximumOpenQuantity, requiredMargin, estimatedLiquidationPrice: null }
  }

  const multiplier = input.direction === 'long'
    ? 1 - (1 / leverage) + maintenanceRate
    : 1 + (1 / leverage) - maintenanceRate
  const estimatedLiquidationPrice = positiveFinite(price * multiplier)
  return { maximumOpenQuantity, requiredMargin, estimatedLiquidationPrice }
}

function positiveFinite(value: number | null | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : null
}

function positiveNumericValue(value: unknown): number | null {
  const parsed = typeof value === 'number' ? value : Number(value)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null
}

function nonNegativeFinite(value: number | null | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null
}

function finiteOrNull(value: number): number | null {
  return Number.isFinite(value) && value >= 0 ? value : null
}
