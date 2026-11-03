// A drawing that was not saved is kept as a draft (DR-13): a crash, a killed app or a power cut would otherwise lose
// it, and an iPad stops apps whenever it likes. The draft is in the webview's IndexedDB on this device, not in the
// graph (it is neither a note nor meant to be synced, ST-2). The saving is separated from js-draw: it is given a
// function that makes the SVG, and a store, so it can be tested alone.

export interface Draft {
  svg: string
  /** ms since the epoch */
  savedAt: number
}

export interface DraftStore {
  get(key: string): Promise<Draft | undefined>
  put(key: string, draft: Draft): Promise<void>
  delete(key: string): Promise<void>
}

/** What a draft belongs to: the drawing it edits, or for a new one the page it is being made on. */
export function draftKey(target: { path: string | null; page: string | null }): string {
  return target.path ? `file:${target.path.replace(/^(\.\.\/)+/, '')}` : `new:${target.page ?? ''}`
}

const DB = 'notatnik-drawing'
const STORE = 'drafts'

/** The drafts in IndexedDB. Every call may fail (private window, blocked storage): the callers ignore it. */
export function idbStore(): DraftStore {
  const open = () =>
    new Promise<IDBDatabase>((resolve, reject) => {
      const req = indexedDB.open(DB, 1)
      req.onupgradeneeded = () => req.result.createObjectStore(STORE)
      req.onsuccess = () => resolve(req.result)
      req.onerror = () => reject(req.error)
    })
  const run = async <T>(mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> => {
    const db = await open()
    try {
      return await new Promise<T>((resolve, reject) => {
        const tx = db.transaction(STORE, mode)
        const req = f(tx.objectStore(STORE))
        tx.oncomplete = () => resolve(req.result)
        tx.onerror = () => reject(tx.error)
        tx.onabort = () => reject(tx.error)
      })
    } finally {
      db.close()
    }
  }
  return {
    get: (key) => run('readonly', (s) => s.get(key) as IDBRequest<Draft | undefined>),
    put: async (key, draft) => void (await run('readwrite', (s) => s.put(draft, key))),
    delete: async (key) => void (await run('readwrite', (s) => s.delete(key))),
  }
}

/** Writes the draft a few seconds after the first change, not at every stroke: a drawing can be megabytes. */
export class DraftSaver {
  private timer: ReturnType<typeof setTimeout> | null = null
  private gone = false

  /**
   * @param svg the drawing as SVG now, or null when there is nothing to keep (empty, or as it was opened): then
   *   a draft that was kept is removed
   */
  constructor(
    private readonly store: DraftStore,
    private readonly key: string,
    private readonly svg: () => string | null,
    private readonly delayMs = 3000,
    private readonly now: () => number = Date.now,
  ) {}

  /** Something changed: write the draft soon (a write that is already waiting covers this change as well). */
  touch(): void {
    if (this.gone || this.timer) return
    this.timer = setTimeout(() => void this.flush(), this.delayMs)
  }

  /** Writes now (the page is being hidden or closed). Never rejects: a draft is a convenience. */
  async flush(): Promise<void> {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    if (this.gone) return
    try {
      const svg = this.svg()
      if (svg === null) await this.store.delete(this.key)
      else await this.store.put(this.key, { svg, savedAt: this.now() })
    } catch {
      /* no storage: the drawing is just not protected */
    }
  }

  /** The drawing was saved or thrown away: the draft goes, and nothing is written any more. */
  async clear(): Promise<void> {
    this.gone = true
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    try {
      await this.store.delete(this.key)
    } catch {
      /* nothing to remove, or nothing that can be */
    }
  }
}
