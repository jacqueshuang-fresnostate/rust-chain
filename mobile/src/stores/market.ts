import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { fetchMarketTickers } from '@/api/market'
import { subscribeTickers } from '@/api/marketSocket'
import { normalizeSymbol } from '@/core/format'
import { applyLiveMarketTickerUpdate, mergeMarketTickerSnapshots } from '@/core/marketTickerFreshness'
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
      tickers.value = mergeMarketTickerSnapshots(tickers.value, next)
      error.value = false
      updatedAt.value = Date.now()
    } catch {
      error.value = true
    } finally {
      loading.value = false
    }
  }

  const liveConsumers = new Set<string>()
  let stopLive: (() => void) | null = null

  // 实时推送只覆盖最新价，列表结构仍以 REST 快照为准。
  function startLiveUpdates(consumerId: string): void {
    const consumer = consumerId.trim()
    if (!consumer) return
    liveConsumers.add(consumer)
    if (stopLive || !tickers.value.length) return
    stopLive = subscribeTickers(
      tickers.value.map((item) => item.symbol),
      (update) => {
        const targetIndex = tickers.value.findIndex((item) => normalizeSymbol(item.symbol) === update.symbol)
        if (targetIndex < 0) return
        const target = tickers.value[targetIndex]
        const next = applyLiveMarketTickerUpdate(target, update)
        if (next === target) return
        tickers.value[targetIndex] = next
        updatedAt.value = Date.now()
      },
    )
  }

  function stopLiveUpdates(consumerId: string): void {
    liveConsumers.delete(consumerId.trim())
    if (liveConsumers.size) return
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
