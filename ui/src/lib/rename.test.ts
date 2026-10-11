import { describe, expect, it } from 'vitest'
import { prefixRename, underPrefix } from './rename'

describe('prefixRename', () => {
  it('detects one changed segment before the last', () => {
    expect(prefixRename('P/CapGemini/sth', 'P/Capgemini12/sth')).toEqual({
      from: 'P/CapGemini',
      to: 'P/Capgemini12',
    })
    expect(prefixRename('A/B/C/d', 'A/X/C/d')).toEqual({ from: 'A/B', to: 'A/X' })
    expect(prefixRename('CapGemini/sth', ' Capgemini12 /sth')).toEqual({
      from: 'CapGemini',
      to: 'Capgemini12',
    })
  })
  it('leaves everything else an ordinary move', () => {
    expect(prefixRename('P/CapGemini/sth', 'P/CapGemini/other')).toBeNull()
    expect(prefixRename('P/CapGemini/sth', 'P/Capgemini12/other')).toBeNull()
    expect(prefixRename('A/B/c', 'X/Y/c')).toBeNull()
    expect(prefixRename('A/B/c', 'A/B/c/d')).toBeNull()
    expect(prefixRename('plain', 'other')).toBeNull()
    expect(prefixRename('A/B/c', 'A//c')).toBeNull()
  })
})

describe('underPrefix', () => {
  const p = { from: 'P/CapGemini', to: 'P/Capgemini12' }
  it('maps the namespace and what is below it', () => {
    expect(underPrefix('P/CapGemini', p)).toBe('P/Capgemini12')
    expect(underPrefix('p/capgemini/x/y', p)).toBe('P/Capgemini12/x/y')
    expect(underPrefix('P/CapGeminiX/y', p)).toBe('P/CapGeminiX/y')
  })
})
