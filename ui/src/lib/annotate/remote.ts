// Documents that are not in the graph (NA-1..NA-8, LV-1..LV-8): a PDF or EPUB on the graph's Nextcloud, or a file of this
// computer, opens in the viewer where it is. The backend checks the link, keeps a copy on this device (Nextcloud) or
// registers the file (this computer) and says where to fetch it; nothing is put in the graph unless the person asks.
// This module also decides, for every link to a PDF or an EPUB, whether a click opens the app's viewer or the system's
// program (LV-2).
import { backend, type LocalOpened, type RemoteOpened } from '../api'
import { openBook } from '../commands/epub'
import { openPdf } from '../commands/pdf'
import { parseEpubHref, parsePdfHref } from '../pdf/highlights'
import { app, errorMessage, toast } from '../stores/app.svelte'
import { CommandError } from '../api'

/** A document that is not in the graph, as the viewers need it. */
export interface ResolvedDoc {
  /** On the graph's Nextcloud, or on this computer. */
  where: 'nextcloud' | 'computer'
  kind: 'pdf' | 'epub'
  /** The file's name. */
  name: string
  /** The address that identifies it: the sidecar's `file`. */
  address: string
  /** For `backend.assetUrl`, `annotation_page` and the system's program: `cache/remote/…` or `local/…`. */
  path: string
  /** The server could not be reached; this is the copy of an earlier open. */
  offline: boolean
  /** When the copy was fetched, `YYYY-MM-DD` (Nextcloud). */
  fetched: string
}

export const fromRemote = (r: RemoteOpened): ResolvedDoc => ({
  where: 'nextcloud',
  kind: r.kind === 'epub' ? 'epub' : 'pdf',
  name: r.name,
  address: r.address,
  path: r.path,
  offline: r.offline,
  fetched: r.fetched,
})

export const fromLocal = (l: LocalOpened): ResolvedDoc => ({
  where: 'computer',
  kind: l.kind === 'epub' ? 'epub' : 'pdf',
  name: l.name,
  address: l.address,
  path: l.path,
  offline: false,
  fetched: '',
})

/** The `path` of a viewer target is the address of a document that is not in the graph (not a path of the graph). */
export const isExternalDoc = (path: string): boolean => /^(?:https?|file):\/\//i.test(path)

/** A `file:` address of a PDF or an EPUB (a `#page=3` or a query may follow). */
export const isLocalDocLink = (href: string): boolean =>
  /^file:\/\/[^#?]*\.(?:pdf|epub)(?:[#?]|$)/i.test(href)

/** An address that may be a document of the graph's server: `<server>/f/<number>` or a WebDAV path of an account. The
 * backend decides whether it is the graph's server; this only keeps ordinary web links from asking. */
export const looksLikeServerDocument = (href: string): boolean =>
  /^https?:\/\/[^/]+\/(?:f\/\d+(?:[/?#]|$)|remote\.php\/dav\/files\/)/i.test(href)

/** Asks the backend for a document by its address (the viewers call this when they open without an answer in hand). */
export async function resolveDocument(address: string, pre?: ResolvedDoc): Promise<ResolvedDoc> {
  if (pre) return pre
  if (/^file:/i.test(address)) return fromLocal(await backend.call('local_open', { url: address }))
  const r = await backend.call('remote_open', { url: address })
  if (!r.opened) throw new Error('This is not a document the app can open.')
  return fromRemote(r)
}

type Where = { page?: number; highlight?: string; cfi?: string }

function show(href: string, doc: ResolvedDoc, where: Where) {
  if (doc.kind === 'pdf') void openPdf({ path: href, doc, ...where })
  else void openBook({ path: href, doc, ...where })
}

/** Opens `href` in the viewer when it is a PDF or an EPUB of the graph's server, else in the browser as before. */
export async function openServerLink(href: string, where: Where = {}): Promise<void> {
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
  show(href, fromRemote(r), where)
}

/** Opens a `file:///` PDF or EPUB in the viewer. Where the viewer cannot read files (a browser, a phone) or the file
 * is not there, the system's program is offered or used. */
export async function openLocalLink(href: string, where: Where = {}): Promise<void> {
  let l: LocalOpened
  try {
    l = await backend.call('local_open', { url: href })
  } catch (e) {
    if (e instanceof CommandError && e.code === 'unsupported') {
      backend.openExternal(href)
      toast("Opened in the system's program: the viewer for files of this computer is in the desktop app.")
      return
    }
    toast(errorMessage(e), 'error', {
      label: "Open in the system's program",
      run: () => backend.openExternal(href),
    })
    return
  }
  show(href, fromLocal(l), where)
}

// ------------------------------------------------------------------ viewer or system program (LV-2, LV-3)

type Choice = 'viewer' | 'system'
const SETTING_KEY = 'notatnik.documentLinks'

/** What a plain click on a link to a PDF or an EPUB opens (this device): the app's viewer, or the system's program. */
export function documentLinkSetting(): Choice {
  try {
    return localStorage.getItem(SETTING_KEY) === 'system' ? 'system' : 'viewer'
  } catch {
    return 'viewer'
  }
}

export function setDocumentLinkSetting(c: Choice): void {
  try {
    localStorage.setItem(SETTING_KEY, c)
  } catch {
    // a private window may refuse; the choice then lasts until the page is closed
    memory = c
  }
}
let memory: Choice | null = null

const choice = (): Choice => memory ?? documentLinkSetting()

/** A link to a PDF or an EPUB: one in the graph, or a file of this computer. Their ↗ and modifiers (LV-2). */
export const isDocumentFileLink = (href: string): boolean =>
  !!(parsePdfHref(href) || parseEpubHref(href) || isLocalDocLink(href))

/** Any link `openDocumentLink` takes: the above and a link that may be a document of the server. */
export const isDocumentLink = (href: string): boolean =>
  isDocumentFileLink(href) || looksLikeServerDocument(href)

/** The system's program, not the viewer, is wanted: a modifier (Ctrl, Cmd, the middle button) turns the setting round. */
export function wantsSystem(e?: { ctrlKey?: boolean; metaKey?: boolean; button?: number }): boolean {
  const modifier = !!e && (!!e.ctrlKey || !!e.metaKey || e.button === 1)
  return choice() === 'system' ? !modifier : modifier
}

/** A click on a link to a PDF or an EPUB. */
export function openDocumentLink(
  href: string,
  e?: { ctrlKey?: boolean; metaKey?: boolean; button?: number },
): void {
  const system = wantsSystem(e)
  const pdf = parsePdfHref(href)
  const book = parseEpubHref(href)
  if (pdf) {
    if (system) backend.openAsset(href)
    else void openPdf({ path: pdf.path, page: pdf.page ?? undefined })
  } else if (book) {
    if (system) backend.openAsset(href)
    else void openBook({ path: book.path, cfi: book.cfi ?? undefined })
  } else if (isLocalDocLink(href)) {
    const page = /#page=(\d+)/i.exec(href)
    if (system) backend.openExternal(href)
    else void openLocalLink(href, { page: page ? Number(page[1]) : undefined })
  } else if (system) {
    backend.openExternal(href) // a link that may be a document of the server: the browser
  } else {
    void openServerLink(href)
  }
}

/** The system's program for the document that is open in a viewer: its file where it is (a file of the graph, of
 * this computer) or the copy on this device (Nextcloud). */
export function openInSystem(assetPath: string): void {
  backend.openAsset(assetPath)
}

/** The page a kept copy is linked from in the answer: the one that is open, else the inbox. */
export function pageForLink(): string {
  return app.route.kind === 'page' ? app.route.name : 'inbox'
}

/** _Keep a copy in the graph_ (NA-6, LV-6). Returns the path of the copy, or `null` when it failed (a toast says why). */
export async function keepCopy(doc: ResolvedDoc): Promise<string | null> {
  try {
    const kept = await backend.call('keep_document', {
      path: doc.path,
      address: doc.address,
      page: pageForLink(),
    })
    toast(`A copy is kept in the graph: ${kept.path}`)
    return kept.path
  } catch (e) {
    toast(errorMessage(e), 'error')
    return null
  }
}
