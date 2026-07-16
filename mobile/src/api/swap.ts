import { client, requestUrl } from './client'
import { asNumber } from '@/core/format'

export interface ConvertPair {
  id: number
  fromAssetId: number
  fromAssetSymbol: string
  toAssetId: number
  toAssetSymbol: string
  minAmount: number
  maxAmount?: number
  feeRate: number
  enabled: boolean
}

export interface ConvertQuote {
  quoteId: string
  pairId: number
  fromAmount: number
  toAmount: number
  rate: number
  feeAmount: number
  expiresAt: number
}

export interface ConvertOrder {
  id: number
  fromAssetId: number
  toAssetId: number
  fromAssetSymbol?: string
  toAssetSymbol?: string
  fromAmount: number
  toAmount: number
  rate: number
  feeAmount: number
  status: string
  createdAt: number
}

interface BackendConvertPair {
  id: number
  from_asset_id: number
  from_asset_symbol: string
  to_asset_id: number
  to_asset_symbol: string
  min_amount: string | number
  max_amount?: string | number | null
  fee_rate?: string | number | null
  enabled?: boolean | null
}

export async function fetchConvertPairs(): Promise<ConvertPair[]> {
  const response = await client.get<{ pairs?: BackendConvertPair[] }>(requestUrl('/convert/pairs'))
  return (response.data.pairs || []).map((pair) => ({
    id: pair.id,
    fromAssetId: pair.from_asset_id,
    fromAssetSymbol: pair.from_asset_symbol.toUpperCase(),
    toAssetId: pair.to_asset_id,
    toAssetSymbol: pair.to_asset_symbol.toUpperCase(),
    minAmount: asNumber(pair.min_amount),
    maxAmount: pair.max_amount === null || pair.max_amount === undefined ? undefined : asNumber(pair.max_amount),
    feeRate: asNumber(pair.fee_rate),
    enabled: pair.enabled !== false,
  })).filter((pair) => pair.enabled)
}

export async function requestConvertQuote(pair: ConvertPair, amount: number): Promise<ConvertQuote> {
  const response = await client.post<Record<string, unknown>>(requestUrl('/convert/quote'), {
    from_asset_id: pair.fromAssetId,
    to_asset_id: pair.toAssetId,
    from_amount: String(amount),
  })
  return {
    quoteId: String(response.data.quote_id || ''),
    pairId: asNumber(response.data.convert_pair_id),
    fromAmount: asNumber(response.data.from_amount),
    toAmount: asNumber(response.data.to_amount),
    rate: asNumber(response.data.rate),
    feeAmount: asNumber(response.data.fee_amount),
    expiresAt: normalizeTimestamp(response.data.expires_at),
  }
}

export async function confirmConvertQuote(quoteId: string): Promise<void> {
  await client.post(requestUrl('/convert/confirm'), { quote_id: quoteId })
}

export async function fetchConvertOrders(limit = 20): Promise<ConvertOrder[]> {
  const [response, pairs] = await Promise.all([
    client.get<{ orders?: Array<Record<string, unknown>> }>(requestUrl('/convert/orders'), { params: { limit } }),
    fetchConvertPairs(),
  ])
  return (response.data.orders || []).map((order) => {
    const fromAssetId = asNumber(order.from_asset_id)
    const toAssetId = asNumber(order.to_asset_id)
    const pair = pairs.find((item) => item.fromAssetId === fromAssetId && item.toAssetId === toAssetId)
    return {
    id: asNumber(order.id),
    fromAssetId,
    toAssetId,
    fromAssetSymbol: text(order.from_asset_symbol) || pair?.fromAssetSymbol,
    toAssetSymbol: text(order.to_asset_symbol) || pair?.toAssetSymbol,
    fromAmount: asNumber(order.from_amount),
    toAmount: asNumber(order.to_amount),
    rate: asNumber(order.rate),
    feeAmount: asNumber(order.fee_amount),
    status: String(order.status || ''),
    createdAt: normalizeTimestamp(order.created_at),
    }
  })
}

function text(value: unknown): string | undefined {
  const result = typeof value === 'string' ? value.trim() : ''
  return result || undefined
}

function normalizeTimestamp(value: unknown): number {
  const timestamp = asNumber(value)
  return timestamp > 0 && timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}
