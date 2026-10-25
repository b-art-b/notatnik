import type { Backend, GraphChangedEvent } from '../api'

/** One graph's subscription to `graph-changed`: components read `bus.version` inside `$effect`s to refresh. */
export class GraphBus {
  version = $state(0)
  last = $state<GraphChangedEvent | null>(null)
  private off: () => void

  constructor(backend: Backend) {
    this.off = backend.onGraphChanged((e) => {
      this.last = e
      this.version++
    })
  }

  dispose() {
    this.off()
  }
}
