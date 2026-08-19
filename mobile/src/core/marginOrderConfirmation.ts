import {
  validateMarginAmount,
  validateMarginLimitPrice,
  type MarginAmountValidation,
  type MarginLimitPriceValidation,
} from './tradeForm.ts'
import type { MarginOrderType } from './types.ts'

export interface MarginOrderReviewInput {
  productId: number
  side: 'buy' | 'sell'
  marginMode: 'cross' | 'isolated'
  leverage: number
  marginAmount: number
  orderType: MarginOrderType | null
  limitPrice: string
  pricePrecision?: number | null
  idempotencyKey?: string
  minMargin?: number
  maxMargin?: number | null
  referencePrice: number
}

export interface MarginOrderRequest {
  productId: number
  side: 'long' | 'short'
  marginMode: 'cross' | 'isolated'
  leverage: number
  marginAmount: number
  orderType: MarginOrderType
  price?: string
  idempotencyKey?: string
}

export interface MarginOrderReview {
  isValid: boolean
  referencePrice: number
  estimatedNotional: number
  estimatedQuantity: number
  marginAmountValidation: MarginAmountValidation
  limitPriceValidation: MarginLimitPriceValidation
  request: MarginOrderRequest
}

export type MarginOrderBackendBoundaryError = 'below-minimum' | 'above-maximum'

/**
 * Builds the contract review and API input from one set of current form values.
 * The reference ticker, limit intent and idempotency key are copied into one immutable review.
 * A limit price is only a trigger boundary; estimated quantity still uses the frozen live reference.
 */
export function createMarginOrderReview(input: MarginOrderReviewInput): MarginOrderReview {
  const marginAmountValidation = validateMarginAmount({
    amount: input.marginAmount,
    minMargin: input.minMargin ?? 0,
    maxMargin: input.maxMargin,
  })
  const limitPriceValidation = validateMarginLimitPrice({
    price: input.limitPrice,
    pricePrecision: input.pricePrecision,
  })
  const rawEstimatedNotional = Number.isFinite(input.marginAmount)
    && input.marginAmount > 0
    && Number.isFinite(input.leverage)
    && input.leverage > 0
    ? input.marginAmount * input.leverage
    : 0
  const estimatedNotional = Number.isFinite(rawEstimatedNotional) ? rawEstimatedNotional : 0
  const rawEstimatedQuantity = estimatedNotional > 0
    && Number.isFinite(input.referencePrice)
    && input.referencePrice > 0
    ? estimatedNotional / input.referencePrice
    : 0
  const estimatedQuantity = Number.isFinite(rawEstimatedQuantity) ? rawEstimatedQuantity : 0
  const request: MarginOrderRequest = {
    productId: input.productId,
    side: input.side === 'buy' ? 'long' : 'short',
    marginMode: input.marginMode,
    leverage: input.leverage,
    marginAmount: input.marginAmount,
    orderType: input.orderType || 'market',
    ...(input.orderType === 'limit' && limitPriceValidation.normalized
      ? { price: limitPriceValidation.normalized }
      : {}),
    ...(input.idempotencyKey ? { idempotencyKey: input.idempotencyKey } : {}),
  }

  return {
    isValid: Number.isFinite(input.productId)
      && input.productId > 0
      && input.orderType !== null
      && marginAmountValidation.isValid
      && (input.orderType !== 'limit' || limitPriceValidation.isValid)
      && Number.isFinite(input.referencePrice)
      && input.referencePrice > 0
      && estimatedNotional > 0
      && estimatedQuantity > 0,
    referencePrice: input.referencePrice,
    estimatedNotional,
    estimatedQuantity,
    marginAmountValidation,
    limitPriceValidation,
    request,
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
