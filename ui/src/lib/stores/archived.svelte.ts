// Archived pages of the active graph (`ArchivedStore`): a page with `archived: true` in its frontmatter, and every page
// below it, is hidden from the sidebar, Recent and All pages (unless asked for) and its tasks stay off the agenda.
import { active } from './active.svelte'

export const archived = {
  get names() {
    return active().archived.names
  },
}

export const refreshArchived = () => active().archived.refresh()
export const isArchived = (name: string) => active().archived.is(name)
export const isArchivedItself = (name: string) => active().archived.itself(name)
