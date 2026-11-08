import { describe, expect, it } from 'vitest'
import { layoutGraph, shortName } from './graphLayout'

const node = (name: string, depth: number) => ({ name, depth })

describe('layoutGraph', () => {
  it('puts the page in the middle and the neighbours on a ring', () => {
    const l = layoutGraph([node('Hub', 0), node('A', 1), node('B', 1), node('C', 1)], [])
    const hub = l.nodes.find((n) => n.name === 'Hub')!
    expect(hub.x).toBeCloseTo(l.width / 2, 0)
    expect(hub.y).toBeCloseTo(l.height / 2, 0)
    const radii = l.nodes.filter((n) => n.depth === 1).map((n) => Math.hypot(n.x - hub.x, n.y - hub.y))
    expect(Math.max(...radii) - Math.min(...radii)).toBeLessThan(0.5)
  })

  it('looks the same twice, whatever the order the pages come in', () => {
    const a = layoutGraph(
      [node('Hub', 0), node('B', 1), node('A', 1), node('C', 2)],
      [{ from: 'A', to: 'C' }],
    )
    const b = layoutGraph(
      [node('C', 2), node('A', 1), node('Hub', 0), node('B', 1)],
      [{ from: 'A', to: 'C' }],
    )
    const key = (l: typeof a) => l.nodes.map((n) => `${n.name}:${n.x},${n.y}`).sort()
    expect(key(a)).toEqual(key(b))
  })

  it('keeps nodes of one ring apart, also with a full ring of 59 pages', () => {
    const ring1 = Array.from({ length: 30 }, (_, i) => node(`P${String(i).padStart(2, '0')}`, 1))
    const ring2 = Array.from({ length: 29 }, (_, i) => node(`Q${String(i).padStart(2, '0')}`, 2))
    const edges = ring2.map((q, i) => ({ from: ring1[i % 5].name, to: q.name }))
    const l = layoutGraph([node('Hub', 0), ...ring1, ...ring2], edges)
    for (const depth of [1, 2]) {
      const ring = l.nodes.filter((n) => n.depth === depth)
      for (let i = 0; i < ring.length; i++)
        for (let j = i + 1; j < ring.length; j++)
          expect(Math.hypot(ring[i].x - ring[j].x, ring[i].y - ring[j].y)).toBeGreaterThan(12)
    }
    for (const n of l.nodes) {
      expect(n.x).toBeGreaterThan(0)
      expect(n.x).toBeLessThan(l.width)
      expect(n.y).toBeGreaterThan(0)
      expect(n.y).toBeLessThan(l.height)
    }
  })

  it('puts a page two steps away beside the page that brought it in', () => {
    const l = layoutGraph(
      [node('Hub', 0), node('A', 1), node('B', 1), node('far', 2)],
      [{ from: 'B', to: 'far' }],
    )
    const get = (n: string) => l.nodes.find((x) => x.name === n)!
    const d = (p: string, q: string) => Math.hypot(get(p).x - get(q).x, get(p).y - get(q).y)
    expect(d('far', 'B')).toBeLessThan(d('far', 'A'))
  })

  it('handles a page with no links', () => {
    const l = layoutGraph([node('Alone', 0)], [])
    expect(l.nodes).toHaveLength(1)
  })
})

describe('shortName', () => {
  it('keeps short names and cuts long ones', () => {
    expect(shortName('Short')).toBe('Short')
    expect(shortName('A very long page name indeed')).toHaveLength(16)
    expect(shortName('A very long page name indeed').endsWith('…')).toBe(true)
  })
  it('keeps the last part of a namespace', () => {
    expect(shortName('projects/clients/acme/Contract')).toBe('…/Contract')
  })
})
