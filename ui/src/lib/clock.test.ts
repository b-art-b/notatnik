import { describe, expect, it } from 'vitest'
import { addMinutes, joinValue, parseClock, splitValue, timeLabel, timeProblem } from './clock'

describe('clock', () => {
  it('parses 24 h times', () => {
    expect(parseClock('9:05')).toBe('09:05')
    expect(parseClock(' 23:59 ')).toBe('23:59')
    for (const bad of ['24:00', '12:60', '9', '0905', '9:5', '']) expect(parseClock(bad)).toBeNull()
  })

  it('splits and joins values', () => {
    expect(splitValue('2026-10-14')).toEqual({ date: '2026-10-14', start: '', end: '' })
    expect(splitValue('2026-10-14 09:00')).toEqual({ date: '2026-10-14', start: '09:00', end: '' })
    expect(splitValue('2026-10-14 09:00-10:30')).toEqual({ date: '2026-10-14', start: '09:00', end: '10:30' })
    expect(splitValue('2026-10-14 09:00 – 10:30')).toEqual({
      date: '2026-10-14',
      start: '09:00',
      end: '10:30',
    })
    expect(joinValue('2026-10-14', '', '10:30')).toBe('2026-10-14')
    expect(joinValue('2026-10-14', '09:00', '')).toBe('2026-10-14 09:00')
    expect(joinValue('2026-10-14', '09:00', '10:30')).toBe('2026-10-14 09:00-10:30')
  })

  it('labels for chips', () => {
    expect(timeLabel('2026-10-14')).toBeNull()
    expect(timeLabel('2026-10-14 09:00')).toBe('09:00')
    expect(timeLabel('2026-10-14 09:00-10:30')).toBe('09:00–10:30')
    expect(timeLabel(null)).toBeNull()
  })

  it('adds minutes within the day', () => {
    expect(addMinutes('09:00', 30)).toBe('09:30')
    expect(addMinutes('09:45', 60)).toBe('10:45')
    expect(addMinutes('23:30', 60)).toBeNull()
    expect(addMinutes('nope', 30)).toBeNull()
  })

  it('names the problem with typed times', () => {
    expect(timeProblem('', '')).toBeNull()
    expect(timeProblem('09:00', '')).toBeNull()
    expect(timeProblem('09:00', '10:00')).toBeNull()
    expect(timeProblem('', '10:00')).toMatch(/Start/)
    expect(timeProblem('25:00', '')).toMatch(/Start/)
    expect(timeProblem('09:00', '9')).toMatch(/End time/)
    expect(timeProblem('10:00', '09:00')).toMatch(/after the start/)
    expect(timeProblem('10:00', '10:00')).toMatch(/after the start/)
  })
})
