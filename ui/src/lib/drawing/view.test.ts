import { describe, expect, it } from 'vitest'
import { percent, zoomed } from './view'
import { dataUrlToBlob, exportName, pngSize } from './exportImage'

describe('zoomed and percent (DR-19)', () => {
  it('steps by a quarter and stays within bounds', () => {
    expect(zoomed(1, 1)).toBeCloseTo(1.25)
    expect(zoomed(1, -1)).toBeCloseTo(0.8)
    expect(zoomed(30, 1)).toBe(32)
    expect(zoomed(0.021, -1)).toBe(0.02)
  })
  it('shows whole percent', () => {
    expect(percent(1)).toBe('100 %')
    expect(percent(0.333)).toBe('33 %')
  })
})

describe('pngSize and exportName (DR-22)', () => {
  it('scales the longer side to about 2000 px and keeps the proportions', () => {
    expect(pngSize(1000, 500)).toEqual({ w: 2000, h: 1000 })
    expect(pngSize(100, 400)).toEqual({ w: 500, h: 2000 })
  })
  it('never gives an empty image', () => {
    expect(pngSize(0, 0)).toEqual({ w: 1, h: 1 })
  })
  it('names the file after the drawing, without what a file system refuses', () => {
    expect(exportName('Router sketch', 'png')).toBe('Router sketch.png')
    expect(exportName('  ', 'svg')).toBe('drawing.svg')
    expect(exportName('a/b:c*d', 'png')).toBe('a b c d.png')
  })
})

describe('dataUrlToBlob (DR-22)', () => {
  it('turns base64 and plain data URLs into bytes of their type', async () => {
    const b = dataUrlToBlob('data:image/png;base64,iVBORw0KGgo=')
    expect(b.type).toBe('image/png')
    expect(new Uint8Array(await b.arrayBuffer())).toEqual(new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]))
    const t = dataUrlToBlob('data:text/plain,a%20b')
    expect(await t.text()).toBe('a b')
  })
  it('refuses what is not a data URL', () => {
    expect(() => dataUrlToBlob('https://example.com/x.png')).toThrow()
  })
})
