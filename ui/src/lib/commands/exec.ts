// Runs block commands: turns a Plan from tree.ts into a request and moves the cursor afterwards.
import type { BlockOp } from '../api'
import { editing, startEditing, startSelecting, stopEditing } from '../stores/editing.svelte'
import { undoRedo, type PageDoc } from '../stores/pages.svelte'
import { navigate } from '../stores/app.svelte'
import {
  deleteBlocks, findById, getBlock, indent, indentMany, mergeWithNext, mergeWithPrevious, moveDown, moveUp, nextVisible, outdent,
  outdentMany, previousVisible, split, toMarkdown, visibleBlocks, type Path, type Plan,
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
  return { ...plan, ops: [{ op: 'update_text', block: { path: hit.path, hash: null }, text } as BlockOp, ...plan.ops] }
}

export async function runPlan(ctx: Ctx, plan: Plan): Promise<void> {
  const view = await ctx.doc.apply(plan.ops)
  if (view && plan.focus) {
    const b = getBlock(view.blocks, plan.focus.path)
    if (b) startEditing(ctx.viewKey, ctx.doc.name, b.id, plan.focus.cursor)
  }
}

/** Saves the editor text if it changed (blur, idle, leaving the block). */
export async function commitText(ctx: Ctx, blockId: number, text: string): Promise<void> {
  const view = ctx.doc.view
  if (!view) return
  const hit = findById(view.blocks, blockId) ?? (view.preamble?.id === blockId ? { path: [], block: view.preamble } : undefined)
  if (!hit || hit.block.text === text) return
  await ctx.doc.apply([{ op: 'update_text', block: { path: hit.path, hash: null }, text }])
}

function pathOf(ctx: Ctx, blockId: number): Path | undefined {
  const view = ctx.doc.view
  return view ? findById(view.blocks, blockId)?.path : undefined
}

export type EditorAction =
  | { kind: 'enter'; text: string; cursor: number }
  | { kind: 'indent' | 'outdent' | 'move-up' | 'move-down'; text: string }
  | { kind: 'merge-prev' | 'merge-next'; text: string }
  | { kind: 'nav-up' | 'nav-down'; text: string; column: number }
  | { kind: 'escape'; text: string }

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
        await commitText(ctx, blockId, a.text)
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
      await commitText(ctx, blockId, a.text)
      const fresh = ctx.doc.view
      const hit = fresh ? getBlock(fresh.blocks, target.path) : undefined
      if (hit) startEditing(ctx.viewKey, ctx.doc.name, hit.id, Math.min(a.column, hit.text.split('\n')[a.kind === 'nav-up' ? hit.text.split('\n').length - 1 : 0].length) )
      return true
    }
    case 'escape': {
      await commitText(ctx, blockId, a.text)
      startSelecting(ctx.viewKey, ctx.doc.name, [blockId])
      return true
    }
  }
}

// ---- selection commands (blocks selected, not edited) --------------------------------------

function selectedPaths(ctx: Ctx): Path[] {
  const view = ctx.doc.view
  if (!view) return []
  return editing.selection.map((id) => findById(view.blocks, id)?.path).filter((p): p is Path => !!p)
}

export function selectionMarkdown(ctx: Ctx): string {
  const view = ctx.doc.view
  if (!view) return ''
  const blocks = editing.selection.map((id) => findById(view.blocks, id)?.block).filter((b) => !!b)
  return toMarkdown(blocks)
}

async function runOnSelection(ctx: Ctx, f: (blocks: NonNullable<PageDoc['view']>['blocks'], paths: Path[]) => Plan | null) {
  const view = ctx.doc.view
  if (!view) return
  const paths = selectedPaths(ctx)
  const plan = f(view.blocks, paths)
  if (!plan) return
  const ids = [...editing.selection]
  const after = await ctx.doc.apply(plan.ops)
  if (after) {
    // keep the same blocks selected (runtime ids survive the edit)
    startSelecting(ctx.viewKey, ctx.doc.name, ids.filter((id) => findById(after.blocks, id)), editing.anchorId)
  }
}

export const selectionIndent = (ctx: Ctx) => runOnSelection(ctx, indentMany)
export const selectionOutdent = (ctx: Ctx) => runOnSelection(ctx, outdentMany)

export async function selectionDelete(ctx: Ctx) {
  const view = ctx.doc.view
  if (!view) return
  const plan = deleteBlocks(view.blocks, selectedPaths(ctx))
  if (!plan) return
  const after = await ctx.doc.apply(plan.ops)
  if (after) stopEditing()
}

export async function selectionMove(ctx: Ctx, dir: 'up' | 'down') {
  const view = ctx.doc.view
  if (!view || editing.selection.length !== 1) return
  const path = selectedPaths(ctx)[0]
  const plan = (dir === 'up' ? moveUp : moveDown)(view.blocks, path)
  if (plan) await runOnSelection(ctx, () => ({ ops: plan.ops }))
}

/** Shift+↑/↓: extend the selection among visible siblings; plain ↑/↓: move it. */
export function selectionStep(ctx: Ctx, dir: 'up' | 'down', extend: boolean) {
  const view = ctx.doc.view
  if (!view || editing.selection.length === 0) return
  const all = visibleBlocks(view.blocks)
  const cur = all.findIndex((v) => v.block.id === editing.blockId)
  if (cur < 0) return
  let j = cur + (dir === 'down' ? 1 : -1)
  const curPath = all[cur].path
  // stay among siblings: skip descendants of other blocks, stop at a different parent
  while (j >= 0 && j < all.length) {
    const p = all[j].path
    if (p.length === curPath.length && p.slice(0, -1).join('.') === curPath.slice(0, -1).join('.')) break
    if (!extend && p.length < curPath.length) break
    j += dir === 'down' ? 1 : -1
  }
  if (j < 0 || j >= all.length) return
  const target = all[j].block.id
  if (!extend) {
    startSelecting(ctx.viewKey, ctx.doc.name, [target])
    return
  }
  const anchor = all.findIndex((v) => v.block.id === editing.anchorId)
  const lo = Math.min(anchor, j)
  const hi = Math.max(anchor, j)
  const sibs = all.slice(lo, hi + 1).filter((v) => v.path.length === curPath.length && v.path.slice(0, -1).join('.') === curPath.slice(0, -1).join('.'))
  startSelecting(ctx.viewKey, ctx.doc.name, sibs.map((v) => v.block.id), editing.anchorId)
  editing.blockId = target
}

export function selectionEdit(ctx: Ctx) {
  if (editing.selection.length === 0) return
  startEditing(ctx.viewKey, ctx.doc.name, editing.blockId, 'end')
}

export async function globalUndo(kind: 'undo' | 'redo') {
  const name = await undoRedo(kind)
  if (name) {
    stopEditing()
    if (name !== (window.location.hash.startsWith('#/page/') ? decodeURIComponent(window.location.hash.slice(7).split('?')[0]) : '')) {
      // undo happened on another page: show it
      const onJournals = window.location.hash.startsWith('#/journals') || window.location.hash === ''
      if (!onJournals) navigate({ kind: 'page', name, zoom: [] })
    }
  }
}
