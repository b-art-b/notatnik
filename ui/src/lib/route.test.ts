import { describe, expect, it } from 'vitest'
import { pageHash, parseRoute, routeHash } from './route'

describe('routes', () => {
  it('round-trips page names with slashes, spaces and diacritics', () => {
    for (const name of ['work/projects/alpha', 'a: b', '100%', 'zażółć gęślą', '2026-10-03']) {
      expect(parseRoute(pageHash(name))).toEqual({ kind: 'page', name, zoom: [] })
    }
  })
  it('carries the zoom path', () => {
    const r = parseRoute(pageHash('x', [0, 2, 1]))
    expect(r).toEqual({ kind: 'page', name: 'x', zoom: [0, 2, 1] })
    expect(routeHash(r)).toBe('#/page/x?zoom=0.2.1')
  })
  it('defaults to the journals', () => {
    expect(parseRoute('')).toEqual({ kind: 'journals' })
    expect(parseRoute('#/unknown')).toEqual({ kind: 'journals' })
  })
})
