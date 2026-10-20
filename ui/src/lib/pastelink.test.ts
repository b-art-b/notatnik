import { describe, expect, it } from 'vitest'
import { appTarget, cleanUrl, htmlLink, insideLink, nextcloudFile, pastedUrl, smartPaste } from './pastelink'

const base = { plain: '', html: '', selected: '', before: '', titles: false }

describe('cleanUrl', () => {
  it('drops tracking parameters and keeps the rest', () => {
    expect(cleanUrl('https://x.org/a?utm_source=n&id=3&fbclid=zz#top')).toBe('https://x.org/a?id=3#top')
    expect(cleanUrl('https://x.org/a?utm_medium=m')).toBe('https://x.org/a')
    expect(cleanUrl('https://x.org/a?q=utm_x')).toBe('https://x.org/a?q=utm_x')
    expect(cleanUrl('mailto:a@b.c?subject=hi')).toBe('mailto:a@b.c?subject=hi')
  })
})

describe('addresses', () => {
  it('pastedUrl wants one address and nothing else', () => {
    expect(pastedUrl(' https://x.org/a \n')).toBe('https://x.org/a')
    expect(pastedUrl('see https://x.org')).toBeNull()
    expect(pastedUrl('https://x.org a')).toBeNull()
    expect(pastedUrl('ftp://x.org')).toBeNull()
  })
  it('knows the links of the app', () => {
    expect(appTarget('notatnik://page/My%20Plan#abc123')).toEqual({ page: 'My Plan', block: 'abc123' })
    expect(appTarget('notatnik://page/a%2Fb')).toEqual({ page: 'a/b', block: null })
    expect(appTarget('https://x.org')).toBeNull()
  })
  it('turns a Nextcloud file address into the stable one', () => {
    expect(nextcloudFile('https://nc.x.eu/apps/files/?dir=/Docs&fileid=4711&openfile=true')).toBe(
      'https://nc.x.eu/f/4711',
    )
    expect(nextcloudFile('https://x.eu/nextcloud/index.php/apps/files/files/42?dir=/')).toBe(
      'https://x.eu/nextcloud/index.php/f/42',
    )
    expect(nextcloudFile('https://nc.x.eu/apps/files/?dir=/Docs')).toBeNull()
    expect(nextcloudFile('https://x.org/a')).toBeNull()
  })
})

describe('insideLink', () => {
  it('is true in link targets, references and code', () => {
    expect(insideLink('see [here](')).toBe(true)
    expect(insideLink('a [[Pa')).toBe(true)
    expect(insideLink('a ((ab')).toBe(true)
    expect(insideLink('x `co')).toBe(true)
    expect(insideLink('a <https://')).toBe(true)
    expect(insideLink('a [x](y) and ')).toBe(false)
    expect(insideLink('a `x` and ')).toBe(false)
  })
})

describe('htmlLink', () => {
  it('reads a lone link and not a sentence with one', () => {
    expect(htmlLink('<a href="https://x.org/p">The report</a>', 'The report')).toEqual({
      text: 'The report',
      href: 'https://x.org/p',
    })
    expect(
      htmlLink('<meta charset="utf-8"><a href="https://x.org/p"> The\n report </a>', 'The report'),
    ).toEqual({
      text: 'The report',
      href: 'https://x.org/p',
    })
    expect(htmlLink('<p>read <a href="https://x.org/p">this</a> now</p>', 'read this now')).toBeNull()
    expect(htmlLink('<a href="/relative">x</a>', 'x')).toBeNull()
    expect(htmlLink('<a href="https://a.b">1</a><a href="https://c.d">2</a>', '12')).toBeNull()
  })
})

describe('smartPaste', () => {
  it('an address over selected text makes a link with that text', () => {
    expect(smartPaste({ ...base, plain: 'https://x.org/r', selected: 'the report' })).toEqual({
      insert: '[the report](https://x.org/r)',
    })
  })
  it('tracking parameters go, a clean address is pasted as typed', () => {
    expect(smartPaste({ ...base, plain: 'https://x.org/r?utm_source=a&p=1' })).toEqual({
      insert: 'https://x.org/r?p=1',
    })
    expect(smartPaste({ ...base, plain: 'https://x.org/r' })).toBeNull()
  })
  it('its own page and block addresses become references', () => {
    expect(smartPaste({ ...base, plain: 'notatnik://page/Plan' })?.insert).toBe('[[Plan]]')
    expect(smartPaste({ ...base, plain: 'notatnik://page/Plan', selected: 'the plan' })?.insert).toBe(
      '[[Plan|the plan]]',
    )
    expect(smartPaste({ ...base, plain: 'notatnik://page/Plan#abc123' })?.insert).toBe('((abc123))')
  })
  it('a Nextcloud file keeps working after a move', () => {
    expect(smartPaste({ ...base, plain: 'https://nc.x.eu/apps/files/?dir=/D&fileid=7' })?.insert).toBe(
      '[Nextcloud file](https://nc.x.eu/f/7)',
    )
    expect(
      smartPaste({ ...base, plain: 'https://nc.x.eu/apps/files/?fileid=7', selected: 'contract' })?.insert,
    ).toBe('[contract](https://nc.x.eu/f/7)')
  })
  it('a lone address asks for its title only when the setting is on', () => {
    expect(smartPaste({ ...base, plain: 'https://x.org/r', titles: true })).toEqual({
      insert: 'https://x.org/r',
      fetchTitle: 'https://x.org/r',
    })
    // text selected: the selection is the link text, nothing is looked up
    expect(
      smartPaste({ ...base, plain: 'https://x.org/r', selected: 'a', titles: true })?.fetchTitle,
    ).toBeUndefined()
    // mail addresses have no page
    expect(smartPaste({ ...base, plain: 'mailto:a@b.c', titles: true })).toBeNull()
  })
  it('a link copied from a page keeps its text', () => {
    expect(
      smartPaste({ ...base, plain: 'The report', html: '<a href="https://x.org/p?utm_x=1">The report</a>' }),
    ).toEqual({ insert: '[The report](https://x.org/p)' })
  })
  it('leaves everything else alone', () => {
    expect(smartPaste({ ...base, plain: 'just text' })).toBeNull()
    expect(smartPaste({ ...base, plain: 'https://x.org/r', before: '[a](' })).toBeNull()
    expect(smartPaste({ ...base, plain: 'https://x.org/r', selected: 'two\nlines' })).toBeNull()
  })
})
