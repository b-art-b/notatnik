// The calendar sync of the active graph that runs by itself (CA-7; `CalendarStore`).
import { active } from './active.svelte'

export { AFTER_CHANGE_MS, EVERY_MS, pauseAfter } from './calendar.store.svelte'

export const calendarAuto = {
  get state() {
    return active().calendar.state
  },
  set state(v: 'off' | 'idle' | 'syncing' | 'error' | 'offline' | 'paused') {
    active().calendar.state = v
  },
  get last() {
    return active().calendar.last
  },
  set last(v: string) {
    active().calendar.last = v
  },
  get error() {
    return active().calendar.error
  },
  set error(v: string) {
    active().calendar.error = v
  },
}

export const scheduleCalendarSync = (ms: number) => active().calendar.schedule(ms)
export const stopCalendarSync = () => active().calendar.stop()
export const runCalendarSync = (quiet = true) => active().calendar.run(quiet)
