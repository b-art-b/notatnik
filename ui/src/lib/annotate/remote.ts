// Documents of the graph's Nextcloud (NA-1..NA-8): a link to a PDF or EPUB on the sync server opens in the viewer. The
// backend resolves the link, keeps a copy on this device and says where it is; nothing is put in the graph unless the
// person asks for a copy.
import { backend, type RemoteOpened } from '../api'
import { openBook } from '../commands/epub'
import { openPdf } from '../commands/pdf'
import { app, errorMessage, toast } from '../stores/app.svelte'

/** The `path` of a viewer target is the address of a document on the server (not a path of the graph). */
export const isRemote = (path: string): boolean => /^https?:\/\//i.test(path)

/** An address that may be a document of the graph's server: `<server>/f/<number>` or a WebDAV path of an account. The
 * backend decides whether it is the graph's server; this only keeps ordinary web links from asking. */
export const looksLikeServerDocument = (href: string): boolean =>
  /^https?:\/\/[^/]+\/(?:f\/\d+(?:[/?#]|$)|remote\.php\/dav\/files\/)/i.test(href)

/** Opens `href` in the viewer when it is a PDF or an EPUB of the graph's server, else in the browser as before. */
export async function openServerLink(
  href: string,
  where: { page?: number; highlight?: string; cfi?: string } = {},
): Promise<void> {
  toast('Fetching the document from the server…')
  let r: RemoteOpened
  try {
    r = await backend.call('remote_open', { url: href })
  } catch (e) {
    toast(errorMessage(e), 'error', { label: 'Open in the browser', run: () => backend.openExternal(href) })
    return
  }
  if (!r.opened) {
    backend.openExternal(href)
    return
  }
  if (r.kind === 'pdf') void openPdf({ path: href, remote: r, ...where })
  else void openBook({ path: href, remote: r, ...where })
}

/** The page a kept copy is linked from in the answer: the one that is open, else the inbox. */
export function pageForLink(): string {
  return app.route.kind === 'page' ? app.route.name : 'inbox'
}

/** _Keep a copy in the graph_ (NA-6). Returns the path of the copy, or `null` when it failed (a toast says why). */
export async function keepCopy(info: RemoteOpened): Promise<string | null> {
  try {
    const kept = await backend.call('remote_keep', {
      path: info.path,
      address: info.address,
      page: pageForLink(),
    })
    toast(`A copy is kept in the graph: ${kept.path}`)
    return kept.path
  } catch (e) {
    toast(errorMessage(e), 'error')
    return null
  }
}
