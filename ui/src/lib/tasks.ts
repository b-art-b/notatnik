// Task state cycling and display helpers (TK-1, TK-6).
import type { BlockView, TaskState } from './api'

/** (none) → TODO → DOING → DONE → (none); WAITING goes to DONE, CANCELED clears. */
export function nextTaskState(s: TaskState | null | undefined): TaskState | null {
  switch (s) {
    case undefined:
    case null:
      return 'TODO'
    case 'TODO':
      return 'DOING'
    case 'DOING':
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
