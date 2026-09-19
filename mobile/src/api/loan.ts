import { client, readAuthSessionSnapshot, requestUrl } from './client'
import { canonicalRequestIntent, RetryStableIdempotencyKeys } from './idempotency'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'
import { requiredId, requiredSafeInteger, normalizeTimestamp } from '@/core/numeric'
import { i18n } from '@/i18n'
import { requiredDecimalText, normalizeDecimalText, type DecimalText } from '@/core/decimal'

const applicationKeys = new RetryStableIdempotencyKeys('mobile-loan', undefined, () => globalThis.sessionStorage)

export interface LoanProduct {
  id: number
  loanType: 'credit' | 'collateralized'
  assetId: number
  assetSymbol: string
  name: string
  termDays: number
  interestRate: DecimalText
  interestCalculationMode: string
  minKycLevel: number
  minAmount: DecimalText
  maxAmount?: DecimalText
  minAmountText?: DecimalText
  maxAmountText?: DecimalText
}

export interface LoanOrder {
  id: number
  productId: number
  productName: string
  loanType: 'credit' | 'collateralized'
  assetSymbol: string
  amount: DecimalText
  interestRate: DecimalText
  termDays: number
  collateralAssetSymbol?: string
  collateralAmount?: DecimalText
  status: string
  interestAmount: DecimalText
  repaymentAmount: DecimalText
  dueAt?: number
  createdAt: number
}

export async function fetchLoanProducts(limit = 50, options: ReferenceRequestOptions = {}): Promise<LoanProduct[]> {
  requiredSafeInteger(limit, 'limit', 1)
  const url = requestUrl('/loan/products')
  return referenceRequestRegistry.request(createReferenceRequestKey(url, {
    limit,
    locale: i18n.global.locale.value,
  }), 60_000, async () => {
    const response = await client.get<{ products?: Array<Record<string, unknown>> }>(url, { params: { limit } })
    return (response.data.products || []).map((product) => ({
      id: requiredId(product.id),
      loanType: String(product.loan_type || 'credit').toLowerCase() === 'collateralized' ? 'collateralized' : 'credit',
      assetId: requiredId(product.asset_id),
      assetSymbol: String(product.asset_symbol || '').toUpperCase(),
      name: String(product.name || i18n.global.t('loan.defaultProduct')),
      termDays: requiredSafeInteger(product.term_days, 'term_days', 1),
      interestRate: productDecimal(product.interest_rate),
      interestCalculationMode: String(product.interest_calculation_mode || ''),
      minKycLevel: requiredSafeInteger(product.min_kyc_level, 'min_kyc_level'),
      minAmount: productDecimal(product.min_amount),
      maxAmount: product.max_amount === null || product.max_amount === undefined ? undefined : productDecimal(product.max_amount),
      minAmountText: productDecimal(product.min_amount),
      maxAmountText: product.max_amount === null || product.max_amount === undefined
        ? undefined
        : productDecimal(product.max_amount),
    }))
  }, options)
}

export async function fetchLoanOrders(limit = 50): Promise<LoanOrder[]> {
  requiredSafeInteger(limit, 'limit', 1)
  const response = await client.get<{ orders?: Array<Record<string, unknown>> }>(requestUrl('/loan/orders'), { params: { limit } })
  return (response.data.orders || []).map((order) => ({
    id: requiredId(order.id),
    productId: requiredId(order.product_id),
    productName: String(order.product_name || i18n.global.t('loan.defaultOrder')),
    loanType: String(order.loan_type || 'credit').toLowerCase() === 'collateralized' ? 'collateralized' : 'credit',
    assetSymbol: String(order.asset_symbol || '').toUpperCase(),
    amount: productDecimal(order.amount),
    interestRate: productDecimal(order.interest_rate),
    termDays: requiredSafeInteger(order.term_days, 'term_days', 1),
    collateralAssetSymbol: optionalText(order.collateral_asset_symbol),
    collateralAmount: order.collateral_amount === null || order.collateral_amount === undefined ? undefined : productDecimal(order.collateral_amount),
    status: String(order.status || ''),
    interestAmount: productDecimal(order.interest_amount),
    repaymentAmount: productDecimal(order.repayment_amount),
    dueAt: optionalTimestamp(order.due_at),
    createdAt: normalizeTimestamp(order.created_at),
  }))
}

export async function applyLoan(input: {
  productId: number
  amount: DecimalText
  collateralAssetId?: number
  collateralAmount?: DecimalText
}): Promise<void> {
  const scope = readAuthSessionSnapshot().scope
  if (!scope) throw new Error('authenticated session is required')
  const payload = {
    product_id: requiredId(input.productId),
    amount: normalizeDecimalText(input.amount),
    collateral_asset_id: input.collateralAssetId === undefined ? undefined : requiredId(input.collateralAssetId),
    collateral_amount: input.collateralAmount === undefined
      ? undefined
      : normalizeDecimalText(input.collateralAmount),
  }
  const intent = canonicalRequestIntent({ ...payload, scope })
  const key = applicationKeys.acquire(intent)
  await client.post(requestUrl('/loan/orders'), {
    ...payload,
    idempotency_key: key,
  })
  applicationKeys.complete(intent, key)
}

export async function cancelLoanOrder(orderId: number): Promise<void> {
  requiredId(orderId)
  await client.post(requestUrl(`/loan/orders/${orderId}/cancel`), {})
}

export async function repayLoanOrder(orderId: number): Promise<void> {
  requiredId(orderId)
  await client.post(requestUrl(`/loan/orders/${orderId}/repay`), {})
}

function optionalText(value: unknown): string | undefined {
  const text = typeof value === 'string' ? value.trim() : ''
  return text || undefined
}

function optionalTimestamp(value: unknown): number | undefined {
  const timestamp = normalizeTimestamp(value)
  return timestamp || undefined
}

function productDecimal(value: unknown): DecimalText {
  return requiredDecimalText(value, 'decimal', 'loan', { allowNegative: false, maxIntegerDigits: 20, maxScale: 18 })
}
