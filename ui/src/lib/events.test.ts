import { describe, expect, it } from 'vitest'
import type { CalendarEventView } from './api'
import { eventsOn, eventTime } from './events'

const ev = (o: Partial<CalendarEventView>): CalendarEventView => ({
  uid: 'u',
  title: 't',
  date: '2026-10-20',
  end_date: '2026-10-20',
  start: null,
  end: null,
  location: null,
  calendar: 'c',
  ...o,
})

describe('events', () => {
  it('a multi-day event is on each of its days', () => {
    const e = ev({ date: '2026-10-23', end_date: '2026-10-25' })
    expect(eventsOn([e], '2026-10-22')).toHaveLength(0)
    expect(eventsOn([e], '2026-10-24')).toHaveLength(1)
    expect(eventsOn([e], '2026-10-26')).toHaveLength(0)
  })
  it('says the time as it is on that day', () => {
    expect(eventTime(ev({}), '2026-10-20')).toBe('all day')
    expect(eventTime(ev({ start: '09:30', end: '10:30' }), '2026-10-20')).toBe('09:30–10:30')
    expect(eventTime(ev({ start: '09:30', end: '10:30' }), '2026-10-21')).toBe('all day')
    const night = ev({ start: '22:00', end: '02:00', end_date: '2026-10-21' })
    expect(eventTime(night, '2026-10-20')).toBe('from 22:00')
    expect(eventTime(night, '2026-10-21')).toBe('until 02:00')
  })
})
