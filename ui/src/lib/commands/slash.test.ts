import { describe, expect, it } from 'vitest'
import { EditorState } from '@codemirror/state'
import {
  dateChanges,
  logChanges,
  markerChanges,
  propertyChanges,
  rankCommands,
  slashAt,
  slashCommands,
  slashPool,
} from './slash'

function run(doc: string, marker: string): string {
  const f = doc.indexOf('/')
  const t = f + 1 + /^[a-z]*/.exec(doc.slice(f + 1))![0].length
  const state = EditorState.create({ doc })
  return state.update({ changes: markerChanges(doc, f, t, marker) }).state.doc.toString()
}

describe('markerChanges', () => {
  it('prepends the marker wherever the command was typed', () => {
    expect(run('buy milk /todo', 'TODO')).toBe('TODO buy milk')
    expect(run('/todo buy milk', 'TODO')).toBe('TODO buy milk')
    expect(run('buy /doing milk', 'DOING')).toBe('DOING buy milk')
  })
  it('replaces an existing marker', () => {
    expect(run('TODO buy milk /done', 'DONE')).toBe('DONE buy milk')
    expect(run('DOING /waiting x', 'WAITING')).toBe('WAITING x')
    expect(run('LATER /now x', 'NOW')).toBe('NOW x')
    expect(run('NOW /later x', 'LATER')).toBe('LATER x')
  })
  it('only touches the first line', () => {
    expect(run('first\nsecond /todo', 'TODO')).toBe('TODO first\nsecond')
  })
})

function applyDate(doc: string, at: number, kind: 'link' | 'scheduled' | 'deadline', iso: string): string {
  return EditorState.create({ doc })
    .update({ changes: dateChanges(doc, at, kind, iso) })
    .state.doc.toString()
}

describe('dateChanges', () => {
  it('inserts a date link at the position', () => {
    expect(applyDate('call  now', 5, 'link', '2026-10-05')).toBe('call [[2026-10-05]] now')
  })
  it('appends a property line when missing', () => {
    expect(applyDate('pay rent', 0, 'deadline', '2026-10-05')).toBe('pay rent\ndeadline:: 2026-10-05')
  })
  it('puts a new property under the first line, above a logbook drawer', () => {
    const doc = 'TODO x\n:LOGBOOK:\n2026-10-03 first\n:END:'
    expect(applyDate(doc, 0, 'scheduled', '2026-10-05')).toBe(
      'TODO x\nscheduled:: 2026-10-05\n:LOGBOOK:\n2026-10-03 first\n:END:',
    )
  })
  it('puts it after the properties already there and before body lines', () => {
    const doc = 'TODO x\ndeadline:: 2026-10-10\npriority:: high\nsome body\n:LOGBOOK:\n:END:'
    expect(applyDate(doc, 0, 'scheduled', '2026-10-05')).toBe(
      'TODO x\ndeadline:: 2026-10-10\npriority:: high\nscheduled:: 2026-10-05\nsome body\n:LOGBOOK:\n:END:',
    )
  })
  it('a property-only tail is appended after the last property', () => {
    expect(applyDate('x\na:: 1', 0, 'deadline', '2026-10-05')).toBe('x\na:: 1\ndeadline:: 2026-10-05')
  })
  it('replaces an existing property value only', () => {
    expect(applyDate('TODO x\nscheduled:: 2026-01-01\nprio:: a', 0, 'scheduled', '2026-10-05')).toBe(
      'TODO x\nscheduled:: 2026-10-05\nprio:: a',
    )
  })
})

function runLog(doc: string, command = '/log'): { text: string; cursor: number } {
  const f = doc.indexOf(command)
  const r = logChanges(doc, f, f + command.length, '2026-10-04 14:30')
  const text = EditorState.create({ doc }).update({ changes: r.changes }).state.doc.toString()
  return { text, cursor: r.cursor }
}

describe('logChanges', () => {
  it('creates the drawer at the end of the block', () => {
    const { text, cursor } = runLog('TODO write spec /log')
    expect(text).toBe('TODO write spec\n:LOGBOOK:\n2026-10-04 14:30 \n:END:')
    expect(text.slice(0, cursor)).toBe('TODO write spec\n:LOGBOOK:\n2026-10-04 14:30 ')
  })
  it('keeps properties above the drawer', () => {
    const { text } = runLog('TODO x\ndeadline:: 2026-10-10\n/log')
    expect(text).toBe('TODO x\ndeadline:: 2026-10-10\n\n:LOGBOOK:\n2026-10-04 14:30 \n:END:')
  })
  it('adds a line to an existing drawer', () => {
    const doc = 'TODO x /log\n:LOGBOOK:\n2026-10-03 first\n:END:'
    const { text, cursor } = runLog(doc)
    expect(text).toBe('TODO x\n:LOGBOOK:\n2026-10-03 first\n2026-10-04 14:30 \n:END:')
    expect(text.slice(0, cursor).endsWith('2026-10-04 14:30 ')).toBe(true)
  })
})

describe('rankCommands', () => {
  const names = (typed: string) => rankCommands(slashCommands(), typed).map((c) => c.name)
  it('keeps the menu order when nothing is typed', () => {
    expect(names('')[0]).toBe('todo')
    expect(names('')).toHaveLength(slashCommands().length)
  })
  it('puts the exact name first, then prefixes', () => {
    expect(names('do').slice(0, 2)).toEqual(['doing', 'done'])
    expect(names('done')[0]).toBe('done')
  })
  it('finds commands by keyword and description', () => {
    expect(names('task')[0]).toBe('todo')
    expect(names('due')[0]).toBe('deadline')
    expect(names('logbook')[0]).toBe('log')
    expect(names('calendar')).toContain('date')
  })
  it('returns nothing for gibberish', () => {
    expect(names('zzzz')).toEqual([])
  })
})

function applyProp(doc: string, key: string, value: string): string {
  return EditorState.create({ doc })
    .update({ changes: propertyChanges(doc, key, value) })
    .state.doc.toString()
}

describe('propertyChanges', () => {
  it('adds the property under the first line, above body text and a logbook', () => {
    expect(applyProp('TODO x\nbody\n:LOGBOOK:\n2026-10-03 a\n:END:', 'priority', 'high')).toBe(
      'TODO x\npriority:: high\nbody\n:LOGBOOK:\n2026-10-03 a\n:END:',
    )
  })
  it('replaces the value of an existing property', () => {
    expect(applyProp('TODO x\npriority:: low\nbody', 'priority', 'high')).toBe(
      'TODO x\npriority:: high\nbody',
    )
  })
  it('works for a one-line block', () => {
    expect(applyProp('TODO x', 'priority', 'medium')).toBe('TODO x\npriority:: medium')
  })
})

describe('slashAt', () => {
  it('finds a command at the start of a word', () => {
    expect(slashAt('buy milk /to')).toEqual({ typed: 'to', midWord: false })
    expect(slashAt('/')).toEqual({ typed: '', midWord: false })
    expect(slashAt('(/todo')).toEqual({ typed: 'todo', midWord: false })
    expect(slashAt('so,/log')).toEqual({ typed: 'log', midWord: false })
  })
  it('finds a command in the middle of a word', () => {
    expect(slashAt('some wor/d')).toEqual({ typed: 'd', midWord: true })
    expect(slashAt('word/log')).toEqual({ typed: 'log', midWord: true })
    expect(slashAt('x1/done')).toEqual({ typed: 'done', midWord: true })
  })
  it('leaves URLs, paths and bare slashes alone', () => {
    for (const t of ['see https://example.com/doc', 'a/b/c', 'file.txt/done', 'x@y.z/d', 'wor/', '1/2/3']) {
      expect(slashAt(t), t).toBeNull()
    }
  })
})

describe('slashPool', () => {
  const names = (typed: string, midWord: boolean) => slashPool({ typed, midWord }).map((c) => c.name)
  it('offers every command at the start of a word', () => {
    expect(names('', false)).toContain('today')
  })
  it('offers metadata commands only, by prefix, in the middle of a word', () => {
    expect(names('d', true)).toEqual(expect.arrayContaining(['done', 'doing', 'deadline']))
    expect(names('d', true)).not.toContain('date') // inserts a link at the cursor
    expect(names('tod', true)).toContain('todo')
    expect(names('tod', true)).not.toContain('today')
    expect(names('log', true)).toContain('log')
    expect(names('xyz', true)).toEqual([])
    expect(names('h', true)).toEqual([]) // `km/h`: nothing starts with it
  })
})
