import { describe, expect, it } from 'vitest'
import type { BlockView, PageView } from '../api'
import {
  areaHighlightBlock,
  cfiPath,
  compareCfi,
  epubHighlightBlock,
  parseEpubHref,
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

const fm = (file: string) => ({ props: [{ key: 'file', raw: file }] })

describe('highlights of a book', () => {
  it('writes an EPUB highlight and reads it back; the file decides which locator counts', () => {
    const text = epubHighlightBlock({
      cfi: 'epubcfi(/6/14!/4/2/1,:20,:97)',
      chapter: '3 · The Storm',
      color: 'blue',
      quote: 'It was a [dark] night',
      context: 'before | after',
      uid: 'a1b2c3',
    })
    expect(text.split('\n')).toEqual([
      'It was a \\[dark\\] night ^a1b2c3',
      'hl-cfi:: epubcfi(/6/14!/4/2/1,:20,:97)',
      'hl-chapter:: 3 · The Storm',
      'hl-color:: blue',
      'hl-context:: before | after',
    ])
    const blocks = [
      block(1, text.replace(' ^a1b2c3', ''), 'a1b2c3'),
      block(2, 'a pdf one\nhl-page:: 1\nhl-from:: 0\nhl-to:: 5'),
      block(3, 'broken\nhl-cfi:: not a cfi'),
    ]
    const book = highlightsOf({ frontmatter: fm('assets/b.epub'), blocks } as unknown as PageView)
    expect(book).toHaveLength(1)
    expect(book[0]).toMatchObject({
      kind: 'epub',
      cfi: 'epubcfi(/6/14!/4/2/1,:20,:97)',
      chapter: '3 · The Storm',
      color: 'blue',
      quote: 'It was a [dark] night',
    })
    const pdf = highlightsOf({ frontmatter: fm('assets/b.pdf'), blocks } as unknown as PageView)
    expect(pdf.map((h) => h.kind)).toEqual(['text'])
  })

  it('puts highlights in the order of the book', () => {
    const mk = (cfi: string, q: string) => block(1, `${q}\nhl-cfi:: ${cfi}`, q)
    const view = {
      frontmatter: fm('assets/b.epub'),
      blocks: [
        mk('epubcfi(/6/14!/4/2/1,:50,:60)', 'late in 7'),
        mk('epubcfi(/6/4!/4/10/1,:5,:9)', 'chapter 2'),
        mk('epubcfi(/6/14!/4/2/1,:10,:20)', 'early in 7'),
        mk('epubcfi(/6/14!/4/2,/1:5,/3:10)', 'across'),
      ],
    } as unknown as PageView
    expect(highlightsOf(view).map((h) => h.quote)).toEqual(['chapter 2', 'across', 'early in 7', 'late in 7'])
  })

  it('reads the path of a CFI', () => {
    expect(cfiPath('epubcfi(/6/14!/4/2/1:20)')).toEqual([6, 14, 4, 2, 1, 20])
    expect(cfiPath('epubcfi(/6/14[chap]!/4/2,/1:5,/3:10)')).toEqual([6, 14, 4, 2, 1, 5])
    expect(compareCfi('epubcfi(/6/4!/2)', 'epubcfi(/6/4!/2)')).toBe(0)
    expect(compareCfi('epubcfi(/6/4)', 'epubcfi(/6/4!/2)')).toBe(-1)
  })

  it('knows a link to an EPUB and its CFI', () => {
    expect(parseEpubHref('../assets/books/Novel.epub')).toEqual({
      path: 'assets/books/Novel.epub',
      cfi: null,
    })
    expect(parseEpubHref('assets/a%20b.EPUB#epubcfi(/6/14!/4/2)')).toEqual({
      path: 'assets/a b.EPUB',
      cfi: 'epubcfi(/6/14!/4/2)',
    })
    expect(parseEpubHref('assets/a.epub#epubcfi(')?.cfi).toBeNull()
    for (const bad of ['https://x/a.epub', '../assets/a.pdf', 'assets/../a.epub', 'notes/a.epub'])
      expect(parseEpubHref(bad)).toBeNull()
  })
})
