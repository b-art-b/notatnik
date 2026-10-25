// The calendar side of one graph. Two parts:
//
// Targets (CA-4): where tasks are synced to, read by the task rows to show the calendar of a task and to offer the
// others. Settings → Calendar fills it whenever it loads or saves; the app fills it when a graph is opened.
//
// Auto sync (CA-7): the calendar sync that runs by itself: once when a graph is opened, every few minutes, a few
// seconds after the notes change, and again after a failure with growing pauses. "Sync now" in Settings and the palette
// run the same sync at once. It only runs while the app is open; with calendar sync off it does nothing.
import type { Backend, CalendarStatus, TaskCalendarView } from '../api'
import { describeCalendarSync } from '../calendarDescribe'
import { errorMessage, toast } from './app.svelte'

export const EVERY_MS = 5 * 60_000
export const AFTER_CHANGE_MS = 10_000
const MAX_BACKOFF_MS = 30 * 60_000

/** The pause after `failures` failed syncs in a row: the normal interval, doubled each time, at most 30 minutes. */
export function pauseAfter(failures: number): number {
  return Math.min(EVERY_MS * 2 ** Math.max(0, failures - 1), MAX_BACKOFF_MS)
}

export class CalendarStore {
  // ---- targets
  /** Calendar sync is on and has a calendar for tasks. */
  on = $state(false)
  /** Open tasks without a date are sent too. */
  undated = $state(false)
  /** The calendars a task can go to, the default first. */
  calendars = $state<TaskCalendarView[]>([])
  /** The names of every calendar in use (tasks and events), by `href`. */
  names = $state<Record<string, string>>({})
  /** The colours chosen for calendars (`#rrggbb`), by `href`. */
  colors = $state<Record<string, string>>({})

  // ---- auto sync
  /** `off` (no calendar sync), `paused` (switched off for now), `idle`, `syncing`, `error` or `offline`. */
  state = $state<'off' | 'idle' | 'syncing' | 'error' | 'offline' | 'paused'>('off')
  /** When the last sync worked (ISO time). */
  last = $state('')
  /** What the last failed sync said. */
  error = $state('')
  private failures = 0
  private timer: ReturnType<typeof setTimeout> | undefined
  private dueAt = 0
  private running = false

  constructor(private readonly backend: Backend) {}

  /** Takes over what the calendar settings say. */
  setTargets(st: CalendarStatus | null) {
    this.on = !!st && st.enabled && !!st.tasks
    this.undated = !!st && st.undated_tasks
    this.calendars = st ? st.task_calendars : []
    this.names = st ? st.calendar_names : {}
    this.colors = st ? st.calendar_colors : {}
  }

  /** Asks the backend (a graph was opened). */
  async refreshTargets() {
    try {
      this.setTargets(await this.backend.call('calendar_status', {}))
    } catch {
      this.setTargets(null)
    }
  }

  /** The calendar a task goes to: the one its `cal::` value names, else the default. `null`: not synced (`no`). */
  calendarOf(cal: string | undefined): TaskCalendarView | null | undefined {
    const v = cal?.trim().toLowerCase()
    if (v === 'no') return null
    return (
      this.calendars.find((c) => v && c.name.toLowerCase() === v) ?? this.calendars.find((c) => c.default)
    )
  }

  /** Syncs again in `ms`, unless a sync is already due earlier. */
  schedule(ms: number) {
    const at = Date.now() + ms
    if (this.timer && this.dueAt <= at) return
    clearTimeout(this.timer)
    this.dueAt = at
    this.timer = setTimeout(() => void this.run(), ms)
  }

  /** Stops the timer (graph closed). */
  stop() {
    clearTimeout(this.timer)
    this.timer = undefined
    this.state = 'off'
  }

  /** One sync now; schedules the next. `quiet` keeps the toast away when nothing notable happened. */
  async run(quiet = true): Promise<void> {
    if (this.running) return
    this.running = true
    this.timer = undefined
    try {
      const st = await this.backend.call('calendar_status', {})
      if (!st.enabled || (!st.tasks && !st.read.length)) {
        this.state = 'off'
        return
      }
      if (st.paused) {
        // nothing runs until it is resumed, which starts the timer again
        this.state = 'paused'
        return
      }
      this.state = 'syncing'
      const r = await this.backend.call('calendar_sync', {})
      this.failures = 0
      this.state = 'idle'
      this.error = ''
      this.last = new Date().toISOString()
      const notable = r.pulled || r.imported || r.logged || r.conflicts.length || r.removed_in_calendar
      if (notable || !quiet) toast(describeCalendarSync(r), 'info')
      this.schedule(EVERY_MS)
    } catch (e) {
      this.failures++
      const code = (e as { code?: string })?.code
      this.state = code === 'offline' ? 'offline' : 'error'
      this.error = errorMessage(e)
      // a refused login is not retried until the person has changed something (a new password, another calendar)
      if (code !== 'auth') this.schedule(pauseAfter(this.failures))
    } finally {
      this.running = false
    }
  }
}
