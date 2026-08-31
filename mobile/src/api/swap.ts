import { client, requestUrl } from './client'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'
import { asNumber } from '@/core/format'
import {
  mapDirectionalConvertPairs,
  type BackendConvertPair,
  type ConvertPair,
} from '@/core/swapAssetLogos'

export type { ConvertPair } from '@/core/swapAssetLogos'

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

export async function fetchConvertPairs(options: ReferenceRequestOptions = {}): Promise<ConvertPair[]> {
  const url = requestUrl('/convert/pairs')
  return referenceRequestRegistry.request(createReferenceRequestKey(url), 2 * 60_000, async () => {
    const response = await client.get<{ pairs?: BackendConvertPair[] }>(url)
    return mapDirectionalConvertPairs(response.data.pairs || [])
  }, options)
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
