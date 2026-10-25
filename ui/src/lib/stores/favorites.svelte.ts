// Favorite pages of the active graph (NS-26; the lists are per graph: `FavoritesStore`).
import { active } from './active.svelte'

export const favorites = {
  get items() {
    return active().favorites.items
  },
}

export const loadFavorites = () => active().favorites.load()
export const isFavorite = (name: string) => active().favorites.isFavorite(name)
export const setFavorite = (name: string, on: boolean) => active().favorites.set(name, on)
export const moveFavorite = (name: string, to: number) => active().favorites.move(name, to)
