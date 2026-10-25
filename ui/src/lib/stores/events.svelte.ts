import { active } from './active.svelte'

/** The active graph's `graph-changed` subscription; components read `bus.version` inside `$effect`s to refresh. */
export const bus = {
  get version() {
    return active().bus.version
  },
  set version(v: number) {
    active().bus.version = v
  },
  get last() {
    return active().bus.last
  },
}
