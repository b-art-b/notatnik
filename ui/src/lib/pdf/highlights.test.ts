import { describe, expect, it } from 'vitest'
import type { BlockView, PageView } from '../api'
import {
  areaHighlightBlock,
  escapeQuote,
  highlightsOf,
  parsePdfHref,
  textHighlightBlock,
  unescapeQuote,
} from './highlights'

function block(id: number, text: string, uid: string | null = null): BlockView {
  const props = text
    .split('\n')
    .slice(1)
    .map((l) => /^([a-z-]+):: (.*)$/.exec(l))
    .filter((m): m is RegExpExecArray => !!m)
    .map((m) => ({ key: m[1], raw: m[2] }))
  return { id, uid, text, props, children: [] } as unknown as BlockView
}

describe('escapeQuote', () => {
  it('leaves nothing that the notes would read as a link, tag, reference or list', () => {
    const q = 'C# is [[not]] a #tag, see ((abc123)) and *this* 1. thing'
    const e = escapeQuote(q)
    expect(e).not.toMatch(/(^|[^\\])\[\[/)
    expect(e).not.toMatch(/(^|[^\\])#/)
    expect(e).not.toMatch(/(^|[^\\])\(\(/)
    expect(unescapeQuote(e)).toBe(q)
  })
  it('protects the start of a line', () => {
    expect(escapeQuote('- not a list')).toBe('\\- not a list')
    expect(escapeQuote('12. not a list')).toBe('12\\. not a list')
    expect(unescapeQuote(escapeQuote('12. not a list'))).toBe('12. not a list')
  })
})

describe('the block text', () => {
  it('writes a text highlight and reads it back', () => {
    const text = textHighlightBlock({
      page: 12,
      color: 'green',
      from: 10,
      to: 40,
      quote: 'A quote with [[brackets]]',
      context: 'before | after',
      uid: 'a1b2c3',
    })
    expect(text.split('\n')[0]).toBe('A quote with \\[\\[brackets\\]\\] ^a1b2c3')
    const view = { blocks: [block(1, text.replace(' ^a1b2c3', ''), 'a1b2c3')] } as PageView
    const [h] = highlightsOf(view)
    expect(h).toMatchObject({
      uid: 'a1b2c3',
      kind: 'text',
      page: 12,
      color: 'green',
      from: 10,
      to: 40,
      quote: 'A quote with [[brackets]]',
      context: 'before | after',
    })
  })

  it('reads an area and puts highlights in reading order', () => {
    const area = areaHighlightBlock({
      page: 2,
      color: 'red',
      rect: [0.2, 0.4, 0.6, 0.25],
      image: '../../assets/hls/x.png',
      uid: 'd4e5f6',
    })
    const view = {
      blocks: [
        block(1, area),
        block(2, 'second\nhl-page:: 1\nhl-from:: 50\nhl-to:: 60'),
        block(3, 'first\nhl-page:: 1\nhl-from:: 5\nhl-to:: 9\nhl-color:: purple'),
        block(4, 'no page here\nfoo:: 1'),
        block(5, 'broken\nhl-page:: 1\nhl-from:: 9\nhl-to:: 5'),
      ],
    } as PageView
    const list = highlightsOf(view)
    expect(list.map((h) => h.quote)).toEqual(['first', 'second', 'area'])
    expect(list[0].color).toBe('purple')
    expect(list[1].color).toBe('yellow')
    expect(list[2]).toMatchObject({ kind: 'area', rect: [0.2, 0.4, 0.6, 0.25] })
  })
})

describe('parsePdfHref', () => {
  it('knows a link to a PDF in the graph and its page', () => {
    expect(parsePdfHref('../assets/report.pdf')).toEqual({ path: 'assets/report.pdf', page: null })
    expect(parsePdfHref('../../assets/2026/a%20b.PDF#page=12')).toEqual({
      path: 'assets/2026/a b.PDF',
      page: 12,
    })
    expect(parsePdfHref('assets/x.pdf#zoom=2&page=3')?.page).toBe(3)
  })
  it('refuses anything else', () => {
    for (const bad of [
      'https://x.org/a.pdf',
      '../assets/a.png',
      'file:///home/a.pdf',
      'assets/../pages/a.pdf',
      '../notes/a.pdf',
    ])
      expect(parsePdfHref(bad)).toBeNull()
  })
})
