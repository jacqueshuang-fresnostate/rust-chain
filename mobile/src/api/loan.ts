import { client, requestUrl } from './client'
import { asNumber } from '@/core/format'
import { i18n } from '@/i18n'

export interface LoanProduct {
  id: number
  loanType: 'credit' | 'collateralized'
  assetId: number
  assetSymbol: string
  name: string
  termDays: number
  interestRate: number
  interestCalculationMode: string
  minKycLevel: number
  minAmount: number
  maxAmount?: number
}

export interface LoanOrder {
  id: number
  productId: number
  productName: string
  loanType: 'credit' | 'collateralized'
  assetSymbol: string
  amount: number
  interestRate: number
  termDays: number
  collateralAssetSymbol?: string
  collateralAmount?: number
  status: string
  interestAmount: number
  repaymentAmount: number
  dueAt?: number
  createdAt: number
}

export async function fetchLoanProducts(limit = 50): Promise<LoanProduct[]> {
  const response = await client.get<{ products?: Array<Record<string, unknown>> }>(requestUrl('/loan/products'), { params: { limit } })
  return (response.data.products || []).map((product) => ({
    id: asNumber(product.id),
    loanType: String(product.loan_type || 'credit').toLowerCase() === 'collateralized' ? 'collateralized' : 'credit',
    assetId: asNumber(product.asset_id),
    assetSymbol: String(product.asset_symbol || '').toUpperCase(),
    name: String(product.name || i18n.global.t('loan.defaultProduct')),
    termDays: asNumber(product.term_days),
    interestRate: asNumber(product.interest_rate),
    interestCalculationMode: String(product.interest_calculation_mode || ''),
    minKycLevel: asNumber(product.min_kyc_level),
    minAmount: asNumber(product.min_amount),
    maxAmount: product.max_amount === null || product.max_amount === undefined ? undefined : asNumber(product.max_amount),
  }))
}

export async function fetchLoanOrders(limit = 50): Promise<LoanOrder[]> {
  const response = await client.get<{ orders?: Array<Record<string, unknown>> }>(requestUrl('/loan/orders'), { params: { limit } })
  return (response.data.orders || []).map((order) => ({
    id: asNumber(order.id),
    productId: asNumber(order.product_id),
    productName: String(order.product_name || i18n.global.t('loan.defaultOrder')),
    loanType: String(order.loan_type || 'credit').toLowerCase() === 'collateralized' ? 'collateralized' : 'credit',
    assetSymbol: String(order.asset_symbol || '').toUpperCase(),
    amount: asNumber(order.amount),
    interestRate: asNumber(order.interest_rate),
    termDays: asNumber(order.term_days),
    collateralAssetSymbol: optionalText(order.collateral_asset_symbol),
    collateralAmount: order.collateral_amount === null || order.collateral_amount === undefined ? undefined : asNumber(order.collateral_amount),
    status: String(order.status || ''),
    interestAmount: asNumber(order.interest_amount),
    repaymentAmount: asNumber(order.repayment_amount),
    dueAt: optionalTimestamp(order.due_at),
    createdAt: normalizeTimestamp(order.created_at),
  }))
}

export async function applyLoan(input: { productId: number; amount: number; collateralAssetId?: number; collateralAmount?: number }): Promise<void> {
  await client.post(requestUrl('/loan/orders'), {
    product_id: input.productId,
    amount: String(input.amount),
    collateral_asset_id: input.collateralAssetId,
    collateral_amount: input.collateralAmount === undefined ? undefined : String(input.collateralAmount),
    idempotency_key: createIdempotencyKey('mobile-loan'),
  })
}

export async function cancelLoanOrder(orderId: number): Promise<void> {
  await client.post(requestUrl(`/loan/orders/${orderId}/cancel`), {})
}

export async function repayLoanOrder(orderId: number): Promise<void> {
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

function normalizeTimestamp(value: unknown): number {
  const timestamp = asNumber(value)
  return timestamp > 0 && timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}

function createIdempotencyKey(scope: string): string {
  return `${scope}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}
