// Calendar sync from the UI (Settings → Calendar and the "Sync tasks to calendar" command).
import { backend, type CalendarSyncReport } from './api'
import { errorMessage, toast } from './stores/app.svelte'

/** One sentence for what a calendar sync did. */
export function describeCalendarSync(r: CalendarSyncReport): string {
  const parts: string[] = []
  if (r.created) parts.push(`${r.created} sent`)
  if (r.updated) parts.push(`${r.updated} updated`)
  if (r.retired) parts.push(`${r.retired} marked done (task gone)`)
  if (r.deleted) parts.push(`${r.deleted} removed (task gone)`)
  if (r.ids_added) parts.push(`${r.ids_added} block IDs added`)
  if (r.conflicts.length) parts.push(`${r.conflicts.length} changed elsewhere, not overwritten`)
  if (!parts.length) return `Calendar is up to date (${r.unchanged} tasks)`
  return `Calendar: ${parts.join(', ')}`
}

/** Sends the dated tasks to the calendar and tells the result; returns the report, or null on failure. */
export async function calendarSyncNow(): Promise<CalendarSyncReport | null> {
  try {
    const r = await backend.call('calendar_sync', {})
    toast(describeCalendarSync(r), 'info')
    return r
  } catch (e) {
    toast(errorMessage(e), 'error')
    return null
  }
}
