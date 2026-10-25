// Recently opened pages (most recent first), remembered on this device, separately for each graph.
const LEGACY_KEY = 'notatnik-recent' // one list for all graphs, before the lists were per graph
const MAX = 12

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase()

export class RecentStore {
  items = $state<string[]>([])
  private readonly key: string

  /** `graphId` empty: no graph, nothing is read or remembered. */
  constructor(graphId: string) {
    this.key = graphId ? `${LEGACY_KEY}:${graphId}` : ''
    if (!graphId) return
    this.items = this.load()
    try {
      localStorage.removeItem(LEGACY_KEY)
    } catch {
      /* nothing to remove */
    }
    this.save()
  }

  private load(): string[] {
    try {
      // the first graph to be opened takes over the old shared list
      const raw = localStorage.getItem(this.key) ?? localStorage.getItem(LEGACY_KEY)
      const v = JSON.parse(raw ?? '[]')
      return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string').slice(0, MAX) : []
    } catch {
      return []
    }
  }

  private save() {
    if (!this.key) return
    try {
      localStorage.setItem(this.key, JSON.stringify(this.items))
    } catch {
      /* not remembered */
    }
  }

  /** Moves a page to the top (adding it if new). */
  touch(name: string) {
    if (!name || (this.items[0] && same(this.items[0], name))) return
    this.items = [name, ...this.items.filter((x) => !same(x, name))].slice(0, MAX)
    this.save()
  }

  /** A page was renamed: keep its place in the list under the new name. */
  rename(old: string, to: string) {
    if (!this.items.some((x) => same(x, old))) return
    this.items = this.items.map((x) => (same(x, old) ? to : x))
    this.save()
  }

  /** A whole namespace was renamed: the page and everything below it keep their place under the new names. */
  renameBelow(from: string, to: string) {
    const f = from.toLowerCase()
    const hit = (x: string) => x.toLowerCase() === f || x.toLowerCase().startsWith(f + '/')
    if (!this.items.some(hit)) return
    this.items = this.items.map((x) => (hit(x) ? to + x.slice(from.length) : x))
    this.save()
  }

  /** A page was deleted: it leaves the list. */
  remove(name: string) {
    if (!this.items.some((x) => same(x, name))) return
    this.items = this.items.filter((x) => !same(x, name))
    this.save()
  }

  clear() {
    this.items = []
    this.save()
  }
}
