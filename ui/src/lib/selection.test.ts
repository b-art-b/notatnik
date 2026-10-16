import { describe, expect, it } from 'vitest'
import type { BlockView } from './api'
import {
  afterRoots,
  areSiblings,
  comparePaths,
  deleteBlocks,
  insertOps,
  moveMany,
  rangeIds,
  selectionRoots,
  subtreeIds,
  toMarkdown,
  toPasted,
} from './tree'

let nextId = 1
function b(text: string, children: BlockView[] = [], extra: Partial<BlockView> = {}): BlockView {
  return {
    id: nextId++,
    path: [],
    hash: 'h' + text,
    uid: null,
    text,
    content: [],
    task: null,
    props: [],
    logbook: [],
    collapsed: false,
    children,
    ...extra,
  }
}
const id = (blocks: BlockView[], path: number[]) => {
  let list = blocks
  let cur: BlockView = blocks[0]
  for (const i of path) {
    cur = list[i]
    list = cur.children
  }
  return cur.id
}
// - a
//   - a1
//   - a2
// - b
// - c
//   - c1
const tree = () => [b('a', [b('a1'), b('a2')]), b('b'), b('c', [b('c1')])]

describe('selecting blocks', () => {
  it('a range follows the order on screen, children included, either direction', () => {
    const t = tree()
    const ids = rangeIds(t, id(t, [0, 1]), id(t, [2]))
    expect(ids).toEqual([id(t, [0, 1]), id(t, [1]), id(t, [2])])
    expect(rangeIds(t, id(t, [2]), id(t, [0, 1]))).toEqual(ids)
    // collapsed children are not on screen, so not in the range
    t[0].collapsed = true
    expect(rangeIds(t, id(t, [0]), id(t, [1]))).toEqual([id(t, [0]), id(t, [1])])
    expect(rangeIds(t, id(t, [0, 0]), id(t, [1]))).toEqual([]) // a hidden block cannot be an end
  })

  it('the roots of a selection leave out what is below another selected block, in document order', () => {
    const t = tree()
    const sel = [id(t, [2, 0]), id(t, [0, 1]), id(t, [0]), id(t, [2]), id(t, [1])]
    expect(selectionRoots(t, sel).map((r) => r.path)).toEqual([[0], [1], [2]])
    expect(selectionRoots(t, [id(t, [0, 1]), id(t, [2, 0])]).map((r) => r.path)).toEqual([
      [0, 1],
      [2, 0],
    ])
    expect(selectionRoots(t, [9999])).toEqual([])
  })

  it('Ctrl+click takes a block with everything below it', () => {
    const t = tree()
    expect(subtreeIds(t[0])).toEqual([t[0].id, t[0].children[0].id, t[0].children[1].id])
  })

  it('knows siblings and document order', () => {
    expect(
      areSiblings([
        [0, 1],
        [0, 3],
      ]),
    ).toBe(true)
    expect(areSiblings([[0, 1], [1]])).toBe(false)
    expect(areSiblings([])).toBe(false)
    expect([[1], [0, 2], [0], [0, 1]].sort(comparePaths)).toEqual([[0], [0, 1], [0, 2], [1]])
  })
})

describe('commands on a selection', () => {
  it('delete takes the last block first so earlier paths stay valid, at any level', () => {
    const t = tree()
    const plan = deleteBlocks(t, [[0, 1], [1], [2, 0]])!
    expect(plan.ops.map((o) => (o as { block: { path: number[] } }).block.path)).toEqual([
      [2, 0],
      [1],
      [0, 1],
    ])
    // the cursor goes to the visible block before the first deleted one
    expect(plan.focus?.path).toEqual([0, 0])
    expect(deleteBlocks(t, [])).toBeNull()
  })

  it('moves siblings together, up and down, and stops at the edge', () => {
    const t = [b('a'), b('b'), b('c'), b('d'), b('e')]
    const up = moveMany(t, [[2], [3]], 'up')!
    expect(up.ops).toEqual([
      { op: 'move', block: { path: [2], hash: null }, new_parent: [], index: 1 },
      { op: 'move', block: { path: [3], hash: null }, new_parent: [], index: 2 },
    ])
    const down = moveMany(t, [[2], [3]], 'down')!
    expect(down.ops).toEqual([
      { op: 'move', block: { path: [3], hash: null }, new_parent: [], index: 4 },
      { op: 'move', block: { path: [2], hash: null }, new_parent: [], index: 3 },
    ])
    expect(moveMany(t, [[0], [1]], 'up')).toBeNull()
    expect(moveMany(t, [[3], [4]], 'down')).toBeNull()
    expect(moveMany(t, [[0], [1, 0]], 'up')).toBeNull() // not siblings
  })

  it('copies without IDs, cuts with them', () => {
    const t = [b('a', [b('a1', [], { uid: 'child1' })], { uid: 'abc123' })]
    expect(toMarkdown(t)).toBe('- a ^abc123\n  - a1 ^child1')
    expect(toMarkdown(t, '  ', 0, false)).toBe('- a\n  - a1')
  })

  it('duplicates and pastes below the last selected block, at its level', () => {
    const t = tree()
    expect(afterRoots([[0, 1], [2]])).toEqual({ parent: [], index: 3 })
    expect(
      afterRoots([
        [0, 0],
        [0, 1],
      ]),
    ).toEqual({ parent: [0], index: 2 })
    expect(afterRoots([])).toBeNull()
    const ops = insertOps(toPasted([t[0]]), [], 1)
    expect(ops).toEqual([
      { op: 'insert', parent: [], index: 1, text: 'a' },
      { op: 'insert', parent: [1], index: 0, text: 'a1' },
      { op: 'insert', parent: [1], index: 1, text: 'a2' },
    ])
  })
})
