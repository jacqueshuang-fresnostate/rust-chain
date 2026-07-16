export type LoanNumericValue = string | number | { value?: unknown; int_val?: unknown; scale?: unknown } | null | undefined

export interface LoanEstimateProduct {
  interest_rate?: LoanNumericValue
  interestRate?: LoanNumericValue
  rate?: LoanNumericValue
  max_amount?: LoanNumericValue
  maxAmount?: LoanNumericValue
  min_amount: LoanNumericValue
  minAmount?: LoanNumericValue
}

export interface LoanRepaymentEstimateOrder {
  amount: LoanNumericValue
  interest_rate: LoanNumericValue
  interest_calculation_mode?: 'full_term' | 'actual_days' | string | null
  term_days?: number | string | null
  disbursed_at?: number | string | null
  status?: string | null
  interest_amount?: LoanNumericValue
  repayment_amount?: LoanNumericValue
}

const MILLIS_PER_DAY = 86_400_000

export function parseLoanNumber(value: LoanNumericValue): number | null {
  if (value === null || value === undefined) return null
  if (typeof value === 'object') {
    const directValue = parseLoanNumber(value.value as LoanNumericValue)
    if (directValue !== null) return directValue
    const intValue = parseLoanNumber(value.int_val as LoanNumericValue)
    const scaleValue = parseLoanNumber(value.scale as LoanNumericValue)
    if (intValue !== null && scaleValue !== null) return intValue / 10 ** scaleValue
    return null
  }
  const normalized = String(value).replace(/,/g, '').trim()
  if (!normalized) return null
  const parsed = Number(normalized)
  return Number.isFinite(parsed) ? parsed : null
}

export function normalizeLoanAmountInput(value: LoanNumericValue): string {
  if (value === null || value === undefined) return ''
  if (typeof value === 'object') {
    const parsed = parseLoanNumber(value)
    return parsed === null ? '' : String(parsed)
  }
  return String(value).replace(/,/g, '').trim()
}

export function loanProductInterestRate(product: LoanEstimateProduct | null | undefined): number | null {
  return parseLoanNumber(product?.interest_rate ?? product?.interestRate ?? product?.rate)
}

export function loanProductMinAmount(product: LoanEstimateProduct | null | undefined): number | null {
  return parseLoanNumber(product?.min_amount ?? product?.minAmount)
}

export function loanProductMaxAmount(product: LoanEstimateProduct | null | undefined): number | null {
  return parseLoanNumber(product?.max_amount ?? product?.maxAmount)
}

export function estimateLoanInterest(amount: LoanNumericValue, product: LoanEstimateProduct | null | undefined): number {
  const amountValue = parseLoanNumber(amount)
  const rateValue = loanProductInterestRate(product)
  if (amountValue === null || rateValue === null || amountValue <= 0 || rateValue < 0) return 0
  return amountValue * rateValue
}

export function estimateLoanRepayment(amount: LoanNumericValue, product: LoanEstimateProduct | null | undefined): number {
  const amountValue = parseLoanNumber(amount)
  if (amountValue === null || amountValue <= 0) return 0
  return amountValue + estimateLoanInterest(amountValue, product)
}

export function estimateLoanOrderInterest(order: LoanRepaymentEstimateOrder, now = Date.now()): number {
  const settledInterest = parseLoanNumber(order.interest_amount) ?? 0
  if (order.status !== 'disbursed') return settledInterest

  const principal = parseLoanNumber(order.amount)
  const rate = parseLoanNumber(order.interest_rate)
  if (principal === null || rate === null || principal <= 0 || rate < 0) return settledInterest

  if (order.interest_calculation_mode === 'actual_days') {
    const termDays = normalizedTermDays(order.term_days)
    const disbursedAt = parseTimestampMillis(order.disbursed_at) ?? now
    const elapsedMillis = Math.max(now - disbursedAt, 0)
    const elapsedDays = Math.max(Math.ceil(elapsedMillis / MILLIS_PER_DAY), 1)
    const chargedDays = Math.min(elapsedDays, termDays)
    return (principal * rate * chargedDays) / termDays
  }

  return principal * rate
}

export function estimateLoanOrderRepayment(order: LoanRepaymentEstimateOrder, now = Date.now()): number {
  const settledRepayment = parseLoanNumber(order.repayment_amount) ?? 0
  if (order.status !== 'disbursed') return settledRepayment

  const principal = parseLoanNumber(order.amount)
  if (principal === null || principal <= 0) return settledRepayment
  return principal + estimateLoanOrderInterest(order, now)
}

export function loanAmountRangeError(amount: LoanNumericValue, product: LoanEstimateProduct): 'invalid' | 'below_min' | 'above_max' | null {
  const amountValue = parseLoanNumber(amount)
  const minAmount = loanProductMinAmount(product)
  const maxAmount = loanProductMaxAmount(product)
  if (amountValue === null || amountValue <= 0) return 'invalid'
  if (minAmount !== null && amountValue < minAmount) return 'below_min'
  if (maxAmount !== null && amountValue > maxAmount) return 'above_max'
  return null
}

function normalizedTermDays(value: number | string | null | undefined): number {
  const parsed = Number(value)
  if (!Number.isFinite(parsed) || parsed <= 0) return 1
  return Math.max(Math.floor(parsed), 1)
}

function parseTimestampMillis(value: number | string | null | undefined): number | null {
  if (value === null || value === undefined || value === '') return null
  const numericValue = Number(value)
  if (Number.isFinite(numericValue) && numericValue > 0) {
    return numericValue < 10_000_000_000 ? numericValue * 1000 : numericValue
  }
  const parsed = Date.parse(String(value))
  return Number.isFinite(parsed) ? parsed : null
}
