// The PDF viewer (PA-1): `openPdf` shows the full-screen view and waits until it is closed.
import type { ResolvedDoc } from '../annotate/remote'
import { app } from '../stores/app.svelte'

export interface PdfOpen {
  /** Graph-relative path of the PDF, `assets/…/name.pdf`. */
  path: string
  /** The document is not in the graph: `path` is its address (`https://…/f/12345`, `file:///…`) and this is what the
   * backend said about it (the viewer asks again when it is absent). */
  doc?: ResolvedDoc
  /** The page to show first (from 1). */
  page?: number
  /** A highlight to scroll to and flash: its block `^id`. */
  highlight?: string
}

let resolver: (() => void) | null = null

/** Opens the viewer; resolves when it is closed. */
export function openPdf(open: PdfOpen): Promise<void> {
  resolver?.() // a viewer that is still open is replaced
  app.pdf = open
  return new Promise((resolve) => (resolver = resolve))
}

/** Called by the viewer when it closes. */
export function finishPdf(): void {
  app.pdf = null
  const done = resolver
  resolver = null
  done?.()
}
