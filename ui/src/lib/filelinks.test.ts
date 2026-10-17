import { describe, expect, it } from 'vitest'
import { fileLink, fileUrl, humanSize, isLinkable, lastName, nameForAddress } from './filelinks'

describe('fileLink', () => {
  it('writes a plain Markdown link', () => {
    expect(fileLink('Nextcloud Manual.pdf', 'http://localhost:8080/f/209')).toBe(
      '[Nextcloud Manual.pdf](http://localhost:8080/f/209)',
    )
  })
  it('escapes brackets in the name and wraps an address that has spaces or parentheses', () => {
    expect(fileLink('a [1].pdf', 'file:///x/a b.pdf')).toBe('[a \\[1\\].pdf](<file:///x/a b.pdf>)')
    expect(fileLink('x', 'https://h/a(1).pdf')).toBe('[x](<https://h/a(1).pdf>)')
  })
})

describe('fileUrl', () => {
  it('encodes each part of a path', () => {
    expect(fileUrl('/home/me/My Docs/a#1.pdf')).toBe('file:///home/me/My%20Docs/a%231.pdf')
  })
  it('handles a Windows path', () => {
    expect(fileUrl('C:\\Users\\me\\a b.pdf')).toBe('file:///C:/Users/me/a%20b.pdf')
  })
})

describe('names', () => {
  it('takes the file name of a path or address, decoded', () => {
    expect(lastName('/x/y/report.pdf')).toBe('report.pdf')
    expect(lastName('https://h/x/a%20b.pdf?dl=1')).toBe('a b.pdf')
    expect(lastName('https://h/dir/')).toBe('dir')
  })
  it('names a pasted address after its file, else its host', () => {
    expect(nameForAddress('https://h.example.org/x/report.pdf')).toBe('report.pdf')
    expect(nameForAddress('https://h.example.org')).toBe('h.example.org')
    expect(nameForAddress('https://h.example.org/')).toBe('h.example.org')
  })
  it('accepts only addresses the app opens', () => {
    for (const ok of ['https://a.b/c', 'http://localhost:8080/f/1', 'mailto:a@b.c', 'file:///home/me/a.pdf'])
      expect(isLinkable(ok)).toBe(true)
    for (const bad of ['javascript:alert(1)', 'ftp://x/y', 'just words', '', 'file://relative'])
      expect(isLinkable(bad)).toBe(false)
  })
})

describe('humanSize', () => {
  it('uses the largest unit', () => {
    expect(humanSize(12)).toBe('12 B')
    expect(humanSize(2048)).toBe('2.0 KB')
    expect(humanSize(3 * 1024 * 1024)).toBe('3.0 MB')
  })
})
