// Hiding pages (NS-20, NS-27) from a page, a list or the selection of All pages: one backend call that marks or
// unmarks the pages, a refresh of what depends on it, and a toast with Undo.
import { backend, type HideResult } from './api'
import { active } from './stores/active.svelte'
import { errorMessage, toast } from './stores/app.svelte'
import { isArchivedItself } from './stores/archived.svelte'
import { openDocs } from './stores/pages.svelte'

const what = (names: string[]) => (names.length === 1 ? `“${names[0]}”` : `${names.length} pages`)

/** Hides or unhides pages (each with everything below it) and tells the person what happened. A page that stays hidden
 * because a folder above it is hidden is named in the message. Returns the pages that changed. */
export async function hidePages(names: string[], hidden: boolean): Promise<string[]> {
  let r: HideResult
  try {
    r = await backend.call('hide_pages', { pages: names, hidden })
  } catch (e) {
    toast(errorMessage(e), 'error')
    return []
  }
  // the lists and the open pages (the front matter changed) are brought up to date at once
  await active().archived.refresh()
  for (const d of openDocs()) if (r.changed.includes(d.name)) void d.load()
  if (r.inherited.length) {
    const i = r.inherited[0]
    toast(`“${i.name}” is hidden because “${i.by}” is hidden: unhide that one.`, 'info')
  }
  if (r.changed.length) {
    toast(`${hidden ? 'Hid' : 'Unhid'} ${what(r.changed)}`, 'info', {
      label: 'Undo',
      run: async () => {
        try {
          await backend.call('hide_pages', { pages: r.changed, hidden: !hidden })
          await active().archived.refresh()
          for (const d of openDocs()) if (r.changed.includes(d.name)) void d.load()
        } catch (e) {
          toast(errorMessage(e), 'error')
        }
      },
    })
  } else if (hidden && !r.inherited.length) {
    toast('Hidden already.', 'info')
  }
  return r.changed
}

/** Hides the page, or unhides a page that carries the mark itself. */
export async function toggleHide(name: string): Promise<void> {
  await hidePages([name], !isArchivedItself(name))
}
