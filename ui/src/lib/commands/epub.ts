// The EPUB reader (EA-1): `openBook` shows the full-screen reader and waits until it is closed.
import type { ResolvedDoc } from '../annotate/remote'
import { finishPdf } from './pdf'
import { app } from '../stores/app.svelte'
import { leaveDocumentTab, opensDocked, showDocumentTab } from '../stores/dock.svelte'

export interface BookOpen {
  /** Graph-relative path of the EPUB, `assets/…/name.epub`. */
  path: string
  /** The document is shown in the side panel, beside the notes, and not full screen (DD-1). */
  docked?: boolean
  /** The document is not in the graph: `path` is its address (`https://…/f/12345`, `file:///…`) and this is what the
   * backend said about it (the viewer asks again when it is absent). */
  doc?: ResolvedDoc
  /** A place to open at: an EPUB CFI. */
  cfi?: string
  /** A highlight to open at and flash: its block `^id`. */
  highlight?: string
}

let resolver: (() => void) | null = null

/** Opens the reader; resolves when it is closed. */
export function openBook(open: BookOpen): Promise<void> {
  resolver?.() // a reader that is still open is replaced
  // one document at a time: another one that is open is closed, and a docked one is replaced in the panel (DD-4, DD-8)
  const docked = opensDocked() // before the other one is closed: it may be the docked one
  if (app.pdf) finishPdf()
  app.book = { ...open, docked }
  if (docked) showDocumentTab()
  return new Promise((resolve) => (resolver = resolve))
}

/** Called by the reader when it closes. */
export function finishBook(): void {
  app.book = null
  leaveDocumentTab()
  const done = resolver
  resolver = null
  done?.()
}
