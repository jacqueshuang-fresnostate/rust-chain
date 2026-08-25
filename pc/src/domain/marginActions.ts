export type MarginMode = 'cross' | 'isolated'
export type MarginCloseAction = 'close_long' | 'close_short'

export interface ClosableMarginPosition {
  id: number
  symbol: string
  usdtBuyPosition: number
  usdtBuyPrice: number
  usdtSellPosition: number
  usdtSellPrice: number
}

export interface MarginProductSymbol {
  id: number
  symbol: string
}

export interface MarginBatchFailure {
  id: string
  code: string
  message: string
}

export interface MarginBatchActionResult {
  succeeded: string[]
  failures: MarginBatchFailure[]
}

export type MarginBatchOutcome = 'success' | 'partial_failure' | 'failure'

export function resolveSelectedMarginMode(
  supportedModes: readonly MarginMode[],
  userSetting: string | null | undefined,
): MarginMode | null {
  const uniqueSupported = [...new Set(supportedModes)]
  const normalizedSetting = normalizeMarginMode(userSetting)
  return normalizedSetting && uniqueSupported.includes(normalizedSetting) ? normalizedSetting : null
}

/** Position rows bind to a product id so a numeric pair fallback can never target the wrong symbol. */
export function resolveMarginPositionSymbol(
  rawSymbol: string,
  productId: number | null,
  products: readonly MarginProductSymbol[],
): string {
  if (productId !== null) {
    const product = products.find((item) => item.id === productId)
    if (product) return product.symbol
  }
  return rawSymbol
}

export function findClosablePositionByAction<T extends ClosableMarginPosition>(
  positions: readonly T[],
  symbol: string,
  action: MarginCloseAction,
): T | null {
  const normalizedSymbol = normalizeSymbol(symbol)
  if (!normalizedSymbol) return null
  const target = positions
    .filter((position) => normalizeSymbol(position.symbol) === normalizedSymbol)
    .filter((position) => {
      const amount = action === 'close_long' ? position.usdtBuyPosition : position.usdtSellPosition
      const entryPrice = action === 'close_long' ? position.usdtBuyPrice : position.usdtSellPrice
      return Number.isFinite(amount) && amount > 0 && Number.isFinite(entryPrice) && entryPrice > 0
    })
    .sort((left, right) => left.id - right.id)[0]
  return target ?? null
}

/** Async setting responses may update the form only while they still belong to the active product. */
export function isMarginSettingRequestCurrent(
  expectedProductId: number,
  currentProductId: number | null | undefined,
  expectedGeneration: number,
  currentGeneration: number,
): boolean {
  return expectedProductId === currentProductId && expectedGeneration === currentGeneration
}

export function summarizeMarginBatchAction(result: MarginBatchActionResult): MarginBatchOutcome {
  if (result.failures.length === 0) return 'success'
  return result.succeeded.length > 0 ? 'partial_failure' : 'failure'
}

function normalizeMarginMode(value: string | null | undefined): MarginMode | null {
  const normalized = value?.trim().toLowerCase()
  return normalized === 'cross' || normalized === 'isolated' ? normalized : null
}

function normalizeSymbol(symbol: string): string {
  return symbol.replace(/[-_/]/g, '').toUpperCase()
}
