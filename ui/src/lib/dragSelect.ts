// Selecting blocks by dragging the mouse across them: press on a block's text, move over another block, and every
// block from the first to the one under the pointer is selected (as they appear on screen). A drag inside one block
// stays an ordinary text selection. The click that ends a drag is swallowed, so it does not open an editor.
import { editing, startSelecting } from './stores/editing.svelte'
import { getDoc } from './stores/pages.svelte'
import { rangeIds } from './tree'

const THRESHOLD = 6

interface Start {
  id: number
  doc: string
  view: string
  x: number
  y: number
}

const blockAt = (x: number, y: number) => {
  const el = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-block-id][data-doc]')
  return el
    ? { id: Number(el.dataset.blockId), doc: el.dataset.doc ?? '', view: el.dataset.view ?? '' }
    : null
}

/** Installs the drag selection on the document; returns the function that removes it. */
export function installDragSelect(): () => void {
  let start: Start | null = null
  let dragging = false
  let selectable = ''

  const down = (e: PointerEvent) => {
    start = null
    if (e.button !== 0 || e.shiftKey || e.ctrlKey || e.metaKey || e.altKey) return
    const t = e.target as HTMLElement
    // only from a block's text: links, buttons, fields, the editor and the bullet keep their own behaviour
    if (!t.closest('.body') || t.closest('a, button, input, textarea, select, .cm-editor, .logbook, summary'))
      return
    const b = t.closest<HTMLElement>('[data-block-id][data-doc]')
    if (!b) return
    start = {
      id: Number(b.dataset.blockId),
      doc: b.dataset.doc ?? '',
      view: b.dataset.view ?? '',
      x: e.clientX,
      y: e.clientY,
    }
    dragging = false
  }

  const move = (e: PointerEvent) => {
    if (!start || e.buttons !== 1) return
    const here = blockAt(e.clientX, e.clientY)
    if (!dragging) {
      if (Math.hypot(e.clientX - start.x, e.clientY - start.y) < THRESHOLD) return
      // still inside the block it started in: a text selection
      if (!here || (here.id === start.id && here.doc === start.doc && here.view === start.view)) return
      dragging = true
      selectable = document.body.style.userSelect
      document.body.style.userSelect = 'none'
    }
    if (!here || here.doc !== start.doc || here.view !== start.view) return
    const view = getDoc(start.doc).view
    if (!view) return
    const ids = rangeIds(view.blocks, start.id, here.id)
    if (ids.length === 0) return
    window.getSelection()?.removeAllRanges()
    startSelecting(start.view, start.doc, ids, start.id)
    editing.blockId = here.id
  }

  const up = () => {
    if (dragging) {
      document.body.style.userSelect = selectable
      // the click that follows the drag must not edit the block it ends on
      const swallow = (e: MouseEvent) => {
        e.stopPropagation()
        e.preventDefault()
      }
      window.addEventListener('click', swallow, { capture: true, once: true })
      setTimeout(() => window.removeEventListener('click', swallow, true), 0)
    }
    start = null
    dragging = false
  }

  window.addEventListener('pointerdown', down, true)
  window.addEventListener('pointermove', move, true)
  window.addEventListener('pointerup', up, true)
  window.addEventListener('pointercancel', up, true)
  return () => {
    window.removeEventListener('pointerdown', down, true)
    window.removeEventListener('pointermove', move, true)
    window.removeEventListener('pointerup', up, true)
    window.removeEventListener('pointercancel', up, true)
  }
}
