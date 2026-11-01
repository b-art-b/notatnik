import { describe, expect, it } from 'vitest'
import { drawingMarkdown, drawingPath, replaceDrawingRef } from './drawing'

const P = 'assets/drawings/draw-0f3a9c21.svg'
const H = `../${P}`

describe('drawing references', () => {
  it('knows a drawing by its path', () => {
    expect(drawingPath(H)).toBe(P)
    expect(drawingPath(P)).toBe(P)
    expect(drawingPath('../assets/2026/10/x.svg')).toBeNull()
    expect(drawingPath('assets/drawings/x.svg')).toBeNull()
  })

  it('writes a picture or a link, with the name as its label', () => {
    expect(drawingMarkdown(H, '', 'picture')).toBe(`![](${H})`)
    expect(drawingMarkdown(H, ' Router sketch ', 'picture')).toBe(`![Router sketch](${H})`)
    expect(drawingMarkdown(H, 'Router sketch', 'link')).toBe(`[Router sketch](${H})`)
    expect(drawingMarkdown(H, '', 'link')).toBe(`[Drawing](${H})`)
    // brackets in a name do not end the label early
    expect(drawingMarkdown(H, 'a [b]', 'link')).toBe(`[a \\[b\\]](${H})`)
  })

  it('rewrites the reference in a block, whatever its form, keeping the target as it is written', () => {
    const asLink = (href: string) => drawingMarkdown(href, 'New', 'link')
    expect(replaceDrawingRef(`start ![old](${H}) end`, P, asLink)).toBe(`start [New](${H}) end`)
    // a path without the `../` prefix stays without it
    expect(replaceDrawingRef(`![](${P})`, P, (h) => drawingMarkdown(h, 'n', 'picture'))).toBe(`![n](${P})`)
    // a label with escaped brackets, and a target in angle brackets
    expect(replaceDrawingRef(`[a \\[b\\]](${H})`, P, asLink)).toBe(`[New](${H})`)
    expect(replaceDrawingRef(`x ![](<${H}>)`, P, asLink)).toBe(`x [New](${H})`)
    // another drawing is left alone
    const other = '../assets/drawings/draw-aaaaaaaa.svg'
    expect(replaceDrawingRef(`![](${other})`, P, asLink)).toBe(`![](${other})`)
  })

  it('removes the reference with the blank before it', () => {
    expect(replaceDrawingRef(`start ![x](${H})`, P, null)).toBe('start')
    expect(replaceDrawingRef(`![x](${H}) after`, P, null)).toBe('after')
    expect(replaceDrawingRef(`a ![x](${H}) b`, P, null)).toBe('a b')
    expect(replaceDrawingRef(`![x](${H})`, P, null)).toBe('')
  })
})
