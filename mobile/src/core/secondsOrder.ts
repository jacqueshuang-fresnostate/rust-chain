import { asNumber } from './format.ts'

export interface SecondsOrder {
  id: number
  symbol: string
  stakeAssetSymbol: string
  direction: 'up' | 'down'
  stakeAmount: number
  durationSeconds: number
  payoutRate: number
  entryPrice?: number
  settlementPrice?: number
  status: string
  result?: string
  expiresAt: number
  createdAt: number
}

export function mapSecondsOrder(order: Record<string, unknown>): SecondsOrder {
  return {
    id: asNumber(order.id),
    symbol: String(order.symbol || ''),
    stakeAssetSymbol: String(order.stake_asset_symbol || '').toUpperCase(),
    direction: secondsDirection(order.direction),
    stakeAmount: asNumber(order.stake_amount),
    durationSeconds: asNumber(order.duration_seconds),
    payoutRate: asNumber(order.payout_rate),
    entryPrice: optionalNumber(order.entry_price),
    settlementPrice: optionalNumber(order.settlement_price),
    status: String(order.status || ''),
    result: optionalText(order.result),
    expiresAt: normalizeTimestamp(order.expires_at),
    createdAt: normalizeTimestamp(order.created_at),
  }
}

function secondsDirection(value: unknown): SecondsOrder['direction'] {
  const direction = String(value || '').trim().toLowerCase()
  if (direction === 'up' || direction === 'down') return direction
  throw new Error('Seconds-contract order response contains an invalid direction')
}

function optionalText(value: unknown): string | undefined {
  const text = typeof value === 'string' ? value.trim() : ''
  return text || undefined
}

function optionalNumber(value: unknown): number | undefined {
  if (value === null || value === undefined || value === '') return undefined
  const number = asNumber(value)
  return Number.isFinite(number) ? number : undefined
}

function normalizeTimestamp(value: unknown): number {
  const timestamp = asNumber(value)
  return timestamp > 0 && timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}
