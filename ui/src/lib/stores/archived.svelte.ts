// Archived pages: a page with `archived: true` in its frontmatter, and every page below it, is hidden from the
// sidebar, Recent and All pages (unless asked for) and its tasks stay off the agenda. The file itself is not moved.
import { backend } from '../api'
import { bus } from './events.svelte'

export const archived = $state({ names: [] as string[] })

/** Reloads the list (the sidebar calls this whenever the graph changes). */
export async function refreshArchived(): Promise<void> {
  try {
    archived.names = (await backend.call('archived_pages', {})).items
  } catch {
    /* keep the last list */
  }
}

/** Is the page archived itself or below an archived page? Case-insensitive, like page names. */
export function isArchived(name: string): boolean {
  const n = name.toLowerCase()
  return archived.names.some((a) => {
    const k = a.toLowerCase()
    return n === k || n.startsWith(k + '/')
  })
}

/** Is it the page that carries the property (not just below one)? Only that one can be unarchived there. */
export function isArchivedItself(name: string): boolean {
  return archived.names.some((a) => a.toLowerCase() === name.toLowerCase())
}

// one subscription for the whole app: reload on every graph change
let started = false
export function watchArchived(): void {
  if (started) return
  started = true
  void refreshArchived()
  $effect.root(() => {
    $effect(() => {
      void bus.version
      void refreshArchived()
    })
  })
}
