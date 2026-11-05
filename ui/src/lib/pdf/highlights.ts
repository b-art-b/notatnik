// Highlights as blocks of the sidecar page (PA-2, PA-3, PA-5): reading them from a page view and writing the text of a
// new one. The format is in `docs/file-format.md` ("PDF annotations").
import type { BlockOp, BlockView, PageView } from '../api'

export const COLORS = ['yellow', 'green', 'blue', 'red', 'purple'] as const
export type Color = (typeof COLORS)[number]

/** `x, y, w, h` as fractions of the page, from its top left. */
export type Rect = [number, number, number, number]

export interface Highlight {
  /** The block's `^id`; null only for a block that was typed in by hand without one. */
  uid: string | null
  blockId: number
  path: number[]
  /** `text` and `area` are highlights of a PDF page, `epub` one of a book (EA-4). */
  kind: 'text' | 'area' | 'epub'
  /** From 1 (a PDF page); 0 for a book. */
  page: number
  color: Color
  /** Offsets of the quote in the text of the page (text highlights; 0 for an area). */
  from: number
  to: number
  /** The quote as written in the block (an area: its alt text). */
  quote: string
  context: string
  rect: Rect | null
  /** An EPUB CFI range, `epubcfi(/6/14!/4/2/1,:20,:97)` (books); empty for a PDF. */
  cfi: string
  /** The chapter as it is shown, `3 · The Storm` (books). */
  chapter: string
  /** The block, whose children are the comment. */
  block: BlockView
}

const prop = (b: BlockView, key: string): string | null =>
  b.props.find((p) => p.key.toLowerCase() === key)?.raw ?? null

/** Escapes what Markdown or the note's syntax would take for something else, so a quote shows as it was in the PDF. */
export function escapeQuote(s: string): string {
  let out = s.replace(/[\\`*_[\]()#^~<>|!&]/g, '\\$&')
  // at the start of a line: a list, a heading, a quote, a numbered list
  out = out.replace(/^([-+=])/, '\\$1').replace(/^(\d+)([.)])/, '$1\\$2')
  return out
}

/** The reverse of `escapeQuote` (a backslash before punctuation is dropped). */
export function unescapeQuote(s: string): string {
  return s.replace(/\\([!-/:-@[-`{-~])/g, '$1')
}

function colorOf(raw: string | null): Color {
  const c = (raw ?? '').trim().toLowerCase()
  return (COLORS as readonly string[]).includes(c) ? (c as Color) : 'yellow'
}

function rectOf(raw: string | null): Rect | null {
  if (!raw) return null
  const n = raw.split(',').map((x) => Number(x.trim()))
  if (n.length !== 4 || n.some((x) => !Number.isFinite(x))) return null
  return [n[0], n[1], n[2], n[3]]
}

/** The first line of a block's text, without its Markdown escapes. */
function firstLine(b: BlockView): string {
  return unescapeQuote(b.text.split('\n')[0] ?? '').trim()
}

/** The highlights of a sidecar page: its top-level blocks that have the locator of the file the page is about
 * (`hl-page` for a PDF, `hl-cfi` for an EPUB; the `file` in the frontmatter decides, a block with the other kind of
 * locator is ignored), in reading order. */
export function highlightsOf(view: PageView | null | undefined): Highlight[] {
  if (!view) return []
  const file = view.frontmatter?.props.find((p) => p.key === 'file')?.raw ?? ''
  const book = /\.epub$/i.test(file)
  const out: Highlight[] = []
  for (const [i, b] of view.blocks.entries()) {
    if (book) {
      const cfi = prop(b, 'hl-cfi')?.trim() ?? ''
      if (!/^epubcfi\(.+\)$/.test(cfi)) continue
      out.push({
        uid: b.uid,
        blockId: b.id,
        path: [i],
        kind: 'epub',
        page: 0,
        color: colorOf(prop(b, 'hl-color')),
        from: 0,
        to: 0,
        quote: firstLine(b),
        context: prop(b, 'hl-context') ?? ' | ',
        rect: null,
        cfi,
        chapter: prop(b, 'hl-chapter')?.trim() ?? '',
        block: b,
      })
      continue
    }
    const page = Number(prop(b, 'hl-page'))
    if (!Number.isInteger(page) || page < 1) continue
    const area = (prop(b, 'hl-type') ?? '').trim().toLowerCase() === 'area'
    const rect = rectOf(prop(b, 'hl-rect'))
    if (area && !rect) continue
    const from = Number(prop(b, 'hl-from'))
    const to = Number(prop(b, 'hl-to'))
    if (!area && !(Number.isInteger(from) && Number.isInteger(to) && to > from && from >= 0)) continue
    const alt = /^!\[([^\]]*)\]/.exec(b.text)
    out.push({
      uid: b.uid,
      blockId: b.id,
      path: [i],
      kind: area ? 'area' : 'text',
      page,
      color: colorOf(prop(b, 'hl-color')),
      from: area ? 0 : from,
      to: area ? 0 : to,
      quote: area ? (alt?.[1] ?? 'area') : firstLine(b),
      context: prop(b, 'hl-context') ?? ' | ',
      rect: area ? rect : null,
      cfi: '',
      chapter: '',
      block: b,
    })
  }
  if (book) return out.sort((a, b) => compareCfi(a.cfi, b.cfi))
  return out.sort((a, b) => a.page - b.page || (a.rect?.[1] ?? 0) - (b.rect?.[1] ?? 0) || a.from - b.from)
}

/** The numbers of the path to the start of a CFI (`/6/14!/4/2/1:20` is 6, 14, 4, 2, 1, 20), for putting highlights of a
 * book in reading order. For a range CFI (`parent,start,end`) it is the parent followed by the start. */
export function cfiPath(cfi: string): number[] {
  const inner = cfi
    .trim()
    .replace(/^epubcfi\(/, '')
    .replace(/\)$/, '')
  const [parent = '', start = ''] = inner.split(',')
  return (parent + start)
    .replace(/\[[^\]]*\]/g, '') // assertions such as [chapter3] or [;s=b]
    .split(/[/:!]/)
    .filter((x) => /^\d+$/.test(x))
    .map(Number)
}

/** -1, 0 or 1: which of two CFIs comes first in the book. */
export function compareCfi(a: string, b: string): number {
  const x = cfiPath(a)
  const y = cfiPath(b)
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    if (x[i] === undefined) return -1
    if (y[i] === undefined) return 1
    if (x[i] !== y[i]) return x[i] < y[i] ? -1 : 1
  }
  return 0
}

export interface NewEpubHighlight {
  cfi: string
  chapter: string
  color: Color
  quote: string
  context: string
  uid: string
}

/** The text of the block for a new EPUB highlight. */
export function epubHighlightBlock(h: NewEpubHighlight): string {
  return [
    `${escapeQuote(h.quote)} ^${h.uid}`,
    `hl-cfi:: ${h.cfi}`,
    ...(h.chapter ? [`hl-chapter:: ${h.chapter}`] : []),
    `hl-color:: ${h.color}`,
    `hl-context:: ${h.context}`,
  ].join('\n')
}

export interface NewTextHighlight {
  page: number
  color: Color
  from: number
  to: number
  quote: string
  context: string
  uid: string
}

/** The text of the block for a new text highlight (first line with the ID, then the properties). */
export function textHighlightBlock(h: NewTextHighlight): string {
  return [
    `${escapeQuote(h.quote)} ^${h.uid}`,
    `hl-page:: ${h.page}`,
    `hl-color:: ${h.color}`,
    `hl-from:: ${h.from}`,
    `hl-to:: ${h.to}`,
    `hl-context:: ${h.context}`,
  ].join('\n')
}

export interface NewAreaHighlight {
  page: number
  color: Color
  rect: Rect
  /** The picture, as the sidecar page links it. */
  image: string
  uid: string
}

export function areaHighlightBlock(h: NewAreaHighlight): string {
  const r = h.rect.map((x) => x.toFixed(4)).join(',')
  return [
    `![area](${h.image}) ^${h.uid}`,
    `hl-page:: ${h.page}`,
    `hl-color:: ${h.color}`,
    `hl-type:: area`,
    `hl-rect:: ${r}`,
  ].join('\n')
}

/** The op that appends a highlight block to the sidecar page. */
export function appendBlock(view: PageView | null, text: string): BlockOp {
  return { op: 'insert', parent: [], index: view?.blocks.length ?? 0, text }
}

/** A link to an EPUB of the graph: `../assets/x/book.epub`, with an optional `#epubcfi(…)`. */
export function parseEpubHref(href: string): { path: string; cfi: string | null } | null {
  const m = /^((?:\.\.?\/)*)(assets\/[^#?]*\.epub)(?:#(.*))?$/i.exec(href.trim())
  if (!m) return null
  let path = m[2]
  try {
    path = decodeURIComponent(path)
  } catch {
    // an address that is not percent-encoded: use it as it is
  }
  if (path.split('/').some((s) => s === '..' || s === '.')) return null
  let frag = m[3] ?? ''
  try {
    frag = decodeURIComponent(frag)
  } catch {
    // as written
  }
  const cfi = /^epubcfi\(.+\)$/.test(frag) ? frag : null
  return { path, cfi }
}

/** A link to a PDF of the graph: `../assets/x/report.pdf`, `assets/report.pdf#page=12`. */
export function parsePdfHref(href: string): { path: string; page: number | null } | null {
  const m = /^((?:\.\.?\/)*)(assets\/[^#?]*\.pdf)(?:#(.*))?$/i.exec(href.trim())
  if (!m) return null
  let path = m[2]
  try {
    path = decodeURIComponent(path)
  } catch {
    // an address that is not percent-encoded: use it as it is
  }
  if (path.split('/').some((s) => s === '..' || s === '.')) return null
  const page = /(?:^|&)page=(\d+)/.exec(m[3] ?? '')
  return { path, page: page ? Math.max(1, Number(page[1])) : null }
}
