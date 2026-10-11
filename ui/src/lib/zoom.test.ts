import { describe, expect, it } from 'vitest'
import { stepZoom } from './zoom'

describe('stepZoom', () => {
  it('moves along the Firefox steps', () => {
    expect(stepZoom(1, 1)).toBe(1.1)
    expect(stepZoom(1, -1)).toBe(0.9)
    expect(stepZoom(1.33, 1)).toBe(1.5)
  })
  it('clamps at the ends', () => {
    expect(stepZoom(5, 1)).toBe(5)
    expect(stepZoom(0.3, -1)).toBe(0.3)
  })
  it('snaps an off-step value to the next step', () => {
    expect(stepZoom(1.05, 1)).toBe(1.1)
    expect(stepZoom(1.05, -1)).toBe(1)
  })
})
