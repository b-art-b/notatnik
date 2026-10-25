// Archived pages: a page with `archived: true` in its frontmatter, and every page below it, is hidden from the
// sidebar, Recent and All pages (unless asked for) and its tasks stay off the agenda. The file itself is not moved.
import type { Backend } from '../api'
import type { GraphBus } from './graphBus.store.svelte'

export class ArchivedStore {
  names = $state<string[]>([])
  private stop: (() => void) | undefined

  constructor(
    private readonly backend: Backend,
    private readonly bus: GraphBus,
  ) {}

  /** Reloads the list. */
  async refresh(): Promise<void> {
    try {
      this.names = (await this.backend.call('archived_pages', {})).items
    } catch {
      /* keep the last list */
    }
  }

  /** Is the page archived itself or below an archived page? Case-insensitive, like page names. */
  is(name: string): boolean {
    const n = name.toLowerCase()
    return this.names.some((a) => {
      const k = a.toLowerCase()
      return n === k || n.startsWith(k + '/')
    })
  }

  /** Is it the page that carries the property (not just below one)? Only that one can be unarchived there. */
  itself(name: string): boolean {
    return this.names.some((a) => a.toLowerCase() === name.toLowerCase())
  }

  /** Reload on every change of the graph (once). */
  watch(): void {
    if (this.stop) return
    void this.refresh()
    this.stop = $effect.root(() => {
      $effect(() => {
        void this.bus.version
        void this.refresh()
      })
    })
  }

  dispose() {
    this.stop?.()
    this.stop = undefined
  }
}
