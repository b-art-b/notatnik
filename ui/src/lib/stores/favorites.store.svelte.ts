// Favorite pages (NS-26): a list kept in the graph's settings (`favorites` in `.notatnik/config.toml`), so it is the
// same on every device. The sidebar shows it; the star of a page and the palette change it.
import type { Backend, FavoriteView } from '../api'
import { errorMessage, toast } from './app.svelte'

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase()

export class FavoritesStore {
  items = $state<FavoriteView[]>([])

  constructor(private readonly backend: Backend) {}

  /** Reads the list from the graph. */
  async load() {
    try {
      this.items = await this.backend.call('favorites', {})
    } catch {
      /* the graph was closed meanwhile */
    }
  }

  isFavorite(name: string) {
    return this.items.some((f) => same(f.name, name))
  }

  /** Adds a page to the favorites or removes it. */
  async set(name: string, on: boolean) {
    try {
      this.items = await this.backend.call('favorite_set', { page: name, favorite: on })
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  /** Puts a favorite at another place of the list (0 is the first). */
  async move(name: string, to: number) {
    try {
      this.items = await this.backend.call('favorite_move', { page: name, to })
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
}
