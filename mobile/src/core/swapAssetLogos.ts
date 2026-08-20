export type SwapPickerSide = 'from' | 'to'

export class ConvertPairContractError extends Error {
  override readonly name = 'ConvertPairContractError'
}

export interface BackendConvertPair {
  id: number
  from_asset_id: number
  from_asset_symbol: string
  from_asset_logo_url?: string | null
  to_asset_id: number
  to_asset_symbol: string
  to_asset_logo_url?: string | null
  min_amount: string | number
  max_amount?: string | number | null
  target_min_amount?: string | number | null
  target_max_amount?: string | number | null
  fee_rate?: string | number | null
  enabled?: boolean | null
}

export interface ConvertPair {
  id: number
  fromAssetId: number
  fromAssetSymbol: string
  fromAssetLogoUrl?: string
  toAssetId: number
  toAssetSymbol: string
  toAssetLogoUrl?: string
  minAmount: number
  maxAmount?: number
  feeRate: number
  enabled: boolean
}

export interface SwapPickerAssetLogo {
  symbol: string
  logoUrl?: string
}

export interface SwapBalanceSource {
  symbol: string
  available: number
}

export function normalizeConvertPairLogoUrl(value: unknown): string | undefined {
  if (value === null || value === undefined) return undefined
  if (typeof value !== 'string') {
    throw new ConvertPairContractError('invalid convert pair Logo URL')
  }
  const normalized = value.trim()
  return normalized || undefined
}

export function normalizeSwapAssetSymbol(value: unknown): string {
  if (typeof value !== 'string') {
    throw new ConvertPairContractError('invalid convert pair asset symbol')
  }
  const normalized = value.trim().toUpperCase()
  if (!normalized) {
    throw new ConvertPairContractError('invalid convert pair asset symbol')
  }
  return normalized
}

export function mapConvertPair(pair: BackendConvertPair): ConvertPair {
  return {
    id: pair.id,
    fromAssetId: pair.from_asset_id,
    fromAssetSymbol: normalizeSwapAssetSymbol(pair.from_asset_symbol),
    fromAssetLogoUrl: normalizeConvertPairLogoUrl(pair.from_asset_logo_url),
    toAssetId: pair.to_asset_id,
    toAssetSymbol: normalizeSwapAssetSymbol(pair.to_asset_symbol),
    toAssetLogoUrl: normalizeConvertPairLogoUrl(pair.to_asset_logo_url),
    minAmount: toFiniteNumber(pair.min_amount),
    maxAmount: pair.max_amount === null || pair.max_amount === undefined
      ? undefined
      : toFiniteNumber(pair.max_amount),
    feeRate: toFiniteNumber(pair.fee_rate),
    enabled: pair.enabled !== false,
  }
}

/**
 * Projects backend convert rules into every direction accepted by the quote API.
 * Explicit rows win over a reverse projection so independently configured pairs
 * retain their own fee and limit contract.
 */
export function mapDirectionalConvertPairs(pairs: readonly BackendConvertPair[]): ConvertPair[] {
  const enabledPairs = pairs.filter((pair) => pair.enabled !== false)
  const explicitPairs = enabledPairs.map(mapConvertPair)
  const directionalPairs = new Map<string, ConvertPair>()

  for (const pair of explicitPairs) {
    directionalPairs.set(swapPairDirectionKey(pair), pair)
  }

  for (const [index, source] of enabledPairs.entries()) {
    const forward = explicitPairs[index]
    if (!forward) continue
    const reverse = mapReverseConvertPair(source, forward)
    const reverseKey = swapPairDirectionKey(reverse)
    if (!directionalPairs.has(reverseKey)) directionalPairs.set(reverseKey, reverse)
  }

  return [...directionalPairs.values()]
}

/** Distinguishes two directions that legitimately share one backend config ID. */
export function swapPairSelectionKey(
  pair: Pick<ConvertPair, 'id' | 'fromAssetId' | 'toAssetId'>,
): string {
  return `${pair.id}:${pair.fromAssetId}:${pair.toAssetId}`
}

export function buildSwapPickerAssetLogos(
  pairs: readonly Pick<ConvertPair, 'fromAssetSymbol' | 'fromAssetLogoUrl' | 'toAssetSymbol' | 'toAssetLogoUrl'>[],
  side: SwapPickerSide,
): SwapPickerAssetLogo[] {
  const assetsBySymbol = new Map<string, SwapPickerAssetLogo>()

  for (const pair of pairs) {
    const symbol = normalizeSwapAssetSymbol(
      side === 'from' ? pair.fromAssetSymbol : pair.toAssetSymbol,
    )
    const logoUrl = normalizeConvertPairLogoUrl(
      side === 'from' ? pair.fromAssetLogoUrl : pair.toAssetLogoUrl,
    )
    const existing = assetsBySymbol.get(symbol)

    if (!existing) {
      assetsBySymbol.set(symbol, logoUrl ? { symbol, logoUrl } : { symbol })
    } else if (!existing.logoUrl && logoUrl) {
      assetsBySymbol.set(symbol, { symbol, logoUrl })
    }
  }

  return [...assetsBySymbol.values()]
}

export function buildSwapAvailableBalanceMap(
  accounts: readonly SwapBalanceSource[],
): Map<string, number> {
  return new Map(accounts.map((account) => [
    normalizeSwapAssetSymbol(account.symbol),
    account.available,
  ]))
}

export function resolveSelectedSwapPair(
  pairs: readonly ConvertPair[],
  selectionKey: string,
): ConvertPair | undefined {
  return pairs.find((pair) => swapPairSelectionKey(pair) === selectionKey) || pairs[0]
}

export function resolveReverseSwapPair(
  pairs: readonly ConvertPair[],
  current: ConvertPair,
): ConvertPair | undefined {
  return pairs.find((pair) => (
    pair.fromAssetId === current.toAssetId
    && pair.toAssetId === current.fromAssetId
  ))
}

export function resolveSwapPickerPair(
  pairs: readonly ConvertPair[],
  side: SwapPickerSide,
  symbol: string,
  current?: ConvertPair,
): ConvertPair | undefined {
  const selectedSymbol = normalizeSwapAssetSymbol(symbol)
  const currentCounterSymbol = current
    ? normalizeSwapAssetSymbol(side === 'from' ? current.toAssetSymbol : current.fromAssetSymbol)
    : undefined

  const matchesSelectedSide = (pair: ConvertPair): boolean => normalizeSwapAssetSymbol(
    side === 'from' ? pair.fromAssetSymbol : pair.toAssetSymbol,
  ) === selectedSymbol
  const matchesCurrentCounterSide = (pair: ConvertPair): boolean => normalizeSwapAssetSymbol(
    side === 'from' ? pair.toAssetSymbol : pair.fromAssetSymbol,
  ) === currentCounterSymbol

  return pairs.find((pair) => matchesSelectedSide(pair) && matchesCurrentCounterSide(pair))
    || pairs.find(matchesSelectedSide)
}

function toFiniteNumber(value: unknown): number {
  const parsed = typeof value === 'number' ? value : Number(value)
  return Number.isFinite(parsed) ? parsed : 0
}

function mapReverseConvertPair(source: BackendConvertPair, forward: ConvertPair): ConvertPair {
  const reverseMaximum = source.target_max_amount === undefined
    ? source.max_amount
    : source.target_max_amount
  return {
    id: forward.id,
    fromAssetId: forward.toAssetId,
    fromAssetSymbol: forward.toAssetSymbol,
    fromAssetLogoUrl: forward.toAssetLogoUrl,
    toAssetId: forward.fromAssetId,
    toAssetSymbol: forward.fromAssetSymbol,
    toAssetLogoUrl: forward.fromAssetLogoUrl,
    minAmount: toFiniteNumber(source.target_min_amount ?? source.min_amount),
    maxAmount: reverseMaximum === null || reverseMaximum === undefined
      ? undefined
      : toFiniteNumber(reverseMaximum),
    feeRate: forward.feeRate,
    enabled: forward.enabled,
  }
}

function swapPairDirectionKey(
  pair: Pick<ConvertPair, 'fromAssetId' | 'toAssetId'>,
): string {
  return `${pair.fromAssetId}:${pair.toAssetId}`
}
