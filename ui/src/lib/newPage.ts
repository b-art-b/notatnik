// Opening a page that may not exist yet ("New page" dialog, palette). A page is a file only once something is
// written, so this creates nothing: it opens the page with its first block ready for typing.
import { backend } from './api'
import { app, errorMessage, navigate } from './stores/app.svelte'

/** Opens `name`. A page that does not exist yet opens with an editor for its first block. Returns an error text
 * when the server refuses the name, `null` otherwise. */
export async function openPage(name: string): Promise<string | null> {
  const n = name.trim().replace(/^\/+|\/+$/g, '')
  if (!n) return 'Type a name'
  try {
    // the server validates the name (and gives its canonical spelling) without creating anything
    const page = await backend.call('get_page', { name: n })
    if (!page.exists) app.draftFor = page.name
    navigate({ kind: 'page', name: page.name, zoom: [] })
    return null
  } catch (e) {
    return errorMessage(e)
  }
}
