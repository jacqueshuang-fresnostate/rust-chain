import { computed, ref } from 'vue'
import { normalizeSymbol } from './format.ts'
import type { MarketFavorite } from './types.ts'

export interface MarketFavoritesApi {
  fetch(): Promise<MarketFavorite[]>
  add(symbol: string): Promise<MarketFavorite>
  remove(symbol: string): Promise<void>
}

export function createMarketFavoritesState(api: MarketFavoritesApi) {
  const favorites = ref<MarketFavorite[]>([])
  const pendingSymbols = ref(new Set<string>())
  const loading = ref(false)
  const loaded = ref(false)
  const error = ref(false)
  const favoriteSymbols = computed(() => new Set(
    favorites.value.map((favorite) => normalizeSymbol(favorite.symbol)),
  ))
  let loadPromise: Promise<void> | null = null
  let sessionVersion = 0
  let stateRevision = 0

  function isFavorite(symbol: string): boolean {
    return favoriteSymbols.value.has(normalizeSymbol(symbol))
  }

  function isPending(symbol: string): boolean {
    return pendingSymbols.value.has(normalizeSymbol(symbol))
  }

  async function load(force = false): Promise<void> {
    if (loadPromise) return loadPromise
    if (loaded.value && !force) return
    const version = sessionVersion
    const revision = stateRevision
    loading.value = true
    const request = (async () => {
      try {
        const next = await api.fetch()
        if (version !== sessionVersion) return
        // A force refresh may overlap a local mutation. Do not let an older GET
        // snapshot erase that newer optimistic/committed state.
        if (revision === stateRevision) favorites.value = next
        loaded.value = true
        error.value = false
      } catch {
        if (version !== sessionVersion) return
        error.value = true
      } finally {
        if (version === sessionVersion) loading.value = false
      }
    })()
    loadPromise = request
    await request
    if (loadPromise === request) loadPromise = null
  }

  async function add(symbol: string): Promise<boolean> {
    const normalized = normalizeSymbol(symbol)
    if (!normalized || isFavorite(normalized) || isPending(normalized)) return false
    const version = sessionVersion
    stateRevision += 1
    favorites.value = [...favorites.value, {
      marketId: 0,
      symbol: normalized,
    }]
    setPending(normalized, true)
    try {
      const saved = await api.add(normalized)
      if (version !== sessionVersion) return false
      favorites.value = [
        ...favorites.value.filter((favorite) => normalizeSymbol(favorite.symbol) !== normalized),
        saved,
      ]
      error.value = false
      return true
    } catch {
      if (version !== sessionVersion) return false
      favorites.value = favorites.value.filter(
        (favorite) => normalizeSymbol(favorite.symbol) !== normalized,
      )
      error.value = true
      return false
    } finally {
      if (version === sessionVersion) setPending(normalized, false)
    }
  }

  async function remove(symbol: string): Promise<boolean> {
    const normalized = normalizeSymbol(symbol)
    if (!normalized || !isFavorite(normalized) || isPending(normalized)) return false
    const version = sessionVersion
    const removedIndex = favorites.value.findIndex(
      (favorite) => normalizeSymbol(favorite.symbol) === normalized,
    )
    const removedFavorite = favorites.value[removedIndex]
    stateRevision += 1
    favorites.value = favorites.value.filter(
      (favorite) => normalizeSymbol(favorite.symbol) !== normalized,
    )
    setPending(normalized, true)
    try {
      await api.remove(normalized)
      if (version !== sessionVersion) return false
      error.value = false
      return true
    } catch {
      if (version !== sessionVersion) return false
      if (removedFavorite && !isFavorite(normalized)) {
        const next = [...favorites.value]
        next.splice(Math.max(0, removedIndex), 0, removedFavorite)
        favorites.value = next
      }
      error.value = true
      return false
    } finally {
      if (version === sessionVersion) setPending(normalized, false)
    }
  }

  async function toggle(symbol: string): Promise<boolean> {
    const normalized = normalizeSymbol(symbol)
    if (!normalized || isPending(normalized)) return false
    const version = sessionVersion
    if (!loaded.value) await load()
    if (version !== sessionVersion || !loaded.value) return false
    return isFavorite(normalized) ? remove(normalized) : add(normalized)
  }

  function reset(): void {
    sessionVersion += 1
    stateRevision = 0
    favorites.value = []
    pendingSymbols.value = new Set()
    loading.value = false
    loaded.value = false
    error.value = false
    loadPromise = null
  }

  function setPending(symbol: string, pending: boolean): void {
    const next = new Set(pendingSymbols.value)
    if (pending) next.add(symbol)
    else next.delete(symbol)
    pendingSymbols.value = next
  }

  return {
    favorites,
    favoriteSymbols,
    pendingSymbols,
    loading,
    loaded,
    error,
    load,
    add,
    remove,
    toggle,
    reset,
    isFavorite,
    isPending,
  }
}
