import type { Cursor } from '../tree'
import type { PageDoc } from './pages.svelte'

/** What the user is doing right now: editing one block's text, or having blocks selected. */
export const editing = $state({
  mode: 'none' as 'none' | 'edit' | 'select',
  viewKey: '',
  docName: '',
  blockId: 0,
  cursor: 'end' as Cursor,
  /** Selected block IDs (always siblings), when mode is 'select'. */
  selection: [] as number[],
  /** The block where a Shift-selection started. */
  anchorId: 0,
})

export function startEditing(viewKey: string, docName: string, blockId: number, cursor: Cursor = 'end') {
  editing.mode = 'edit'
  editing.viewKey = viewKey
  editing.docName = docName
  editing.blockId = blockId
  editing.cursor = cursor
  editing.selection = []
}

export function startSelecting(viewKey: string, docName: string, ids: number[], anchorId?: number) {
  editing.mode = 'select'
  editing.viewKey = viewKey
  editing.docName = docName
  editing.selection = ids
  editing.blockId = ids[ids.length - 1] ?? 0
  editing.anchorId = anchorId ?? ids[0] ?? 0
}

export function stopEditing() {
  editing.mode = 'none'
  editing.selection = []
  editing.blockId = 0
}

export function isEditingDoc(doc: PageDoc): boolean {
  return editing.mode === 'edit' && editing.docName === doc.name
}
