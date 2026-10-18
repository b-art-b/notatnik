// Where tasks are synced to (CA-4): read by the task rows to show the calendar of a task and to offer the others.
// Settings → Calendar fills it whenever it loads or saves; the app fills it when a graph is opened.
import { backend, type CalendarStatus, type TaskCalendarView } from '../api'

export const calendarTargets = $state({
  /** Calendar sync is on and has a calendar for tasks. */
  on: false,
  /** Open tasks without a date are sent too. */
  undated: false,
  /** The calendars a task can go to, the default first. */
  calendars: [] as TaskCalendarView[],
})

/** Takes over what the calendar settings say. */
export function setCalendarTargets(st: CalendarStatus | null) {
  calendarTargets.on = !!st && st.enabled && !!st.tasks
  calendarTargets.undated = !!st && st.undated_tasks
  calendarTargets.calendars = st ? st.task_calendars : []
}

/** Asks the backend (a graph was opened). */
export async function refreshCalendarTargets() {
  try {
    setCalendarTargets(await backend.call('calendar_status', {}))
  } catch {
    setCalendarTargets(null)
  }
}

/** The calendar a task goes to: the one its `cal::` value names, else the default. `null`: not synced (`no`). */
export function calendarOf(cal: string | undefined): TaskCalendarView | null | undefined {
  const v = cal?.trim().toLowerCase()
  if (v === 'no') return null
  return (
    calendarTargets.calendars.find((c) => v && c.name.toLowerCase() === v) ??
    calendarTargets.calendars.find((c) => c.default)
  )
}
