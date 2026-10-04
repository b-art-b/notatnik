import { describe, expect, it } from 'vitest'
import type { ResultRow } from './api'
import {
  addMonths,
  daysBetween,
  kanbanColumns,
  kanbanKey,
  monthGrid,
  moveDateValue,
  sortRows,
  weekOf,
} from './views'

const row = (id: string, props: Record<string, string> = {}, extra: Partial<ResultRow> = {}): ResultRow =>
  ({
    id,
    page: 'p',
    group: null,
    block: null,
    breadcrumbs: [],
    props,
    scheduled: null,
    deadline: null,
    task_state: null,
    ...extra,
  }) as ResultRow

describe('table sorting', () => {
  const rows = [
    row('a', { rating: '3' }),
    row('b', {}),
    row('c', { rating: '10' }),
    row('d', { rating: '2' }),
  ]
  it('sorts numbers numerically and puts empty values last in both directions', () => {
    expect(sortRows(rows, 'prop.rating', false).map((r) => r.id)).toEqual(['d', 'a', 'c', 'b'])
    expect(sortRows(rows, 'prop.rating', true).map((r) => r.id)).toEqual(['c', 'a', 'd', 'b'])
  })
  it('sorts dates and text', () => {
    const d = [row('x', {}, { deadline: '2026-10-05' }), row('y', {}, { deadline: '2026-10-01' }), row('z')]
    expect(sortRows(d, 'deadline', false).map((r) => r.id)).toEqual(['y', 'x', 'z'])
    expect(sortRows(d, null, false)).toBe(d)
  })
})

describe('kanban', () => {
  it('uses fixed task columns, or observed group values with (none) last', () => {
    expect(kanbanColumns([], 'task')).toEqual(['todo', 'doing', 'waiting', 'done', 'canceled'])
    expect(kanbanKey(row('a', {}, { task_state: 'doing' }), null)).toBe('doing')
    const rows = [
      row('a', {}, { group: 'reading' }),
      row('b', {}, { group: null }),
      row('c', {}, { group: 'done' }),
    ]
    expect(kanbanColumns(rows, 'prop.status')).toEqual(['reading', 'done', '(none)'])
  })
})

describe('calendar', () => {
  it('builds October 2026 starting on Monday', () => {
    const g = monthGrid('2026-10', 'monday')
    expect(g[0][0].date).toBe('2026-09-28')
    expect(g[0][3]).toEqual({ date: '2026-10-01', inMonth: true })
    expect(g.flat().find((d) => d.date === '2026-10-31')?.inMonth).toBe(true)
    expect(g.every((w) => w.length === 7)).toBe(true)
    expect(monthGrid('2026-10', 'sunday')[0][0].date).toBe('2026-09-27')
  })
  it('finds the week of a date', () => {
    expect(weekOf('2026-10-03', 'monday').map((d) => d.date)).toEqual([
      '2026-09-28',
      '2026-09-29',
      '2026-09-30',
      '2026-10-01',
      '2026-10-02',
      '2026-10-03',
      '2026-10-04',
    ])
  })
  it('moves dates keeping the time and counts days', () => {
    expect(moveDateValue('2026-10-08 14:00', '2026-10-10')).toBe('2026-10-10 14:00')
    expect(moveDateValue('2026-10-08', '2026-10-10')).toBe('2026-10-10')
    expect(daysBetween('2026-10-08', '2026-10-10')).toBe(2)
    expect(addMonths('2026-12', 1)).toBe('2027-01')
    expect(addMonths('2026-01', -1)).toBe('2025-12')
  })
})
