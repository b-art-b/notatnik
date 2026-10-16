// Runs block commands: turns a Plan from tree.ts into a request and moves the cursor afterwards.
import { backend, type BlockOp } from '../api'
import { editing, startEditing, startSelecting, stopEditing } from '../stores/editing.svelte'
import { undoRedo, type PageDoc } from '../stores/pages.svelte'
import { app, navigate, toast, errorMessage } from '../stores/app.svelte'
import { doneAt, nextTaskState } from '../tasks'
import {
  afterRoots,
  areSiblings,
  deleteBlocks,
  findById,
  getBlock,
  indent,
  indentMany,
  insertOps,
  mergeWithNext,
  mergeWithPrevious,
  moveDown,
  moveMany,
  moveUp,
  nextVisible,
  outdent,
  outdentMany,
  parsePasted,
  previousVisible,
  rangeIds,
  selectionRoots,
  split,
  subtreeIds,
  toMarkdown,
  toPasted,
  visibleBlocks,
  type PastedBlock,
  type Path,
  type Plan,
} from '../tree'

/** Everything a command needs to know about where it was triggered. */
export interface Ctx {
  doc: PageDoc
  viewKey: string
}

/** If the editor text differs from the stored block text, an UpdateText op goes first. */
function withText(doc: PageDoc, blockId: number, text: string | undefined, plan: Plan): Plan {
  const view = doc.view
  if (!view || text === undefined) return plan
  const hit = findById(view.blocks, blockId)
  if (!hit || hit.block.text === text) return plan
  return {
    ...plan,
    ops: [{ op: 'update_text', block: { path: hit.path, hash: null }, text } as BlockOp, ...plan.ops],
  }
}

export async function runPlan(ctx: Ctx, plan: Plan): Promise<void> {
  await ctx.doc.loaded
  // a move re-parents the editor's DOM while the request is answered, which takes its focus away: the block being
  // edited is marked first, so that blur does not end the editing
  if (editing.mode === 'edit') {
    editing.planBlock = editing.blockId
    editing.planAt = Date.now()
  }
  const view = await ctx.doc.apply(plan.ops)
  if (view && plan.focus) {
    const b = getBlock(view.blocks, plan.focus.path)
    if (b) {
      startEditing(ctx.viewKey, ctx.doc.name, b.id, plan.focus.cursor)
      editing.planBlock = b.id
      editing.planAt = Date.now()
    }
  }
}

/**
 * Saves the editor text if it changed (blur, idle, leaving the block).
 *
 * `prev` is the text the editor last synced with the block. If the block no longer has that text, the
 * block changed behind the editor's back (an external edit, or a runtime id that now points elsewhere), so
 * nothing is written and a conflict is raised instead: the user chooses "keep mine" or "take theirs".
 */
export async function commitText(
  ctx: Ctx,
  blockId: number,
  text: string,
  prev?: string,
  opts: { onlyIfActive?: boolean } = {},
): Promise<void> {
  await ctx.doc.applyWith((view) => {
    if (!view) return null
    // a late commit from an editor that has since been replaced must not write anything
    if (opts.onlyIfActive && !(editing.mode === 'edit' && editing.blockId === blockId)) return null
    const hit =
      findById(view.blocks, blockId) ??
      (view.preamble?.id === blockId ? { path: [], block: view.preamble } : undefined)
    if (!hit || hit.block.text === text) return null
    if (prev !== undefined && hit.block.text !== prev) {
      if (text !== prev && editing.mode === 'edit' && editing.blockId === blockId)
        editing.conflict = { blockId, mine: text, theirs: hit.block.text }
      return null
    }
    return [{ op: 'update_text', block: { path: hit.path, hash: null }, text }]
  })
}

/** "Keep mine": writes the editor text over whatever is on disk now. */
export async function keepMine(ctx: Ctx, blockId: number): Promise<void> {
  const c = editing.conflict
  const view = ctx.doc.view
  if (!c || c.blockId !== blockId || !view) return
  const hit = findById(view.blocks, blockId)
  editing.conflict = null
  if (hit) await ctx.doc.apply([{ op: 'update_text', block: { path: hit.path, hash: null }, text: c.mine }])
  editing.nonce++
}

/** "Take theirs": drops the editor text and reloads the editor with the block as it is on disk. */
export function takeTheirs(): void {
  editing.conflict = null
  editing.nonce++
}

function pathOf(ctx: Ctx, blockId: number): Path | undefined {
  const view = ctx.doc.view
  return view ? findById(view.blocks, blockId)?.path : undefined
}

/** The ops that insert a template's blocks at `path`. In an empty block the first template block takes its place;
 * otherwise the blocks go after it, as siblings. `stored` is the block's text in the page, `typed` the editor's. */
export function templateOps(
  path: Path,
  stored: string,
  typed: string,
  blocks: { parent: number[]; index: number; text: string }[],
  props: BlockOp[] = [],
): BlockOp[] {
  // the template's page properties first (only the ones the page lacks: the server leaves the others out)
  const ops: BlockOp[] = [...props]
  const empty = typed.trim() === ''
  if (!empty && stored !== typed) ops.push({ op: 'update_text', block: { path, hash: null }, text: typed })
  const parent = path.slice(0, -1)
  const base = path[path.length - 1] + (empty ? 0 : 1)
  for (const b of blocks) {
    if (b.parent.length === 0) {
      if (empty && b.index === 0) ops.push({ op: 'update_text', block: { path, hash: null }, text: b.text })
      else ops.push({ op: 'insert', parent, index: base + b.index, text: b.text })
    } else {
      const [top, ...rest] = b.parent
      const at = empty && top === 0 ? path : [...parent, base + top]
      ops.push({ op: 'insert', parent: [...at, ...rest], index: b.index, text: b.text })
    }
  }
  return ops
}

/** The `^id` of a block, made when it has none (the page is edited through the queue like any other change).
 * `null` when the block is gone. */
export async function ensureBlockUid(doc: PageDoc, blockId: number): Promise<string | null> {
  await doc.loaded
  const found = doc.view ? findById(doc.view.blocks, blockId) : undefined
  if (!found) return null
  if (found.block.uid) return found.block.uid
  const id = (await backend.call('new_block_id', {})).value
  const after = await doc.applyWith((view) => {
    const hit = view ? findById(view.blocks, blockId) : undefined
    if (!hit || hit.block.uid) return null
    return [{ op: 'ensure_block_id', block: { path: hit.path, hash: null }, id }]
  })
  const now = after ? findById(after.blocks, blockId) : undefined
  return now?.block.uid ?? null
}

/** Puts text on the clipboard. The async API needs a secure context and a user gesture; a hidden textarea is the
 * fallback (older webviews). */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    const ta = document.createElement('textarea')
    ta.value = text
    ta.style.position = 'fixed'
    ta.style.opacity = '0'
    document.body.appendChild(ta)
    ta.select()
    const ok = document.execCommand('copy')
    ta.remove()
    return ok
  }
}

/** Tags and page links of a task's first line (not dates): what a follow-up is about as well. */
export function carriedTokens(firstLine: string): string[] {
  const found = firstLine.match(/#\[\[[^\]\n]+\]\]|(?<=^|\s)#[\p{L}\p{N}_/-]+|\[\[[^\]\n]+\]\]/gu) ?? []
  const out: string[] = []
  for (const t of found) {
    if (/^\[\[\d{4}-\d{2}-\d{2}\]\]$/.test(t)) continue
    if (!out.includes(t)) out.push(t)
  }
  return out
}

/** The text of a follow-up: an open task with the tags, links and priority of the finished one, linked back with
 * `follows::`. Returns the text and where the cursor goes (the end of the first line). */
export function followUpText(
  firstLine: string,
  priority: string | null,
  id: string,
): { text: string; cursor: number } {
  const head = ['TODO', ...carriedTokens(firstLine)].join(' ') + ' '
  const lines = [head]
  if (priority) lines.push(`priority:: ${priority}`)
  lines.push(`follows:: ((${id}))`)
  return { text: lines.join('\n'), cursor: head.length }
}

/** `/followup`: marks the task done (as Ctrl+Enter does: completion time, repeat) and adds the next task right after
 * it, as a sibling, linked with `follows::` (TK-15). The editor goes to the new task. */
async function followUp(ctx: Ctx, blockId: number, typed: string): Promise<void> {
  await ctx.doc.loaded
  const first = ctx.doc.view ? findById(ctx.doc.view.blocks, blockId) : undefined
  if (!first) return
  // the task needs an ID to be referenced; one is made here when it has none
  let id = first.block.uid
  if (!id) {
    try {
      id = (await backend.call('new_block_id', {})).value
    } catch (e) {
      toast(errorMessage(e), 'error')
      return
    }
  }
  const needsId = !first.block.uid
  let newPath: number[] | null = null
  const after = await ctx.doc.applyWith((view) => {
    const hit = view ? findById(view.blocks, blockId) : undefined
    if (!hit) return null
    const block = { path: hit.path, hash: null }
    const ops: BlockOp[] = []
    if (typed !== hit.block.text) ops.push({ op: 'update_text', block, text: typed })
    if (needsId && !hit.block.uid) ops.push({ op: 'ensure_block_id', block, id })
    if (hit.block.task?.state !== 'DONE')
      ops.push({
        op: 'set_task_state',
        block,
        state: 'DONE',
        done_at: doneAt(app.today),
        completed_limit: app.graph?.completed_log_limit ?? 0,
      })
    const next = followUpText(typed.split('\n')[0], hit.block.task?.priority ?? null, hit.block.uid ?? id)
    const parent = hit.path.slice(0, -1)
    const index = hit.path[hit.path.length - 1] + 1
    newPath = [...parent, index]
    ops.push({ op: 'insert', parent, index, text: next.text })
    return ops
  })
  const created = after && newPath ? getBlock(after.blocks, newPath) : undefined
  if (created) {
    const cursor = created.text.split('\n')[0].length
    startEditing(ctx.viewKey, ctx.doc.name, created.id, cursor)
  } else stopEditing()
}

export type EditorAction =
  | { kind: 'enter'; text: string; cursor: number; prev: string }
  | { kind: 'indent' | 'outdent' | 'move-up' | 'move-down'; text: string; prev: string }
  | { kind: 'merge-prev' | 'merge-next'; text: string; prev: string }
  | { kind: 'nav-up' | 'nav-down'; text: string; column: number; prev: string }
  | { kind: 'escape'; text: string; prev: string }
  | { kind: 'cycle-task'; text: string; prev: string }
  | { kind: 'template'; name: string; text: string; prev: string }
  | { kind: 'followup'; text: string; prev: string }
  | { kind: 'select-all'; text: string; prev: string }

/** Handles an action raised by the editor. Returns true when the key press was consumed. */
export async function editorAction(ctx: Ctx, blockId: number, a: EditorAction): Promise<boolean> {
  const view = ctx.doc.view
  const path = pathOf(ctx, blockId)
  if (!view || !path) return false
  const blocks = view.blocks
  switch (a.kind) {
    case 'enter': {
      const plan = split(blocks, path, a.text, a.cursor)
      if (!plan) return false
      await runPlan(ctx, withText(ctx.doc, blockId, undefined, plan))
      return true
    }
    case 'indent':
    case 'outdent':
    case 'move-up':
    case 'move-down': {
      const f = { indent, outdent, 'move-up': moveUp, 'move-down': moveDown }[a.kind]
      const plan = f(blocks, path)
      if (!plan) {
        await commitText(ctx, blockId, a.text, a.prev)
        return true
      }
      await runPlan(ctx, withText(ctx.doc, blockId, a.text, plan))
      return true
    }
    case 'merge-prev': {
      const plan = mergeWithPrevious(blocks, path, a.text)
      if (!plan) return false
      await runPlan(ctx, plan)
      return true
    }
    case 'merge-next': {
      const plan = mergeWithNext(blocks, path, a.text)
      if (!plan) return false
      await runPlan(ctx, plan)
      return true
    }
    case 'nav-up':
    case 'nav-down': {
      const target = a.kind === 'nav-up' ? previousVisible(blocks, path) : nextVisible(blocks, path)
      if (!target) return false
      await commitText(ctx, blockId, a.text, a.prev)
      const fresh = ctx.doc.view
      const hit = fresh ? getBlock(fresh.blocks, target.path) : undefined
      if (hit)
        startEditing(
          ctx.viewKey,
          ctx.doc.name,
          hit.id,
          Math.min(
            a.column,
            hit.text.split('\n')[a.kind === 'nav-up' ? hit.text.split('\n').length - 1 : 0].length,
          ),
        )
      return true
    }
    case 'cycle-task': {
      const hit = findById(view.blocks, blockId)
      if (!hit) return false
      await setTaskState(ctx, blockId, nextTaskState(hit.block.task?.state), a.text)
      return true
    }
    case 'select-all': {
      // the second Ctrl+A in a block: leave the editor (its text is saved) and select every block of the page
      await commitText(ctx, blockId, a.text, a.prev)
      selectAllBlocks(ctx)
      return true
    }
    case 'followup': {
      await followUp(ctx, blockId, a.text)
      return true
    }
    case 'template': {
      let tpl
      try {
        tpl = await backend.call('template_blocks', { name: a.name, page: ctx.doc.name })
      } catch (e) {
        toast(errorMessage(e), 'error')
        return true
      }
      // built from the page as it is when the request reaches the front of the queue
      await ctx.doc.applyWith((v) => {
        const hit = v ? findById(v.blocks, blockId) : undefined
        if (!hit) return null
        return templateOps(hit.path, hit.block.text, a.text, tpl.blocks, tpl.props)
      })
      stopEditing()
      return true
    }
    case 'escape': {
      await commitText(ctx, blockId, a.text, a.prev)
      if (editing.conflict?.blockId === blockId) return true // stay in the editor: the user has to choose first
      startSelecting(ctx.viewKey, ctx.doc.name, [blockId])
      return true
    }
  }
}

// ---- selection commands (blocks selected, not edited) --------------------------------------
//
// A selection is a set of blocks of one page (ids). The blocks that count are its *roots*: those with no selected
// ancestor, because a selected block takes its children along. Every command builds its ops when the request reaches
// the front of the page's queue (`applyWith`), from the page as it is then.

type View = NonNullable<PageDoc['view']>

/** The roots of the selection in `view`, in document order. */
const rootsIn = (view: View, ids: number[]) => selectionRoots(view.blocks, ids)

export function selectionMarkdown(ctx: Ctx, withIds = false): string {
  const view = ctx.doc.view
  if (!view) return ''
  return toMarkdown(
    rootsIn(view, editing.selection).map((r) => r.block),
    '  ',
    0,
    withIds,
  )
}

const plural = (n: number) => `${n} block${n === 1 ? '' : 's'}`

/** Ctrl+C: the selected blocks and their children as Markdown, without block IDs (the pasted copies get their own). */
export async function selectionCopy(ctx: Ctx): Promise<void> {
  const n = ctx.doc.view ? rootsIn(ctx.doc.view, editing.selection).length : 0
  if (n === 0) return
  const ok = await copyText(selectionMarkdown(ctx, false))
  toast(ok ? `Copied ${plural(n)}` : 'Could not copy', ok ? 'info' : 'error')
}

/** Ctrl+X: copy with the block IDs, then delete. Pasted back, the blocks keep their IDs, so references to them
 * survive the move. */
export async function selectionCut(ctx: Ctx): Promise<void> {
  const view = ctx.doc.view
  const n = view ? rootsIn(view, editing.selection).length : 0
  if (n === 0) return
  const ok = await copyText(selectionMarkdown(ctx, true))
  if (!ok) return void toast('Could not copy: nothing was cut', 'error')
  await selectionDelete(ctx)
  toast(`Cut ${plural(n)}`)
}

/** Paths of the roots of `ids` in `view`. */
const rootPaths = (view: View, ids: number[]): Path[] => rootsIn(view, ids).map((r) => r.path)

/** Runs a command on the selected blocks (their roots). The ops are built when the request reaches the front of the
 * page's queue: a key pressed right after another command (Tab, then Delete) must not use a page that the first
 * command has not finished changing yet. Returns whether anything was sent. */
async function runOnSelection(
  ctx: Ctx,
  f: (blocks: View['blocks'], paths: Path[]) => Plan | null,
): Promise<boolean> {
  const ids = [...editing.selection]
  let sent = false
  const after = await ctx.doc.applyWith((view) => {
    if (!view) return null
    const plan = f(view.blocks, rootPaths(view, ids))
    sent = !!plan && plan.ops.length > 0
    return plan ? plan.ops : null
  })
  if (sent && after) {
    // keep the same blocks selected (runtime ids survive the edit)
    startSelecting(
      ctx.viewKey,
      ctx.doc.name,
      ids.filter((id) => findById(after.blocks, id)),
      editing.anchorId,
    )
  }
  return sent
}

// Tab and Shift+Tab work on blocks at one level (siblings); a mixed selection is left alone
export const selectionIndent = (ctx: Ctx) =>
  runOnSelection(ctx, (b, paths) => (areSiblings(paths) ? indentMany(b, paths) : null))
export const selectionOutdent = (ctx: Ctx) =>
  runOnSelection(ctx, (b, paths) => (areSiblings(paths) ? outdentMany(b, paths) : null))

export async function selectionDelete(ctx: Ctx) {
  const ids = [...editing.selection]
  let sent = false
  const after = await ctx.doc.applyWith((view) => {
    if (!view) return null
    const plan = deleteBlocks(view.blocks, rootPaths(view, ids))
    sent = !!plan
    return plan ? plan.ops : null
  })
  if (sent && after) stopEditing()
}

/** Alt+↑/↓: the selected siblings move one place together. */
export async function selectionMove(ctx: Ctx, dir: 'up' | 'down') {
  await runOnSelection(ctx, (blocks, paths) => moveMany(blocks, paths, dir))
}

/** Selects `ids` (blocks of this page) and puts the cursor of the selection on `focus`. */
function select(ctx: Ctx, ids: number[], anchor: number, focus: number) {
  startSelecting(ctx.viewKey, ctx.doc.name, ids, anchor)
  editing.blockId = focus
}

/** ↑/↓ move the selection to the neighbouring block; Shift+↑/↓ extend it from where it started, over every block in
 * between as they appear on screen (children included). */
export function selectionStep(ctx: Ctx, dir: 'up' | 'down', extend: boolean) {
  const view = ctx.doc.view
  if (!view || editing.selection.length === 0) return
  const all = visibleBlocks(view.blocks)
  const cur = all.findIndex((v) => v.block.id === editing.blockId)
  if (cur < 0) return
  if (extend) {
    const j = cur + (dir === 'down' ? 1 : -1)
    if (j < 0 || j >= all.length) return
    const target = all[j].block.id
    select(ctx, rangeIds(view.blocks, editing.anchorId, target), editing.anchorId, target)
    return
  }
  let j = cur + (dir === 'down' ? 1 : -1)
  const curPath = all[cur].path
  // stay among siblings: skip descendants of other blocks, stop at a different parent
  while (j >= 0 && j < all.length) {
    const p = all[j].path
    if (p.length === curPath.length && p.slice(0, -1).join('.') === curPath.slice(0, -1).join('.')) break
    if (p.length < curPath.length) break
    j += dir === 'down' ? 1 : -1
  }
  if (j < 0 || j >= all.length) return
  startSelecting(ctx.viewKey, ctx.doc.name, [all[j].block.id])
}

/** The block a selection with the mouse (or Shift+click) starts from: the selection's anchor, or the block being edited. */
function anchorOf(ctx: Ctx): number {
  if (editing.docName === ctx.doc.name) {
    if (editing.mode === 'select' && editing.anchorId) return editing.anchorId
    if (editing.mode === 'edit') return editing.blockId
  }
  return editing.lastDoc === ctx.doc.name ? editing.lastBlockId : 0
}

/** Shift+click: select from the anchor to the clicked block (all blocks between, as on screen). */
export function selectRangeTo(ctx: Ctx, blockId: number) {
  const view = ctx.doc.view
  if (!view) return
  const anchor = anchorOf(ctx) || blockId
  select(ctx, rangeIds(view.blocks, anchor, blockId), anchor, blockId)
}

/** Ctrl+click: adds the block (with its children) to the selection, or takes it out again. */
export function toggleInSelection(ctx: Ctx, blockId: number) {
  const view = ctx.doc.view
  const hit = view ? findById(view.blocks, blockId) : undefined
  if (!view || !hit) return
  const mine = new Set(subtreeIds(hit.block))
  let ids: number[] = []
  if (editing.docName === ctx.doc.name && editing.mode === 'select') ids = [...editing.selection]
  else if (editing.docName === ctx.doc.name && editing.mode === 'edit') {
    const open = findById(view.blocks, editing.blockId)
    if (open) ids = subtreeIds(open.block)
  }
  const had = ids.includes(blockId)
  ids = had ? ids.filter((id) => !mine.has(id)) : [...new Set([...ids, ...mine])]
  if (ids.length === 0) return stopEditing()
  select(ctx, ids, blockId, blockId)
}

/** Ctrl+A on a selection, or Ctrl+A a second time in a block: every block of the page. */
export function selectAllBlocks(ctx: Ctx) {
  const view = ctx.doc.view
  if (!view) return
  const all = visibleBlocks(view.blocks)
  if (all.length === 0) return
  select(
    ctx,
    all.map((v) => v.block.id),
    all[0].block.id,
    all[all.length - 1].block.id,
  )
}

/** Selects the blocks at `parent`/`from`.. (`count` of them) once they exist: the copies a duplicate or a paste made. */
function selectNew(ctx: Ctx, view: View | null, parent: Path, from: number, count: number) {
  const ids: number[] = []
  for (let i = 0; i < count; i++) {
    const b = view ? getBlock(view.blocks, [...parent, from + i]) : undefined
    if (b) ids.push(b.id)
  }
  if (ids.length) select(ctx, ids, ids[0], ids[ids.length - 1])
}

/** Ctrl+D: a copy of the selected blocks (with their children) right below the last one. */
export async function selectionDuplicate(ctx: Ctx): Promise<void> {
  const ids = [...editing.selection]
  let where: { parent: Path; index: number } | null = null
  let count = 0
  const after = await ctx.doc.applyWith((view) => {
    if (!view) return null
    const roots = rootsIn(view, ids)
    where = afterRoots(roots.map((r) => r.path))
    if (!where) return null
    count = roots.length
    return insertOps(
      toPasted(roots.map((r) => r.block)),
      (where as { parent: Path; index: number }).parent,
      (where as { parent: Path; index: number }).index,
    )
  })
  const w = where as { parent: Path; index: number } | null
  if (w && after) selectNew(ctx, after, w.parent, w.index, count)
}

/** Ctrl+V on a selection: the copied blocks go right below the last selected block, at its level, with their
 * structure. Returns whether anything was pasted. */
export async function selectionPaste(ctx: Ctx, text: string): Promise<boolean> {
  const pasted = parsePasted(text)
  if (pasted.length === 0) return false
  const ids = [...editing.selection]
  let where: { parent: Path; index: number } | null = null
  const after = await ctx.doc.applyWith((view) => {
    if (!view) return null
    where = afterRoots(rootPaths(view, ids))
    if (!where) return null
    const w = where as { parent: Path; index: number }
    return insertOps(pasted, w.parent, w.index)
  })
  const w = where as { parent: Path; index: number } | null
  if (w && after) selectNew(ctx, after, w.parent, w.index, pasted.length)
  return !!w
}

/** Ctrl+Enter on a selection: every selected block gets the state that follows the first one's (so a mixed
 * selection ends up the same). */
export async function selectionCycleTask(ctx: Ctx): Promise<void> {
  const ids = [...editing.selection]
  await ctx.doc.applyWith((view) => {
    if (!view) return null
    const hits = ids.map((id) => findById(view.blocks, id)).filter((h) => !!h)
    if (hits.length === 0) return null
    const state = nextTaskState(hits[0].block.task?.state)
    return hits.map((h) => taskStateOp(h.path, state))
  })
}

/** Ctrl+Shift+C: a `((id))` reference per selected block (a block without an ID gets one), one per line. */
export async function selectionCopyRefs(ctx: Ctx): Promise<void> {
  const view = ctx.doc.view
  if (!view) return
  const roots = rootsIn(view, editing.selection).map((r) => r.block.id)
  const refs: string[] = []
  for (const id of roots) {
    const uid = await ensureBlockUid(ctx.doc, id)
    if (uid) refs.push(`((${uid}))`)
  }
  if (refs.length === 0) return
  const ok = await copyText(refs.join('\n'))
  toast(
    ok ? `Copied ${refs.length} reference${refs.length === 1 ? '' : 's'}` : 'Could not copy',
    ok ? 'info' : 'error',
  )
}

export function selectionEdit(ctx: Ctx) {
  if (editing.selection.length === 0) return
  startEditing(ctx.viewKey, ctx.doc.name, editing.blockId, 'end')
}

export async function globalUndo(kind: 'undo' | 'redo') {
  const name = await undoRedo(kind)
  if (name) {
    stopEditing()
    if (
      name !==
      (window.location.hash.startsWith('#/page/')
        ? decodeURIComponent(window.location.hash.slice(7).split('?')[0])
        : '')
    ) {
      // undo happened on another page: show it
      const onJournals = window.location.hash.startsWith('#/journals') || window.location.hash === ''
      if (!onJournals) navigate({ kind: 'page', name, zoom: [] })
    }
  }
}

/** Multi-line paste: the pasted outline replaces an empty block or follows the current one. */
export async function pasteBlocks(ctx: Ctx, blockId: number, text: string): Promise<boolean> {
  const view = ctx.doc.view
  const path = pathOf(ctx, blockId)
  const block = view && path ? getBlock(view.blocks, path) : undefined
  const pasted = parsePasted(text)
  if (!view || !path || !block || pasted.length === 0) return false
  const ops: BlockOp[] = []
  const parent = path.slice(0, -1)
  let at = path[path.length - 1] + 1
  const replaceCurrent = block.text.trim() === ''
  const emit = (list: PastedBlock[], par: Path, start: number) => {
    list.forEach((p, i) => {
      ops.push({ op: 'insert', parent: par, index: start + i, text: p.text })
      emit(p.children, [...par, start + i], 0)
    })
  }
  emit(pasted, parent, at)
  if (replaceCurrent) ops.push({ op: 'delete', block: { path, hash: null } })
  void at
  at = 0
  await ctx.doc.apply(ops)
  stopEditing()
  return true
}

/** The op that changes a task's state, with the clock values the backend needs for `done::` and repeats. */
export function taskStateOp(path: number[], state: import('../api').TaskState | null): BlockOp {
  return {
    op: 'set_task_state',
    block: { path, hash: null },
    state,
    done_at: doneAt(app.today),
    completed_limit: app.graph?.completed_log_limit ?? 0,
  }
}

/** Sets the task state of a block (marker click, Ctrl+Enter, kanban drop). Keeps the editor open and remounts it. */
export async function setTaskState(
  ctx: Ctx,
  blockId: number,
  state: import('../api').TaskState | null,
  editorText?: string,
): Promise<void> {
  await ctx.doc.loaded
  const view = ctx.doc.view
  const hit = view ? findById(view.blocks, blockId) : undefined
  if (!view || !hit) return
  const ops: BlockOp[] = []
  if (editorText !== undefined && editorText !== hit.block.text)
    ops.push({ op: 'update_text', block: { path: hit.path, hash: null }, text: editorText })
  ops.push({
    op: 'set_task_state',
    block: { path: hit.path, hash: null },
    state,
    done_at: doneAt(app.today),
    completed_limit: app.graph?.completed_log_limit ?? 0,
  })
  const wasEditing = editing.mode === 'edit' && editing.blockId === blockId
  await ctx.doc.apply(ops)
  if (wasEditing) {
    editing.nonce++
    startEditing(ctx.viewKey, ctx.doc.name, blockId, 'end')
  }
}

/** Like `setProperty`, for a block known by its runtime id: the path is looked up when the request reaches the
 * front of the queue, so an edit made right before cannot make it hit another block. */
export async function setPropertyOf(
  doc: PageDoc,
  blockId: number,
  key: string,
  value: string | null,
): Promise<void> {
  await doc.applyWith((view) => {
    const hit = view ? findById(view.blocks, blockId) : undefined
    if (!hit) return null
    const block = { path: hit.path, hash: null }
    return [
      value === null ? { op: 'remove_property', block, key } : { op: 'set_property', block, key, value },
    ]
  })
}

/** Sets (or, with `null`, removes) a property of a block. */
export async function setProperty(
  doc: PageDoc,
  blockPath: number[],
  key: string,
  value: string | null,
): Promise<void> {
  const op: BlockOp =
    value === null
      ? { op: 'remove_property', block: { path: blockPath, hash: null }, key }
      : { op: 'set_property', block: { path: blockPath, hash: null }, key, value }
  await doc.apply([op])
}
