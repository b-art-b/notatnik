import { describe, expect, it } from 'vitest'
import { describeRange, docText, refind } from './docText'

function chapter(html: string): Document {
  return new DOMParser().parseFromString(`<html><body>${html}</body></html>`, 'text/html')
}

describe('docText', () => {
  const doc = chapter(
    '<h1>Storm</h1><p>It was a <em>dark</em> and stormy night.</p><script>var x = "night"</script><p>The rain fell.</p>',
  )

  it('joins the text nodes and leaves out scripts', () => {
    const dt = docText(doc)
    expect(dt.text).toBe('StormIt was a dark and stormy night.The rain fell.')
  })

  it('goes from a point to an offset and back to a range across elements', () => {
    const dt = docText(doc)
    const from = dt.text.indexOf('dark')
    const to = dt.text.indexOf('night') + 5
    const r = dt.range(from, to)!
    expect(r.toString()).toBe('dark and stormy night')
    expect(dt.offsetOf(r.startContainer, r.startOffset)).toBe(from)
    expect(dt.offsetOf(r.endContainer, r.endOffset)).toBe(to)
    expect(describeRange(dt, r)).toEqual({
      quote: 'dark and stormy night',
      context: 'StormIt was a | .The rain fell.',
    })
  })

  it('finds a highlight again by its quote and uses the context to choose', () => {
    const d = chapter('<p>the end of one. the end of two. the end of three.</p>')
    const dt = docText(d)
    const r = refind(dt, { quote: 'the end', context: 'of one. | of three.' })
    expect(r).not.toBeNull()
    expect(dt.offsetOf(r!.startContainer, r!.startOffset)).toBe(dt.text.indexOf('the end of two'))
    expect(refind(dt, { quote: 'not in this chapter', context: ' | ' })).toBeNull()
  })
})

describe('paragraphs', () => {
  const doc = chapter(
    '<h1>Storm</h1><p>It was a <em>dark</em> and\n  stormy night.</p><p>The rain<br/>fell hard.</p><ul><li>one</li><li>two</li></ul><table><tr><td>cellA</td><td>cellB</td></tr></table>',
  )
  const dt = docText(doc)
  const between = (a: string, b: string) => dt.paragraphs(dt.text.indexOf(a), dt.text.indexOf(b) + b.length)

  it('is one line for a selection inside a paragraph, markup and white space folded', () => {
    expect(between('dark', 'night.')).toBe('dark and stormy night.')
  })

  it('keeps a paragraph break, a line break, list items and cells as lines', () => {
    expect(between('It was', 'fell hard.')).toBe('It was a dark and stormy night.\nThe rain\nfell hard.')
    expect(between('one', 'two')).toBe('one\ntwo')
    expect(between('cellA', 'cellB')).toBe('cellA\ncellB')
    expect(between('Storm', 'night.')).toBe('Storm\nIt was a dark and stormy night.')
  })

  it('cuts at the ends of the range, not at the ends of the paragraphs', () => {
    expect(dt.paragraphs(dt.text.indexOf('stormy'), dt.text.indexOf('rain') + 4)).toBe(
      'stormy night.\nThe rain',
    )
  })

  it('goes into the quote of a highlight, and the context stays one line', () => {
    const r = dt.range(dt.text.indexOf('night.'), dt.text.indexOf('fell') + 4)!
    const d = describeRange(dt, r)!
    expect(d.quote).toBe('night.\nThe rain\nfell')
    expect(d.context).not.toContain('\n')
  })
})
