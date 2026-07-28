import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { fetchMarketTickers } from '@/api/market'
import { subscribeTickers } from '@/api/marketSocket'
import { normalizeSymbol } from '@/core/format'
import type { MarketTicker } from '@/core/types'

export const useMarketStore = defineStore('mobile-market', () => {
  const tickers = ref<MarketTicker[]>([])
  const loading = ref(false)
  const error = ref(false)
  const updatedAt = ref(0)

  const topTickers = computed(() => tickers.value.slice(0, 12))

  async function refresh(force = false): Promise<void> {
    if (loading.value || (!force && updatedAt.value && Date.now() - updatedAt.value < 20_000)) return
    loading.value = true
    try {
      const next = await fetchMarketTickers()
      if (!next.length) throw new Error('market list is empty')
      tickers.value = next
      error.value = false
      updatedAt.value = Date.now()
    } catch {
      error.value = true
    } finally {
      loading.value = false
    }
  }

  let stopLive: (() => void) | null = null

  // 实时推送只覆盖最新价，列表结构仍以 REST 快照为准。
  function startLiveUpdates(): void {
    if (stopLive || !tickers.value.length) return
    stopLive = subscribeTickers(
      tickers.value.map((item) => item.symbol),
      (update) => {
        const target = tickers.value.find((item) => normalizeSymbol(item.symbol) === update.symbol)
        if (!target || update.lastPrice <= 0) return
        target.lastPrice = update.lastPrice
        if (target.openPrice > 0) {
          target.changePercent = ((update.lastPrice - target.openPrice) / target.openPrice) * 100
        }
        updatedAt.value = Date.now()
      },
    )
  }

  function stopLiveUpdates(): void {
    stopLive?.()
    stopLive = null
  }

  function tickerFor(symbol: string): MarketTicker | undefined {
    const normalized = normalizeSymbol(symbol)
    return tickers.value.find((item) => normalizeSymbol(item.symbol) === normalized)
  }

  return {
    tickers,
    topTickers,
    loading,
    error,
    updatedAt,
    refresh,
    tickerFor,
    startLiveUpdates,
    stopLiveUpdates,
  }
})
