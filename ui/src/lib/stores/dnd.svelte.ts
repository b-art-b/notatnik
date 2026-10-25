// State of a block being dragged with the mouse (the browser only exposes the data of a drag on drop, so the
// target needs this to draw the drop line while the pointer moves).
import type { DropZone, Path } from '../tree'

export const dnd = $state({
  /** The block being dragged. */
  from: null as null | { docName: string; path: Path; slot: string },
  /** Where it would land if dropped now. */
  over: null as null | { viewKey: string; path: Path; zone: DropZone },
})

export function endDrag() {
  dnd.from = null
  dnd.over = null
}
