import type { KlinePoint, MarketTicker, NewsItem, OrderBookLevel, TradePrint } from '@/core/types'

export const fallbackTickers: MarketTicker[] = [
  { symbol: 'BTC/USDT', base: 'BTC', quote: 'USDT', lastPrice: 64_125, openPrice: 63_910, highPrice: 64_694, lowPrice: 62_957, volume: 7_514, changePercent: 0.34 },
  { symbol: 'ETH/USDT', base: 'ETH', quote: 'USDT', lastPrice: 3_492.16, openPrice: 3_426.7, highPrice: 3_524.11, lowPrice: 3_398.74, volume: 98_721, changePercent: 1.91 },
  { symbol: 'SOL/USDT', base: 'SOL', quote: 'USDT', lastPrice: 151.89, openPrice: 148.71, highPrice: 153.72, lowPrice: 146.28, volume: 433_812, changePercent: 2.14 },
  { symbol: 'XRP/USDT', base: 'XRP', quote: 'USDT', lastPrice: 0.59379, openPrice: 0.57549, highPrice: 0.6014, lowPrice: 0.5682, volume: 1_244_093, changePercent: 3.18 },
  { symbol: 'DOGE/USDT', base: 'DOGE', quote: 'USDT', lastPrice: 0.1246, openPrice: 0.1271, highPrice: 0.1303, lowPrice: 0.1217, volume: 2_711_492, changePercent: -1.97 },
]

export const fallbackNews: NewsItem[] = [
  { id: 1, title: 'Market services and product updates' },
  { id: 2, title: 'Verify the network and address before transfers' },
  { id: 3, title: 'Account security features have been upgraded' },
]

export function createFallbackKlines(ticker: MarketTicker, size = 90): KlinePoint[] {
  const interval = 15 * 60 * 1000
  const end = Math.floor(Date.now() / interval) * interval
  const rows: KlinePoint[] = []
  let close = ticker.openPrice || ticker.lastPrice
  const amplitude = Math.max(ticker.lastPrice * 0.006, ticker.lastPrice < 1 ? 0.003 : 0.4)

  for (let index = 0; index < size; index += 1) {
    const wave = Math.sin(index * 0.43) * amplitude + Math.cos(index * 0.17) * amplitude * 0.45
    const open = close
    close = Math.max(0.0000001, open + wave)
    rows.push({
      time: end - (size - index) * interval,
      open,
      close,
      high: Math.max(open, close) + amplitude * 0.35,
      low: Math.max(0.0000001, Math.min(open, close) - amplitude * 0.35),
      volume: Math.max(1, ticker.volume / size * (0.45 + (index % 7) * 0.11)),
    })
  }
  return rows
}

export function createFallbackDepth(lastPrice: number): { bids: OrderBookLevel[]; asks: OrderBookLevel[] } {
  const step = Math.max(lastPrice * 0.0008, lastPrice < 1 ? 0.00001 : 0.01)
  return {
    asks: Array.from({ length: 8 }, (_, index) => ({ price: lastPrice + step * (index + 1), quantity: 18 + index * 11 })),
    bids: Array.from({ length: 8 }, (_, index) => ({ price: Math.max(0.0000001, lastPrice - step * (index + 1)), quantity: 22 + index * 9 })),
  }
}

export function createFallbackTrades(lastPrice: number): TradePrint[] {
  return Array.from({ length: 8 }, (_, index) => ({
    id: `sample-${index}`,
    side: index % 3 === 0 ? 'sell' : 'buy',
    price: lastPrice + (index % 2 ? 1 : -1) * Math.max(lastPrice * 0.0004, lastPrice < 1 ? 0.00001 : 0.01),
    quantity: 12 + index * 4.5,
    time: Date.now() - index * 23_000,
  }))
}
