import { describe, expect, it } from 'vitest'
import type { BlockView } from './api'
import {
  deleteBlocks,
  dropMove,
  getBlock,
  indent,
  indentMany,
  insideFence,
  mergeWithNext,
  mergeWithPrevious,
  moveDown,
  moveUp,
  nextVisible,
  outdent,
  parsePasted,
  previousVisible,
  split,
  toMarkdown,
  visibleBlocks,
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

// - a
//   - a1
//   - a2
// - b
// - c
const tree = () => [b('a', [b('a1'), b('a2')]), b('b'), b('c')]

describe('navigation', () => {
  it('walks blocks in visible order and skips collapsed children', () => {
    const t = tree()
    expect(visibleBlocks(t).map((v) => v.block.text)).toEqual(['a', 'a1', 'a2', 'b', 'c'])
    t[0].collapsed = true
    expect(visibleBlocks(t).map((v) => v.block.text)).toEqual(['a', 'b', 'c'])
    expect(nextVisible(t, [0])?.block.text).toBe('b')
    expect(previousVisible(t, [1])?.block.text).toBe('a')
    expect(previousVisible(t, [0])).toBeUndefined()
    expect(nextVisible(t, [2])).toBeUndefined()
  })
  it('finds blocks by path', () => {
    expect(getBlock(tree(), [0, 1])?.text).toBe('a2')
    expect(getBlock(tree(), [5])).toBeUndefined()
  })
})

describe('indent / outdent', () => {
  it('indent at index 0 is a no-op', () => {
    expect(indent(tree(), [0])).toBeNull()
    expect(indent(tree(), [0, 0])).toBeNull()
  })
  it('indent makes the block the last child of the previous sibling', () => {
    const plan = indent(tree(), [1])!
    expect(plan.ops).toEqual([{ op: 'indent', block: { path: [1], hash: null } }])
    expect(plan.focus).toEqual({ path: [0, 2], cursor: 'end' })
  })
  it('outdent at the top level is a no-op', () => {
    expect(outdent(tree(), [1])).toBeNull()
  })
  it('outdent focuses the block after its former parent', () => {
    expect(outdent(tree(), [0, 1])!.focus).toEqual({ path: [1], cursor: 'end' })
  })
})

describe('move', () => {
  it('moves within siblings and is a no-op at the ends', () => {
    expect(moveUp(tree(), [0])).toBeNull()
    expect(moveDown(tree(), [2])).toBeNull()
    expect(moveDown(tree(), [0, 0])!.ops[0]).toEqual({
      op: 'move',
      block: { path: [0, 0], hash: null },
      new_parent: [0],
      index: 1,
    })
    expect(moveUp(tree(), [2])!.focus).toEqual({ path: [1], cursor: 'end' })
  })
})

describe('split (Enter)', () => {
  it('at the end creates an empty next sibling', () => {
    const plan = split(tree(), [1], 'b', 1)!
    expect(plan.ops).toEqual([{ op: 'insert', parent: [], index: 2, text: '' }])
    expect(plan.focus).toEqual({ path: [2], cursor: 'start' })
  })
  it('in the middle splits the text', () => {
    const t = [b('hello world')]
    const plan = split(t, [0], 'hello world', 5)!
    expect(plan.ops).toEqual([
      { op: 'update_text', block: { path: [0], hash: null }, text: 'hello' },
      { op: 'insert', parent: [], index: 1, text: 'world' },
    ])
  })
  it('on an expanded block with children the new block becomes the first child', () => {
    const plan = split(tree(), [0], 'a', 1)!
    expect(plan.ops).toEqual([{ op: 'insert', parent: [0], index: 0, text: '' }])
    expect(plan.focus!.path).toEqual([0, 0])
    const t = tree()
    t[0].collapsed = true
    expect(split(t, [0], 'a', 1)!.focus!.path).toEqual([1])
  })
  it('keeps property lines with the original block', () => {
    const t = [b('TODO write\ndeadline:: 2026-10-05')]
    const plan = split(t, [0], 'TODO write\ndeadline:: 2026-10-05', 4)!
    expect(plan.ops[0]).toEqual({
      op: 'update_text',
      block: { path: [0], hash: null },
      text: 'TODO\ndeadline:: 2026-10-05',
    })
    expect(plan.ops[1]).toMatchObject({ op: 'insert', text: 'write' })
  })
  it('does nothing outside the first line and inside code fences', () => {
    const text = 'title\n```js\nlet a\n```'
    expect(split([b(text)], [0], text, 8)).toBeNull()
    expect(split([b('```query\ntask: todo\n```')], [0], '```query\ntask: todo\n```', 8)).toBeNull()
  })
})

describe('insideFence', () => {
  const text = 'x\n```\ncode\n```\ny'
  it('knows where the fence is', () => {
    expect(insideFence(text, 0)).toBe(false)
    expect(insideFence(text, 7)).toBe(true) // on "```" opener line? opener line itself is outside
  })
})

describe('merge', () => {
  it('Backspace at the start joins into the previous visible block', () => {
    const plan = mergeWithPrevious(tree(), [1], 'b')!
    expect(plan.ops).toEqual([
      { op: 'update_text', block: { path: [0, 1], hash: null }, text: 'a2b' },
      { op: 'delete', block: { path: [1], hash: null } },
    ])
    expect(plan.focus).toEqual({ path: [0, 1], cursor: 2 })
  })
  it('an empty block is simply deleted', () => {
    const plan = mergeWithPrevious([b('x'), b('')], [1], '')!
    expect(plan.ops).toEqual([{ op: 'delete', block: { path: [1], hash: null } }])
  })
  it('refuses to merge a block that has children, or the first block', () => {
    expect(mergeWithPrevious(tree(), [0], 'a')).toBeNull()
    const t = [b('x'), b('y', [b('z')])]
    expect(mergeWithPrevious(t, [1], 'y')).toBeNull()
  })
  it('Delete at the end pulls the next block in', () => {
    const plan = mergeWithNext(tree(), [1], 'b')!
    expect(plan.ops[0]).toEqual({ op: 'update_text', block: { path: [1], hash: null }, text: 'bc' })
    expect(plan.ops[1]).toEqual({ op: 'delete', block: { path: [2], hash: null } })
    expect(mergeWithNext(tree(), [2], 'c')).toBeNull()
  })
})

describe('multi-block operations', () => {
  it('delete removes from the last so paths stay valid', () => {
    const plan = deleteBlocks(tree(), [[1], [2]])!
    expect(plan.ops.map((o) => (o as { block: { path: number[] } }).block.path)).toEqual([[2], [1]])
    expect(plan.focus).toEqual({ path: [0, 1], cursor: 'end' })
  })
  it('indent of three selected siblings keeps their order', () => {
    const t = [b('p'), b('x'), b('y'), b('z')]
    const plan = indentMany(t, [[1], [2], [3]])!
    expect(plan.ops.map((o) => (o as { block: { path: number[] } }).block.path)).toEqual([[1], [1], [1]])
    expect(indentMany(t, [[0], [1]])).toBeNull()
  })
})

describe('markdown', () => {
  it('serialises blocks with children, ids and continuation lines', () => {
    const t = [b('a ', [b('child')], { uid: 'abc123' }), b('x\ny')]
    expect(toMarkdown(t)).toBe('- a  ^abc123\n  - child\n- x\n  y')
  })
  it('parses pasted outlines', () => {
    const p = parsePasted('- a\n  - b\n- c')
    expect(p.map((x) => x.text)).toEqual(['a', 'c'])
    expect(p[0].children.map((x) => x.text)).toEqual(['b'])
    expect(parsePasted('line one\nline two').map((x) => x.text)).toEqual(['line one', 'line two'])
    expect(parsePasted('1. one\n2. two').map((x) => x.text)).toEqual(['one', 'two'])
    expect(parsePasted('- a\n\tnested with tab\n').length).toBeGreaterThan(0)
  })
})

describe('dropMove (drag and drop)', () => {
  const mv = (op: ReturnType<typeof dropMove>) =>
    op && op.op === 'move' ? { parent: op.new_parent, index: op.index } : null
  it('moves among siblings, counting the moved block out', () => {
    expect(mv(dropMove([0], [2], 'before'))).toEqual({ parent: [], index: 1 })
    expect(mv(dropMove([0], [2], 'after'))).toEqual({ parent: [], index: 2 })
    expect(mv(dropMove([2], [0], 'before'))).toEqual({ parent: [], index: 0 })
    expect(mv(dropMove([2], [0], 'after'))).toEqual({ parent: [], index: 1 })
  })
  it('does nothing when the block would stay where it is', () => {
    expect(dropMove([1], [1], 'before')).toBeNull()
    expect(dropMove([1], [2], 'before')).toBeNull() // already right before [2]
    expect(dropMove([1], [0], 'after')).toBeNull() // already right after [0]
  })
  it('makes a block the first child of the target', () => {
    expect(mv(dropMove([3], [1], 'child'))).toEqual({ parent: [1], index: 0 })
    // from a sibling list into a nested one, paths are those of the current page
    expect(mv(dropMove([0], [1, 0], 'before'))).toEqual({ parent: [1], index: 0 })
    expect(mv(dropMove([0, 2], [0], 'after'))).toEqual({ parent: [], index: 1 })
  })
  it('refuses a block onto itself or into its own subtree', () => {
    expect(dropMove([0], [0], 'child')).toBeNull()
    expect(dropMove([0], [0, 1], 'before')).toBeNull()
    expect(dropMove([0], [0, 1, 2], 'child')).toBeNull()
  })
  it('moves a nested block up and out', () => {
    expect(mv(dropMove([1, 2], [0], 'before'))).toEqual({ parent: [], index: 0 })
    expect(mv(dropMove([1, 2], [1, 0], 'before'))).toEqual({ parent: [1], index: 0 })
  })
})
