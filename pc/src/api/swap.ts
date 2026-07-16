import request from './request'
import {
  backendApiUrl,
  mapConvertOrdersToPcRows,
  mapConvertPairsToPcCoins,
  mapConvertPairsToPcPairOptions,
  mapConvertQuoteToPcQuote,
  type BackendConvertPair,
  type BackendConvertOrdersResponse,
  type BackendConvertPairsResponse,
  type BackendConvertQuote,
  type BackendWalletAccountsResponse,
  type PcApiResponse,
  type PcSwapOrderRow,
  type PcSwapPairOption,
  type PcSwapQuote,
  type PcTradeWalletBalance,
} from './backendAdapters'

export async function fetchSwapCoin(): Promise<{ data: any }> {
  const response = await request.instance.get<BackendConvertPairsResponse>(backendApiUrl('/convert/pairs'))
  return { data: mapConvertPairsToPcCoins(response.data) }
}

export async function fetchSwapPairs(): Promise<{ data: PcApiResponse<PcSwapPairOption[]> }> {
  const response = await request.instance.get<BackendConvertPairsResponse>(backendApiUrl('/convert/pairs'))
  return { data: mapConvertPairsToPcPairOptions(response.data) }
}

export async function fetchSwapBalances(): Promise<{ data: PcApiResponse<PcTradeWalletBalance[]> }> {
  const response = await request.instance.get<BackendWalletAccountsResponse>(backendApiUrl('/wallet/accounts'))
  return {
    data: {
      code: 0,
      message: 'success',
      data: response.data.accounts.map((account) => ({
        symbol: account.symbol.toUpperCase(),
        balance: Number(account.available) || 0,
        frozenBalance: (Number(account.frozen) || 0) + (Number(account.locked) || 0),
        logoUrl: typeof account.logo_url === 'string' ? account.logo_url.trim() || undefined : undefined,
      })),
    },
  }
}

export async function requestSwapQuote(pair: PcSwapPairOption, amount: number): Promise<{ data: PcApiResponse<PcSwapQuote> }> {
  const quote = await request.instance.post<BackendConvertQuote>(backendApiUrl('/convert/quote'), {
    from_asset_id: pair.fromAssetId,
    to_asset_id: pair.toAssetId,
    from_amount: String(amount),
  })
  return {
    data: {
      code: 0,
      message: 'success',
      data: mapConvertQuoteToPcQuote(quote.data),
    },
  }
}

export async function confirmSwapQuote(quoteId: string): Promise<{ data: PcApiResponse<{ quoteId: string; confirmed: boolean }> }> {
  const confirmed = await request.instance.post<{ quote_id: string; confirmed: boolean }>(backendApiUrl('/convert/confirm'), {
    quote_id: quoteId,
  })
  return {
    data: {
      code: 0,
      message: 'success',
      data: {
        quoteId: confirmed.data.quote_id,
        confirmed: confirmed.data.confirmed,
      },
    },
  }
}

export async function fetchSwapOrders(limit = 20): Promise<{ data: PcApiResponse<PcSwapOrderRow[]> }> {
  const [ordersResponse, pairsResponse, accountsResponse] = await Promise.all([
    request.instance.get<BackendConvertOrdersResponse>(backendApiUrl('/convert/orders'), { params: { limit } }),
    request.instance.get<BackendConvertPairsResponse>(backendApiUrl('/convert/pairs')).catch(() => null),
    request.instance.get<BackendWalletAccountsResponse>(backendApiUrl('/wallet/accounts')).catch(() => null),
  ])
  return {
    data: mapConvertOrdersToPcRows(ordersResponse.data, {
      accounts: accountsResponse?.data,
      pairs: pairsResponse?.data,
    }),
  }
}

export async function submitSwap(fromUnit: string, toUnit: string, amount: number): Promise<{ data: any }> {
  const pair = await resolveConvertPair(fromUnit, toUnit)
  const quote = await requestSwapQuote(backendPairToOption(pair), amount)
  const confirmed = await confirmSwapQuote(quote.data.data.quoteId)

  return {
    data: {
      code: 0,
      message: 'success',
      data: confirmed.data.data,
    },
  }
}

export async function getSwapRate(fromUnit: string, toUnit: string): Promise<{ data: any }> {
  const pair = await resolveConvertPair(fromUnit, toUnit)
  const quote = await requestSwapQuote(backendPairToOption(pair), 1)

  return {
    data: {
      code: 0,
      message: 'success',
      data: {
        quoteId: quote.data.data.quoteId,
        rate: quote.data.data.rate,
        toAmount: quote.data.data.toAmount,
        expiresAt: quote.data.data.expiresAt,
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

  const pair =
    pairsResponse.data.pairs.find((item) => item.enabled && pairMatchesDirection(item, fromAssetId, toAssetId, fromSymbol, toSymbol, false)) ||
    pairsResponse.data.pairs.find((item) => item.enabled && pairMatchesDirection(item, fromAssetId, toAssetId, fromSymbol, toSymbol, true))

  if (!pair) {
    throw new Error(`Convert pair unavailable: ${fromSymbol}/${toSymbol}`)
  }

  return pairMatchesDirection(pair, fromAssetId, toAssetId, fromSymbol, toSymbol, true) ? reverseConvertPairDirection(pair) : pair
}

function pairMatchesDirection(
  pair: BackendConvertPair,
  fromAssetId: number | null,
  toAssetId: number | null,
  fromSymbol: string,
  toSymbol: string,
  reverse: boolean,
): boolean {
  const sourceAssetId = reverse ? pair.to_asset_id : pair.from_asset_id
  const targetAssetId = reverse ? pair.from_asset_id : pair.to_asset_id
  const sourceSymbol = reverse ? pair.to_asset_symbol : pair.from_asset_symbol
  const targetSymbol = reverse ? pair.from_asset_symbol : pair.to_asset_symbol
  const idsMatch = fromAssetId !== null && toAssetId !== null && sourceAssetId === fromAssetId && targetAssetId === toAssetId
  const symbolsMatch = normalizeUnit(sourceSymbol) === fromSymbol && normalizeUnit(targetSymbol) === toSymbol
  return idsMatch || symbolsMatch
}

function reverseConvertPairDirection(pair: BackendConvertPair): BackendConvertPair {
  return {
    ...pair,
    from_asset_id: pair.to_asset_id,
    from_asset_symbol: pair.to_asset_symbol,
    to_asset_id: pair.from_asset_id,
    to_asset_symbol: pair.from_asset_symbol,
    min_amount: pair.target_min_amount ?? pair.min_amount,
    max_amount: pair.target_max_amount ?? pair.max_amount,
    target_min_amount: pair.min_amount,
    target_max_amount: pair.max_amount,
  }
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

function backendPairToOption(pair: BackendConvertPair): PcSwapPairOption {
  return {
    id: pair.id,
    fromAssetId: pair.from_asset_id,
    toAssetId: pair.to_asset_id,
    fromUnit: (pair.from_asset_symbol || String(pair.from_asset_id)).toUpperCase(),
    toUnit: (pair.to_asset_symbol || String(pair.to_asset_id)).toUpperCase(),
    minAmount: Number(pair.min_amount) || 0,
    maxAmount: Number(pair.max_amount) || 0,
    feeRate: Number(pair.fee_rate) || 0,
    enabled: pair.enabled,
  }
}
