// Where tasks of the active graph are synced to (CA-4; `CalendarStore`): read by the task rows.
import type { CalendarStatus, TaskCalendarView } from '../api'
import { active } from './active.svelte'

export const calendarTargets = {
  get on() {
    return active().calendar.on
  },
  get undated() {
    return active().calendar.undated
  },
  get calendars() {
    return active().calendar.calendars
  },
  get names() {
    return active().calendar.names
  },
  get colors() {
    return active().calendar.colors
  },
}

/** Takes over what the calendar settings say. */
export const setCalendarTargets = (st: CalendarStatus | null) => active().calendar.setTargets(st)

/** Asks the backend (a graph was opened). */
export const refreshCalendarTargets = () => active().calendar.refreshTargets()

/** The calendar a task goes to: the one its `cal::` value names, else the default. `null`: not synced (`no`). */
export const calendarOf = (cal: string | undefined): TaskCalendarView | null | undefined =>
  active().calendar.calendarOf(cal)
