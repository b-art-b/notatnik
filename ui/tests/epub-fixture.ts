// EPUB files for the browser tests (EA-1..EA-10), made here so no binary has to be reviewed. A book is a zip: `mimetype`
// first and stored, then the container, the package (OPF), the navigation document and the chapters. The zip writer
// stores the files without compression.
import { crc32 } from 'node:zlib'

export interface Chapter {
  title: string
  /** The content of `<body>` (after the `<h1>` that holds the title). */
  html: string
  /** Extra attributes for the `<html>` element. */
  attrs?: string
}

export interface BookSpec {
  title?: string
  chapters: Chapter[]
  /** Extra files below `OEBPS/` (name -> bytes), declared in the manifest with `type`. */
  files?: { name: string; type: string; bytes: Buffer }[]
  fixedLayout?: boolean
}

function zip(entries: { name: string; data: Buffer }[]): Buffer {
  const parts: Buffer[] = []
  const central: Buffer[] = []
  let offset = 0
  for (const e of entries) {
    const name = Buffer.from(e.name)
    const crc = crc32(e.data)
    const local = Buffer.alloc(30)
    local.writeUInt32LE(0x04034b50, 0)
    local.writeUInt16LE(20, 4) // version needed
    local.writeUInt16LE(0x0800, 6) // names are UTF-8
    local.writeUInt16LE(0, 8) // stored
    local.writeUInt32LE(crc, 14)
    local.writeUInt32LE(e.data.length, 18)
    local.writeUInt32LE(e.data.length, 22)
    local.writeUInt16LE(name.length, 26)
    parts.push(local, name, e.data)
    const c = Buffer.alloc(46)
    c.writeUInt32LE(0x02014b50, 0)
    c.writeUInt16LE(20, 4)
    c.writeUInt16LE(20, 6)
    c.writeUInt16LE(0x0800, 8)
    c.writeUInt32LE(crc, 16)
    c.writeUInt32LE(e.data.length, 20)
    c.writeUInt32LE(e.data.length, 24)
    c.writeUInt16LE(name.length, 28)
    c.writeUInt32LE(offset, 42)
    central.push(c, name)
    offset += 30 + name.length + e.data.length
  }
  const dir = Buffer.concat(central)
  const end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50, 0)
  end.writeUInt16LE(entries.length, 8)
  end.writeUInt16LE(entries.length, 10)
  end.writeUInt32LE(dir.length, 12)
  end.writeUInt32LE(offset, 16)
  return Buffer.concat([...parts, dir, end])
}

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;')

export function makeEpub(spec: BookSpec): Buffer {
  const title = spec.title ?? 'Test Book'
  const chapters = spec.chapters
  const files = spec.files ?? []
  const xhtml = (c: Chapter, i: number) =>
    `<?xml version="1.0" encoding="utf-8"?>\n<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" ${c.attrs ?? ''}>` +
    `<head><title>${esc(c.title)}</title><meta charset="utf-8"/>${spec.fixedLayout ? '<meta name="viewport" content="width=600,height=800"/>' : ''}</head>` +
    `<body><h1 id="top${i}">${esc(c.title)}</h1>${c.html}</body></html>`
  const nav =
    `<?xml version="1.0" encoding="utf-8"?>\n<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head><body>` +
    `<nav epub:type="toc" id="toc"><ol>${chapters.map((c, i) => `<li><a href="ch${i + 1}.xhtml">${esc(c.title)}</a></li>`).join('')}</ol></nav></body></html>`
  const opf =
    `<?xml version="1.0" encoding="utf-8"?>\n<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">` +
    `<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:12345678-1234-1234-1234-123456789abc</dc:identifier>` +
    `<dc:title>${esc(title)}</dc:title><dc:language>en</dc:language><meta property="dcterms:modified">2026-01-01T00:00:00Z</meta>` +
    `${spec.fixedLayout ? '<meta property="rendition:layout">pre-paginated</meta>' : ''}</metadata><manifest>` +
    `<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>` +
    chapters
      .map((_, i) => `<item id="ch${i + 1}" href="ch${i + 1}.xhtml" media-type="application/xhtml+xml"/>`)
      .join('') +
    files.map((f, i) => `<item id="f${i}" href="${f.name}" media-type="${f.type}"/>`).join('') +
    `</manifest><spine>${chapters.map((_, i) => `<itemref idref="ch${i + 1}"/>`).join('')}</spine></package>`
  return zip([
    { name: 'mimetype', data: Buffer.from('application/epub+zip') },
    {
      name: 'META-INF/container.xml',
      data: Buffer.from(
        `<?xml version="1.0"?>\n<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>`,
      ),
    },
    { name: 'OEBPS/content.opf', data: Buffer.from(opf) },
    { name: 'OEBPS/nav.xhtml', data: Buffer.from(nav) },
    ...chapters.map((c, i) => ({ name: `OEBPS/ch${i + 1}.xhtml`, data: Buffer.from(xhtml(c, i + 1)) })),
    ...files.map((f) => ({ name: `OEBPS/${f.name}`, data: f.bytes })),
  ])
}

const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
  'base64',
)

const para = (s: string) => `<p>${s}</p>`

/** A novel of three chapters: sentences the tests look for, a picture, and a footnote that links to chapter 3. */
export const NOVEL = {
  title: 'The Storm',
  s1: 'It was a dark and stormy night, and the rain fell in torrents.',
  s2: 'The lighthouse keeper climbed the stairs one slow step at a time.',
  s3: 'Morning came with a pale and silent light over the grey sea.',
  chapters: [
    {
      title: 'Loomings',
      html:
        para('It was a dark and stormy night, and the rain fell in torrents.') +
        para('The wind howled through the rigging while the crew held on.') +
        para(
          'A picture follows: <img src="pic.png" alt="a dot"/> and a note<a epub:type="noteref" href="ch3.xhtml#fn1">1</a>.',
        ),
    },
    {
      title: 'The Storm',
      html:
        para('The lighthouse keeper climbed the stairs one slow step at a time.') +
        para('At the top he lit the lamp and watched the black water.') +
        para('Zebrafish are not found in the open sea, he muttered.'),
    },
    {
      title: 'Calm',
      html:
        para('Morning came with a pale and silent light over the grey sea.') +
        para('The survivors counted what was left of the cargo.') +
        `<aside id="fn1" epub:type="footnote"><p>The footnote: a barometer dropped that night.</p></aside>`,
    },
  ] as Chapter[],
  files: [{ name: 'pic.png', type: 'image/png', bytes: PNG }],
}

export const novel = () => makeEpub({ title: NOVEL.title, chapters: NOVEL.chapters, files: NOVEL.files })

/** A book of `n` chapters with a line to look for in each. */
export const longBook = (n: number) =>
  makeEpub({
    title: 'Long Book',
    chapters: Array.from({ length: n }, (_, i) => ({
      title: `Chapter ${i + 1}`,
      html: para(`Text of chapter ${i + 1} begins here.`) + para(`And chapter ${i + 1} ends here.`),
    })),
  })

/** A book that tries everything: scripts, a handler, a `javascript:` link, remote picture and style, a web link. */
export const hostile = () =>
  makeEpub({
    title: 'Hostile',
    chapters: [
      {
        title: 'Trap',
        html:
          `<script>window.parent.__pwned = 'script'; fetch('https://leak.invalid/s')</script>` +
          `<img src="https://leak.invalid/x.png" alt="remote" onerror="window.parent.__pwned = 'onerror'"/>` +
          `<link rel="stylesheet" href="https://leak.invalid/s.css"/>` +
          `<p id="p1" onclick="window.parent.__pwned = 'onclick'">Safe text before the traps.</p>` +
          `<p><a id="js" href="javascript:window.parent.__pwned = 'javascript'">a javascript link</a> ` +
          `<a id="web" href="https://example.org/page">a web link</a> ` +
          `<a id="ftp" href="ftp://example.org/file">an ftp link</a></p>`,
      },
    ],
  })

export const fixedLayoutBook = () =>
  makeEpub({
    title: 'Fixed',
    chapters: [{ title: 'Page', html: para('A page of a fixed layout book.') }],
    fixedLayout: true,
  })

/** One chapter of `n` paragraphs, long enough for several pages. */
export const longChapter = (n: number) =>
  makeEpub({
    title: 'Long Chapter',
    chapters: [
      {
        title: 'Everything',
        html: Array.from({ length: n }, (_, i) =>
          para(
            `Paragraph ${i + 1}: the quick brown fox jumps over the lazy dog again and again until the page is full.`,
          ),
        ).join(''),
      },
    ],
  })
