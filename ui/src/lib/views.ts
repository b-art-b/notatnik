// Pure logic behind the query views: table sorting, kanban columns, calendar grid, date dragging.
import type { ResultRow } from './api'

export function cellValue(r: ResultRow, col: string): string {
  if (col === 'page') return r.page
  if (col === 'text') return (r.block?.text ?? '').split('\n')[0]
  if (col === 'task') return r.task_state ?? ''
  if (col === 'scheduled') return r.scheduled ?? ''
  if (col === 'deadline') return r.deadline ?? ''
  if (col.startsWith('prop.')) return r.props[col.slice(5)] ?? ''
  return ''
}

/** Sorts by a column. Empty values are always last, numbers numerically, everything else as text. */
export function sortRows(list: ResultRow[], col: string | null, descending: boolean): ResultRow[] {
  if (!col) return list
  return [...list].sort((a, b) => {
    const x = cellValue(a, col)
    const y = cellValue(b, col)
    if (x === '' && y === '') return 0
    if (x === '') return 1
    if (y === '') return -1
    const nx = Number(x)
    const ny = Number(y)
    const c = !Number.isNaN(nx) && !Number.isNaN(ny) ? nx - ny : x.localeCompare(y)
    return descending ? -c : c
  })
}

export const TASK_COLUMNS = ['todo', 'doing', 'waiting', 'done', 'canceled']

/** The kanban column of a row: task state when grouping by task (or not at all), else the group label. */
export function kanbanKey(r: ResultRow, groupBy: string | null): string {
  if (!groupBy || groupBy === 'task') return r.task_state ?? '(none)'
  return r.group ?? '(none)'
}

/** Column order: fixed task states, or the group values in the order they were returned. */
export function kanbanColumns(rows: ResultRow[], groupBy: string | null): string[] {
  if (!groupBy || groupBy === 'task') return TASK_COLUMNS
  const seen: string[] = []
  for (const r of rows) {
    const k = kanbanKey(r, groupBy)
    if (!seen.includes(k)) seen.push(k)
  }
  return seen.includes('(none)') ? [...seen.filter((x) => x !== '(none)'), '(none)'] : seen
}

const DAY = 86_400_000
const iso = (d: Date) => d.toISOString().slice(0, 10)
const utc = (s: string) => {
  const [y, m, d] = s.split('-').map(Number)
  return new Date(Date.UTC(y, m - 1, d))
}

export interface Day {
  date: string
  inMonth: boolean
}

/** Weeks of a month as rows of 7 days; `weekStart` is `monday` or `sunday`. */
export function monthGrid(month: string, weekStart: 'monday' | 'sunday'): Day[][] {
  const first = utc(`${month}-01`)
  const lead = weekStart === 'monday' ? (first.getUTCDay() + 6) % 7 : first.getUTCDay()
  const start = new Date(first.getTime() - lead * DAY)
  const weeks: Day[][] = []
  for (let w = 0; w < 6; w++) {
    const row: Day[] = []
    for (let i = 0; i < 7; i++) {
      const d = new Date(start.getTime() + (w * 7 + i) * DAY)
      row.push({ date: iso(d), inMonth: iso(d).slice(0, 7) === month })
    }
    if (w >= 4 && !row.some((d) => d.inMonth)) break
    weeks.push(row)
  }
  return weeks
}

/** The week (7 days) that contains `date`. */
export function weekOf(date: string, weekStart: 'monday' | 'sunday'): Day[] {
  const d = utc(date)
  const back = weekStart === 'monday' ? (d.getUTCDay() + 6) % 7 : d.getUTCDay()
  const start = new Date(d.getTime() - back * DAY)
  return Array.from({ length: 7 }, (_, i) => ({
    date: iso(new Date(start.getTime() + i * DAY)),
    inMonth: true,
  }))
}

/** Moves `YYYY-MM-DD[ HH:MM]` to another day, keeping the time of day. */
export function moveDateValue(value: string, newDate: string): string {
  return value.length > 10 ? `${newDate}${value.slice(10)}` : newDate
}

/** Whole days between two `YYYY-MM-DD` dates. */
export function daysBetween(a: string, b: string): number {
  return Math.round((utc(b).getTime() - utc(a).getTime()) / DAY)
}

export function addMonths(month: string, n: number): string {
  const [y, m] = month.split('-').map(Number)
  const d = new Date(Date.UTC(y, m - 1 + n, 1))
  return iso(d).slice(0, 7)
}
