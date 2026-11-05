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
  kind: 'text' | 'area'
  /** From 1. */
  page: number
  color: Color
  /** Offsets of the quote in the text of the page (text highlights; 0 for an area). */
  from: number
  to: number
  /** The quote as written in the block (an area: its alt text). */
  quote: string
  context: string
  rect: Rect | null
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

/** The highlights of a sidecar page: its top-level blocks that have `hl-page`, in reading order (page, then place). */
export function highlightsOf(view: PageView | null | undefined): Highlight[] {
  if (!view) return []
  const out: Highlight[] = []
  for (const [i, b] of view.blocks.entries()) {
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
      block: b,
    })
  }
  return out.sort((a, b) => a.page - b.page || (a.rect?.[1] ?? 0) - (b.rect?.[1] ?? 0) || a.from - b.from)
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
