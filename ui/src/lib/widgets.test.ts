import { describe, expect, it } from 'vitest'
import { linkRoute } from './widgets'

describe('linkRoute', () => {
  it('opens the capture screen', () => {
    expect(linkRoute('notatnik://capture')).toEqual({ kind: 'capture' })
  })
  it('opens the agenda', () => {
    expect(linkRoute('notatnik://agenda')).toEqual({ kind: 'agenda' })
  })
  it('opens a page, decoding its name', () => {
    expect(linkRoute('notatnik://page/Projects%2FHome')).toEqual({
      kind: 'page',
      name: 'Projects/Home',
      zoom: [],
    })
    expect(linkRoute('notatnik://page/2026-11-08')).toEqual({ kind: 'page', name: '2026-11-08', zoom: [] })
  })
  it('ignores anything else', () => {
    for (const l of [
      '',
      'https://example.com',
      'notatnik://settings',
      'notatnik://agenda/x',
      'notatnik://page/',
      'notatnik://page/%',
      'notatnik://page/a%00b',
      'notatnik://capture/x',
      `notatnik://page/${'a'.repeat(400)}`,
    ])
      expect(linkRoute(l)).toBeNull()
  })
})
