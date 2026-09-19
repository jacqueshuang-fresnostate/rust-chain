export interface MarketProvenance {
  source?: string
  provider?: string
}

export function mapMarketProvenance(payload: { source?: unknown; provider?: unknown }): MarketProvenance {
  return {
    ...(payload.source !== undefined
      ? { source: typeof payload.source === 'string' && payload.source.trim() ? payload.source : 'unknown' } : {}),
    ...(typeof payload.provider === 'string' && payload.provider.trim() ? { provider: payload.provider } : {}),
  }
}

export function marketSource(provenance: MarketProvenance = {}): string {
  const { source, provider } = provenance
  if (source === 'platform' && provider === 'platform') return 'platform'
  if (source === 'strategy' || source === 'default' || source === 'generated') {
    return provider === 'strategy' ? source : 'unknown'
  }
  if (source === 'external') {
    return ['bitget', 'htx', 'coinbase'].includes(provider ?? '') ? 'external' : 'unknown'
  }
  // Legacy frames may have a provider but no semantic source.
  if (source === undefined) {
    if (provider === 'strategy') return 'generated'
    if (['bitget', 'htx', 'coinbase'].includes(provider ?? '')) return 'external'
  }
  return 'unknown'
}

export function marketTradeIdentity(trade: MarketProvenance & { id: string }): string {
  return JSON.stringify([trade.source ?? '', trade.provider ?? '', trade.id])
}
