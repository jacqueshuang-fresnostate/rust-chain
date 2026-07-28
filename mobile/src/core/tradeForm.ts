export interface BalancePercentageInput {
  available: number
  mode: 'spot' | 'contract'
  percentage: number
  price: number
  side: 'buy' | 'sell'
}

export function quantityForBalancePercentage(input: BalancePercentageInput): number {
  if (!Number.isFinite(input.available) || input.available <= 0) return 0
  if (!Number.isFinite(input.percentage) || input.percentage <= 0) return 0

  const percentage = Math.min(input.percentage, 1)
  const budget = input.available * percentage
  if (input.mode === 'contract' || input.side === 'sell') return budget
  if (!Number.isFinite(input.price) || input.price <= 0) return 0
  return budget / input.price
}
