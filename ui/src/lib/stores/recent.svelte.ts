// Recently opened pages of the active graph (the lists are per graph: `RecentStore`).
import { active } from './active.svelte'

export const recent = {
  get items() {
    return active().recent.items
  },
  set items(v: string[]) {
    active().recent.items = v
  },
}

export const touchRecent = (name: string) => active().recent.touch(name)
export const renameRecent = (old: string, to: string) => active().recent.rename(old, to)
export const renameRecentBelow = (from: string, to: string) => active().recent.renameBelow(from, to)
export const removeRecent = (name: string) => active().recent.remove(name)
export const clearRecent = () => active().recent.clear()
