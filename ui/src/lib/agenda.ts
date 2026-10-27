// Sorting and grouping of agenda items inside a section (the same keys the query views offer).
import type { AgendaItem } from './api'

const STATE_RANK: Record<string, number> = {
  DOING: 0,
  NOW: 0,
  TODO: 1,
  LATER: 1,
  WAITING: 2,
  DONE: 3,
  CANCELED: 4,
}

/** The `<field> <asc|desc>` values the Sort select offers; `''` keeps the agenda's own order (by date). */
export const SORTS = [
  { value: '', label: 'date' },
  { value: 'deadline asc', label: 'deadline' },
  { value: 'scheduled asc', label: 'scheduled' },
  { value: 'priority desc', label: 'priority' },
  { value: 'page asc', label: 'page' },
  { value: 'task asc', label: 'state' },
] as const

/** The Group select: `''` keeps the items as they are. */
export const GROUPS = [
  { value: '', label: 'none' },
  { value: 'page', label: 'page' },
  { value: 'task', label: 'state' },
  { value: 'deadline', label: 'deadline' },
  { value: 'scheduled', label: 'scheduled' },
] as const

/** The sort key of an item; items without a value go last in either direction. */
function sortKey(it: AgendaItem, field: string): string | number | null {
  const t = it.block.task
  switch (field) {
    case 'deadline':
      return t?.deadline ?? null
    case 'scheduled':
      return t?.scheduled ?? null
    case 'priority':
      return t?.priority_rank ?? null
    case 'page':
      return it.page.toLowerCase()
    case 'task':
      return t ? (STATE_RANK[t.state] ?? 9) : 9
    default:
      return null
  }
}

/** A stable sort; `sort` is `"<field> asc|desc"`. Priority "desc" means high first, like the query's. */
export function sortItems(items: AgendaItem[], sort: string): AgendaItem[] {
  const [field, dir] = sort.split(' ')
  // priority ranks are small numbers where 0 is the most important, so "desc" (most important first) is ascending rank
  const sign = (dir === 'desc') !== (field === 'priority') ? -1 : 1
  return items
    .map((it, i) => ({ it, i, k: sortKey(it, field) }))
    .sort((a, b) => {
      if (a.k === null || b.k === null) {
        if (a.k === b.k) return a.i - b.i
        return a.k === null ? 1 : -1
      }
      if (a.k < b.k) return -sign
      if (a.k > b.k) return sign
      return a.i - b.i
    })
    .map((x) => x.it)
}

export interface ItemGroup {
  key: string
  label: string
  items: AgendaItem[]
}

/** Splits items by a key, groups in first-seen order (so a sort applies to the groups too). `''` is one group. */
export function groupItems(items: AgendaItem[], group: string): ItemGroup[] {
  if (!group) return [{ key: '', label: '', items }]
  const label = (it: AgendaItem): string => {
    const t = it.block.task
    switch (group) {
      case 'page':
        return it.page
      case 'task':
        return t?.state ?? 'no state'
      case 'deadline':
        return t?.deadline ?? 'no deadline'
      case 'scheduled':
        return t?.scheduled ?? 'no date scheduled'
      default:
        return ''
    }
  }
  const out = new Map<string, ItemGroup>()
  for (const it of items) {
    const l = label(it)
    const g = out.get(l) ?? { key: l, label: l, items: [] }
    g.items.push(it)
    out.set(l, g)
  }
  return [...out.values()]
}

/** A block's raw text as plain words: `[[Page]]` → `Page`, `[[Page|label]]` → `label`, `[text](url)` → `text`, `#tag` →
 * `tag`, task marker and bold/italic marks dropped. For places that show a task as one line of text (a tooltip, a chip,
 * the calendar's grid and tray). */
export function plainWords(text: string): string {
  return text
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[\[([^\]|]+)\|([^\]]+)\]\]/g, '$2')
    .replace(/\[\[([^\]]+)\]\]/g, '$1')
    .replace(/^(TODO|LATER|DOING|NOW|WAITING|DONE|CANCELED|\[[ xX]\])\s+/, '')
    .replace(/(^|\s)#(?=[\w[])/g, '$1')
    .replace(/[*_~`]+/g, '')
    .trim()
}

/** Tooltip of the source link: the note, then the parent blocks the task is nested under. */
export function sourceTitle(page: string, breadcrumbs: string[]): string {
  const crumbs = breadcrumbs.map(plainWords).filter(Boolean)
  return crumbs.length ? `${page} › ${crumbs.join(' › ')}` : page
}

/** The parent blocks as plain words (`Notes › Meeting`), for a chip. */
export function crumbText(breadcrumbs: string[]): string {
  return breadcrumbs.map(plainWords).filter(Boolean).join(' › ')
}

/** The source chip's text: a journal day in the device's date format, a page below `relativeTo` without that
 * prefix (`Conference/Example` on the page `Conference` is `Example`), any other page by its full name. */
export function sourceLabel(
  page: string,
  relativeTo = '',
  format: (iso: string) => string = (d) => d,
): string {
  if (/^\d{4}-\d{2}-\d{2}$/.test(page)) return format(page)
  if (relativeTo && page.toLowerCase().startsWith(relativeTo.toLowerCase() + '/'))
    return page.slice(relativeTo.length + 1)
  return page
}
