// Dragging a page or a block from one pane to the other (MG-6). The two panes are two pages (frames), so the browser's
// drag data is what crosses between them. The payload says which graph it comes from: a drop on the pane of the same
// graph is left to the code that moves blocks within a page.
import { app } from './stores/app.svelte'
import { backend } from './api'

export const PAGE_TYPE = 'application/x-notatnik-page'
export const BLOCK_TYPE = 'application/x-notatnik-block'

/** A page or block being dragged in this pane (the other pane's drags are not ours). */
export const mine = { page: false }

/** Starts dragging a page (its title, its row in a list). */
export function startPageDrag(e: DragEvent, name: string, hasSubpages = false) {
  if (!e.dataTransfer) return
  e.dataTransfer.effectAllowed = 'copyMove'
  e.dataTransfer.setData(
    PAGE_TYPE,
    JSON.stringify({ name, hasSubpages, graph: app.graph?.id ?? '', slot: backend.slot }),
  )
  e.dataTransfer.setData('text/plain', name)
  mine.page = true
}

export function endPageDrag() {
  mine.page = false
}

interface Dropped {
  kind: 'pages' | 'blocks'
  page: string
  path?: number[]
  hasSubpages?: boolean
  graph: string
  slot: string
}

function read(dt: DataTransfer): Dropped | null {
  try {
    const page = dt.getData(PAGE_TYPE)
    if (page) {
      const p = JSON.parse(page)
      return { kind: 'pages', page: p.name, hasSubpages: !!p.hasSubpages, graph: p.graph, slot: p.slot ?? '' }
    }
    const block = dt.getData(BLOCK_TYPE)
    if (block) {
      const b = JSON.parse(block)
      return { kind: 'blocks', page: b.docName, path: b.path, graph: b.graph ?? '', slot: b.slot ?? '' }
    }
  } catch {
    /* not ours */
  }
  return null
}

/** Is something from the other pane being dragged over? (Only the types are known before the drop.) */
export function foreignDrag(e: DragEvent, ownBlockDrag: boolean): boolean {
  const types = e.dataTransfer?.types ?? []
  if (types.includes(PAGE_TYPE)) return !mine.page
  if (types.includes(BLOCK_TYPE)) return !ownBlockDrag
  return false
}

/** A drop from the other pane: opens the dialog that takes the notes into this pane's graph (`page`: the page on
 * screen, where blocks are added). Returns whether it was one. */
export function dropFromOtherPane(e: DragEvent, page: string | null): boolean {
  if (!e.dataTransfer) return false
  const d = read(e.dataTransfer)
  if (!d || !d.graph || d.graph === app.graph?.id) return false
  e.preventDefault()
  app.transfer = {
    kind: d.kind,
    page: d.page,
    blocks: d.path ? [d.path] : undefined,
    hasSubpages: d.hasSubpages,
    from: d.slot,
    to: backend.slot,
    targetPage: d.kind === 'blocks' ? (page ?? d.page) : undefined,
  }
  return true
}
