import { describe, expect, it } from 'vitest'
import { buildPageText, locate } from './pageText'
import { contextOf, quoteOf, resolveAnchor, splitContext, squash } from './anchor'

const items = [
  { str: 'The mitochondrion is', hasEOL: true },
  { str: 'the powerhouse of' },
  { str: ' the cell.', hasEOL: true },
  { type: 'beginMarkedContent' },
  { str: 'Second line.' },
]

describe('buildPageText', () => {
  it('joins the items, adds a newline after a line end and skips entries without text', () => {
    const pt = buildPageText(items)
    expect(pt.text).toBe('The mitochondrion is\nthe powerhouse of the cell.\nSecond line.')
    expect(pt.starts).toEqual([0, 21, 38, 49])
  })

  it('finds the item of an offset', () => {
    const pt = buildPageText(items)
    const lens = items.filter((i) => i.str !== undefined).map((i) => i.str!.length)
    expect(locate(pt, lens, 0)).toEqual({ item: 0, at: 0 })
    expect(locate(pt, lens, 20)).toEqual({ item: 0, at: 20 }) // the newline after item 0
    expect(locate(pt, lens, 21)).toEqual({ item: 1, at: 0 })
    expect(locate(pt, lens, 40)).toEqual({ item: 2, at: 2 })
    expect(locate(pt, lens, 9999)).toEqual({ item: 3, at: 12 })
    expect(locate(buildPageText([]), [], 3)).toBeNull()
  })
})

describe('quote and context', () => {
  const page = buildPageText(items).text
  it('squashes line breaks in the quote', () => {
    expect(quoteOf(page, 4, 38)).toBe('mitochondrion is the powerhouse of')
    expect(squash('  a \n b\t c ')).toBe('a b c')
  })
  it('keeps text on both sides, without the separator', () => {
    const c = contextOf(page, 4, 38)
    expect(c).toBe('The | the cell. Second line.')
    expect(splitContext(c)).toEqual({ before: 'The', after: 'the cell. Second line.' })
    expect(contextOf('a | b c | d e', 5, 8)).not.toMatch(/\|.*\|/)
    expect(splitContext('| only after')).toEqual({ before: '', after: 'only after' })
    expect(splitContext('only before |')).toEqual({ before: 'only before', after: '' })
  })
})

describe('resolveAnchor', () => {
  const text = 'Alpha beta gamma.\nDelta beta gamma. Epsilon beta gamma.'
  const at = (s: string, n = 0) => {
    let i = -1
    for (let k = 0; k <= n; k++) i = text.indexOf(s, i + 1)
    return i
  }
  const stored = (from: number, to: number, quote: string) => ({
    from,
    to,
    quote,
    context: contextOf(text, from, to),
  })

  it('keeps numbers that fit', () => {
    const from = at('beta gamma', 1)
    expect(resolveAnchor(text, stored(from, from + 10, 'beta gamma'))).toEqual({
      from,
      to: from + 10,
    })
  })

  it('ignores line breaks and spaces when comparing', () => {
    const from = at('gamma.')
    const to = at('Delta') + 5
    const q = quoteOf(text, from, to)
    expect(resolveAnchor(text, stored(from, to, q))).toEqual({ from, to })
  })

  it('finds the quote when the numbers are wrong and uses the context to choose', () => {
    const from = at('beta gamma', 2)
    const s = stored(from, from + 10, 'beta gamma')
    const r = resolveAnchor(text, { ...s, from: 3, to: 13 })
    expect(r).toEqual({ from, to: from + 10 })
  })

  it('takes the hit nearest to the old numbers when the context cannot tell', () => {
    const t = 'one two one two one'
    const r = resolveAnchor(t, { from: 9, to: 12, quote: 'one', context: ' | ' })
    expect(r).toEqual({ from: 8, to: 11 })
  })

  it('trusts the numbers when the quote was edited but the context still fits', () => {
    const from = at('beta gamma', 1)
    const s = stored(from, from + 10, 'my own words')
    expect(resolveAnchor(text, s)).toEqual({ from, to: from + 10 })
  })

  it('gives up when nothing fits', () => {
    expect(resolveAnchor(text, { from: 2, to: 7, quote: 'not on the page', context: 'zzz | yyy' })).toBeNull()
    expect(resolveAnchor(text, { from: 0, to: 0, quote: '', context: ' | ' })).toBeNull()
    expect(resolveAnchor('', { from: 0, to: 5, quote: 'abc', context: ' | ' })).toBeNull()
  })
})
