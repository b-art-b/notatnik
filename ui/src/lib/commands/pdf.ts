// The PDF viewer (PA-1): `openPdf` shows the full-screen view and waits until it is closed.
import type { ResolvedDoc } from '../annotate/remote'
import { finishBook } from './epub'
import { app } from '../stores/app.svelte'
import { leaveDocumentTab, opensDocked, showDocumentTab } from '../stores/dock.svelte'

export interface PdfOpen {
  /** Graph-relative path of the PDF, `assets/…/name.pdf`. */
  path: string
  /** The document is shown in the side panel, beside the notes, and not full screen (DD-1). */
  docked?: boolean
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
  // one document at a time: another one that is open is closed, and a docked one is replaced in the panel (DD-4, DD-8)
  const docked = opensDocked() // before the other one is closed: it may be the docked one
  if (app.book) finishBook()
  app.pdf = { ...open, docked }
  if (docked) showDocumentTab()
  return new Promise((resolve) => (resolver = resolve))
}

/** Called by the viewer when it closes. */
export function finishPdf(): void {
  app.pdf = null
  leaveDocumentTab()
  const done = resolver
  resolver = null
  done?.()
}
