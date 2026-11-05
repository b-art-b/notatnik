// The EPUB reader (EA-1): `openBook` shows the full-screen reader and waits until it is closed.
import type { RemoteOpened } from '../api'
import { app } from '../stores/app.svelte'

export interface BookOpen {
  /** Graph-relative path of the EPUB, `assets/…/name.epub`. */
  path: string
  /** The document is on the graph's Nextcloud: `path` is its address, and this is what `remote_open` said about it (the
   * viewer asks again when it is absent). */
  remote?: RemoteOpened
  /** A place to open at: an EPUB CFI. */
  cfi?: string
  /** A highlight to open at and flash: its block `^id`. */
  highlight?: string
}

let resolver: (() => void) | null = null

/** Opens the reader; resolves when it is closed. */
export function openBook(open: BookOpen): Promise<void> {
  resolver?.() // a reader that is still open is replaced
  app.book = open
  return new Promise((resolve) => (resolver = resolve))
}

/** Called by the reader when it closes. */
export function finishBook(): void {
  app.book = null
  const done = resolver
  resolver = null
  done?.()
}
