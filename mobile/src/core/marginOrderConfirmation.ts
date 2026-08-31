import {
  validateMarginAmount,
  validateMarginLimitPrice,
  type MarginAmountValidation,
  type MarginLimitPriceValidation,
} from './tradeForm.ts'
import type { MarginOrderType } from './types.ts'
import {
  decimalDivide,
  decimalMultiply,
  decimalTextFromBoundary,
  decimalTextFromFiniteNumber,
  normalizeDecimalText,
  positiveDecimalInput,
  type DecimalBoundary,
  type DecimalText,
} from './decimal.ts'

export interface MarginOrderReviewInput {
  productId: number
  side: 'buy' | 'sell'
  marginMode: 'cross' | 'isolated'
  leverage: number
  marginAmount: string
  orderType: MarginOrderType | null
  limitPrice: string
  pricePrecision?: number | null
  idempotencyKey?: string
  minMargin?: DecimalBoundary
  maxMargin?: DecimalBoundary
  referencePrice: number
  referencePriceText?: DecimalBoundary
}

export interface MarginOrderRequest {
  productId: number
  side: 'long' | 'short'
  marginMode: 'cross' | 'isolated'
  leverage: number
  marginAmount: DecimalText
  orderType: MarginOrderType
  price?: DecimalText
  idempotencyKey?: string
}

export interface MarginOrderReview {
  isValid: boolean
  referencePrice: number
  referencePriceText: DecimalText | null
  estimatedNotional: number
  estimatedNotionalText: DecimalText
  estimatedQuantity: number
  estimatedQuantityText: DecimalText
  marginAmountValidation: MarginAmountValidation
  limitPriceValidation: MarginLimitPriceValidation
  request: MarginOrderRequest
  marginAmountText: DecimalText | null
}

export type MarginOrderBackendBoundaryError = 'below-minimum' | 'above-maximum'

/** Freezes exact money text and derives review estimates without using IEEE-754 money math. */
export function createMarginOrderReview(input: MarginOrderReviewInput): MarginOrderReview {
  const marginAmountText = positiveDecimalInput(input.marginAmount)
  const marginAmountValidation = validateMarginAmount({
    amount: input.marginAmount,
    minMargin: input.minMargin,
    maxMargin: input.maxMargin,
  })
  const limitPriceValidation = validateMarginLimitPrice({
    price: input.limitPrice,
    pricePrecision: input.pricePrecision,
  })
  const referencePriceText = decimalTextFromBoundary(
    input.referencePriceText ?? input.referencePrice,
    { allowNegative: false, allowZero: false },
  )
  const leverageText = Number.isFinite(input.leverage) && input.leverage > 0
    ? decimalTextFromFiniteNumber(input.leverage)
    : null
  const estimatedNotionalText = marginAmountText && leverageText
    ? decimalMultiply(marginAmountText, leverageText)
    : normalizeDecimalText('0')
  const estimatedQuantityText = referencePriceText && estimatedNotionalText !== '0'
    ? decimalDivide(estimatedNotionalText, referencePriceText, 18)
    : normalizeDecimalText('0')
  const request: MarginOrderRequest = {
    productId: input.productId,
    side: input.side === 'buy' ? 'long' : 'short',
    marginMode: input.marginMode,
    leverage: input.leverage,
    marginAmount: marginAmountText || normalizeDecimalText('0'),
    orderType: input.orderType || 'market',
    ...(input.orderType === 'limit' && limitPriceValidation.normalized
      ? { price: limitPriceValidation.normalized }
      : {}),
    ...(input.idempotencyKey ? { idempotencyKey: input.idempotencyKey } : {}),
  }
  const estimatedNotional = displayNumber(estimatedNotionalText)
  const estimatedQuantity = displayNumber(estimatedQuantityText)

  return {
    isValid: Number.isFinite(input.productId)
      && input.productId > 0
      && marginAmountText !== null
      && input.orderType !== null
      && marginAmountValidation.isValid
      && (input.orderType !== 'limit' || limitPriceValidation.isValid)
      && referencePriceText !== null
      && estimatedNotionalText !== '0'
      && estimatedQuantityText !== '0',
    referencePrice: input.referencePrice,
    referencePriceText,
    estimatedNotional,
    estimatedNotionalText,
    estimatedQuantity,
    estimatedQuantityText,
    marginAmountValidation,
    limitPriceValidation,
    request,
    marginAmountText,
  }
}

/** Recognizes only the two stable backend margin-boundary diagnostics. */
export function classifyMarginOrderBackendBoundaryError(
  message: string,
): MarginOrderBackendBoundaryError | null {
  const normalized = message.trim().toLowerCase()
  if (normalized.includes('margin amount is below product minimum')) return 'below-minimum'
  if (normalized.includes('margin amount exceeds product maximum')) return 'above-maximum'
  return null
}

function displayNumber(value: DecimalText): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : 0
}
