import { backend, type GraphChangedEvent } from '../api'

/** One shared subscription to `graph-changed`; components read `bus.version` inside `$effect`s to refresh. */
export const bus = $state({ version: 0, last: null as GraphChangedEvent | null })

backend.onGraphChanged((e) => {
  bus.last = e
  bus.version++
})
