// Calendar sync from the UI (Settings → Calendar and the "Sync tasks with the calendar" command).
import { backend, type CalendarStatus, type CalendarSyncReport } from './api'
import { errorMessage, toast } from './stores/app.svelte'
import { calendarAuto, EVERY_MS, scheduleCalendarSync, stopCalendarSync } from './stores/calendarAuto.svelte'

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

/** Sends the dated tasks to the calendar and tells the result; returns the report, or null on failure. */
export async function calendarSyncNow(): Promise<CalendarSyncReport | null> {
  try {
    const r = await backend.call('calendar_sync', {})
    toast(describeCalendarSync(r), 'info')
    calendarAuto.state = 'idle'
    calendarAuto.error = ''
    calendarAuto.last = new Date().toISOString()
    scheduleCalendarSync(EVERY_MS)
    return r
  } catch (e) {
    toast(errorMessage(e), 'error')
    return null
  }
}

/** Pauses the sync of every calendar for now, or resumes it. The choices stay; what happened meanwhile is brought
 * in step at the first sync after the resume. Returns the new settings, or null on failure. */
export async function setCalendarPaused(paused: boolean): Promise<CalendarStatus | null> {
  try {
    const st = await backend.call('calendar_pause', { paused })
    if (paused) {
      stopCalendarSync()
      calendarAuto.state = 'paused'
    } else {
      calendarAuto.state = 'idle'
      scheduleCalendarSync(1500)
    }
    toast(paused ? 'Calendar sync paused' : 'Calendar sync resumed', 'info')
    return st
  } catch (e) {
    toast(errorMessage(e), 'error')
    return null
  }
}

/** The palette command: pause when running, resume when paused. */
export async function togglePauseCalendarSync(): Promise<void> {
  try {
    const st = await backend.call('calendar_status', {})
    if (!st.enabled) return void toast('Calendar sync is not set up (Settings → Calendar)', 'info')
    await setCalendarPaused(!st.paused)
  } catch (e) {
    toast(errorMessage(e), 'error')
  }
}
