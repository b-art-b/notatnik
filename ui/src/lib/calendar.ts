// Calendar sync from the UI (Settings → Calendar and the "Sync tasks with the calendar" command).
import { backend, type CalendarStatus, type CalendarSyncReport } from './api'
import { errorMessage, toast } from './stores/app.svelte'
import { calendarAuto, EVERY_MS, scheduleCalendarSync, stopCalendarSync } from './stores/calendarAuto.svelte'

import { describeCalendarSync } from './calendarDescribe'

export { describeCalendarSync }

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
