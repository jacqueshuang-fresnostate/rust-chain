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
        icon: t.icon,
      time: t.time,
      chg: t.open ? ((t.close - t.open) / t.open * 100)  : 0, // Recalculate chg
      zone: t.zone
    }))
  }

  function updateTicker(ticker: Ticker) {
    const index = tickers.value.findIndex(t => t.symbol === ticker.symbol)
    if (index !== -1) {
      // Merge updates
        tickers.value[index].close = ticker.close
        tickers.value[index].high = ticker.high
        tickers.value[index].low = ticker.low
        tickers.value[index].open = ticker.open
        tickers.value[index].volume = ticker.volume
        tickers.value[index].turnover = ticker.turnover
        // Recalculate chg
        const open = tickers.value[index].open
        const close = tickers.value[index].close
        tickers.value[index].chg = open ? ((close - open) / open * 100 )  : 0
    } else {
        // Calculate chg for new ticker
        const newTicker = { ...ticker }
        newTicker.chg = newTicker.open ? ((newTicker.close - newTicker.open) / newTicker.open * 100)  : 0
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
