// Task state cycling and display helpers (TK-1, TK-6).
import type { BlockView, TaskState } from './api'

/** (none) → TODO → DOING → DONE → (none); LATER → NOW → DONE; WAITING goes to DONE, CANCELED clears. */
export function nextTaskState(s: TaskState | null | undefined): TaskState | null {
  switch (s) {
    case undefined:
    case null:
      return 'TODO'
    case 'TODO':
      return 'DOING'
    case 'LATER':
      return 'NOW'
    case 'DOING':
    case 'NOW':
    case 'WAITING':
      return 'DONE'
    default:
      return null
  }
}

export interface Progress {
  done: number
  total: number
}

/** Counts task blocks among all descendants (grandchildren included); `done` = DONE or CANCELED. */
export function progress(block: BlockView): Progress {
  let done = 0
  let total = 0
  const walk = (list: BlockView[]) => {
    for (const b of list) {
      if (b.task) {
        total++
        if (b.task.state === 'DONE' || b.task.state === 'CANCELED') done++
      }
      walk(b.children)
    }
  }
  walk(block.children)
  return { done, total }
}

/** `YYYY-MM-DDTHH:MM` in the server's "today" and the browser's wall clock time. */
export function doneAt(today: string, now = new Date()): string {
  const hh = String(now.getHours()).padStart(2, '0')
  const mm = String(now.getMinutes()).padStart(2, '0')
  return `${today}T${hh}:${mm}`
}

/** True when a `YYYY-MM-DD[ HH:MM]` value is before `today` (dates only). */
export function isOverdue(value: string | null | undefined, today: string): boolean {
  return !!value && value.slice(0, 10) < today
}

export type DueTone = 'overdue' | 'today' | 'soon' | 'later'

export interface DueIndicator {
  /** Which date it shows: the deadline if there is one, else the scheduled date. */
  kind: 'deadline' | 'scheduled'
  date: string
  /** `3d overdue`, `today`, `tomorrow`, or `null` when the caller should format the date itself. */
  text: string | null
  tone: DueTone
}

const DAY = 86_400_000
const dayNumber = (iso: string) => Math.floor(Date.parse(iso.slice(0, 10) + 'T00:00:00Z') / DAY)

/** The one date a compact task row shows (TK-13): the deadline, else the scheduled date; nothing for a finished
 * task. Overdue and today get words and a tone so urgency stays visible without opening the details. */
export function dueIndicator(
  task: { state: string; scheduled: string | null; deadline: string | null },
  today: string,
): DueIndicator | null {
  if (task.state === 'DONE' || task.state === 'CANCELED') return null
  const kind = task.deadline ? 'deadline' : task.scheduled ? 'scheduled' : null
  if (!kind) return null
  const date = (kind === 'deadline' ? task.deadline : task.scheduled) as string
  const diff = dayNumber(date) - dayNumber(today)
  if (Number.isNaN(diff)) return { kind, date, text: null, tone: 'later' }
  if (diff < 0) return { kind, date, text: `${-diff}d overdue`, tone: 'overdue' }
  if (diff === 0) return { kind, date, text: 'today', tone: 'today' }
  if (diff === 1) return { kind, date, text: 'tomorrow', tone: 'soon' }
  return { kind, date, text: null, tone: diff <= 7 ? 'soon' : 'later' }
}
