import { describe, expect, it } from 'vitest'
import type { BlockView } from './api'
import { doneAt, isOverdue, nextTaskState, progress } from './tasks'

import type { TaskState } from './api'

const blk = (state: TaskState | null, children: BlockView[] = []): BlockView =>
  ({
    id: 1,
    path: [],
    hash: '',
    uid: null,
    text: '',
    content: [],
    task: state ? { state, scheduled: null, deadline: null, priority: null, repeat: null } : null,
    props: [],
    collapsed: false,
    children,
  }) as BlockView

describe('tasks', () => {
  it('cycles none → TODO → DOING → DONE → none', () => {
    expect(nextTaskState(null)).toBe('TODO')
    expect(nextTaskState('TODO')).toBe('DOING')
    expect(nextTaskState('DOING')).toBe('DONE')
    expect(nextTaskState('DONE')).toBeNull()
    expect(nextTaskState('WAITING')).toBe('DONE')
    expect(nextTaskState('CANCELED')).toBeNull()
  })
  it('counts task descendants including grandchildren', () => {
    const b = blk(null, [blk('DONE'), blk('TODO', [blk('DONE'), blk('TODO')]), blk(null, [blk('CANCELED')])])
    expect(progress(b)).toEqual({ done: 3, total: 5 })
    expect(progress(blk('TODO'))).toEqual({ done: 0, total: 0 })
  })
  it('formats the completion time and detects overdue dates', () => {
    expect(doneAt('2026-10-03', new Date(2026, 9, 3, 8, 5))).toBe('2026-10-03T08:05')
    expect(isOverdue('2026-10-02 17:00', '2026-10-03')).toBe(true)
    expect(isOverdue('2026-10-03', '2026-10-03')).toBe(false)
    expect(isOverdue(null, '2026-10-03')).toBe(false)
  })
})
