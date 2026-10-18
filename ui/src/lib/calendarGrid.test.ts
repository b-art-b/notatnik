import { describe, expect, it } from 'vitest'
import type { CalendarEventView } from './api'
import {
  colorFor,
  eventSegments,
  layoutDay,
  movedValue,
  newTaskText,
  placingField,
  rolledOver,
  rangeLabel,
  resizedValue,
  snap,
  taskSegment,
  toClock,
  toMinutes,
} from './calendarGrid'

const ev = (over: Partial<CalendarEventView>): CalendarEventView => ({
  uid: 'e',
  title: 'Event',
  date: '2026-10-20',
  end_date: '2026-10-20',
  start: null,
  end: null,
  location: null,
  calendar: '/cal/',
  ...over,
})

describe('clock helpers', () => {
  it('converts both ways and snaps', () => {
    expect(toMinutes('09:30')).toBe(570)
    expect(toClock(570)).toBe('09:30')
    expect(toClock(2000)).toBe('23:59')
    expect(snap(557)).toBe(555)
    expect(snap(563)).toBe(570)
  })
})

describe('which date places a task', () => {
  it('takes the earlier date, the scheduled one on a tie', () => {
    expect(placingField('2026-10-22', '2026-10-20', 'earlier')?.field).toBe('deadline')
    expect(placingField('2026-10-20', '2026-10-22', 'earlier')?.field).toBe('scheduled')
    expect(placingField('2026-10-20', '2026-10-20 17:00', 'earlier')?.field).toBe('scheduled')
    expect(placingField(null, '2026-10-20', 'earlier')?.field).toBe('deadline')
    expect(placingField(null, null, 'earlier')).toBeNull()
  })
  it('can be forced to one field', () => {
    expect(placingField('2026-10-22', '2026-10-20', 'scheduled')?.field).toBe('scheduled')
    expect(placingField(null, '2026-10-20', 'scheduled')).toBeNull()
  })
})

describe('where a task sits', () => {
  it('a plain date is all day', () => {
    const s = taskSegment('t', 'x', '2026-10-20', 'scheduled')
    expect(s.allDay).toBe(true)
    expect(s.date).toBe('2026-10-20')
  })
  it('a start time makes a block of one hour, an end time its length', () => {
    const a = taskSegment('t', 'x', '2026-10-20 09:00', 'scheduled')
    expect([a.allDay, a.start, a.end]).toEqual([false, 540, 600])
    const b = taskSegment('t', 'x', '2026-10-20 09:00-10:30', 'scheduled')
    expect([b.start, b.end]).toEqual([540, 630])
  })
  it('a block late in the evening ends at midnight', () => {
    const a = taskSegment('t', 'x', '2026-10-20 23:30', 'scheduled')
    expect([a.start, a.end]).toEqual([1410, 1440])
  })
  it('a deadline time is the end of an hour before it', () => {
    const a = taskSegment('t', 'x', '2026-10-20 17:00', 'deadline')
    expect([a.start, a.end, a.due]).toEqual([960, 1020, true])
    const early = taskSegment('t', 'x', '2026-10-20 00:10', 'deadline')
    expect(early.start).toBe(0)
    expect(early.end).toBeGreaterThan(early.start)
  })
})

describe('where an event sits', () => {
  it('an event without a time is all day', () => {
    const [s] = eventSegments(ev({}), '2026-10-19', '2026-10-25')
    expect(s.allDay).toBe(true)
  })
  it('a timed event uses its times; without an end it takes an hour', () => {
    expect(eventSegments(ev({ start: '09:30', end: '10:15' }), '2026-10-19', '2026-10-25')[0]).toMatchObject({
      start: 570,
      end: 615,
    })
    expect(eventSegments(ev({ start: '09:30' }), '2026-10-19', '2026-10-25')[0]).toMatchObject({
      start: 570,
      end: 630,
    })
  })
  it('a multi-day all-day event is one piece per day, only those in the range', () => {
    const segs = eventSegments(ev({ date: '2026-10-22', end_date: '2026-10-25' }), '2026-10-23', '2026-10-24')
    expect(segs.map((s) => s.date)).toEqual(['2026-10-23', '2026-10-24'])
    expect(segs.every((s) => s.allDay && s.before)).toBe(true)
    expect(segs[0].after).toBe(true)
  })
  it('a timed event over midnight runs to the end of the first day and from the start of the last', () => {
    const segs = eventSegments(
      ev({ date: '2026-10-20', end_date: '2026-10-21', start: '22:00', end: '02:00' }),
      '2026-10-19',
      '2026-10-25',
    )
    expect(segs[0]).toMatchObject({ date: '2026-10-20', start: 1320, end: 1440, after: true })
    expect(segs[1]).toMatchObject({ date: '2026-10-21', start: 0, end: 120, before: true })
  })
})

describe('layout of overlapping pieces', () => {
  it('pieces that do not overlap get the whole width', () => {
    const m = layoutDay([
      { id: 'a', start: 0, end: 60 },
      { id: 'b', start: 60, end: 120 },
    ])
    expect(m.get('a')).toEqual({ col: 0, cols: 1 })
    expect(m.get('b')).toEqual({ col: 0, cols: 1 })
  })
  it('overlapping pieces share the width', () => {
    const m = layoutDay([
      { id: 'a', start: 0, end: 120 },
      { id: 'b', start: 30, end: 90 },
      { id: 'c', start: 100, end: 160 },
    ])
    expect(m.get('a')).toEqual({ col: 0, cols: 2 })
    expect(m.get('b')).toEqual({ col: 1, cols: 2 })
    // c starts after b ended: it takes b's column
    expect(m.get('c')).toEqual({ col: 1, cols: 2 })
  })
  it('two groups on one day are independent', () => {
    const m = layoutDay([
      { id: 'a', start: 0, end: 60 },
      { id: 'b', start: 0, end: 60 },
      { id: 'c', start: 300, end: 360 },
    ])
    expect(m.get('a')?.cols).toBe(2)
    expect(m.get('c')).toEqual({ col: 0, cols: 1 })
  })
})

describe('values written after a drag', () => {
  it('moving keeps the length of a range', () => {
    expect(movedValue('2026-10-20 09:00-10:30', 'scheduled', '2026-10-21', 13 * 60)).toBe(
      '2026-10-21 13:00-14:30',
    )
  })
  it('a start without an end stays without an end', () => {
    expect(movedValue('2026-10-20 09:00', 'scheduled', '2026-10-20', 600)).toBe('2026-10-20 10:00')
  })
  it('a plain date given a time becomes an hour', () => {
    expect(movedValue('2026-10-20', 'scheduled', '2026-10-20', 14 * 60)).toBe('2026-10-20 14:00-15:00')
  })
  it('a range that would pass midnight is moved up', () => {
    expect(movedValue('2026-10-20 09:00-11:00', 'scheduled', '2026-10-20', 23 * 60)).toBe(
      '2026-10-20 21:59-23:59',
    )
  })
  it('a deadline gets the end of the dropped hour', () => {
    expect(movedValue('2026-10-20 17:00', 'deadline', '2026-10-21', 9 * 60)).toBe('2026-10-21 10:00')
  })
  it('resizing sets the end, at least 15 minutes after the start', () => {
    expect(resizedValue('2026-10-20 09:00', 11 * 60)).toBe('2026-10-20 09:00-11:00')
    expect(resizedValue('2026-10-20 09:00-10:00', 9 * 60 + 5)).toBe('2026-10-20 09:00-09:15')
    expect(resizedValue('2026-10-20', 600)).toBeNull()
  })
  it('a task made by clicking a slot lasts an hour', () => {
    expect(newTaskText(' Call ', '2026-10-20', 14 * 60)).toBe('TODO Call\nscheduled:: 2026-10-20 14:00-15:00')
  })
})

describe('colours and labels', () => {
  it('a chosen colour wins; the others are stable', () => {
    expect(colorFor('/a/', { '/a/': '#112233' })).toBe('#112233')
    expect(colorFor('/b/', {})).toBe(colorFor('/b/', {}))
    expect(colorFor('/b/', {})).toMatch(/^#[0-9a-f]{6}$/)
  })
  it('names a range', () => {
    expect(rangeLabel(['2026-10-19', '2026-10-25'])).toBe('19 – 25 Oct 2026')
    expect(rangeLabel(['2026-10-26', '2026-11-01'])).toBe('26 Oct – 1 Nov 2026')
    expect(rangeLabel(['2026-10-20'])).toContain('20 Oct 2026')
  })
})

describe('rolling overdue tasks over to today', () => {
  it("puts a task from an earlier day in today's strip and says how late it is", () => {
    const seg = taskSegment('t', 'x', '2026-10-01 09:00-10:30', 'scheduled')
    const r = rolledOver(seg, '2026-10-03')!
    expect(r.seg).toMatchObject({ date: '2026-10-03', allDay: true, start: 0, end: 1440 })
    expect(r.from).toBe('2026-10-01')
    expect(r.days).toBe(2)
  })
  it('leaves today, later days and events alone', () => {
    expect(rolledOver(taskSegment('t', 'x', '2026-10-03', 'scheduled'), '2026-10-03')).toBeNull()
    expect(rolledOver(taskSegment('t', 'x', '2026-10-09', 'deadline'), '2026-10-03')).toBeNull()
    expect(rolledOver(eventSegments(ev({}), '2026-10-01', '2026-10-30')[0], '2026-10-30')).toBeNull()
  })
})
