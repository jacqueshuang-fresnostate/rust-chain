import { defineStore } from 'pinia'
import {
  addMarketFavorite,
  fetchMarketFavorites,
  removeMarketFavorite,
} from '@/api/marketFavorites'
import { createMarketFavoritesState } from '@/core/marketFavoritesState'

export const useMarketFavoritesStore = defineStore('mobile-market-favorites', () => (
  createMarketFavoritesState({
    fetch: fetchMarketFavorites,
    add: addMarketFavorite,
    remove: removeMarketFavorite,
  })
))
