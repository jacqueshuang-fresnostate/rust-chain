import { publicMarketWebSocketUrl } from '@/config/app'
import {
  createMarketTickerStream,
  type TickerUpdate,
} from './marketTickerStream'

export type { TickerUpdate }

type TickerListener = (update: TickerUpdate) => void

const tickerStream = createMarketTickerStream({
  getUrl: publicMarketWebSocketUrl,
})

/**
 * Shares one market socket across view-scoped symbol leases. The returned
 * disposer removes only this listener's symbols and closes the socket after
 * the final lease is released.
 */
export function subscribeTickers(
  symbols: readonly string[],
  listener: TickerListener,
): () => void {
  return tickerStream.subscribe(symbols, listener)
}
