// Recently opened pages (most recent first), remembered on this device.
const KEY = 'notatnik-recent'
const MAX = 12

function load(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? '[]')
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string').slice(0, MAX) : []
  } catch {
    return []
  }
}

export const recent = $state({ items: load() })

function save() {
  try {
    localStorage.setItem(KEY, JSON.stringify(recent.items))
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
