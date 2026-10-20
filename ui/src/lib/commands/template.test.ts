import { describe, expect, it } from 'vitest'
import { templateOps } from './exec'
import { rankCommands, setTemplateNames, slashPool, templateChoices } from './slash'

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

describe('templateOps with properties', () => {
  it('sets the template properties first', () => {
    const props = [{ op: 'set_page_property', key: 'partner', value: '' } as const]
    expect(templateOps([0], '', '', [{ parent: [], index: 0, text: 'Contact' }], [...props])).toEqual([
      { op: 'set_page_property', key: 'partner', value: '' },
      { op: 'update_text', block: { path: [0], hash: null }, text: 'Contact' },
    ])
  })
})

describe('/template commands', () => {
  it('`/template` is one entry that opens the list, however many templates there are', () => {
    setTemplateNames(['person', 'Meeting Notes'], ['Standup'])
    for (const typed of ['', 'temp', 'template']) {
      const pool = slashPool({ typed, midWord: false })
      expect(pool.filter((c) => c.template)).toEqual([])
      expect(rankCommands(pool, typed).filter((c) => c.pickTemplate).length).toBe(1)
    }
    expect(rankCommands(slashPool({ typed: 'template', midWord: false }), 'template')[0].pickTemplate).toBe(
      true,
    )
    // never in the middle of a word
    expect(slashPool({ typed: 'template', midWord: true }).some((c) => c.pickTemplate || c.template)).toBe(
      false,
    )
    setTemplateNames([])
  })

  it('a template is still found by name after `template-`, saved blocks as snippets', () => {
    setTemplateNames(['person', 'Meeting Notes'], ['Standup'])
    const pool = slashPool({ typed: 'template-m', midWord: false })
    const hit = rankCommands(pool, 'template-m').find((c) => c.template)
    expect(hit?.template).toBe('Meeting Notes')
    const snip = rankCommands(slashPool({ typed: 'template-snippet-st', midWord: false }), 'template-snippet-st').find(
      (c) => c.template,
    )
    expect([snip?.name, snip?.template]).toEqual(['template-snippet-standup', 'snippets/Standup'])
    setTemplateNames([])
  })

  it('the list holds templates first, then saved blocks, each sorted', () => {
    setTemplateNames(['person', 'Meeting Notes'], ['Standup', 'Agenda'])
    expect(templateChoices().map((c) => [c.kind, c.template])).toEqual([
      ['template', 'Meeting Notes'],
      ['template', 'person'],
      ['snippet', 'snippets/Agenda'],
      ['snippet', 'snippets/Standup'],
    ])
    setTemplateNames([])
  })
})
