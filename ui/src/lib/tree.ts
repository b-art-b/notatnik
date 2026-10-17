// Pure logic for the outline tree: paths, visible-order navigation and the block commands.
// Every command returns the BlockOps to send plus where the cursor should land afterwards,
// or null when the command is a no-op (e.g. "indent the first sibling").
import type { BlockOp, BlockView } from './api'

export type Path = number[]
/** Where the editor cursor goes: a character offset, or the start/end of the block text. */
export type Cursor = number | 'start' | 'end'
export interface Focus {
  path: Path
  cursor: Cursor
}
export interface Plan {
  ops: BlockOp[]
  focus?: Focus
}

export const samePath = (a: Path, b: Path) => a.length === b.length && a.every((x, i) => x === b[i])
export const isAncestor = (a: Path, b: Path) => a.length < b.length && a.every((x, i) => x === b[i])
export const parentOf = (p: Path): Path => p.slice(0, -1)
const ref = (path: Path) => ({ path, hash: null })

export function getBlock(blocks: BlockView[], path: Path): BlockView | undefined {
  let list = blocks
  let b: BlockView | undefined
  for (const i of path) {
    b = list[i]
    if (!b) return undefined
    list = b.children
  }
  return b
}

function siblingsOf(blocks: BlockView[], path: Path): BlockView[] {
  return path.length <= 1 ? blocks : (getBlock(blocks, parentOf(path))?.children ?? [])
}

export interface VisibleBlock {
  path: Path
  block: BlockView
}

/** Blocks in the order they appear on screen (children of collapsed blocks are skipped). */
export function visibleBlocks(blocks: BlockView[], base: Path = []): VisibleBlock[] {
  const out: VisibleBlock[] = []
  const walk = (list: BlockView[], prefix: Path) => {
    list.forEach((b, i) => {
      const path = [...prefix, i]
      out.push({ path, block: b })
      if (!b.collapsed) walk(b.children, path)
    })
  }
  walk(blocks, base)
  return out
}

export function previousVisible(blocks: BlockView[], path: Path): VisibleBlock | undefined {
  const all = visibleBlocks(blocks)
  const i = all.findIndex((v) => samePath(v.path, path))
  return i > 0 ? all[i - 1] : undefined
}

export function nextVisible(blocks: BlockView[], path: Path): VisibleBlock | undefined {
  const all = visibleBlocks(blocks)
  const i = all.findIndex((v) => samePath(v.path, path))
  return i >= 0 ? all[i + 1] : undefined
}

/** Make the block the last child of its previous sibling. */
export function indent(blocks: BlockView[], path: Path): Plan | null {
  const idx = path[path.length - 1]
  if (path.length === 0 || idx === 0) return null
  const prevPath = [...parentOf(path), idx - 1]
  const prev = getBlock(blocks, prevPath)
  if (!prev) return null
  return {
    ops: [{ op: 'indent', block: ref(path) }],
    focus: { path: [...prevPath, prev.children.length], cursor: 'end' },
  }
}

/** Move the block up one level, after its parent. */
export function outdent(blocks: BlockView[], path: Path): Plan | null {
  if (path.length < 2) return null
  // Note: later siblings become children of the outdented block (semantics of the Outdent op).
  const parent = parentOf(path)
  return {
    ops: [{ op: 'outdent', block: ref(path) }],
    focus: { path: [...parentOf(parent), parent[parent.length - 1] + 1], cursor: 'end' },
  }
}

export function moveUp(blocks: BlockView[], path: Path): Plan | null {
  const idx = path[path.length - 1]
  if (path.length === 0 || idx === 0) return null
  return {
    ops: [{ op: 'move', block: ref(path), new_parent: parentOf(path), index: idx - 1 }],
    focus: { path: [...parentOf(path), idx - 1], cursor: 'end' },
  }
}

export function moveDown(blocks: BlockView[], path: Path): Plan | null {
  const sibs = siblingsOf(blocks, path)
  const idx = path[path.length - 1]
  if (path.length === 0 || idx >= sibs.length - 1) return null
  return {
    ops: [{ op: 'move', block: ref(path), new_parent: parentOf(path), index: idx + 1 }],
    focus: { path: [...parentOf(path), idx + 1], cursor: 'end' },
  }
}

/** Where a dragged block lands relative to the block it is dropped on. */
export type DropZone = 'before' | 'after' | 'child'

/**
 * The move for dropping the block at `from` (with all its children) onto the block at `to`:
 * `before`/`after` make it a sibling, `child` makes it the first child. Paths are those of the page as it is now
 * (the backend resolves the destination before it removes the block). Null when nothing would change, or when the
 * target is the block itself or inside it.
 */
export function dropMove(from: Path, to: Path, zone: DropZone): BlockOp | null {
  if (from.length === 0 || to.length === 0) return null
  if (samePath(from, to) || isAncestor(from, to)) return null
  if (zone === 'child') {
    return { op: 'move', block: ref(from), new_parent: to, index: 0 }
  }
  const parent = parentOf(to)
  let index = to[to.length - 1]
  // the block being moved is skipped when counting its new siblings
  if (samePath(parentOf(from), parent) && from[from.length - 1] < index) index--
  if (zone === 'after') index++
  if (samePath(parentOf(from), parent) && index === from[from.length - 1]) return null
  return { op: 'move', block: ref(from), new_parent: parent, index }
}

const FENCE = /^\s*(```|~~~)/

/** True when the cursor's line is inside a fenced code block of `text` (the closing fence line counts as outside). */
export function insideFence(text: string, cursor: number): boolean {
  const lines = text.split('\n')
  const cursorLine = text.slice(0, cursor).split('\n').length - 1
  let open = false
  for (let i = 0; i < cursorLine; i++) if (FENCE.test(lines[i])) open = !open
  if (open && FENCE.test(lines[cursorLine] ?? '')) return false
  return open
}

/** Enter: split the block at the cursor (only on the first line, and not inside a code fence). */
export function split(blocks: BlockView[], path: Path, text: string, cursor: number): Plan | null {
  const firstEnd = text.indexOf('\n') === -1 ? text.length : text.indexOf('\n')
  if (cursor > firstEnd) return null
  if (FENCE.test(text.slice(0, firstEnd))) return null // a fence opener on the first line: Enter is a newline
  if (insideFence(text, cursor)) return null
  const block = getBlock(blocks, path)
  if (!block) return null
  const head = text.slice(0, cursor).replace(/[ \t]+$/, '')
  const rest = text.slice(cursor).replace(/^[ \t]+/, '')
  const firstLineRest = rest.slice(0, rest.indexOf('\n') === -1 ? rest.length : rest.indexOf('\n'))
  const tail = rest.slice(firstLineRest.length) // further lines (properties, continuation) stay with the original
  const keep = head + tail
  const expanded = block.children.length > 0 && !block.collapsed
  const ops: BlockOp[] = []
  if (keep !== block.text) ops.push({ op: 'update_text', block: ref(path), text: keep })
  const newPath = expanded ? [...path, 0] : [...parentOf(path), path[path.length - 1] + 1]
  ops.push({
    op: 'insert',
    parent: expanded ? path : parentOf(path),
    index: expanded ? 0 : path[path.length - 1] + 1,
    text: firstLineRest,
  })
  return { ops, focus: { path: newPath, cursor: 'start' } }
}

/** Backspace at the very start: merge into the previous visible block, or delete an empty block. */
export function mergeWithPrevious(blocks: BlockView[], path: Path, text: string): Plan | null {
  const block = getBlock(blocks, path)
  const prev = previousVisible(blocks, path)
  if (!block || !prev) return null
  if (block.children.length > 0) return null
  const prevText = prev.block.text
  const firstLineEnd = prevText.indexOf('\n') === -1 ? prevText.length : prevText.indexOf('\n')
  const joined =
    text === '' ? prevText : prevText.slice(0, firstLineEnd) + text + prevText.slice(firstLineEnd)
  const ops: BlockOp[] = []
  if (joined !== prevText) ops.push({ op: 'update_text', block: ref(prev.path), text: joined })
  ops.push({ op: 'delete', block: ref(path) })
  return { ops, focus: { path: prev.path, cursor: firstLineEnd } }
}

/** Delete at the very end: pull the next visible block's text into this one. */
export function mergeWithNext(blocks: BlockView[], path: Path, text: string): Plan | null {
  const next = nextVisible(blocks, path)
  const block = getBlock(blocks, path)
  if (!next || !block) return null
  if (next.block.children.length > 0) return null
  const firstLineEnd = text.indexOf('\n') === -1 ? text.length : text.indexOf('\n')
  const joined = text.slice(0, firstLineEnd) + next.block.text + text.slice(firstLineEnd)
  return {
    ops: [
      { op: 'update_text', block: ref(path), text: joined },
      { op: 'delete', block: ref(next.path) },
    ],
    focus: { path, cursor: firstLineEnd },
  }
}

/** Document order of two paths: a block before its children, an earlier sibling before a later one. */
export function comparePaths(a: Path, b: Path): number {
  for (let i = 0; i < Math.min(a.length, b.length); i++) if (a[i] !== b[i]) return a[i] - b[i]
  return a.length - b.length
}

/** The selected blocks that count: those with no selected ancestor (a selected block takes its children along), in
 * document order. `ids` are runtime ids; ids that are not in the page are ignored. */
export function selectionRoots(blocks: BlockView[], ids: number[]): VisibleBlock[] {
  const want = new Set(ids)
  const out: VisibleBlock[] = []
  const walk = (list: BlockView[], prefix: Path, inside: boolean) => {
    list.forEach((b, i) => {
      const path = [...prefix, i]
      const here = want.has(b.id)
      if (here && !inside) out.push({ path, block: b })
      walk(b.children, path, inside || here)
    })
  }
  walk(blocks, [], false)
  return out
}

/** The ids of the blocks from `a` to `b` in the order they appear on screen (either may come first): what Shift+click
 * and Shift+↑/↓ select. Empty when one of them is not visible. */
export function rangeIds(blocks: BlockView[], a: number, b: number): number[] {
  const all = visibleBlocks(blocks)
  const i = all.findIndex((v) => v.block.id === a)
  const j = all.findIndex((v) => v.block.id === b)
  if (i < 0 || j < 0) return []
  return all.slice(Math.min(i, j), Math.max(i, j) + 1).map((v) => v.block.id)
}

/** A block and everything below it (what Ctrl+click toggles). */
export function subtreeIds(block: BlockView): number[] {
  return [block.id, ...block.children.flatMap(subtreeIds)]
}

/** True when all of the paths have the same parent (Tab, Shift+Tab and Alt+↑/↓ work on those). */
export const areSiblings = (paths: Path[]) =>
  paths.length > 0 && paths.every((p) => samePath(parentOf(p), parentOf(paths[0])))

/** Deletes blocks (a selection, as roots: none below another); the cursor goes to the visible block before them. */
export function deleteBlocks(blocks: BlockView[], paths: Path[]): Plan | null {
  if (paths.length === 0) return null
  // the last one in the document first, so the paths of the others stay valid
  const sorted = [...paths].sort((a, b) => comparePaths(b, a))
  const first = [...paths].sort(comparePaths)[0]
  const prev = previousVisible(blocks, first)
  return {
    ops: sorted.map((p) => ({ op: 'delete' as const, block: ref(p) })),
    focus: prev ? { path: prev.path, cursor: 'end' } : undefined,
  }
}

/** Indent a selection of siblings under the previous sibling, keeping their order. */
export function indentMany(blocks: BlockView[], paths: Path[]): Plan | null {
  if (paths.length === 0) return null
  const sorted = [...paths].sort((a, b) => a[a.length - 1] - b[b.length - 1])
  const firstIdx = sorted[0][sorted[0].length - 1]
  if (firstIdx === 0) return null
  // after each indent the next selected sibling shifts down by one position
  const ops: BlockOp[] = sorted.map((p, k) => ({
    op: 'indent' as const,
    block: ref([...parentOf(p), p[p.length - 1] - k]),
  }))
  return { ops }
}

/** Outdent selected blocks, at any levels: each one moves up one level, the ones at the top level stay. They are
 * done from the last in the document to the first, so the paths of the others stay valid. */
export function outdentMany(blocks: BlockView[], paths: Path[]): Plan | null {
  const movable = paths.filter((p) => p.length >= 2).sort((a, b) => comparePaths(b, a))
  if (movable.length === 0) return null
  return { ops: movable.map((p) => ({ op: 'outdent' as const, block: ref(p) })) }
}

/** Markdown of blocks with their children (what copy puts on the clipboard). `withIds` keeps the `^id` of each
 * block: a cut keeps them (so references survive the move), a copy leaves them out (the pasted blocks get their
 * own). */
export function toMarkdown(blocks: BlockView[], indentUnit = '  ', depth = 0, withIds = true): string {
  const out: string[] = []
  for (const b of blocks) {
    const pad = indentUnit.repeat(depth)
    const lines = b.text.split('\n')
    const id = withIds && b.uid ? ` ^${b.uid}` : ''
    out.push(`${pad}- ${lines[0]}${id}`)
    for (const l of lines.slice(1)) out.push(l === '' ? '' : `${pad}  ${l}`)
    if (b.children.length) out.push(toMarkdown(b.children, indentUnit, depth + 1, withIds))
  }
  return out.join('\n')
}

/** The blocks as a pasted outline (their text and children, no IDs): what a duplicate inserts. */
export function toPasted(blocks: BlockView[]): PastedBlock[] {
  return blocks.map((b) => ({ text: b.text, children: toPasted(b.children) }))
}

/** The inserts for an outline: `list` goes into `parent` from `start` on, each block followed by its children. */
export function insertOps(list: PastedBlock[], parent: Path, start: number): BlockOp[] {
  const ops: BlockOp[] = []
  list.forEach((p, i) => {
    ops.push({ op: 'insert', parent, index: start + i, text: p.text })
    ops.push(...insertOps(p.children, [...parent, start + i], 0))
  })
  return ops
}

/** Where blocks pasted or duplicated "after the selection" go: right below the last selected block, at its level. */
export function afterRoots(roots: Path[]): { parent: Path; index: number } | null {
  if (roots.length === 0) return null
  const last = [...roots].sort(comparePaths)[roots.length - 1]
  return { parent: parentOf(last), index: last[last.length - 1] + 1 }
}

/** Alt+↑/↓ for a selection of siblings: each moves one place, together. Null when the first (up) or last (down)
 * is already at the edge, or the blocks are not siblings. */
export function moveMany(blocks: BlockView[], paths: Path[], dir: 'up' | 'down'): Plan | null {
  if (!areSiblings(paths)) return null
  const sorted = [...paths].sort(comparePaths)
  const order = dir === 'up' ? sorted : sorted.reverse()
  const ops: BlockOp[] = []
  for (const p of order) {
    const plan = (dir === 'up' ? moveUp : moveDown)(blocks, p)
    if (!plan) return null
    ops.push(...plan.ops)
  }
  return { ops }
}

/** Parses pasted Markdown/plain text into a nested outline of block texts. */
export interface PastedBlock {
  text: string
  children: PastedBlock[]
}

export function parsePasted(text: string): PastedBlock[] {
  const lines = text.replace(/\r\n/g, '\n').split('\n')
  const root: PastedBlock = { text: '', children: [] }
  const stack: { indent: number; node: PastedBlock }[] = [{ indent: -1, node: root }]
  let current: PastedBlock | null = null
  for (const raw of lines) {
    if (raw.trim() === '') continue
    const m = /^(\s*)([-*+]\s+|\d+\.\s+)?(.*)$/.exec(raw)
    if (!m) continue
    const indent = m[1].replace(/\t/g, '    ').length
    const bullet = m[2]
    if (!bullet && current && indent > 0 && stack.length > 1) {
      current.text += '\n' + raw.trim()
      continue
    }
    const node: PastedBlock = { text: m[3], children: [] }
    while (stack.length > 1 && stack[stack.length - 1].indent >= indent) stack.pop()
    stack[stack.length - 1].node.children.push(node)
    stack.push({ indent, node })
    current = node
  }
  return root.children
}

/** Finds a block by its runtime ID, with its path. */
export function findById(blocks: BlockView[], id: number): { path: Path; block: BlockView } | undefined {
  const walk = (list: BlockView[], prefix: Path): { path: Path; block: BlockView } | undefined => {
    for (let i = 0; i < list.length; i++) {
      const path = [...prefix, i]
      if (list[i].id === id) return { path, block: list[i] }
      const hit = walk(list[i].children, path)
      if (hit) return hit
    }
    return undefined
  }
  return walk(blocks, [])
}

/** Hashes of all collapsed blocks (what the backend stores per page). */
export function collapsedHashes(blocks: BlockView[]): string[] {
  const out: string[] = []
  const walk = (list: BlockView[]) => {
    for (const b of list) {
      if (b.collapsed) out.push(b.hash)
      walk(b.children)
    }
  }
  walk(blocks)
  return out
}
