// Smart paste of links (ED-27): what a pasted address or a copied link becomes in the text of a block. Pure
// functions; the editor decides where the result goes (`Editor.svelte`).
import { fileLink, fileUrl, nameForAddress } from './filelinks'

/** Query parameters that only track where a click came from; they are dropped from pasted web addresses. */
const TRACKING =
  /^(utm_[a-z0-9_]+|fbclid|gclid|dclid|msclkid|yclid|igshid|mc_cid|mc_eid|_hsenc|_hsmi|mkt_tok)$/i

/** A pasted text that is one web, mail or app address and nothing else. */
export function pastedUrl(text: string): string | null {
  const t = text.trim()
  if (/^https?:\/\/[^\s/?#]+\S*$/i.test(t) || /^(mailto:|notatnik:\/\/page\/)[^\s]+$/i.test(t)) return t
  return null
}

/** A pasted text that is one file of this computer: a `file:///…` address, or an absolute path with a file name
 * (`/home/me/My Docs/a.pdf`, `C:\Users\me\a.pdf`). Returns the `file://` address. A path needs a folder and an
 * extension, so a sentence that happens to start with a slash is not taken for one. */
export function pastedFile(text: string): string | null {
  const t = text.trim()
  if (/[\n\r]/.test(t)) return null
  if (/^file:\/\/\/[^\s]+$/i.test(t)) return t
  const unix = /^\/[^/\0]+(?:\/[^/\0]+)*\/[^/\0]*[^/\0\s.]\.[A-Za-z0-9]{1,8}$/.test(t)
  const windows = /^[A-Za-z]:[\\/][^\0<>|"?*]*[^\\/\0\s.]\.[A-Za-z0-9]{1,8}$/.test(t)
  return unix || windows ? fileUrl(t) : null
}

/** The readable short form of a GitHub issue, pull request or commit address, or `null`:
 * `…/o/r/issues/12` and `…/o/r/pull/12` give `o/r#12`, `…/o/r/commit/abcdef1…` gives `o/r@abcdef1`. */
export function githubShort(url: string): string | null {
  let u: URL
  try {
    u = new URL(url)
  } catch {
    return null
  }
  if (u.protocol !== 'https:' || u.hostname.toLowerCase() !== 'github.com') return null
  const m = /^\/([\w.-]+)\/([\w.-]+)\/(issues|pull|commit)\/(\w+)\/?$/.exec(u.pathname)
  if (!m) return null
  const [, owner, repo, kind, id] = m
  if (kind === 'commit') return /^[0-9a-f]{7,40}$/i.test(id) ? `${owner}/${repo}@${id.slice(0, 7)}` : null
  return /^\d+$/.test(id) ? `${owner}/${repo}#${id}` : null
}

/** The address without tracking parameters (`?utm_source=x&id=3` becomes `?id=3`); the rest stays as typed. */
export function cleanUrl(url: string): string {
  if (!/^https?:\/\//i.test(url)) return url
  const hash = url.indexOf('#')
  const base = hash < 0 ? url : url.slice(0, hash)
  const frag = hash < 0 ? '' : url.slice(hash)
  const q = base.indexOf('?')
  if (q < 0) return url
  const kept = base
    .slice(q + 1)
    .split('&')
    .filter((p) => p !== '' && !TRACKING.test(p.split('=')[0]))
  return base.slice(0, q) + (kept.length ? `?${kept.join('&')}` : '') + frag
}

/** What an address of this app points to: `notatnik://page/Plan#abc123` (the link a task has in the calendar). */
export function appTarget(url: string): { page: string; block: string | null } | null {
  const m = /^notatnik:\/\/page\/([^#\s]+)(?:#(\S+))?$/i.exec(url.trim())
  if (!m) return null
  try {
    return { page: decodeURIComponent(m[1]), block: m[2] ?? null }
  } catch {
    return null
  }
}

/** A Nextcloud address of a file in the Files app (`…/apps/files/?dir=/x&fileid=123`, `…/apps/files/files/123`)
 * as the address that keeps working when the file is renamed or moved: `…/f/123`. `null` for anything else. */
export function nextcloudFile(url: string): string | null {
  let u: URL
  try {
    u = new URL(url)
  } catch {
    return null
  }
  if (u.protocol !== 'https:' && u.protocol !== 'http:') return null
  const at = u.pathname.indexOf('/apps/files')
  if (at < 0) return null
  const id =
    u.searchParams.get('fileid') ?? /^\/apps\/files\/files\/(\d+)\/?$/.exec(u.pathname.slice(at))?.[1]
  if (!id || !/^\d+$/.test(id)) return null
  return `${u.origin}${u.pathname.slice(0, at)}/f/${id}`
}

/** A link copied from a web page: the clipboard's HTML holds exactly one `<a>` and nothing but its text. */
export function htmlLink(html: string, plain: string): { text: string; href: string } | null {
  if (!html || typeof DOMParser === 'undefined') return null
  const doc = new DOMParser().parseFromString(html, 'text/html')
  const links = doc.querySelectorAll('a[href]')
  if (links.length !== 1) return null
  const a = links[0]
  const text = (a.textContent ?? '').replace(/\s+/g, ' ').trim()
  const href = a.getAttribute('href') ?? ''
  const all = (doc.body.textContent ?? '').replace(/\s+/g, ' ').trim()
  const copied = plain.replace(/\s+/g, ' ').trim()
  // the copied text is the link's text: a sentence with a link in it is pasted as it is
  if (!text || text !== all || (copied && copied !== text)) return null
  if (!/^(https?:\/\/[^\s/?#]+\S*|mailto:\S+)$/i.test(href)) return null
  return { text, href }
}

/** The cursor is inside the address or the target of a link, a reference or code: nothing is converted there. */
export function insideLink(before: string): boolean {
  return (
    /\]\([^)]*$/.test(before) ||
    /\[\[[^\]]*$/.test(before) ||
    /\(\([^)]*$/.test(before) ||
    /<[^>\s]*$/.test(before) ||
    ((before.match(/`/g) ?? []).length & 1) === 1
  )
}

export interface PasteInput {
  /** `text/plain` of the clipboard. */
  plain: string
  /** `text/html` of the clipboard (may be empty). */
  html: string
  /** The text selected in the block. */
  selected: string
  /** The line up to the cursor. */
  before: string
  /** Look up the title of a lone web address (the device setting). */
  titles: boolean
}

export interface PasteResult {
  /** Replaces the selection. */
  insert: string
  /** The inserted text is this bare address; its title is to be looked up and written as `[title](address)`. */
  fetchTitle?: string
}

/** The link to write for `href` with this text: pages and blocks of the app become `[[Page]]` / `((id))`, a
 * Nextcloud file the stable address, tracking parameters are dropped. `text` empty: no link text known. */
function link(href: string, text: string): { md: string; bare: boolean } {
  const app = appTarget(href)
  if (app) {
    if (app.block && !text) return { md: `((${app.block}))`, bare: false }
    if (!app.block)
      return { md: text && text !== app.page ? `[[${app.page}|${text}]]` : `[[${app.page}]]`, bare: false }
  }
  const cloud = nextcloudFile(href)
  if (cloud) return { md: fileLink(text || 'Nextcloud file', cleanUrl(cloud)), bare: false }
  const clean = cleanUrl(href)
  if (text) return { md: fileLink(text, clean), bare: false }
  // a known site has a short form that needs no request
  const short = githubShort(clean)
  if (short) return { md: fileLink(short, clean), bare: false }
  return { md: clean, bare: true }
}

/** What a paste becomes, or `null` when the editor should paste the text as it is. */
export function smartPaste(i: PasteInput): PasteResult | null {
  if (insideLink(i.before)) return null
  const sel = i.selected
  const oneLineSelection = sel.trim() !== '' && !sel.includes('\n')
  if (sel !== '' && !oneLineSelection) return null

  // a file of this computer: a link named after the file (a selection names it instead)
  const file = pastedFile(i.plain)
  if (file) return { insert: fileLink(oneLineSelection ? sel : nameForAddress(file), file) }

  const url = pastedUrl(i.plain)
  if (url) {
    const text = oneLineSelection ? sel : ''
    const r = link(url, text)
    if (r.bare) {
      // a plain address: pasted as it is unless it changed or its title is wanted
      const wantTitle = i.titles && /^https?:/i.test(r.md)
      if (r.md === url && !wantTitle) return null
      return wantTitle ? { insert: r.md, fetchTitle: r.md } : { insert: r.md }
    }
    return { insert: r.md }
  }

  // a link copied from a page: its text and address
  const h = oneLineSelection ? null : htmlLink(i.html, i.plain)
  if (h) return { insert: link(h.href, h.text).md }
  return null
}

/** The lines of a paste that is nothing but addresses and files, one per line (two or more); else `null`. */
export function pastedAddressList(text: string): string[] | null {
  const lines = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l !== '')
  if (lines.length < 2) return null
  const out: string[] = []
  for (const l of lines) {
    const target = pastedUrl(l) ?? pastedFile(l)
    if (!target) return null
    out.push(target)
  }
  return out
}

/** The Markdown for one address of a list, once its title is known (`title` empty: no title). A file is named
 * after its file name, an address that has a short form gets it. */
export function addressLine(address: string, title: string): string {
  if (/^file:\/\//i.test(address)) return fileLink(nameForAddress(address), address)
  const r = link(address, '')
  return r.bare && title ? fileLink(title, r.md) : r.md
}

/** The address of a list line is a plain web address whose title is worth asking for (not a page of the app, a
 * Nextcloud file or a GitHub issue, which have their own readable form). */
export function titleWanted(address: string): boolean {
  return /^https?:/i.test(address) && link(address, '').bare
}
