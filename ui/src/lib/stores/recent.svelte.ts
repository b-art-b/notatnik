// Recently opened pages (most recent first), remembered on this device, separately for each graph.
const LEGACY_KEY = 'notatnik-recent' // one list for all graphs, before the lists were per graph
const MAX = 12
/** The storage key of the open graph's list; empty until `useRecentFor` has been called. */
let key = ''

function load(k: string): string[] {
  try {
    // the first graph to be opened takes over the old shared list
    const raw = localStorage.getItem(k) ?? localStorage.getItem(LEGACY_KEY)
    const v = JSON.parse(raw ?? '[]')
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string').slice(0, MAX) : []
  } catch {
    return []
  }
}

export const recent = $state<{ items: string[] }>({ items: [] })

/** Switches the list to the one of graph `graphId`. */
export function useRecentFor(graphId: string) {
  // pages opened before the graph was known (the route is read first) belong to this graph
  const early = key ? [] : recent.items
  key = `${LEGACY_KEY}:${graphId}`
  recent.items = [...early, ...load(key).filter((x) => !early.some((e) => same(e, x)))].slice(0, MAX)
  try {
    localStorage.removeItem(LEGACY_KEY)
  } catch {
    /* nothing to remove */
  }
  save()
}

function save() {
  try {
    if (key) localStorage.setItem(key, JSON.stringify(recent.items))
  } catch {
    /* not remembered */
  }
}

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase()

/** Moves a page to the top (adding it if new). */
export function touchRecent(name: string) {
  if (!name || (recent.items[0] && same(recent.items[0], name))) return
  recent.items = [name, ...recent.items.filter((x) => !same(x, name))].slice(0, MAX)
  save()
}

/** A page was renamed: keep its place in the list under the new name. */
export function renameRecent(old: string, to: string) {
  if (!recent.items.some((x) => same(x, old))) return
  recent.items = recent.items.map((x) => (same(x, old) ? to : x))
  save()
}

/** A whole namespace was renamed: the page and everything below it keep their place under the new names. */
export function renameRecentBelow(from: string, to: string) {
  const f = from.toLowerCase()
  const hit = (x: string) => x.toLowerCase() === f || x.toLowerCase().startsWith(f + '/')
  if (!recent.items.some(hit)) return
  recent.items = recent.items.map((x) => (hit(x) ? to + x.slice(from.length) : x))
  save()
}

/** A page was deleted: it leaves the list. */
export function removeRecent(name: string) {
  if (!recent.items.some((x) => same(x, name))) return
  recent.items = recent.items.filter((x) => !same(x, name))
  save()
}

export function clearRecent() {
  recent.items = []
  save()
}
