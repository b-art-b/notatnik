import { describe, expect, it } from 'vitest'
import { belowLine, clampShift } from './makeSpaceMath'

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
