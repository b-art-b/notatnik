// The drawing editor (DR-1, DR-8..): `openDrawing` shows the full-screen dialog and waits until it is closed.
import { app } from '../stores/app.svelte'
import { fileLink } from '../filelinks'

/** How a drawing is written in the note: a picture (`![name](…)`) or a link that opens the editor (`[name](…)`). */
export type DrawingMode = 'picture' | 'link'

/** What a block offers a drawing in it: rewrite or remove its reference (`Block.svelte` provides it, `Inlines`
 * reads it). Without one (a table cell, an embed) a drawing can only be redrawn. */
export interface DrawingHost {
  /** The page the block belongs to. */
  page: string
  /** Rewrites the reference to the drawing in the block's text: `build` makes the Markdown from the target as the
   * block writes it (`../assets/drawings/…`); `null` removes the reference. */
  replace(path: string, build: ((href: string) => string) | null): Promise<void>
}
export const DRAWING_HOST = Symbol('drawing-host')

export interface DrawingOpen {
  /** The page a new drawing is linked from (the link is written relative to it); null for an existing one. */
  page: string | null
  /** The drawing to edit; null: a new one. */
  path: string | null
  /** The name and form it has now. */
  name: string
  mode: DrawingMode
  /** The block it is in; without it name, form and delete cannot be changed. */
  host: DrawingHost | null
}

export interface DrawingResult {
  /** Graph-relative path of the stored SVG, `assets/drawings/draw-<8 hex>.svg`. */
  path: string
  /** The Markdown to write into a block (a new drawing); for an existing one the block was rewritten already. */
  markdown: string
  /** The drawing was deleted. */
  deleted?: boolean
}

let resolver: ((r: DrawingResult | null) => void) | null = null

/** Opens the editor and resolves with what was saved, or `null` when it was closed without saving. */
export function openDrawing(open: DrawingOpen): Promise<DrawingResult | null> {
  resolver?.(null) // an editor that is still open is replaced
  app.drawing = open
  return new Promise((resolve) => (resolver = resolve))
}

/** Called by the dialog when it closes. */
export function finishDrawing(r: DrawingResult | null): void {
  app.drawing = null
  if (r && !r.deleted) app.drawingStamp[r.path] = Date.now()
  const done = resolver
  resolver = null
  done?.(r)
}

/** A drawing of this app in the text of a block: the path as a link writes it, or null for any other image. */
export function drawingPath(src: string): string | null {
  const m = /(?:^|\/)(assets\/drawings\/draw-[0-9a-f]{8}\.svg)$/.exec(src)
  return m ? m[1] : null
}

/** The Markdown of a drawing: `![name](href)` as a picture, `[name](href)` as a link (named `Drawing` without a name). */
export function drawingMarkdown(href: string, name: string, mode: DrawingMode): string {
  const label = name.trim()
  return mode === 'picture' ? `!${fileLink(label, href)}` : fileLink(label || 'Drawing', href)
}

/** `text` with the reference to the drawing `path` (an image or a link, whatever its name and however the path is
 * written) rewritten: `build` gets the target as it is written there, `../assets/drawings/…`, and returns the new
 * Markdown. With `null` the reference goes, with the blank before it. */
export function replaceDrawingRef(
  text: string,
  path: string,
  build: ((href: string) => string) | null,
): string {
  const target = path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const ref = `!?\\[(?:\\\\.|[^\\]\\\\])*\\]\\(<?((?:\\.\\.?/)*${target})>?\\)`
  const re = new RegExp(`([ \\t]*)${ref}`)
  const next = text.replace(re, (_m, blank: string, href: string) =>
    build === null ? '' : blank + build(href),
  )
  return build === null ? next.replace(/^[ \t]+|[ \t]+$/g, '') : next
}
