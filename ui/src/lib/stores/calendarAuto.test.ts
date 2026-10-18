import { describe, expect, it } from 'vitest'
import { EVERY_MS, pauseAfter } from './calendarAuto.svelte'

describe('pauseAfter', () => {
  it('starts at the normal interval, doubles, and stops at 30 minutes', () => {
    expect(pauseAfter(1)).toBe(EVERY_MS)
    expect(pauseAfter(2)).toBe(2 * EVERY_MS)
    expect(pauseAfter(3)).toBe(4 * EVERY_MS)
    expect(pauseAfter(4)).toBe(30 * 60_000)
    expect(pauseAfter(40)).toBe(30 * 60_000)
  })
})
