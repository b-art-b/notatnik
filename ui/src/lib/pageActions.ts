// Archive and unarchive a page from a list (All pages): the same property change as the page's own button.
import { isArchivedItself } from './stores/archived.svelte'
import { getDoc } from './stores/pages.svelte'
import { errorMessage, toast } from './stores/app.svelte'

/** Archives the page, or takes the archive mark off a page that carries it; a toast offers Undo. */
export async function toggleArchive(name: string): Promise<void> {
  const was = isArchivedItself(name)
  const change = (on: boolean) =>
    getDoc(name).apply([
      on
        ? { op: 'set_page_property', key: 'archived', value: 'true' }
        : { op: 'remove_page_property', key: 'archived' },
    ])
  try {
    await change(!was)
    toast(`${was ? 'Unarchived' : 'Archived'} “${name}”`, 'info', {
      label: 'Undo',
      run: async () => {
        try {
          await change(was)
        } catch (e) {
          toast(errorMessage(e), 'error')
        }
      },
    })
  } catch (e) {
    toast(errorMessage(e), 'error')
  }
}
