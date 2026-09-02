import {
  decimalAdd,
  decimalCompare,
  decimalDivide,
  decimalMultiply,
  decimalSign,
  decimalTextFromBoundary,
  formatDecimalText,
  normalizeDecimalText,
  positiveDecimalInput,
  type DecimalBoundary,
  type DecimalText,
} from './decimal.ts'
import { formatFinancialAmount } from './financialDisplay.ts'

export type TradeFinancialOrderType = 'limit' | 'market'

export interface TradeEffectivePriceInput {
  orderType: TradeFinancialOrderType | null
  limitPrice: string | DecimalText
  marketPrice: string | DecimalText | null | undefined
  maximumScale?: number
}

export interface SpotOrderReviewSnapshot {
  readonly symbol: string
  readonly baseAsset: string
  readonly quoteAsset: string
  readonly side: 'buy' | 'sell'
  readonly orderType: TradeFinancialOrderType
  readonly quantity: DecimalText
  readonly price: DecimalText
  readonly quoteAmount: DecimalText
}

export interface SpotOrderReviewInput {
  symbol: string
  side: 'buy' | 'sell'
  orderType: TradeFinancialOrderType
  quantity: string | DecimalText
  limitPrice: string | DecimalText
  marketPrice: string | DecimalText | null | undefined
}

export interface TradeWalletFinancialSource {
  available: DecimalBoundary
  frozen: DecimalBoundary
  locked: DecimalBoundary
  availableText?: DecimalBoundary
  frozenText?: DecimalBoundary
  lockedText?: DecimalBoundary
}

export interface TradeBookFinancialLevel {
  /** Legacy render coordinate. It is intentionally ignored for order-price selection. */
  price?: DecimalBoundary
  /** Exact backend decimal retained by the depth adapter. */
  priceText?: string | DecimalText | null
}

export interface TradeMarginFinancialSource {
  minMarginText?: unknown
  maxMarginText?: unknown
  hourlyInterestRate?: DecimalBoundary
  hourlyInterestRateText?: unknown
}

export interface TradeMarginFinancialBounds {
  minimum: DecimalText | null
  maximum: DecimalText | null | undefined
  isExact: boolean
}

export interface TradeFinancialPresentationOptions {
  locale: () => string
  translate: (key: string, params: Record<string, string>) => string
}

const ZERO = normalizeDecimalText('0')
const TRADE_PERCENTAGE_POINTS = new Map(
  Array.from({ length: 101 }, (_, value) => [String(value), value] as const),
)

/** Resolves the order price without passing an input or ticker through IEEE-754. */
export function resolveTradeEffectivePrice(input: TradeEffectivePriceInput): DecimalText | null {
  if (!input.orderType) return null
  const maximumScale = input.maximumScale ?? 18
  if (input.orderType === 'limit') {
    return positiveDecimalInput(input.limitPrice, maximumScale)
  }
  return exactPositiveTradeText(input.marketPrice, maximumScale)
}

/** Exact quote amount for a base quantity. The full product scale is retained. */
export function quoteAmountFromBaseQuantity(
  baseQuantity: string | DecimalText | null | undefined,
  price: string | DecimalText | null | undefined,
): DecimalText | null {
  const quantityText = exactPositiveTradeText(baseQuantity)
  const priceText = exactPositiveTradeText(price)
  return quantityText && priceText ? decimalMultiply(quantityText, priceText) : null
}

/** Exact base quantity with deterministic truncation toward zero at the asset scale. */
export function baseQuantityFromQuoteAmount(
  quoteAmount: string | DecimalText | null | undefined,
  price: string | DecimalText | null | undefined,
  scale = 18,
): DecimalText | null {
  const quoteAmountText = exactPositiveTradeText(quoteAmount)
  const priceText = exactPositiveTradeText(price)
  return quoteAmountText && priceText
    ? decimalDivide(quoteAmountText, priceText, scale)
    : null
}

/** Freezes the exact spot review values that are later displayed and submitted. */
export function createSpotOrderReviewSnapshot(
  input: SpotOrderReviewInput,
): Readonly<SpotOrderReviewSnapshot> | null {
  const quantity = exactPositiveTradeText(input.quantity)
  const price = resolveTradeEffectivePrice({
    orderType: input.orderType,
    limitPrice: input.limitPrice,
    marketPrice: input.marketPrice,
  })
  const quoteAmount = quantity && price
    ? quoteAmountFromBaseQuantity(quantity, price)
    : null
  if (!quantity || !price || !quoteAmount) return null

  const normalizedSymbol = input.symbol.trim().toUpperCase().replace(/[_-]/g, '/')
  const [baseAsset = '', quoteAsset = ''] = normalizedSymbol.split('/')
  if (!baseAsset || !quoteAsset) return null

  return Object.freeze({
    symbol: normalizedSymbol,
    baseAsset,
    quoteAsset,
    side: input.side,
    orderType: input.orderType,
    quantity,
    price,
    quoteAmount,
  })
}

/**
 * Selects an exact long-ask/short-bid price without allowing a legacy numeric book level to
 * become an order input. Until a depth adapter exposes `priceText`, the exact ticker is the
 * fail-safe fallback rather than stringifying an IEEE-754 coordinate.
 */
export function resolveTradeLimitPriceFromBook(input: {
  side: 'buy' | 'sell'
  bids: readonly TradeBookFinancialLevel[]
  asks: readonly TradeBookFinancialLevel[]
  latestPrice: string | DecimalText | null | undefined
}): DecimalText | null {
  const levels = input.side === 'buy' ? input.asks : input.bids
  const exactPrices = levels
    .map((level) => exactPositiveTradeText(level.priceText))
    .filter((value): value is DecimalText => value !== null)
  if (exactPrices.length) {
    return exactPrices.reduce((selected, candidate) => {
      const comparison = decimalCompare(candidate, selected)
      return input.side === 'buy'
        ? comparison < 0 ? candidate : selected
        : comparison > 0 ? candidate : selected
    })
  }
  return exactPositiveTradeText(input.latestPrice)
}

export function resolveTradeMarginFinancialBounds(
  source: TradeMarginFinancialSource | null | undefined,
): TradeMarginFinancialBounds {
  const minimum = exactPositiveTradeText(source?.minMarginText)
  const rawMaximum = source?.maxMarginText
  const maximum = rawMaximum === null
    ? null
    : rawMaximum === undefined
      ? undefined
      : exactPositiveTradeText(rawMaximum) ?? undefined
  return { minimum, maximum, isExact: minimum !== null && maximum !== undefined }
}

/** Maps the range control's finite enum without parsing user financial text as a number. */
export function tradePercentagePointFromText(value: string | undefined): number | null {
  return value === undefined ? null : TRADE_PERCENTAGE_POINTS.get(value) ?? null
}

/** Adds wallet components as DecimalText for visibility and presentation decisions. */
export function totalTradeBalance(
  available: DecimalBoundary,
  frozen: DecimalBoundary,
  locked: DecimalBoundary,
): DecimalText | null {
  const availableText = nonNegativeTradeBoundary(available)
  const frozenText = nonNegativeTradeBoundary(frozen)
  const lockedText = nonNegativeTradeBoundary(locked)
  if (!availableText || !frozenText || !lockedText) return null
  return decimalAdd(decimalAdd(availableText, frozenText), lockedText)
}

export function hasPositiveTradeBalance(
  available: DecimalBoundary,
  frozen: DecimalBoundary,
  locked: DecimalBoundary,
): boolean {
  const total = totalTradeBalance(available, frozen, locked)
  return total !== null && decimalSign(total) > 0
}

export function addTradeFinancialValues(
  left: DecimalBoundary,
  right: DecimalBoundary,
): DecimalText | null {
  const leftText = nonNegativeTradeBoundary(left)
  const rightText = nonNegativeTradeBoundary(right)
  return leftText && rightText ? decimalAdd(leftText, rightText) : null
}

export function positiveTradeBoundary(
  value: DecimalBoundary,
  maximumScale = 18,
): DecimalText | null {
  if (typeof value === 'string') return positiveDecimalInput(value, maximumScale)
  const normalized = decimalTextFromBoundary(value, {
    allowNegative: false,
    allowZero: false,
    maxIntegerDigits: 20,
    maxScale: maximumScale,
  })
  return normalized && decimalCompare(normalized, ZERO) > 0 ? normalized : null
}

export function nonNegativeTradeBoundary(
  value: DecimalBoundary,
  maximumScale = 18,
): DecimalText | null {
  return decimalTextFromBoundary(value, {
    allowNegative: false,
    maxIntegerDigits: 20,
    maxScale: maximumScale,
  })
}

function exactPositiveTradeText(value: unknown, maximumScale = 18): DecimalText | null {
  return typeof value === 'string' ? positiveDecimalInput(value, maximumScale) : null
}

/** Plain decimal tick-size label; exponent notation never leaks into a price UI. */
export function tradePriceStep(pricePrecision: number | null | undefined): string {
  if (!Number.isSafeInteger(pricePrecision) || pricePrecision === undefined || pricePrecision === null) {
    return '--'
  }
  if (pricePrecision < 0 || pricePrecision > 100) return '--'
  if (pricePrecision === 0) return '1'
  return `0.${'0'.repeat(pricePrecision - 1)}1`
}

export function formatTradeFinancial(
  value: DecimalBoundary,
  locale: string,
  maximumFractionDigits = 8,
  unavailable = '--',
): string {
  return formatFinancialAmount(value, locale, {
    maximumFractionDigits,
    unavailable,
  })
}

/** Formats a decimal rate (0.8) as a percent value (80) without float math. */
export function formatTradeRatePercent(
  value: DecimalBoundary,
  locale: string,
  fractionDigits: number,
): string {
  const normalized = decimalTextFromBoundary(value)
  if (!normalized) return '--'
  const percent = decimalMultiply(normalized, normalizeDecimalText('100'))
  return formatDecimalText(percent, locale, {
    maximumFractionDigits: fractionDigits,
    minimumFractionDigits: fractionDigits,
    preserveNonZero: false,
    useGrouping: false,
  })
}

/** Binds locale-only presentation helpers so TradeView keeps derivations out of the SFC. */
export function createTradeFinancialPresentation(options: TradeFinancialPresentationOptions) {
  const formatValue = (
    value: DecimalBoundary,
    maximumFractionDigits = 8,
    unavailable = '--',
  ) => formatTradeFinancial(value, options.locale(), maximumFractionDigits, unavailable)
  const walletAmount = (
    wallet: TradeWalletFinancialSource,
    field: 'available' | 'frozen' | 'locked',
  ): DecimalBoundary => wallet[`${field}Text`] ?? wallet[field]
  const frozenWalletAmount = (wallet: TradeWalletFinancialSource): DecimalText => (
    addTradeFinancialValues(walletAmount(wallet, 'frozen'), walletAmount(wallet, 'locked')) ?? ZERO
  )
  const formatRatePercent = (value: DecimalBoundary, digits: number): string => (
    formatTradeRatePercent(value, options.locale(), digits)
  )
  const formatChangePercent = (value: number | null | undefined): string => {
    if (value === null || value === undefined || !Number.isFinite(value)) return '--'
    return `${value >= 0 ? '+' : ''}${formatValue(value, 2)}%`
  }
  const formatHourlyInterest = (
    source: TradeMarginFinancialSource | null | undefined,
  ): string => {
    const exactRate = exactTradeDisplayText(source?.hourlyInterestRateText)
    const rate = exactRate ?? source?.hourlyInterestRate
    const formatted = formatRatePercent(rate, 4)
    return formatted === '--' ? '-- / --' : `${formatted}% / 1h`
  }
  const formatMarginRange = (
    source: TradeMarginFinancialSource | null | undefined,
    asset: string,
  ): string => {
    const { minimum, maximum, isExact } = resolveTradeMarginFinancialBounds(source)
    if (!isExact || !minimum) return ''
    const params = { minimum: formatValue(minimum, 8), asset }
    return maximum === null
      ? options.translate('trade.marginRangeWithoutMaximum', params)
      : options.translate('trade.marginRangeWithMaximum', {
          ...params,
          maximum: formatValue(maximum, 8),
        })
  }
  return {
    formatChangePercent,
    formatHourlyInterest,
    formatMarginRange,
    formatRatePercent,
    formatValue,
    frozenWalletAmount,
    walletAmount,
  }
}

function exactTradeDisplayText(value: unknown): DecimalText | null {
  return typeof value === 'string' ? decimalTextFromBoundary(value) : null
}

/**
 * Compatibility adapter for existing display-only component props.
 * Never use the returned number in a mutation, comparison, or financial derivation.
 */
export function legacyTradeDisplayNumber(value: DecimalBoundary): number {
  const normalized = decimalTextFromBoundary(value)
  if (!normalized) return 0
  const display = Number(normalized)
  return Number.isFinite(display) ? display : 0
}
