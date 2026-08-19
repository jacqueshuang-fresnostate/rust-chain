import {
  validateMarginAmount,
  type MarginAmountValidation,
} from './tradeForm.ts'

export interface MarginOrderReviewInput {
  productId: number
  side: 'buy' | 'sell'
  marginMode: 'cross' | 'isolated'
  leverage: number
  marginAmount: number
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
  idempotencyKey?: string
}

export interface MarginOrderReview {
  isValid: boolean
  referencePrice: number
  estimatedNotional: number
  estimatedQuantity: number
  marginAmountValidation: MarginAmountValidation
  request: MarginOrderRequest
}

export type MarginOrderBackendBoundaryError = 'below-minimum' | 'above-maximum'

/**
 * Builds the contract review and API input from one set of current form values.
 * The live market price is review-only because margin positions execute at market.
 */
export function createMarginOrderReview(input: MarginOrderReviewInput): MarginOrderReview {
  const marginAmountValidation = validateMarginAmount({
    amount: input.marginAmount,
    minMargin: input.minMargin ?? 0,
    maxMargin: input.maxMargin,
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
    ...(input.idempotencyKey ? { idempotencyKey: input.idempotencyKey } : {}),
  }

  return {
    isValid: Number.isFinite(input.productId)
      && input.productId > 0
      && marginAmountValidation.isValid
      && estimatedNotional > 0
      && estimatedQuantity > 0,
    referencePrice: input.referencePrice,
    estimatedNotional,
    estimatedQuantity,
    marginAmountValidation,
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
