// The time part of a scheduled or deadline value (TK-2): `YYYY-MM-DD`, `YYYY-MM-DD HH:MM` or
// `YYYY-MM-DD HH:MM-HH:MM` (24 h, one day). Pure helpers for the date picker and the chips.

/** `9:05` or `09:05` → `09:05`; anything else (or an impossible time) → `null`. */
export function parseClock(s: string): string | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(s.trim())
  if (!m) return null
  const h = Number(m[1])
  const min = Number(m[2])
  if (h > 23 || min > 59) return null
  return `${String(h).padStart(2, '0')}:${String(min).padStart(2, '0')}`
}

export interface Parts {
  date: string
  /** `HH:MM` or `''` */
  start: string
  /** `HH:MM` or `''` */
  end: string
}

/** Splits a value into its date, start and end. Reads a hyphen or an en dash, with or without spaces. */
export function splitValue(value: string): Parts {
  const date = value.slice(0, 10)
  const m = /^[ T](\d{2}:\d{2})(?::\d{2})?(?:\s*[-–]\s*(\d{2}:\d{2})(?::\d{2})?)?$/.exec(value.slice(10))
  return { date, start: m?.[1] ?? '', end: m?.[2] ?? '' }
}

/** The value to write: the date, then ` start` and `-end` when there are any. An end needs a start. */
export function joinValue(date: string, start: string, end: string): string {
  if (!start) return date
  return end ? `${date} ${start}-${end}` : `${date} ${start}`
}

/** `09:00–10:30`, `09:00`, or `null` for a plain date. For chips and indicators. */
export function timeLabel(value: string | null | undefined): string | null {
  if (!value) return null
  const { start, end } = splitValue(value)
  if (!start) return null
  return end ? `${start}–${end}` : start
}

/** `start` plus minutes as `HH:MM`, or `null` when it would pass midnight (a range stays on one day). */
export function addMinutes(start: string, minutes: number): string | null {
  const c = parseClock(start)
  if (!c) return null
  const total = Number(c.slice(0, 2)) * 60 + Number(c.slice(3)) + minutes
  if (total < 0 || total >= 24 * 60) return null
  return `${String(Math.floor(total / 60)).padStart(2, '0')}:${String(total % 60).padStart(2, '0')}`
}

/** What is wrong with the typed times, or `null` when they can be written. Empty `start` means no time at all. */
export function timeProblem(start: string, end: string): string | null {
  if (!start.trim() && !end.trim()) return null
  const s = parseClock(start)
  if (!s) return 'Start time: use 24 h HH:MM, for example 09:30'
  if (!end.trim()) return null
  const e = parseClock(end)
  if (!e) return 'End time: use 24 h HH:MM, for example 10:30'
  if (e <= s) return 'The end must be after the start'
  return null
}
