// Dragging a page or a block from one graph to the other (MG-6). Both graphs are in this page, so the drag state is
// ordinary module state: the target knows what comes (and from which graph) while the pointer moves. A drop on the
// graph the notes are in is left to the code that moves blocks within a page.
import { app } from './stores/app.svelte'
import { dnd } from './stores/dnd.svelte'

export const pageDrag = $state({
  name: null as string | null,
  slot: '',
  /** Known when the list that starts the drag knows it; otherwise the dialog asks the graph. */
  hasSubpages: undefined as boolean | undefined,
})

/** Starts dragging a page (its title, a row of a list) that is in the graph of `slot`. */
export function startPageDrag(e: DragEvent, slot: string, name: string, hasSubpages?: boolean) {
  if (!e.dataTransfer) return
  e.dataTransfer.effectAllowed = 'copyMove'
  e.dataTransfer.setData('text/plain', name)
  pageDrag.name = name
  pageDrag.slot = slot
  pageDrag.hasSubpages = hasSubpages
}

export function endPageDrag() {
  pageDrag.name = null
}

/** Is something from another graph than `slot` being dragged? */
export function foreignTo(slot: string): boolean {
  if (pageDrag.name !== null) return pageDrag.slot !== slot
  if (dnd.from) return dnd.from.slot !== slot
  return false
}

/** A drop on the graph in `slot` (a section, or the page area while that graph is active): opens the dialog that takes
 * the notes there. `page` is where blocks go, the page on screen. Returns whether it was a drop from the other graph. */
export function dropOn(e: DragEvent, slot: string, page?: string): boolean {
  if (!foreignTo(slot)) return false
  e.preventDefault()
  if (pageDrag.name !== null) {
    app.transfer = {
      kind: 'pages',
      page: pageDrag.name,
      hasSubpages: pageDrag.hasSubpages,
      from: pageDrag.slot,
      to: slot,
    }
  } else if (dnd.from) {
    app.transfer = {
      kind: 'blocks',
      page: dnd.from.docName,
      blocks: [dnd.from.path.map(Number)],
      from: dnd.from.slot,
      to: slot,
      targetPage: page,
    }
  }
  endPageDrag()
  return true
}
