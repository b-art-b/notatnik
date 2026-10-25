// Everything the UI keeps about one open graph (MG-1, MG-6). The sidebar has a section per graph and each reads its
// own `GraphState` (through Svelte context, see `graphContext.ts`); the page area shows the *active* graph, and the
// modules it already imports (`recent`, `favorites`, `icons`, ...) answer for that one (`active.svelte.ts`).
import { untrack } from 'svelte'
import { backendFor, type Backend, type GraphInfo, type SavedQueryView } from '../api'
import { ArchivedStore } from './archived.store.svelte'
import { AFTER_CHANGE_MS, CalendarStore } from './calendar.store.svelte'
import { EncryptionStore } from './encryption.store.svelte'
import { FavoritesStore } from './favorites.store.svelte'
import { GraphBus } from './graphBus.store.svelte'
import { IconsStore } from './icons.store.svelte'
import { RecentStore } from './recent.store.svelte'
import { SyncStore } from './sync.store.svelte'
import { TreeStore } from './tree.store.svelte'

export class GraphState {
  /** The slot of the graph in the backend: `''` is the first graph, `'1'` the second. */
  readonly slot: string
  readonly backend: Backend
  info = $state<GraphInfo | null>(null)
  readonly bus: GraphBus
  readonly recent: RecentStore
  readonly favorites: FavoritesStore
  readonly icons: IconsStore
  readonly archived: ArchivedStore
  readonly encryption: EncryptionStore
  readonly sync: SyncStore
  readonly calendar: CalendarStore
  readonly tree: TreeStore
  /** The saved queries of the graph (listed in the sidebar). */
  saved = $state<SavedQueryView[]>([])
  /** The graph's section of the sidebar is unfolded (only matters while two graphs are open). The first graph's is,
   * the second's starts folded; the choice is remembered on this device per graph. */
  expanded = $state(true)

  constructor(slot: string, info: GraphInfo | null) {
    this.slot = slot
    this.backend = backendFor(slot)
    this.info = info
    const id = info?.id ?? ''
    this.bus = new GraphBus(this.backend)
    this.recent = new RecentStore(id)
    this.favorites = new FavoritesStore(this.backend)
    this.icons = new IconsStore(this.backend)
    this.archived = new ArchivedStore(this.backend, this.bus)
    this.encryption = new EncryptionStore(this.backend)
    this.sync = new SyncStore(this.backend, () => this.info)
    this.calendar = new CalendarStore(this.backend)
    this.tree = new TreeStore(id)
    this.expanded = this.readExpanded(slot === '')
  }

  private get expandedKey() {
    return `notatnik-section-open:${this.info?.id ?? ''}`
  }

  private readExpanded(dflt: boolean): boolean {
    if (!this.info) return dflt
    try {
      const v = localStorage.getItem(this.expandedKey)
      return v === null ? dflt : v === 'yes'
    } catch {
      return dflt
    }
  }

  /** Has the person folded or unfolded this graph's section before? */
  hasChosenFold(): boolean {
    try {
      return localStorage.getItem(this.expandedKey) !== null
    } catch {
      return false
    }
  }

  setExpanded(on: boolean) {
    if (this.expanded === on) return
    this.expanded = on
    try {
      localStorage.setItem(this.expandedKey, on ? 'yes' : 'no')
    } catch {
      /* not remembered */
    }
  }

  /** The graph's name (the folder's) and color, for the headers that say which graph something belongs to. */
  get name(): string {
    return this.info?.name ?? ''
  }

  /** Reads `graph_info` again (a setting changed, the graph's color was chosen). */
  async refreshInfo(): Promise<void> {
    try {
      this.info = await this.backend.call('graph_info', {})
    } catch {
      /* the graph was closed meanwhile */
    }
  }

  async refreshSaved(): Promise<void> {
    try {
      this.saved = await this.backend.call('list_saved_queries', {})
    } catch {
      /* keep the last list */
    }
  }

  private stopEffects: (() => void) | undefined

  /** Loads what the sidebar and the pages need to show and keeps it fresh: the lists that depend on the pages
   * reload when the graph changes, and the calendar sync starts (once when a graph is opened, again a few seconds
   * after the notes change). */
  start(): void {
    if (this.stopEffects) return
    void this.favorites.load()
    void this.icons.load()
    this.archived.watch()
    void this.encryption.refresh()
    void this.refreshSaved()
    void this.calendar.refreshTargets()
    this.calendar.schedule(1500)
    let seen = 0
    this.stopEffects = $effect.root(() => {
      // page icons depend on page properties: refresh them (a little later, edits come in bursts) when pages change
      $effect(() => {
        void this.bus.version
        const t = setTimeout(() => void this.icons.load(), 300)
        return () => clearTimeout(t)
      })
      // the notes changed on disk or in the app: the calendar sync runs a few seconds after the last change
      $effect(() => {
        const v = this.bus.version
        untrack(() => {
          if (v !== seen) this.calendar.schedule(AFTER_CHANGE_MS)
          seen = v
        })
      })
    })
  }

  /** Stops what runs by itself (timers, subscriptions): the graph is closed. */
  dispose() {
    this.stopEffects?.()
    this.stopEffects = undefined
    this.bus.dispose()
    this.archived.dispose()
    this.calendar.stop()
  }
}
