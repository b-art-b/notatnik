import type { CalendarSyncReport } from './api'

/** One sentence for what a calendar sync did. */
export function describeCalendarSync(r: CalendarSyncReport): string {
  const parts: string[] = []
  if (r.imported) parts.push(`${r.imported} copied into your notes`)
  if (r.pulled) parts.push(`${r.pulled} changed from the calendar`)
  if (r.removed_in_calendar)
    parts.push(`${r.removed_in_calendar} deleted in the calendar (kept in your notes)`)
  if (r.created) parts.push(`${r.created} sent`)
  if (r.updated) parts.push(`${r.updated} updated`)
  if (r.retired) parts.push(`${r.retired} marked done (task gone)`)
  if (r.deleted) parts.push(`${r.deleted} removed (task gone)`)
  if (r.ids_added) parts.push(`${r.ids_added} block IDs added`)
  if (r.conflicts.length) parts.push(`${r.conflicts.length} changed elsewhere, not overwritten`)
  if (r.logged) parts.push(`${r.logged} in the log`)
  if (!parts.length) return `Calendar is up to date (${r.unchanged} tasks)`
  return `Calendar: ${parts.join(', ')}`
}
