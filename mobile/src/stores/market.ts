import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { fetchMarketTickers } from '@/api/market'
import { fallbackTickers } from '@/data/fallback'
import { normalizeSymbol } from '@/core/format'
import type { MarketTicker } from '@/core/types'

export const useMarketStore = defineStore('mobile-market', () => {
  const tickers = ref<MarketTicker[]>([])
  const loading = ref(false)
  const sampleData = ref(false)
  const updatedAt = ref(0)

  const topTickers = computed(() => tickers.value.slice(0, 12))

  async function refresh(force = false): Promise<void> {
    if (loading.value || (!force && updatedAt.value && Date.now() - updatedAt.value < 20_000)) return
    loading.value = true
    try {
      const next = await fetchMarketTickers()
      if (!next.length) throw new Error('market list is empty')
      tickers.value = next
      sampleData.value = false
    } catch {
      tickers.value = fallbackTickers
      sampleData.value = true
    } finally {
      updatedAt.value = Date.now()
      loading.value = false
    }
  }

  function tickerFor(symbol: string): MarketTicker | undefined {
    const normalized = normalizeSymbol(symbol)
    return tickers.value.find((item) => normalizeSymbol(item.symbol) === normalized)
  }

  return { tickers, topTickers, loading, sampleData, updatedAt, refresh, tickerFor }
})
