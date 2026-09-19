import { client, requestUrl } from './client'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'
import { requiredId, requiredSafeInteger, normalizeTimestamp } from '@/core/numeric'
import {
  mapDirectionalConvertPairs,
  type BackendConvertPair,
  type ConvertPair,
} from '@/core/swapAssetLogos'
import { normalizeDecimalText, requiredDecimalText, type DecimalText } from '@/core/decimal'

export type { ConvertPair } from '@/core/swapAssetLogos'

export interface ConvertQuote {
  quoteId: string
  pairId: number
  fromAmount: DecimalText
  toAmount: DecimalText
  rate: DecimalText
  feeAmount: DecimalText
  expiresAt: number
}

export interface ConvertOrder {
  id: number
  fromAssetId: number
  toAssetId: number
  fromAssetSymbol?: string
  toAssetSymbol?: string
  fromAmount: DecimalText
  toAmount: DecimalText
  rate: DecimalText
  feeAmount: DecimalText
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

export async function requestConvertQuote(pair: ConvertPair, amount: DecimalText): Promise<ConvertQuote> {
  const response = await client.post<Record<string, unknown>>(requestUrl('/convert/quote'), {
    from_asset_id: requiredId(pair.fromAssetId),
    to_asset_id: requiredId(pair.toAssetId),
    from_amount: normalizeDecimalText(amount),
  })
  return {
    quoteId: String(response.data.quote_id || ''),
    pairId: requiredId(response.data.convert_pair_id),
    fromAmount: convertDecimal(response.data.from_amount, 'from_amount'),
    toAmount: convertDecimal(response.data.to_amount, 'to_amount'),
    rate: convertDecimal(response.data.rate, 'rate'),
    feeAmount: convertDecimal(response.data.fee_amount, 'fee_amount'),
    expiresAt: normalizeTimestamp(response.data.expires_at),
  }
}

export async function confirmConvertQuote(quoteId: string): Promise<void> {
  await client.post(requestUrl('/convert/confirm'), { quote_id: quoteId })
}

export async function fetchConvertOrders(limit = 20): Promise<ConvertOrder[]> {
  requiredSafeInteger(limit, 'limit', 1)
  const [response, pairs] = await Promise.all([
    client.get<{ orders?: Array<Record<string, unknown>> }>(requestUrl('/convert/orders'), { params: { limit } }),
    fetchConvertPairs(),
  ])
  return (response.data.orders || []).map((order) => {
    const fromAssetId = requiredId(order.from_asset_id)
    const toAssetId = requiredId(order.to_asset_id)
    const pair = pairs.find((item) => item.fromAssetId === fromAssetId && item.toAssetId === toAssetId)
    return {
    id: requiredId(order.id),
    fromAssetId,
    toAssetId,
    fromAssetSymbol: text(order.from_asset_symbol) || pair?.fromAssetSymbol,
    toAssetSymbol: text(order.to_asset_symbol) || pair?.toAssetSymbol,
    fromAmount: convertDecimal(order.from_amount, 'from_amount'),
    toAmount: convertDecimal(order.to_amount, 'to_amount'),
    rate: convertDecimal(order.rate, 'rate'),
    feeAmount: convertDecimal(order.fee_amount, 'fee_amount'),
    status: String(order.status || ''),
    createdAt: normalizeTimestamp(order.created_at),
    }
  })
}

function text(value: unknown): string | undefined {
  const result = typeof value === 'string' ? value.trim() : ''
  return result || undefined
}

function convertDecimal(value: unknown, field: string): DecimalText {
  return requiredDecimalText(value, field, 'convert', { allowNegative: false, maxIntegerDigits: 20, maxScale: 18 })
}
