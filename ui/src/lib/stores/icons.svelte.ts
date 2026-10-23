// Page icons: an emoji per page, from the graph's rules (`type: person` -> 👤) or the page's own `icon:` property.
// Loaded once and refreshed whenever pages change; `iconOf` can be called from any template.
import { backend } from '../api'

export const icons = $state({ map: new Map<string, string>() })

export async function loadIcons(): Promise<void> {
  try {
    const rows = await backend.call('page_icons', {})
    icons.map = new Map(rows.map((r) => [r.name.toLowerCase(), r.icon]))
  } catch {
    /* no graph yet: no icons */
  }
}

/** Whether any page of the graph has an icon: a list then keeps room for it on rows that have none (`PageIcon align`). */
export function anyIcons(): boolean {
  return icons.map.size > 0
}

/** The icon of a page, or an empty string. */
export function iconOf(name: string): string {
  return icons.map.get(name.toLowerCase()) ?? ''
}
