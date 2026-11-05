import { describe, expect, it } from 'vitest'
import { clipText, stepLabel, webAddress } from './clip'

describe('webAddress', () => {
  it('takes an http(s) address, also with text around it', () => {
    expect(webAddress('https://example.org/a?b=1#c')).toBe('https://example.org/a?b=1#c')
    expect(webAddress('  http://example.org/x \n more')).toBe('http://example.org/x')
  })
  it('leaves everything else', () => {
    expect(webAddress('')).toBeNull()
    expect(webAddress('hello world')).toBeNull()
    expect(webAddress('ftp://example.org/x')).toBeNull()
    expect(webAddress('javascript:alert(1)')).toBeNull()
    expect(webAddress('https://')).toBeNull()
  })
  it('does not offer an address with a login in it', () => {
    expect(webAddress('https://me:secret@example.org/')).toBeNull()
  })
})

describe('clipText', () => {
  it('is the link and the source property', () => {
    const c = { markdown: '[A](../assets/web/a-12345678.epub)', source: 'https://e.org/a' }
    expect(clipText({ ...c, path: '', title: 'A', notes: [], replaced: false })).toBe(
      '[A](../assets/web/a-12345678.epub)\nsource:: https://e.org/a',
    )
  })
})

describe('stepLabel', () => {
  it('names the step, and counts pictures', () => {
    expect(stepLabel('fetching', 0, 0)).toBe('Fetching the page…')
    expect(stepLabel('pictures', 2, 5)).toBe('Fetching pictures (2 of 5)…')
    expect(stepLabel('?', 0, 0)).toBe('Working…')
  })
})
