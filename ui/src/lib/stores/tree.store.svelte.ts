// How the Pages part of one graph's sidebar section is folded, remembered on this device per graph: which branches of
// the namespace tree were opened or closed by hand, the default for the others ("Expand all" / "Collapse all"), and the
// counter the two buttons of the section bump so that the tree and the property groups answer them.
const KEY = 'notatnik-ns-open'

export class TreeStore {
  /** Every branch without a choice of its own is open. */
  all = $state(false)
  /** Branches opened (`true`) or closed (`false`) by hand, by page name. */
  open = $state<Record<string, boolean>>({})
  /** The number of fold requests (Collapse all / Expand all) and which one the last was. */
  foldN = $state(0)
  foldCollapsed = $state(true)
  private readonly key: string

  constructor(graphId: string) {
    this.key = graphId ? `${KEY}:${graphId}` : ''
    if (!graphId) return
    try {
      const v = JSON.parse(localStorage.getItem(this.key) ?? localStorage.getItem(KEY) ?? '{}')
      // before "Expand all" the value was the map of open branches itself
      const r = 'open' in v ? { all: !!v.all, open: v.open ?? {} } : { all: false, open: v }
      this.all = r.all
      this.open = r.open
    } catch {
      /* nothing remembered */
    }
  }

  save() {
    if (!this.key) return
    try {
      localStorage.setItem(this.key, JSON.stringify({ all: this.all, open: this.open }))
    } catch {
      /* not remembered */
    }
  }

  foldAll(collapsed: boolean) {
    this.foldCollapsed = collapsed
    this.foldN++
  }
}
