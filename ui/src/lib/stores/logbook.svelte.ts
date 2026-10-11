// Which logbooks are open. Remembered on this device (never in the graph); the key is the page and the block's
// place in it, so an open logbook stays open after a reload.
const KEY = 'notatnik-logbooks-open'
const MAX = 300

function load(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? '[]')
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : []
  } catch {
    return []
  }
}

const state = $state({ open: load() })

export const logKey = (page: string, path: number[]) => `${page.toLowerCase()}#${path.join('.')}`

export function isLogOpen(key: string): boolean {
  return state.open.includes(key)
}

export function toggleLog(key: string): void {
  state.open = state.open.includes(key)
    ? state.open.filter((k) => k !== key)
    : [...state.open, key].slice(-MAX)
  try {
    localStorage.setItem(KEY, JSON.stringify(state.open))
  } catch {
    /* not remembered */
  }
}
