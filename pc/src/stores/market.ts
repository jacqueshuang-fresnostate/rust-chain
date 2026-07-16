import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface Ticker {
  symbol: string
  icon: string
  open: number
  high: number
  low: number
  close: number
  volume: number
  turnover: number
  time: number
  chg: number // percentage change
  zone: number // 0: main, etc
}

export const useMarketStore = defineStore('market', () => {
  const activeSymbol = ref<string>('BTC/USDT')
  const tickers = ref<Ticker[]>([])
  const orderRefreshKey = ref<number>(0)

  function setActiveSymbol(symbol: string) {
    activeSymbol.value = symbol
  }

  function triggerOrderRefresh() {
    orderRefreshKey.value++
  }

  function setTickers(newTickers: any[]) {
    // Map backend response to internal structure if needed
    // Assuming backend structure is similar to interface, but if not, map it here
    tickers.value = newTickers.map(t => ({
      symbol: t.symbol,
      open: t.open,
      high: t.high,
      low: t.low,
      close: t.close,
      volume: t.volume,
      turnover: t.turnover,
      icon: typeof t.icon === 'string' ? t.icon : '',
      time: t.time,
      chg: typeof t.chg === 'number' ? t.chg : (t.open ? ((t.close - t.open) / t.open * 100)  : 0),
      zone: t.zone
    }))
  }

  function updateTicker(ticker: Ticker) {
    const compactSymbol = compactMarketSymbol(ticker.symbol)
    const index = tickers.value.findIndex(t => compactMarketSymbol(t.symbol) === compactSymbol)
    if (index !== -1) {
      const current = tickers.value[index]
      const open = finiteNumber(ticker.open, current.open)
      const close = finiteNumber(ticker.close, current.close)
      tickers.value[index] = {
        ...current,
        ...ticker,
        symbol: current.symbol || ticker.symbol,
        icon: ticker.icon || current.icon,
        open,
        close,
        high: finiteNumber(ticker.high, current.high),
        low: finiteNumber(ticker.low, current.low),
        volume: finiteNumber(ticker.volume, current.volume),
        turnover: finiteNumber(ticker.turnover, current.turnover),
        time: finiteNumber(ticker.time, current.time),
        zone: finiteNumber(ticker.zone, current.zone),
        chg: typeof ticker.chg === 'number' ? ticker.chg : (open ? ((close - open) / open * 100) : 0),
      }
    } else {
        // Calculate chg for new ticker
        const newTicker = { ...ticker }
        newTicker.chg = typeof newTicker.chg === 'number' ? newTicker.chg : (newTicker.open ? ((newTicker.close - newTicker.open) / newTicker.open * 100)  : 0)
        tickers.value.push(newTicker)
    }
  }

  return {
    activeSymbol,
    tickers,
    setActiveSymbol,
    setTickers,
    updateTicker,
    orderRefreshKey,
    triggerOrderRefresh
  }
}, {
  persist: {
    paths: ['activeSymbol']
  }
})

function compactMarketSymbol(symbol: string): string {
  return symbol.replace(/[-_/]/g, '').toUpperCase()
}

function finiteNumber(value: number, fallback: number): number {
  return Number.isFinite(value) ? value : fallback
}
