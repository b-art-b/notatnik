// Clipping a web page (WA-1, WA-2): `openClip` shows the dialog and waits until it is closed.
import { app, toast } from '../stores/app.svelte'
import { getDoc } from '../stores/pages.svelte'
import type { ClippedPage } from '../api'

export interface ClipOpen {
  /** The page the link is written on (the link in the result is relative to its file). */
  page: string
}

let resolver: ((r: ClippedPage | null) => void) | null = null

/** Opens the dialog and resolves with the clip, or `null` when it was closed without one. */
export function openClip(open: ClipOpen): Promise<ClippedPage | null> {
  resolver?.(null) // a dialog that is still open is replaced
  app.clip = open
  return new Promise((resolve) => (resolver = resolve))
}

/** Called by the dialog when it closes. */
export function finishClip(r: ClippedPage | null): void {
  app.clip = null
  const done = resolver
  resolver = null
  done?.(r)
}

/** The first line of `text` when it is one `http` or `https` address (what the clipboard holds after "Copy link"),
 * else `null`. A line with a user name or a password in it is not offered. */
export function webAddress(text: string): string | null {
  const line = text.trim().split(/\s/)[0] ?? ''
  if (!/^https?:\/\//i.test(line)) return null
  try {
    const u = new URL(line)
    return u.username || u.password ? null : line
  } catch {
    return null
  }
}

/** What is written into the block: the link, and where the page came from as a property line below it. */
export function clipText(c: ClippedPage): string {
  return `${c.markdown}\nsource:: ${c.source}`
}

/** The words for the step the backend reports. */
export function stepLabel(step: string, done: number, total: number): string {
  switch (step) {
    case 'fetching':
      return 'Fetching the page…'
    case 'cleaning':
      return 'Cutting out the article…'
    case 'pictures':
      return total > 0 ? `Fetching pictures (${done} of ${total})…` : 'Fetching pictures…'
    case 'writing':
      return 'Saving the book…'
    default:
      return 'Working…'
  }
}

/** The palette command: the clip becomes a new last block of the page that is open. */
export async function clipIntoPage(): Promise<void> {
  if (app.route.kind !== 'page') {
    toast('Open a page first; the link to the clipped page is added to it', 'info')
    return
  }
  const name = app.route.name
  const clip = await openClip({ page: name })
  if (!clip) return
  await getDoc(name).applyWith((view) => [
    { op: 'insert', parent: [], index: view?.blocks.length ?? 0, text: clipText(clip) },
  ])
}
