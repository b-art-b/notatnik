import { describe, expect, it } from 'vitest'
import { pageHash, pageHashIn, parseHash, parseRoute, routeHash, setRouteSlot } from './route'

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
  it('knows the agenda and saved queries', () => {
    expect(parseRoute('#/agenda')).toEqual({ kind: 'agenda' })
    expect(parseRoute(routeHash({ kind: 'saved', name: 'Open work/2' }))).toEqual({
      kind: 'saved',
      name: 'Open work/2',
    })
  })
  it('defaults to the journals', () => {
    expect(parseRoute('')).toEqual({ kind: 'journals' })
    expect(parseRoute('#/unknown')).toEqual({ kind: 'journals' })
  })

  it('puts the graph of a link in the hash, and none for the first graph', () => {
    expect(pageHashIn('', 'a b')).toBe('#/page/a%20b')
    expect(pageHashIn('1', 'a b')).toBe('#/g/1/page/a%20b')
    expect(parseHash('#/g/1/page/a%20b')).toEqual({
      slot: '1',
      route: { kind: 'page', name: 'a b', zoom: [] },
    })
    expect(parseHash('#/page/x')).toEqual({ slot: '', route: { kind: 'page', name: 'x', zoom: [] } })
    expect(parseHash('#/g/1')).toEqual({ slot: '1', route: { kind: 'journals' } })
    expect(parseHash('#/g/1/agenda').route).toEqual({ kind: 'agenda' })
    expect(parseHash(routeHash({ kind: 'property', key: 'type', value: 'person' }, '1'))).toEqual({
      slot: '1',
      route: { kind: 'property', key: 'type', value: 'person' },
    })
  })
  it('writes the links of the active graph with its prefix', () => {
    setRouteSlot('1')
    expect(pageHash('x', [0])).toBe('#/g/1/page/x?zoom=0')
    expect(routeHash({ kind: 'settings' })).toBe('#/g/1/settings')
    setRouteSlot('')
    expect(pageHash('x')).toBe('#/page/x')
  })
})
