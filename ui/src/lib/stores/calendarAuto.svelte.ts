// The calendar sync that runs by itself (CA-7): once when a graph is opened, every few minutes, a few seconds after
// the notes change, and again after a failure with growing pauses. "Sync now" in Settings and the palette runs
// the same sync at once. It only runs while the app is open; with calendar sync off it does nothing.
import { backend } from '../api'
import { describeCalendarSync } from '../calendar'
import { errorMessage, toast } from './app.svelte'

export const EVERY_MS = 5 * 60_000
export const AFTER_CHANGE_MS = 10_000
const MAX_BACKOFF_MS = 30 * 60_000

export const calendarAuto = $state({
  /** `off` (no calendar sync), `idle`, `syncing`, `error` or `offline`. */
  state: 'off' as 'off' | 'idle' | 'syncing' | 'error' | 'offline',
  /** When the last sync worked (ISO time). */
  last: '',
  /** What the last failed sync said. */
  error: '',
})

/** The pause after `failures` failed syncs in a row: the normal interval, doubled each time, at most 30 minutes. */
export function pauseAfter(failures: number): number {
  return Math.min(EVERY_MS * 2 ** Math.max(0, failures - 1), MAX_BACKOFF_MS)
}

let failures = 0
let timer: ReturnType<typeof setTimeout> | undefined
let dueAt = 0
let running = false

/** Syncs again in `ms`, unless a sync is already due earlier. */
export function scheduleCalendarSync(ms: number) {
  const at = Date.now() + ms
  if (timer && dueAt <= at) return
  clearTimeout(timer)
  dueAt = at
  timer = setTimeout(() => void runCalendarSync(), ms)
}

/** Stops the timer (graph closed). */
export function stopCalendarSync() {
  clearTimeout(timer)
  timer = undefined
  calendarAuto.state = 'off'
}

/** One sync now; schedules the next. `quiet` keeps the toast away when nothing notable happened. */
export async function runCalendarSync(quiet = true): Promise<void> {
  if (running) return
  running = true
  timer = undefined
  try {
    const st = await backend.call('calendar_status', {})
    if (!st.enabled || (!st.tasks && !st.read.length)) {
      calendarAuto.state = 'off'
      return
    }
    calendarAuto.state = 'syncing'
    const r = await backend.call('calendar_sync', {})
    failures = 0
    calendarAuto.state = 'idle'
    calendarAuto.error = ''
    calendarAuto.last = new Date().toISOString()
    const notable = r.pulled || r.imported || r.logged || r.conflicts.length || r.removed_in_calendar
    if (notable || !quiet) toast(describeCalendarSync(r), 'info')
    scheduleCalendarSync(EVERY_MS)
  } catch (e) {
    failures++
    const code = (e as { code?: string })?.code
    calendarAuto.state = code === 'offline' ? 'offline' : 'error'
    calendarAuto.error = errorMessage(e)
    // a refused login is not retried until the person has changed something (a new password, another calendar)
    if (code !== 'auth') scheduleCalendarSync(pauseAfter(failures))
  } finally {
    running = false
  }
}
