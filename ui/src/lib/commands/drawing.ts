// The drawing editor (DR-1): `openDrawing` shows the full-screen dialog and waits until it is closed.
import { app } from '../stores/app.svelte'

export interface DrawingResult {
  /** Graph-relative path of the stored SVG, `assets/drawings/draw-<8 hex>.svg`. */
  path: string
  /** `![](…)` relative to the page, to write into a block. */
  markdown: string
}

let resolver: ((r: DrawingResult | null) => void) | null = null

/** Opens the editor for a drawing (`path` null: a new one) that is linked from `page` (null: a drawing that is
 * already linked from a page of its own) and resolves with what was saved, or `null`
 * when it was closed without saving. */
export function openDrawing(page: string | null, path: string | null): Promise<DrawingResult | null> {
  resolver?.(null) // an editor that is still open is replaced
  app.drawing = { page, path }
  return new Promise((resolve) => (resolver = resolve))
}

/** Called by the dialog when it closes. */
export function finishDrawing(r: DrawingResult | null): void {
  app.drawing = null
  if (r) app.drawingStamp[r.path] = Date.now()
  const done = resolver
  resolver = null
  done?.(r)
}

/** A drawing of this app in the text of a block: the path as a link writes it, or null for any other image. */
export function drawingPath(src: string): string | null {
  const m = /(?:^|\/)(assets\/drawings\/draw-[0-9a-f]{8}\.svg)$/.exec(src)
  return m ? m[1] : null
}
