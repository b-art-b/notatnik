import { describe, expect, it } from 'vitest'
import { changed, diffLines } from './diff'

describe('diffLines', () => {
  it('marks nothing for equal texts', () => {
    const d = diffLines('- a\n- b\n', '- a\n- b\n')
    expect(changed(d)).toBe(false)
    expect(d.map((l) => l.text)).toEqual(['- a', '- b', ''])
  })
  it('finds added, removed and changed lines', () => {
    const d = diffLines('- a\n- b\n- c\n', '- a\n- B\n- c\n- d\n')
    expect(d.filter((l) => l.kind === 'del').map((l) => l.text)).toEqual(['- b'])
    expect(d.filter((l) => l.kind === 'add').map((l) => l.text)).toEqual(['- B', '- d'])
    expect(changed(d)).toBe(true)
  })
  it('handles empty texts', () => {
    expect(diffLines('', '- a').map((l) => l.kind)).toEqual(['add'])
    expect(diffLines('- a', '').map((l) => l.kind)).toEqual(['del'])
    expect(diffLines('', '')).toEqual([])
  })
  it('keeps the order of the lines', () => {
    const d = diffLines('1\n2\n3\n4', '1\n3\n2\n4')
    const rebuilt = d.filter((l) => l.kind !== 'del').map((l) => l.text)
    expect(rebuilt).toEqual(['1', '3', '2', '4'])
    const old = d.filter((l) => l.kind !== 'add').map((l) => l.text)
    expect(old).toEqual(['1', '2', '3', '4'])
  })
})
