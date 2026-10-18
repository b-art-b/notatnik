// The maths of the calendar view in the Agenda (CV-1..CV-8): where a task or an event sits in a day, how
// overlapping ones share the width, and which value a drag writes back. Pure functions, no DOM.
import type { CalendarEventView } from './api'
import { joinValue, splitValue } from './clock'
import { daysBetween } from './views'

/** A task without an end time, and an event without one, fill this many minutes. */
export const DEFAULT_MINUTES = 60
/** What dragging and resizing snaps to. */
export const SNAP_MINUTES = 15
export const DAY_MINUTES = 24 * 60

export type DateField = 'scheduled' | 'deadline'
/** Which date places a task: the earlier of the two (the agenda's rule), or one of them. */
export type DateMode = 'earlier' | DateField

/** `09:30` → 570. */
export function toMinutes(clock: string): number {
  return Number(clock.slice(0, 2)) * 60 + Number(clock.slice(3, 5))
}

/** 570 → `09:30`; the end of the day is written as `23:59` because a time range stays on one day. */
export function toClock(minutes: number): string {
  const m = Math.max(0, Math.min(DAY_MINUTES - 1, Math.round(minutes)))
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
}

/** Rounds to the nearest step. */
export function snap(minutes: number, step = SNAP_MINUTES): number {
  return Math.round(minutes / step) * step
}

/** One thing drawn on one day. */
export interface Segment {
  /** Unique within a day. */
  id: string
  kind: 'task' | 'event'
  title: string
  /** The day this piece is on, `YYYY-MM-DD`. */
  date: string
  /** No hours: it goes in the all-day strip. */
  allDay: boolean
  /** Minutes from midnight; for an all-day piece 0 and 1440. */
  start: number
  end: number
  /** The piece continues on the day before / after (a multi-day event). */
  before: boolean
  after: boolean
  /** A task placed by a deadline time (it ends at that time). */
  due: boolean
}

/** Which of the two date fields places the task: `earlier` takes the earlier date, the scheduled one on a tie. */
export function placingField(
  scheduled: string | null,
  deadline: string | null,
  mode: DateMode,
): { field: DateField; value: string } | null {
  if (mode === 'scheduled') return scheduled ? { field: 'scheduled', value: scheduled } : null
  if (mode === 'deadline') return deadline ? { field: 'deadline', value: deadline } : null
  if (scheduled && deadline)
    return deadline.slice(0, 10) < scheduled.slice(0, 10)
      ? { field: 'deadline', value: deadline }
      : { field: 'scheduled', value: scheduled }
  if (scheduled) return { field: 'scheduled', value: scheduled }
  if (deadline) return { field: 'deadline', value: deadline }
  return null
}

/** Where a task's date value puts it in its day. A scheduled time is the start (one hour when there is no end); a
 * deadline time is the end of a block of one hour before it. */
export function taskSegment(id: string, title: string, value: string, field: DateField): Segment {
  const { date, start, end } = splitValue(value)
  const base = { id, kind: 'task' as const, title, date, before: false, after: false }
  if (!start) return { ...base, allDay: true, start: 0, end: DAY_MINUTES, due: false }
  const s = toMinutes(start)
  if (field === 'deadline') {
    const stop = Math.max(s, 30)
    return { ...base, allDay: false, start: Math.max(0, stop - DEFAULT_MINUTES), end: stop, due: true }
  }
  const stop = end ? toMinutes(end) : Math.min(DAY_MINUTES, s + DEFAULT_MINUTES)
  return { ...base, allDay: false, start: s, end: Math.max(stop, s + 5), due: false }
}

/** Where an open task that is overdue shows when overdue tasks are "rolled over to today": in today's all-day strip,
 * with how many days it is late and the day it was planned for. `null` for a task that is not before `today`. The
 * file is not touched: this is only a way of looking at it. */
export function rolledOver(seg: Segment, today: string): { seg: Segment; from: string; days: number } | null {
  if (seg.kind !== 'task' || seg.date >= today) return null
  return {
    seg: { ...seg, date: today, allDay: true, start: 0, end: DAY_MINUTES, due: false },
    from: seg.date,
    days: daysBetween(seg.date, today),
  }
}

/** The pieces of an event on the days `from` to `to` (both included): a timed event that spans days starts at
 * its time on the first day, fills the days between (as all-day pieces) and ends at its time on the last. */
export function eventSegments(e: CalendarEventView, from: string, to: string): Segment[] {
  const out: Segment[] = []
  const days = daysBetween(e.date, e.end_date)
  for (let i = 0; i <= days; i++) {
    const date = addDays(e.date, i)
    if (date < from || date > to) continue
    const first = i === 0
    const last = i === days
    const base = {
      id: `${e.uid}@${date}`,
      kind: 'event' as const,
      title: e.title,
      date,
      before: !first,
      after: !last,
      due: false,
    }
    if (!e.start) {
      out.push({ ...base, allDay: true, start: 0, end: DAY_MINUTES })
    } else if (days === 0) {
      const s = toMinutes(e.start)
      const stop =
        e.end && toMinutes(e.end) > s ? toMinutes(e.end) : Math.min(DAY_MINUTES, s + DEFAULT_MINUTES)
      out.push({ ...base, allDay: false, start: s, end: stop })
    } else if (first) {
      out.push({ ...base, allDay: false, start: toMinutes(e.start), end: DAY_MINUTES })
    } else if (last && e.end) {
      out.push({ ...base, allDay: false, start: 0, end: Math.max(toMinutes(e.end), 15) })
    } else {
      out.push({ ...base, allDay: true, start: 0, end: DAY_MINUTES })
    }
  }
  return out
}

export function addDays(date: string, n: number): string {
  const [y, m, d] = date.split('-').map(Number)
  return new Date(Date.UTC(y, m - 1, d + n)).toISOString().slice(0, 10)
}

export interface Slot {
  /** Column among those that overlap, from 0. */
  col: number
  /** How many columns the overlapping group has. */
  cols: number
}

/** Side-by-side layout for the pieces of one day: pieces that overlap in time share the width; a piece that
 * ends before another starts gets the full width again. */
export function layoutDay(items: { id: string; start: number; end: number }[]): Map<string, Slot> {
  const sorted = [...items].sort((a, b) => a.start - b.start || b.end - a.end)
  const out = new Map<string, Slot>()
  let group: { id: string; col: number; end: number }[] = []
  let groupEnd = -1
  const flush = () => {
    const cols = group.reduce((n, g) => Math.max(n, g.col + 1), 0)
    for (const g of group) out.set(g.id, { col: g.col, cols })
    group = []
  }
  for (const it of sorted) {
    if (group.length && it.start >= groupEnd) flush()
    // the first column whose last piece is over
    const taken = new Set(group.filter((g) => g.end > it.start).map((g) => g.col))
    let col = 0
    while (taken.has(col)) col++
    group.push({ id: it.id, col, end: it.end })
    groupEnd = Math.max(groupEnd, it.end)
  }
  flush()
  return out
}

/** The date value after moving a task to `date` at `startMinutes`. A time range keeps its length (it is moved
 * up when it would pass midnight); a plain date given a time gets the default length for a scheduled date. */
export function movedValue(value: string, field: DateField, date: string, startMinutes: number): string {
  const { start, end } = splitValue(value)
  const length = start && end ? toMinutes(end) - toMinutes(start) : DEFAULT_MINUTES
  if (field === 'deadline') {
    // a deadline is one time, drawn as the hour that ends with it: the block dropped at `startMinutes` ends an hour later
    return joinValue(date, toClock(startMinutes + DEFAULT_MINUTES), '')
  }
  const s = Math.max(0, Math.min(startMinutes, DAY_MINUTES - 1))
  const keepRange = !!end || !start
  const e = s + length
  if (keepRange && e >= DAY_MINUTES) {
    const shifted = DAY_MINUTES - 1 - length
    return joinValue(date, toClock(Math.max(0, shifted)), toClock(DAY_MINUTES - 1))
  }
  return joinValue(date, toClock(s), keepRange ? toClock(e) : '')
}

/** The date value after dragging the bottom edge of a scheduled task to `endMinutes`; at least 15 minutes. */
export function resizedValue(value: string, endMinutes: number): string | null {
  const { date, start } = splitValue(value)
  if (!start) return null
  const s = toMinutes(start)
  const e = Math.max(s + SNAP_MINUTES, Math.min(DAY_MINUTES - 1, endMinutes))
  return joinValue(date, start, toClock(e))
}

/** The end shown for a task block, as `HH:MM`, for the resize handle. */
export function endClock(seg: Segment): string {
  return toClock(seg.end)
}

/** The text of a task made by clicking a slot: an hour from `start`. */
export function newTaskText(title: string, date: string, startMinutes: number): string {
  const s = Math.max(0, Math.min(startMinutes, DAY_MINUTES - 1))
  const end = s + DEFAULT_MINUTES >= DAY_MINUTES ? DAY_MINUTES - 1 : s + DEFAULT_MINUTES
  return `TODO ${title.trim()}\nscheduled:: ${joinValue(date, toClock(s), toClock(end))}`
}

const PALETTE = ['#4c8bf5', '#e8710a', '#12a150', '#a142f4', '#d93025', '#0097a7', '#b06000', '#c2185b']

/** A calendar's colour: the one chosen for it, else one of the palette picked by its address, always the same. */
export function colorFor(href: string, chosen: Record<string, string>): string {
  const c = chosen[href]
  if (c) return c
  let h = 0
  for (const ch of href) h = (h * 31 + ch.charCodeAt(0)) >>> 0
  return PALETTE[h % PALETTE.length]
}

const MONTHS = [
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December',
]
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']

const parts = (d: string) => {
  const [y, m, day] = d.split('-').map(Number)
  return { y, m: m - 1, day, wd: new Date(Date.UTC(y, m - 1, day)).getUTCDay() }
}
const short = (m: number) => MONTHS[m].slice(0, 3)

/** `Mon`, `Tue` ... for a `YYYY-MM-DD` date. */
export function weekdayShort(date: string): string {
  return WEEKDAYS[parts(date).wd]
}

/** `October 2026`. */
export function monthTitle(date: string): string {
  const p = parts(date)
  return `${MONTHS[p.m]} ${p.y}`
}

/** `19 – 25 Oct 2026`, `19 Oct – 1 Nov 2026`, `Mon 19 Oct 2026`. The names are fixed English ones: the same on every
 * system, whatever the locale makes of "Sep". */
export function rangeLabel(days: string[]): string {
  if (days.length === 1) {
    const p = parts(days[0])
    return `${WEEKDAYS[p.wd]} ${p.day} ${short(p.m)} ${p.y}`
  }
  const a = parts(days[0])
  const b = parts(days[days.length - 1])
  return a.m === b.m && a.y === b.y
    ? `${a.day} – ${b.day} ${short(b.m)} ${b.y}`
    : `${a.day} ${short(a.m)} – ${b.day} ${short(b.m)} ${b.y}`
}
