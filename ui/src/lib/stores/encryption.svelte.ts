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

/** Whether the file `file` (a path relative to the graph) is stored encrypted on the server, given the marked
 * `paths` (`encryption.paths` of the config): a path ending in `/` is a folder with everything below it, any other
 * path is one file. The same rule as `notatnik_core::encrypted`, for the cases a page can be in; nothing below
 * `.notatnik/` is ever encrypted, and a conflict copy follows its original. */
export function pathIsEncrypted(paths: string[], file: string): boolean {
  if (file === '.notatnik' || file.startsWith('.notatnik/')) return false
  const hit = (f: string) => paths.some((p) => (p.endsWith('/') ? f.startsWith(p) : p === f))
  if (hit(file)) return true
  const copy = /^(.*) \(conflicted copy \d{4}[^)]*\)(.*)$/.exec(file)
  return !!copy && hit(copy[1] + copy[2])
}

/** The marked paths that cover `file`, for telling the person which rule makes a page encrypted. */
export function coveringPaths(paths: string[], file: string): string[] {
  return paths.filter((p) => pathIsEncrypted([p], file))
}
