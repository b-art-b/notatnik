// Favorite pages (NS-26): a list kept in the graph's settings (`favorites` in `.notatnik/config.toml`), so it is the
// same on every device. The sidebar shows it; the star of a page and the palette change it.
import { backend, type FavoriteView } from '../api'
import { errorMessage, toast } from './app.svelte'

export const favorites = $state({ items: [] as FavoriteView[] })

/** Reads the list from the open graph. */
export async function loadFavorites() {
  try {
    favorites.items = await backend.call('favorites', {})
  } catch {
    /* the graph was closed meanwhile */
  }
}

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase()

export const isFavorite = (name: string) => favorites.items.some((f) => same(f.name, name))

/** Adds a page to the favorites or removes it. */
export async function setFavorite(name: string, on: boolean) {
  try {
    favorites.items = await backend.call('favorite_set', { page: name, favorite: on })
  } catch (e) {
    toast(errorMessage(e), 'error')
  }
}

/** Puts a favorite at another place of the list (0 is the first). */
export async function moveFavorite(name: string, to: number) {
  try {
    favorites.items = await backend.call('favorite_move', { page: name, to })
  } catch (e) {
    toast(errorMessage(e), 'error')
  }
}
