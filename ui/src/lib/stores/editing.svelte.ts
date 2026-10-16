import { findById, type Cursor } from '../tree'
import { getDoc, type PageDoc } from './pages.svelte'

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
  /** The block that was edited or selected last, kept when editing stops: where a Shift+click selection starts when
   * the click itself ended the editing (the editor loses focus before the click arrives). */
  lastDoc: '',
  lastBlockId: 0,
  /** The block a command (move, indent, split...) has just put the cursor in, and when: a move re-parents the DOM of
   * the editor, which takes its focus away; that blur must not end the editing (see `Block.svelte`). */
  planBlock: 0,
  planAt: 0,
  /** Bumped to make the open editor take the focus (and scroll into view): after a move the browser may have taken
   * the focus away without any event (WebKit does), so nothing could notice it. */
  focusNonce: 0,
  /** Bumped to force the editor to remount (after the block's text changed behind it). */
  nonce: 0,
  /** Set when a block changed on disk while it was being edited (P2.27). */
  conflict: null as null | { blockId: number; mine: string; theirs: string },
})

export function startEditing(viewKey: string, docName: string, blockId: number, cursor: Cursor = 'end') {
  editing.mode = 'edit'
  editing.viewKey = viewKey
  editing.docName = docName
  editing.blockId = blockId
  editing.cursor = cursor
  editing.selection = []
  editing.conflict = null
  editing.lastDoc = docName
  editing.lastBlockId = blockId
}

export function startSelecting(viewKey: string, docName: string, ids: number[], anchorId?: number) {
  editing.mode = 'select'
  editing.viewKey = viewKey
  editing.docName = docName
  editing.selection = ids
  editing.blockId = ids[ids.length - 1] ?? 0
  editing.anchorId = anchorId ?? ids[0] ?? 0
  editing.lastDoc = docName
  editing.lastBlockId = editing.anchorId
}

export function stopEditing() {
  editing.mode = 'none'
  editing.selection = []
  editing.blockId = 0
}

export function isEditingDoc(doc: PageDoc): boolean {
  return editing.mode === 'edit' && editing.docName === doc.name
}

let pathCache: { view: unknown; id: number; path: number[] } = { view: null, id: 0, path: [] }

/** Child-index path of the block being edited ([] if none). Memoised on the page view, so each block can
 * ask for it without a search of its own; reading `doc.view` here makes callers re-run when the page changes. */
export function editingPath(): number[] {
  if (editing.mode !== 'edit') return []
  const view = getDoc(editing.docName).view
  if (!view) return []
  if (pathCache.view === view && pathCache.id === editing.blockId) return pathCache.path
  const found = findById(view.blocks, editing.blockId)
  pathCache = { view, id: editing.blockId, path: found ? found.path.map(Number) : [] }
  return pathCache.path
}
