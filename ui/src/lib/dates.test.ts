import { describe, expect, it } from 'vitest'
import { formatDate, formatPattern, parseNaturalDate } from './dates'

const TODAY = '2026-10-03' // a Saturday

describe('parseNaturalDate', () => {
  const cases: [string, string | null][] = [
    ['today', '2026-10-03'],
    ['tomorrow', '2026-10-04'],
    ['yesterday', '2026-10-02'],
    ['next monday', '2026-10-05'],
    ['monday', '2026-10-05'],
    ['saturday', '2026-10-10'],
    ['next saturday', '2026-10-10'],
    ['friday', '2026-10-09'],
    ['last friday', '2026-10-02'],
    ['in 3 days', '2026-10-06'],
    ['in 2 weeks', '2026-10-17'],
    ['in 1 month', '2026-11-03'],
    ['+7d', '2026-10-10'],
    ['-1w', '2026-09-26'],
    ['+1m', '2026-11-03'],
    ['2 days ago', '2026-10-01'],
    ['next week', '2026-10-10'],
    ['2026-12-24', '2026-12-24'],
    ['2026-02-30', null],
    ['jutro', '2026-10-04'],
    ['za 3 dni', '2026-10-06'],
    ['poniedziałek', '2026-10-05'],
    ['gibberish', null],
    ['', null],
  ]
  for (const [input, want] of cases) {
    it(`"${input}" → ${want}`, () => expect(parseNaturalDate(input, TODAY)).toBe(want))
  }
  it('clamps months', () => {
    expect(parseNaturalDate('+1m', '2026-01-31')).toBe('2026-02-28')
  })
  it('formats dates', () => {
    expect(formatDate('2026-10-03')).toBe('Sat, 3 Oct 2026')
  })

  it('formats by format and language', () => {
    expect(formatDate('2026-10-03', 'en-GB', 'long')).toBe('3 October 2026')
    expect(formatDate('2026-10-03', 'en-GB', 'short')).toBe('03/10/2026')
    expect(formatDate('2026-10-03', 'en-US', 'short')).toBe('10/03/2026')
    expect(formatDate('2026-10-03', 'de-DE', 'short')).toBe('03.10.2026')
    expect(formatDate('2026-10-03', 'pl-PL', 'long')).toBe('3 października 2026')
    expect(formatDate('2026-10-03', 'pl-PL', 'iso')).toBe('2026-10-03')
  })

  it('formats with a custom pattern', () => {
    const d = '2026-03-07'
    expect(formatPattern(d, 'YYYY-MM-DD', 'en-GB')).toBe('2026-03-07')
    expect(formatPattern(d, 'D.M.YY', 'en-GB')).toBe('7.3.26')
    expect(formatPattern(d, 'dddd, D MMMM YYYY', 'en-GB')).toBe('Saturday, 7 March 2026')
    expect(formatPattern(d, 'ddd DD MMM', 'en-GB')).toBe('Sat 07 Mar')
    expect(formatPattern(d, 'dddd, D MMMM YYYY', 'pl-PL')).toBe('sobota, 7 marca 2026')
    expect(formatPattern(d, '[Week of] D/M', 'en-GB')).toBe('Week of 7/3')
    expect(formatPattern(d, 'no tokens: !', 'en-GB')).toBe('no tokens: !')
  })
  it('custom falls back to the full format when the pattern is empty', () => {
    expect(formatDate('2026-10-03', 'en-GB', 'custom', '  ')).toBe('Sat, 3 Oct 2026')
    expect(formatDate('2026-10-03', 'en-GB', 'custom', 'D/M/YYYY')).toBe('3/10/2026')
  })
})
