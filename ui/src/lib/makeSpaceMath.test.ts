import { describe, expect, it } from 'vitest'
import { belowLine, clampShift, pickAxis } from './makeSpaceMath'

describe('belowLine (DR-12)', () => {
  const tops = { a: 10, b: 100, c: 100.5, d: 300 }
  const pick = (line: number) => belowLine(Object.keys(tops), (k) => tops[k as keyof typeof tops], line)
  it('takes the items whose top is at or below the line', () => {
    expect(pick(100)).toEqual(['b', 'c', 'd'])
    expect(pick(100.2)).toEqual(['c', 'd'])
  })
  it('takes nothing when the line is under everything, and everything when above', () => {
    expect(pick(1000)).toEqual([])
    expect(pick(-5)).toEqual(['a', 'b', 'c', 'd'])
  })
})

describe('clampShift (DR-12)', () => {
  it('lets the lower part go down by any distance', () => {
    expect(clampShift(250, [120, 300], 100)).toBe(250)
  })
  it('lets it go up only until its highest item reaches the line', () => {
    expect(clampShift(-10, [120, 300], 100)).toBe(-10)
    expect(clampShift(-80, [120, 300], 100)).toBe(-20)
  })
  it('moves nothing when nothing is below the line', () => {
    expect(clampShift(-80, [], 100)).toBe(0)
    expect(clampShift(80, [], 100)).toBe(0)
  })
})

describe('pickAxis (DR-12)', () => {
  it('waits until the drag is long enough to tell', () => {
    expect(pickAxis(2, 3, 6)).toBeNull()
    expect(pickAxis(0, 0, 6)).toBeNull()
  })
  it('a mostly vertical drag makes a horizontal line (y), up or down', () => {
    expect(pickAxis(1, 10, 6)).toBe('y')
    expect(pickAxis(-3, -10, 6)).toBe('y')
  })
  it('a mostly sideways drag makes a vertical line (x), left or right', () => {
    expect(pickAxis(10, 1, 6)).toBe('x')
    expect(pickAxis(-10, 4, 6)).toBe('x')
  })
  it('a diagonal goes to y, so the old way of dragging stays what it was', () => {
    expect(pickAxis(8, 8, 6)).toBe('y')
  })
})
