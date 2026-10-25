import { backend, backendFor, CommandError, type Backend, type BlockOp, type PageView } from '../api'
import { collapsedHashes } from '../tree'
import { app, errorMessage, toast } from './app.svelte'
import { bus } from './events.svelte'

export type SaveStatus = 'saved' | 'saving' | 'error'

/** One open page. Several views (main, sidebar, journals) share one PageDoc, so edits show everywhere. */
export class PageDoc {
  readonly requested: string
  view = $state<PageView | null>(null)
  status = $state<SaveStatus>('saved')
  loadError = $state<string | null>(null)
  /** Set when the file changed on disk while a block of this page was being edited. */
  externalChange = $state(false)
  /** Resolves after the first load attempt; commands wait for it so a click before the page loaded still works. */
  loaded: Promise<void> = Promise.resolve()
  private chain: Promise<unknown> = Promise.resolve()
  private pending = 0
  /** The graph this page is in: a late save must never go to the graph that became active meanwhile. */
  private readonly api: Backend

  constructor(name: string) {
    this.requested = name
    this.api = backendFor(backend.slot)
  }

  get name(): string {
    return this.view?.name ?? this.requested
  }

  get busy(): boolean {
    return this.pending > 0
  }

  async load(): Promise<void> {
    try {
      this.view = await this.api.call('get_page', { name: this.requested })
      this.loadError = null
      if (this.view.name !== this.requested) registry.set(this.view.name, this)
    } catch (e) {
      this.loadError = errorMessage(e)
    }
  }

  /** Like `apply`, but the ops are built when the request reaches the front of the queue, from the then-current
   * view. Return `null` to send nothing. This keeps late editor commits from using a stale view. */
  applyWith(build: (view: PageView | null) => BlockOp[] | null): Promise<PageView | null> {
    return this.enqueue(build)
  }

  /** Sends ops in order, one request at a time, each based on the hash the previous one returned. */
  apply(ops: BlockOp[]): Promise<PageView | null> {
    return this.enqueue(() => ops)
  }

  private enqueue(build: (view: PageView | null) => BlockOp[] | null): Promise<PageView | null> {
    this.pending++
    this.status = 'saving'
    const run = async (): Promise<PageView | null> => {
      await this.loaded // an edit made before the first load finished must use the loaded hash
      try {
        const ops = build(this.view)
        if (!ops || ops.length === 0) return this.view
        const r = await this.api.call('apply_ops', {
          page: this.name,
          base_hash: this.view?.hash ?? null,
          ops,
        })
        this.view = r.page
        if (r.template_error) toast(r.template_error, 'error')
        else if (r.template && r.template_added.length)
          toast(`Added ${r.template_added.join(', ')} from the template “${r.template}”.`)
        return r.page
      } catch (e) {
        this.status = 'error'
        if (e instanceof CommandError && e.code === 'conflict') {
          toast('This page changed on disk. It was reloaded; please redo your last edit.', 'error')
          await this.load()
        } else {
          toast(errorMessage(e), 'error')
        }
        return null
      } finally {
        this.pending--
        if (this.pending === 0 && this.status === 'saving') this.status = 'saved'
      }
    }
    const p = this.chain.then(run, run)
    this.chain = p
    return p
  }

  /** Waits until every queued edit has been written. */
  async flush(): Promise<void> {
    await this.chain
  }

  async toggleCollapsed(id: number): Promise<void> {
    await this.loaded // a click before the first load finished must still work (blocks shown elsewhere, e.g. in references)
    const v = this.view
    if (!v) return
    const flip = (list: PageView['blocks']): boolean => {
      for (const b of list) {
        if (b.id === id) {
          b.collapsed = !b.collapsed
          return true
        }
        if (flip(b.children)) return true
      }
      return false
    }
    flip(v.blocks)
    try {
      await this.api.call('set_collapsed', { page: v.name, collapsed: collapsedHashes(v.blocks) })
      // Blocks shown outside this page's own view (linked references, agenda, query results) are copies that
      // read the saved state when fetched: tell them to fetch again.
      bus.version++
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  setAllCollapsed(collapsed: boolean): Promise<void> {
    const v = this.view
    if (!v) return Promise.resolve()
    const walk = (list: PageView['blocks']) => {
      for (const b of list) {
        b.collapsed = collapsed && b.children.length > 0
        walk(b.children)
      }
    }
    walk(v.blocks)
    return this.api.call('set_collapsed', { page: v.name, collapsed: collapsedHashes(v.blocks) }).then(() => {
      bus.version++
    })
  }
}

const registry = new Map<string, PageDoc>()
/** Bumped when a document is added, so `$derived` values over `openDocs()` update. */
export const registryState = $state({ version: 0 })

/** The shared document for a page name (loaded on first use). */
export function getDoc(name: string): PageDoc {
  let d = registry.get(name)
  if (!d) {
    d = new PageDoc(name)
    registry.set(name, d)
    d.loaded = d.load()
    queueMicrotask(() => registryState.version++) // not during a derived: Svelte forbids state writes there
  }
  return d
}

/** The active graph changed: the open documents belong to the other graph (their edits were flushed first). */
export function resetDocs() {
  registry.clear()
  registryState.version++
}

/** Pages of all open documents (for the graph-changed handler). */
export function openDocs(): PageDoc[] {
  return [...new Set(registry.values())]
}

/** Undo/redo through the backend; reloads the affected document. */
export async function undoRedo(kind: 'undo' | 'redo'): Promise<string | null> {
  try {
    const r = await backend.call(kind, {})
    const doc = getDoc(r.page.name)
    doc.view = r.page
    return r.page.name
  } catch (e) {
    toast(errorMessage(e), 'error')
    return null
  }
}

/** Reloads documents whose page changed on disk. */
export function handleExternalChange(pages: string[], isEditing: (doc: PageDoc) => boolean) {
  for (const d of openDocs()) {
    // the page itself, or a page below it: its list of subpages (and the hierarchy) changed too
    const below = d.name.toLowerCase() + '/'
    if (!pages.some((p) => p === d.name || p.toLowerCase().startsWith(below))) continue
    if (d.busy) continue
    if (isEditing(d)) {
      d.externalChange = true
      void d.load().then(() => undefined)
    } else {
      void d.load()
    }
  }
  void app
}

/** Reloads every open document. A rename rewrites the links in many files (journals included) without telling the
 * documents that are already open, and a moved page keeps a document under its old name. */
export function reloadOpenDocs(): Promise<unknown> {
  return Promise.all(
    openDocs()
      .filter((d) => !d.busy)
      .map((d) => d.load()),
  )
}

export function flushAll(): Promise<unknown> {
  return Promise.all(openDocs().map((d) => d.flush()))
}

/** Overall save state for the status indicator. */
export function overallStatus(): SaveStatus {
  void registryState.version
  const all = openDocs()
  if (all.some((d) => d.status === 'error')) return 'error'
  if (all.some((d) => d.status === 'saving')) return 'saving'
  return 'saved'
}
