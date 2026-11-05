// The sidecar page of a PDF or an EPUB as a viewer uses it (PA-2, EA-3): finding or making it, the highlights read from
// it, and the edits that every viewer makes to them (colour, delete, comment, a new block). The PDF viewer and the EPUB
// reader each hold one `Sidecar`; what is specific to a kind of document stays in the viewer.
import { tick } from 'svelte'
import { backend, type AnnotationPage, type BlockOp, type PageView } from '../api'
import { errorMessage, toast } from '../stores/app.svelte'
import { startEditing } from '../stores/editing.svelte'
import { getDoc, type PageDoc } from '../stores/pages.svelte'
import { highlightsOf, type Color, type Highlight } from '../pdf/highlights'

/** The `viewKey` of the comments shown in the panel (an editing block is found by it). */
export const PANEL_VIEW = 'annotation-panel'

export const refOf = (h: Highlight) => `((${h.uid}))`
export const quoteBlock = (h: Highlight) => `> ${h.quote} ${refOf(h)}`

export async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    toast('The text could not be copied.', 'error')
  }
}

export class Sidecar {
  /** What `annotation_page` answered; `page` is empty while the file has no sidecar. */
  side = $state<AnnotationPage | null>(null)
  /** The sidecar's document, once there is one. It is made here, outside any `$derived`: a state read in the same
   * derived that made it would not be tracked. */
  doc = $state<PageDoc | null>(null)
  /** The sidecar was asked for (a viewer waits for this before choosing the first position). */
  loaded = $state(false)
  /** Highlights that were looked for in the document and are not there (by `uid`, or the block id without one). */
  missing = $state<Record<string, boolean>>({})
  /** The `^id` of the highlight selected in the list. */
  selected = $state('')
  highlights = $derived(highlightsOf(this.doc?.view))
  private fixed = new Set<string>()

  /** The Nextcloud document this sidecar belongs to: its address (NA-4); `path` is then the copy in the device cache. */
  origin: string | undefined

  constructor(public path: string) {}

  /** The document turned out to be a copy of a Nextcloud file: ask for its sidecar by the file's address. */
  use(path: string, origin: string) {
    this.path = path
    this.origin = origin
  }

  /** Looks for the sidecar; makes none. */
  async load() {
    try {
      this.side = await backend.call('annotation_page', {
        path: this.path,
        origin: this.origin,
        create: false,
      })
      if (this.side.page) {
        // `annotation_page` may have rewritten the file (the `sha` of a book that was clipped again): a document kept from
        // an earlier reading has the old hash, and the first repair of a highlight would be refused as a conflict
        const doc = getDoc(this.side.page)
        await doc.load()
        this.doc = doc
      }
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
    this.loaded = true
  }

  /** The sidecar page, made on the first highlight. */
  async ensure(): Promise<PageDoc> {
    if (!this.side?.page) {
      this.side = await backend.call('annotation_page', { path: this.path, origin: this.origin })
      this.doc = getDoc(this.side.page)
      await this.doc.load()
    }
    const doc = this.doc!
    await doc.loaded
    return doc
  }

  static pathOfUid(view: PageView | null, uid: string): number[] | null {
    const i = view?.blocks.findIndex((b) => b.uid === uid) ?? -1
    return i < 0 ? null : [i]
  }

  /** Applies ops built for the block of `h`, from the page as it is when the edit reaches the front of the queue. */
  edit(h: Highlight, build: (path: number[], view: PageView | null) => BlockOp[]) {
    const uid = h.uid
    if (!uid || !this.doc) return Promise.resolve(null)
    return this.doc.applyWith((view) => {
      const path = Sidecar.pathOfUid(view, uid)
      return path ? build(path, view) : null
    })
  }

  setColor(h: Highlight, color: Color) {
    void this.edit(h, (path) => [
      { op: 'set_property', block: { path, hash: null }, key: 'hl-color', value: color },
    ])
  }

  remove(h: Highlight) {
    void this.edit(h, (path) => [{ op: 'delete', block: { path, hash: null } }])
  }

  /** Appends blocks to the sidecar in one edit (one undo step). */
  async insert(texts: string[]) {
    const doc = await this.ensure()
    await doc.applyWith((v) =>
      texts.map<BlockOp>((text, k) => ({
        op: 'insert',
        parent: [],
        index: (v?.blocks.length ?? 0) + k,
        text,
      })),
    )
  }

  /** The text of `n` new block ids. */
  async newIds(n: number): Promise<string[]> {
    const out: string[] = []
    for (let i = 0; i < n; i++) out.push((await backend.call('new_block_id', {})).value)
    return out
  }

  /** Adds an empty child block to the highlight and starts editing it in the panel (the panel has to be open). */
  async addComment(h: Highlight) {
    const uid = h.uid
    if (!uid || !this.doc) return
    const view = await this.edit(h, (path, v) => {
      const block = v?.blocks[path[0]]
      return [{ op: 'insert', parent: path, index: block?.children.length ?? 0, text: '' }]
    })
    const block = view?.blocks.find((b) => b.uid === uid)
    const child = block?.children[block.children.length - 1]
    if (child && this.side) {
      this.selected = uid
      await tick()
      startEditing(PANEL_VIEW, this.side.page, child.id, 'end')
    }
  }

  /** A highlight was looked for in the document: it is there (`ok`) or not. */
  mark(h: Highlight, ok: boolean) {
    const k = h.uid ?? String(h.blockId)
    if ((this.missing[k] ?? false) === ok) this.missing[k] = !ok
  }

  /** Writes new values of the locator properties of `h` once (the key says which change this is). */
  correct(h: Highlight, props: Record<string, string>) {
    if (!h.uid) return
    const key = `${h.uid}:${JSON.stringify(props)}`
    if (this.fixed.has(key)) return
    this.fixed.add(key)
    void this.edit(h, (path) =>
      Object.entries(props).map(([k, value]) => ({
        op: 'set_property',
        block: { path, hash: null },
        key: k,
        value,
      })),
    )
  }
}
