export type MarketSocketFrame =
  | { type: 'subscribed'; channel: string }
  | { type: 'ticker'; symbol: string; lastPrice: number; observedAt?: number }
  | { type: 'pong' }

export function tickerSubscriptionFrame(symbol: string): string {
  return JSON.stringify({ op: 'subscribe', channel: 'ticker', symbol })
}

export function parseMarketSocketFrame(data: unknown): MarketSocketFrame | null {
  if (typeof data !== 'string') return null
  if (data.trim().toLowerCase() === 'pong') return { type: 'pong' }
  try {
    const payload = JSON.parse(data) as Record<string, unknown>
    if (payload.type === 'pong') return { type: 'pong' }
    if (payload.type === 'subscribed' && typeof payload.channel === 'string') {
      return { type: 'subscribed', channel: payload.channel }
    }
    if (typeof payload.symbol !== 'string') return null
    const lastPrice = Number(payload.last_price)
    if (!Number.isFinite(lastPrice)) return null
    return {
      type: 'ticker',
      symbol: payload.symbol,
      lastPrice,
      observedAt: typeof payload.observed_at === 'number' ? payload.observed_at : undefined,
    }
  } catch {
    return null
  }
}
