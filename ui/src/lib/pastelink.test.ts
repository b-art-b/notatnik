import { describe, expect, it } from 'vitest'
import {
  addressLine,
  appTarget,
  cleanUrl,
  githubShort,
  htmlLink,
  insideLink,
  nextcloudFile,
  pastedAddressList,
  pastedFile,
  pastedUrl,
  smartPaste,
  titleWanted,
} from './pastelink'

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

describe('GitHub addresses', () => {
  it('have a short form', () => {
    expect(githubShort('https://github.com/o/r/issues/12')).toBe('o/r#12')
    expect(githubShort('https://github.com/o/r/pull/7/')).toBe('o/r#7')
    expect(githubShort('https://github.com/o/r/commit/ABCDEF1234567')).toBe('o/r@ABCDEF1')
    expect(githubShort('https://github.com/o/r')).toBeNull()
    expect(githubShort('https://github.com/o/r/issues/new')).toBeNull()
    expect(githubShort('https://example.com/o/r/issues/12')).toBeNull()
  })
  it('are pasted as a link with it, without asking for a title', () => {
    const r = smartPaste({ ...base, plain: 'https://github.com/o/r/pull/7?utm_source=x', titles: true })
    expect(r).toEqual({ insert: '[o/r#7](https://github.com/o/r/pull/7)' })
    // a selection still names the link
    expect(smartPaste({ ...base, plain: 'https://github.com/o/r/pull/7', selected: 'the fix' })?.insert).toBe(
      '[the fix](https://github.com/o/r/pull/7)',
    )
  })
})

describe('files of this computer', () => {
  it('are recognised as an address or an absolute path with a file name', () => {
    expect(pastedFile('file:///home/me/a.pdf')).toBe('file:///home/me/a.pdf')
    expect(pastedFile('/home/me/My Docs/a.pdf')).toBe('file:///home/me/My%20Docs/a.pdf')
    expect(pastedFile('C:\\Users\\me\\a.pdf')).toBe('file:///C:/Users/me/a.pdf')
    // not a file: prose that starts with a slash, a bare folder, one segment, several lines
    expect(pastedFile('/ratio of 3.5 to 1')).toBeNull()
    expect(pastedFile('/home/me/folder')).toBeNull()
    expect(pastedFile('/a.pdf')).toBeNull()
    expect(pastedFile('/a/b.pdf\n/a/c.pdf')).toBeNull()
  })
  it('are pasted as a link named after the file', () => {
    expect(smartPaste({ ...base, plain: '/home/me/My Docs/a b.pdf' })?.insert).toBe(
      '[a b.pdf](file:///home/me/My%20Docs/a%20b.pdf)',
    )
    expect(smartPaste({ ...base, plain: 'file:///x/y.pdf', selected: 'the contract' })?.insert).toBe(
      '[the contract](file:///x/y.pdf)',
    )
  })
})

describe('several addresses', () => {
  it('a list wants every line to be an address or a file, two lines at least', () => {
    expect(pastedAddressList('https://a.org/1\n\nhttps://b.org/2\n')).toEqual([
      'https://a.org/1',
      'https://b.org/2',
    ])
    expect(pastedAddressList('https://a.org/1')).toBeNull()
    expect(pastedAddressList('https://a.org/1\nand some text')).toBeNull()
    expect(pastedAddressList('/x/a.pdf\n/x/b.pdf')).toEqual(['file:///x/a.pdf', 'file:///x/b.pdf'])
  })
  it('each line becomes a link with its title, or stays a clean address', () => {
    expect(addressLine('https://a.org/1?utm_source=z', 'A page')).toBe('[A page](https://a.org/1)')
    expect(addressLine('https://a.org/1', '')).toBe('https://a.org/1')
    expect(addressLine('https://github.com/o/r/issues/3', '')).toBe(
      '[o/r#3](https://github.com/o/r/issues/3)',
    )
    expect(addressLine('file:///x/a%20b.pdf', '')).toBe('[a b.pdf](file:///x/a%20b.pdf)')
  })
  it('only a plain web address is worth a title lookup', () => {
    expect(titleWanted('https://a.org/1')).toBe(true)
    expect(titleWanted('https://github.com/o/r/pull/3')).toBe(false)
    expect(titleWanted('notatnik://page/Plan')).toBe(false)
    expect(titleWanted('file:///x/a.pdf')).toBe(false)
  })
})
