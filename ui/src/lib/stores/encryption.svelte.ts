// Encrypting chosen paths on the graph's server (EN-11 to EN-18): what the backend reports, and the one rule the UI
// needs itself, whether a file is stored encrypted (for the lock mark of a page).
import { backend, type EncryptionStatus } from '../api'
import { app } from './app.svelte'

export const encryption = $state({
  status: null as EncryptionStatus | null,
})

/** Reads the status now. */
export async function refreshEncryption() {
  if (!app.graph) {
    encryption.status = null
    return
  }
  try {
    encryption.status = await backend.call('encryption_status', {})
  } catch {
    /* the graph was closed meanwhile */
  }
}

/** The settings folder and the folder of hidden objects: never encrypted, never a page. */
const RESERVED = /^\.notatnik(-enc)?(\/|$)/

/** The part of the status that decides which files are stored encrypted. */
export type EncryptionRules = Pick<EncryptionStatus, 'scope' | 'paths' | 'except'>

/** The list that belongs to the scope: the marked paths, or with `all` the exceptions. */
const listOf = (rules: EncryptionRules) => (rules.scope === 'all' ? rules.except : rules.paths)

/** Whether the file `file` (a path relative to the graph) is stored encrypted on the server. With the scope `marked`
 * the marked `paths` are; with `all` everything is except the `except` list. In a list a path ending in `/` is a
 * folder with everything below it, any other path is one file. The same rule as `notatnik_core::encrypted`, for the
 * cases a page can be in; nothing below `.notatnik/` is ever encrypted, and a conflict copy follows its original. */
export function pathIsEncrypted(rules: EncryptionRules, file: string): boolean {
  if (RESERVED.test(file)) return false
  return (rules.scope === 'all') !== listed(listOf(rules), file)
}

/** Whether `file` is named by an entry of `list`, or is a conflict copy of a file that is. */
function listed(list: string[], file: string): boolean {
  const hit = (f: string) => list.some((p) => (p.endsWith('/') ? f.startsWith(p) : p === f))
  if (hit(file)) return true
  const copy = /^(.*) \(conflicted copy \d{4}[^)]*\)(.*)$/.exec(file)
  return !!copy && hit(copy[1] + copy[2])
}

/** The entries of the active list that name `file`: the marked paths that make it encrypted, or with `all` the
 * exceptions that keep it plain. For telling the person which rule decides. */
export function coveringPaths(rules: EncryptionRules, file: string): string[] {
  return listOf(rules).filter((p) => listed([p], file))
}

/** The padlock shown beside a page: only where the file differs from the default, so it carries news. With
 * everything encrypted (the default of a new vault) that is an open padlock on a file kept plain; with only the marked
 * paths encrypted it is a closed padlock on an encrypted file. `null`: nothing to show (also without a vault). */
export interface PadlockMark {
  /** An open padlock: the file is stored as plain text on the server. */
  open: boolean
  /** The words for the tooltip and screen readers. */
  title: string
}

export function padlockOf(status: EncryptionStatus | null, file: string): PadlockMark | null {
  if (!status || !file || !status.configured || RESERVED.test(file)) return null // never a page
  const encrypted = pathIsEncrypted(status, file)
  const all = status.scope === 'all'
  if (encrypted === all) return null // the default for this graph
  if (encrypted) return { open: false, title: 'Stored encrypted on the server' }
  const why = coveringPaths(status, file)
  return {
    open: true,
    title: `Stored as plain text on the server${why.length ? ` (kept plain by ${why.join(', ')})` : ''}`,
  }
}
