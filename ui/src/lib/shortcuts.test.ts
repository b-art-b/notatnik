import { describe, expect, it } from 'vitest'
import { navForAlt, navForChord, navHint, navTitle } from './shortcuts'

const key = (code: string, extra: object = {}) => ({
  code,
  altKey: true,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  ...extra,
})

describe('navigation keys', () => {
  it('Alt+digit goes to the place, by the physical key', () => {
    expect(navForAlt(key('Digit1'))).toBe('journals')
    expect(navForAlt(key('Digit4'))).toBe('search')
    expect(navForAlt(key('Digit5'))).toBe('settings')
    expect(navForAlt(key('Digit9'))).toBeNull()
  })
  it('other modifiers or no Alt are not navigation', () => {
    expect(navForAlt(key('Digit1', { altKey: false }))).toBeNull()
    expect(navForAlt(key('Digit1', { ctrlKey: true }))).toBeNull()
    expect(navForAlt(key('Digit1', { shiftKey: true }))).toBeNull()
  })
  it('the chord letters', () => {
    expect(navForChord('j')).toBe('journals')
    expect(navForChord('A')).toBe('agenda')
    expect(navForChord('p')).toBe('pages')
    expect(navForChord('s')).toBe('settings')
    expect(navForChord('x')).toBeNull()
  })
  it('the hint and the tooltip say the same keys', () => {
    expect(navHint('agenda')).toBe('Alt+2')
    expect(navTitle('Agenda', 'agenda')).toBe('Agenda (Alt+2, or g then a)')
    expect(navTitle('Search', 'search')).toBe('Search (Alt+4)')
  })
})
