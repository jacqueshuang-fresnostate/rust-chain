import request from './request'
import {
  backendApiUrl,
  mapConvertPairsToPcCoins,
  type BackendConvertPair,
  type BackendConvertPairsResponse,
  type BackendWalletAccountsResponse,
} from './backendAdapters'

interface ConvertQuoteResponse {
  quote_id: string
  convert_pair_id: number
  from_asset_id: number
  to_asset_id: number
  from_amount: string | number
  to_amount: string | number
  rate: string | number
  spread_rate: string | number
  expires_at: number
}

export async function fetchSwapCoin(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendConvertPairsResponse>(backendApiUrl('/convert/pairs'))
  return { data: mapConvertPairsToPcCoins(response.data) }
}

export async function submitSwap(fromUnit: string, toUnit: string, amount: number): Promise<{ data: any }> {
  const pair = await resolveConvertPair(fromUnit, toUnit)
  const quote = await request.instance.post<ConvertQuoteResponse>(backendApiUrl('/convert/quote'), {
    from_asset_id: pair.from_asset_id,
    to_asset_id: pair.to_asset_id,
    from_amount: String(amount),
  })
  const confirmed = await request.instance.post(backendApiUrl('/convert/confirm'), {
    quote_id: quote.data.quote_id,
  })

  return {
    data: {
      code: 0,
      message: 'success',
      data: confirmed.data,
    },
  }
}

export async function getSwapRate(fromUnit: string, toUnit: string): Promise<{ data: any }> {
  const pair = await resolveConvertPair(fromUnit, toUnit)
  const quote = await request.instance.post<ConvertQuoteResponse>(backendApiUrl('/convert/quote'), {
    from_asset_id: pair.from_asset_id,
    to_asset_id: pair.to_asset_id,
    from_amount: '1',
  })

  return {
    data: {
      code: 0,
      message: 'success',
      data: {
        quoteId: quote.data.quote_id,
        rate: Number(quote.data.rate) || 0,
        toAmount: Number(quote.data.to_amount) || 0,
        expiresAt: quote.data.expires_at,
      },
    },
  }
}

async function resolveConvertPair(fromUnit: string, toUnit: string): Promise<BackendConvertPair> {
  const [pairsResponse, accountsResponse] = await Promise.all([
    request.instance.get<BackendConvertPairsResponse>(backendApiUrl('/convert/pairs')),
    request.instance.get<BackendWalletAccountsResponse>(backendApiUrl('/wallet/accounts')).catch(() => null),
  ])
  const fromAssetId = resolveAssetId(fromUnit, accountsResponse?.data)
  const toAssetId = resolveAssetId(toUnit, accountsResponse?.data)
  const fromSymbol = normalizeUnit(fromUnit)
  const toSymbol = normalizeUnit(toUnit)

  const pair = pairsResponse.data.pairs.find((item) => {
    const idsMatch = fromAssetId !== null && toAssetId !== null && item.from_asset_id === fromAssetId && item.to_asset_id === toAssetId
    const symbolsMatch = normalizeUnit(item.from_asset_symbol) === fromSymbol && normalizeUnit(item.to_asset_symbol) === toSymbol
    return item.enabled && (idsMatch || symbolsMatch)
  })

  if (!pair) {
    throw new Error(`Convert pair unavailable: ${fromSymbol}/${toSymbol}`)
  }

  return pair
}

function resolveAssetId(unit: string, accounts?: BackendWalletAccountsResponse): number | null {
  const numeric = Number(unit)
  if (Number.isInteger(numeric) && numeric > 0) return numeric
  const normalized = normalizeUnit(unit)
  return accounts?.accounts.find((account) => normalizeUnit(account.symbol) === normalized)?.asset_id ?? null
}

function normalizeUnit(unit?: string): string {
  return String(unit || '').split('-')[0].toUpperCase()
}
