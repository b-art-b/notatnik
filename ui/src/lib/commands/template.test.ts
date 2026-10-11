import { describe, expect, it } from 'vitest'
import { templateOps } from './exec'
import { rankCommands, setTemplateNames, slashCommands, slashPool } from './slash'

const tpl = [
  { parent: [], index: 0, text: 'Contact' },
  { parent: [0], index: 0, text: 'Phone' },
  { parent: [], index: 1, text: 'Notes' },
]

describe('templateOps', () => {
  it('fills an empty block with the first template block and adds the rest after it', () => {
    expect(templateOps([2], '', '', tpl)).toEqual([
      { op: 'update_text', block: { path: [2], hash: null }, text: 'Contact' },
      { op: 'insert', parent: [2], index: 0, text: 'Phone' },
      { op: 'insert', parent: [], index: 3, text: 'Notes' },
    ])
  })

  it('goes after a block that has text, in the same parent, and keeps the typed text', () => {
    expect(templateOps([1, 0], 'old', 'new', tpl)).toEqual([
      { op: 'update_text', block: { path: [1, 0], hash: null }, text: 'new' },
      { op: 'insert', parent: [1], index: 1, text: 'Contact' },
      { op: 'insert', parent: [1, 1], index: 0, text: 'Phone' },
      { op: 'insert', parent: [1], index: 2, text: 'Notes' },
    ])
  })
})

describe('/template commands', () => {
  it('there is one per template, found by `template` and by the name', () => {
    setTemplateNames(['person', 'Meeting Notes'])
    const cmds = slashCommands().filter((c) => c.template)
    expect(cmds.map((c) => c.name)).toEqual(['template-person', 'template-meeting-notes'])
    const pool = slashPool({ typed: 'template', midWord: false })
    expect(rankCommands(pool, 'template').map((c) => c.template)).toEqual(['person', 'Meeting Notes'])
    expect(rankCommands(pool, 'meeting')[0].template).toBe('Meeting Notes')
    // never in the middle of a word
    expect(slashPool({ typed: 'template', midWord: true }).some((c) => c.template)).toBe(false)
    setTemplateNames([])
  })
})
