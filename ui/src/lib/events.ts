// Calendar events for display (JR-5, QV-6): read from what the calendar sync kept; never part of the notes.
import { backend, type CalendarEventView } from './api'

/** The events that touch the days `from` to `to` (both `YYYY-MM-DD`); empty when calendar sync is off. */
export async function loadEvents(from: string, to: string): Promise<CalendarEventView[]> {
  try {
    return await backend.call('calendar_events', { from, to })
  } catch {
    return []
  }
}

/** The events that are on `date`, a multi-day event on each of its days. */
export function eventsOn(events: CalendarEventView[], date: string): CalendarEventView[] {
  return events.filter((e) => e.date <= date && e.end_date >= date)
}

/** `09:30–10:30`, `from 09:30`, `until 10:30` or `all day`, as the event is on `date`. */
export function eventTime(e: CalendarEventView, date: string): string {
  if (!e.start) return 'all day'
  const starts = e.date === date
  const ends = e.end_date === date
  if (starts && ends) return e.end && e.end !== e.start ? `${e.start}–${e.end}` : e.start
  if (starts) return `from ${e.start}`
  if (ends) return e.end ? `until ${e.end}` : 'all day'
  return 'all day'
}
