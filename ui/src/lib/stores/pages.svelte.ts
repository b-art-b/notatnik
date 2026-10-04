import { backend, CommandError, type BlockOp, type PageView } from '../api'
import { collapsedHashes } from '../tree'
import { app, errorMessage, toast } from './app.svelte'

export type SaveStatus = 'saved' | 'saving' | 'error'

/** One open page. Several views (main, sidebar, journals) share one PageDoc, so edits show everywhere. */
export class PageDoc {
  readonly requested: string
  view = $state<PageView | null>(null)
  status = $state<SaveStatus>('saved')
  loadError = $state<string | null>(null)
  /** Set when the file changed on disk while a block of this page was being edited. */
  externalChange = $state(false)
  private chain: Promise<unknown> = Promise.resolve()
  private pending = 0

  constructor(name: string) {
    this.requested = name
  }

  get name(): string {
    return this.view?.name ?? this.requested
  }

  get busy(): boolean {
    return this.pending > 0
  }

  async load(): Promise<void> {
    try {
      this.view = await backend.call('get_page', { name: this.requested })
      this.loadError = null
      if (this.view.name !== this.requested) registry.set(this.view.name, this)
    } catch (e) {
      this.loadError = errorMessage(e)
    }
  }

  /** Sends ops in order, one request at a time, each based on the hash the previous one returned. */
  apply(ops: BlockOp[]): Promise<PageView | null> {
    this.pending++
    this.status = 'saving'
    const run = async (): Promise<PageView | null> => {
      try {
        const r = await backend.call('apply_ops', {
          page: this.name,
          base_hash: this.view?.hash ?? null,
          ops,
        })
        this.view = r.page
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
      await backend.call('set_collapsed', { page: v.name, collapsed: collapsedHashes(v.blocks) })
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
    return backend
      .call('set_collapsed', { page: v.name, collapsed: collapsedHashes(v.blocks) })
      .then(() => undefined)
  }
}

const registry = new Map<string, PageDoc>()

/** The shared document for a page name (loaded on first use). */
export function getDoc(name: string): PageDoc {
  let d = registry.get(name)
  if (!d) {
    d = new PageDoc(name)
    registry.set(name, d)
    void d.load()
  }
  return d
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
    if (!pages.includes(d.name)) continue
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

export function flushAll(): Promise<unknown> {
  return Promise.all(openDocs().map((d) => d.flush()))
}
