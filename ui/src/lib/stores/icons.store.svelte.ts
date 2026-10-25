// Page icons: an emoji per page, from the graph's rules (`type: person` -> 👤) or the page's own `icon:` property.
// Loaded once and refreshed whenever pages change.
import type { Backend } from '../api'

export class IconsStore {
  map = $state(new Map<string, string>())

  constructor(private readonly backend: Backend) {}

  async load(): Promise<void> {
    try {
      const rows = await this.backend.call('page_icons', {})
      this.map = new Map(rows.map((r) => [r.name.toLowerCase(), r.icon]))
    } catch {
      /* no graph yet: no icons */
    }
  }

  /** Whether any page of the graph has an icon: a list then keeps room for it on rows that have none. */
  any(): boolean {
    return this.map.size > 0
  }

  /** The icon of a page, or an empty string. */
  of(name: string): string {
    return this.map.get(name.toLowerCase()) ?? ''
  }
}
