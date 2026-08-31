import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { fetchMarketTickers } from '@/api/market'
import { subscribeTickers } from '@/api/marketSocket'
import { normalizeSymbol } from '@/core/format'
import {
  createSharedMarketLifecycle,
  type MarketConnectionState,
} from '@/core/marketLifecycle'
import { applyLiveMarketTickerUpdate, mergeMarketTickerSnapshots } from '@/core/marketTickerFreshness'
import type { MarketTicker } from '@/core/types'

export const useMarketStore = defineStore('mobile-market', () => {
  const tickers = ref<MarketTicker[]>([])
  const loading = ref(false)
  const error = ref(false)
  const updatedAt = ref(0)
  const lastFrameAt = ref(0)
  const connection = ref<MarketConnectionState>(isBrowserOnline() ? 'idle' : 'offline')
  const topTickers = computed(() => tickers.value.slice(0, 12))
  const connecting = computed(() => connection.value === 'connecting')
  const live = computed(() => connection.value === 'live')
  const stale = computed(() => connection.value === 'stale')
  const offline = computed(() => connection.value === 'offline')
  let onlineListenersInstalled = false

  const lifecycle = createSharedMarketLifecycle({
    load: async () => {
      const next = await fetchMarketTickers()
      if (!next.length) throw new Error('market list is empty')
      tickers.value = mergeMarketTickerSnapshots(tickers.value, next)
    },
    hasData: () => tickers.value.length > 0,
    liveKey: () => tickers.value
      .map((item) => normalizeSymbol(item.symbol))
      .filter(Boolean)
      .sort()
      .join('|'),
    connect: (onFrame) => subscribeTickers(
      tickers.value.map((item) => item.symbol),
      (update) => {
        const targetIndex = tickers.value.findIndex(
          (item) => normalizeSymbol(item.symbol) === update.symbol,
        )
        if (targetIndex < 0) return
        const target = tickers.value[targetIndex]
        const next = applyLiveMarketTickerUpdate(target, update)
        if (next !== target) tickers.value[targetIndex] = next
        onFrame()
      },
    ),
    isOnline: isBrowserOnline,
  })

  lifecycle.subscribe((snapshot) => {
    loading.value = snapshot.refreshing
    error.value = snapshot.refreshFailed
    updatedAt.value = snapshot.updatedAt
    lastFrameAt.value = snapshot.lastFrameAt
    connection.value = snapshot.connection
  })

  /** Returns the lifecycle's exact shared Promise; do not wrap this in async. */
  function refresh(force = false): Promise<void> {
    return lifecycle.refresh(force)
  }

  function ensureLive(): void {
    lifecycle.ensureLive()
  }

  // REST owns pair/icon structure. The shared live lease atomically replaces
  // price, high/low, volume and percentage fields for one observation frame.
  function startLiveUpdates(consumerId: string): void {
    installOnlineListeners()
    lifecycle.acquire(consumerId)
  }

  function stopLiveUpdates(consumerId: string): void {
    lifecycle.release(consumerId)
    if (lifecycle.snapshot().consumerCount === 0) removeOnlineListeners()
  }

  function tickerFor(symbol: string): MarketTicker | undefined {
    const normalized = normalizeSymbol(symbol)
    return tickers.value.find((item) => normalizeSymbol(item.symbol) === normalized)
  }

  function updateOnlineState(): void {
    lifecycle.setOnline(isBrowserOnline())
  }

  function installOnlineListeners(): void {
    if (onlineListenersInstalled || typeof window === 'undefined') return
    onlineListenersInstalled = true
    window.addEventListener('online', updateOnlineState)
    window.addEventListener('offline', updateOnlineState)
    updateOnlineState()
  }

  function removeOnlineListeners(): void {
    if (!onlineListenersInstalled || typeof window === 'undefined') return
    onlineListenersInstalled = false
    window.removeEventListener('online', updateOnlineState)
    window.removeEventListener('offline', updateOnlineState)
  }

  return {
    tickers,
    topTickers,
    loading,
    error,
    updatedAt,
    connection,
    connecting,
    live,
    stale,
    offline,
    lastFrameAt,
    refresh,
    ensureLive,
    tickerFor,
    startLiveUpdates,
    stopLiveUpdates,
  }
})

function isBrowserOnline(): boolean {
  return typeof navigator === 'undefined' || navigator.onLine !== false
}
