import { describe, expect, it } from 'vitest'
import type { BlockView } from './api'
import { doneAt, dueIndicator, isOverdue, nextTaskState, progress } from './tasks'

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
    logbook: [],
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

describe('dueIndicator', () => {
  const t = (state: string, scheduled: string | null, deadline: string | null) => ({
    state,
    scheduled,
    deadline,
  })
  it('prefers the deadline and uses words for overdue, today and tomorrow', () => {
    expect(dueIndicator(t('TODO', '2026-10-01', '2026-10-08'), '2026-10-11')).toMatchObject({
      kind: 'deadline',
      text: '3d overdue',
      tone: 'overdue',
    })
    expect(dueIndicator(t('TODO', null, '2026-10-11'), '2026-10-11')).toMatchObject({
      text: 'today',
      tone: 'today',
    })
    expect(dueIndicator(t('DOING', '2026-10-12', null), '2026-10-11')).toMatchObject({
      kind: 'scheduled',
      text: 'tomorrow',
    })
  })
  it('leaves the text to the caller for later dates and shows nothing for finished tasks', () => {
    expect(dueIndicator(t('TODO', null, '2026-12-24'), '2026-10-11')).toMatchObject({
      text: null,
      tone: 'later',
    })
    expect(dueIndicator(t('DONE', null, '2026-10-01'), '2026-10-11')).toBeNull()
    expect(dueIndicator(t('TODO', null, null), '2026-10-11')).toBeNull()
  })
})
