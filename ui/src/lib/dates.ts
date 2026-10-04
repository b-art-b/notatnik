// Natural-language dates for `/date` and the date picker (JR-3). Pure: "today" is always passed in.
const DAY_MS = 86_400_000

function parseISO(s: string): Date {
  const [y, m, d] = s.split('-').map(Number)
  return new Date(Date.UTC(y, m - 1, d))
}
function fmt(d: Date): string {
  return d.toISOString().slice(0, 10)
}
const addDays = (d: Date, n: number) => new Date(d.getTime() + n * DAY_MS)
function addMonths(d: Date, n: number): Date {
  const y = d.getUTCFullYear()
  const m = d.getUTCMonth() + n
  const last = new Date(Date.UTC(y, m + 1, 0)).getUTCDate()
  return new Date(Date.UTC(y, m, Math.min(d.getUTCDate(), last)))
}

const WEEKDAYS = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday']
const WEEKDAYS_PL = ['niedziela', 'poniedziałek', 'wtorek', 'środa', 'czwartek', 'piątek', 'sobota']
const UNITS: Record<string, 'd' | 'w' | 'm' | 'y'> = {
  day: 'd',
  days: 'd',
  week: 'w',
  weeks: 'w',
  month: 'm',
  months: 'm',
  year: 'y',
  years: 'y',
  dzień: 'd',
  dni: 'd',
  tydzień: 'w',
  tygodnie: 'w',
  tygodni: 'w',
  miesiąc: 'm',
  miesiące: 'm',
  miesięcy: 'm',
  rok: 'y',
  lata: 'y',
  lat: 'y',
}

function offset(d: Date, n: number, unit: 'd' | 'w' | 'm' | 'y'): Date {
  if (unit === 'd') return addDays(d, n)
  if (unit === 'w') return addDays(d, 7 * n)
  if (unit === 'm') return addMonths(d, n)
  return addMonths(d, 12 * n)
}

/** Parses `today`, `tomorrow`, `next monday`, `in 3 days`, `+2w`, `-1m`, `2026-10-05`, ... → `YYYY-MM-DD`, or null. */
export function parseNaturalDate(input: string, today: string): string | null {
  const s = input.trim().toLowerCase()
  const t = parseISO(today)
  if (/^\d{4}-\d{2}-\d{2}$/.test(s)) {
    const d = parseISO(s)
    return fmt(d) === s ? s : null
  }
  if (s === 'today' || s === 'dziś' || s === 'dzisiaj') return today
  if (s === 'tomorrow' || s === 'jutro') return fmt(addDays(t, 1))
  if (s === 'yesterday' || s === 'wczoraj') return fmt(addDays(t, -1))
  let m = /^([+-])(\d+)([dwmy])$/.exec(s)
  if (m) return fmt(offset(t, (m[1] === '-' ? -1 : 1) * Number(m[2]), m[3] as 'd' | 'w' | 'm' | 'y'))
  m = /^(?:in|za)\s+(\d+)\s+(\p{L}+)$/u.exec(s)
  if (m && UNITS[m[2]]) return fmt(offset(t, Number(m[1]), UNITS[m[2]]))
  m = /^(\d+)\s+(\p{L}+)\s+(ago|temu)$/u.exec(s)
  if (m && UNITS[m[2]]) return fmt(offset(t, -Number(m[1]), UNITS[m[2]]))
  m = /^(next|this|last|przyszły|następny)?\s*(\p{L}+)$/u.exec(s)
  if (m) {
    let wd = WEEKDAYS.indexOf(m[2])
    if (wd < 0) wd = WEEKDAYS_PL.indexOf(m[2])
    if (wd >= 0) {
      const cur = t.getUTCDay()
      if (m[1] === 'last') {
        const back = ((cur - wd + 6) % 7) + 1
        return fmt(addDays(t, -back))
      }
      let fwd = (wd - cur + 7) % 7
      if (fwd === 0 || m[1] === 'next' || m[1] === 'przyszły' || m[1] === 'następny')
        fwd = fwd === 0 ? 7 : fwd
      return fmt(addDays(t, fwd))
    }
  }
  m = /^(next|last)\s+(week|month|year)$/.exec(s)
  if (m) return fmt(offset(t, m[1] === 'next' ? 1 : -1, UNITS[m[2]]))
  return null
}

let currentLocale = 'en-GB'

/** Sets the locale used by `formatDate` (device setting). */
export function setDateLocale(locale: string) {
  currentLocale = locale
}

/** `2026-10-03` → `Sat, 3 Oct 2026` in the configured locale. */
export function formatDate(iso: string, locale = currentLocale): string {
  const d = parseISO(iso)
  return new Intl.DateTimeFormat(locale, {
    weekday: 'short',
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    timeZone: 'UTC',
  }).format(d)
}
