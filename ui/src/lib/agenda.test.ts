import { describe, expect, it } from 'vitest'
import type { AgendaItem } from './api'
import { crumbText, groupItems, sortItems, sourceLabel, sourceTitle } from './agenda'

const item = (
  page: string,
  state: string,
  extra: { deadline?: string; scheduled?: string; priority?: string; rank?: number } = {},
): AgendaItem =>
  ({
    page,
    block: {
      id: 0,
      task: {
        state,
        deadline: extra.deadline ?? null,
        scheduled: extra.scheduled ?? null,
        priority: extra.priority ?? null,
        priority_rank: extra.rank ?? null,
        repeat: null,
      },
    },
    breadcrumbs: [],
    date: null,
  }) as unknown as AgendaItem

const pages = (l: AgendaItem[]) => l.map((i) => i.page)

describe('sortItems', () => {
  const list = [
    item('c', 'TODO', { deadline: '2026-12-01', priority: 'C', rank: 2 }),
    item('a', 'DOING'),
    item('b', 'WAITING', { deadline: '2026-10-05', priority: 'A', rank: 0 }),
  ]
  it('sorts by deadline with undated last', () => {
    expect(pages(sortItems(list, 'deadline asc'))).toEqual(['b', 'c', 'a'])
    expect(pages(sortItems(list, 'deadline desc'))).toEqual(['c', 'b', 'a'])
  })
  it('puts high priority first for desc and keeps unprioritised last', () => {
    expect(pages(sortItems(list, 'priority desc'))).toEqual(['b', 'c', 'a'])
  })
  it('sorts by page and by state', () => {
    expect(pages(sortItems(list, 'page asc'))).toEqual(['a', 'b', 'c'])
    expect(pages(sortItems(list, 'task asc'))).toEqual(['a', 'c', 'b'])
  })
  it('is stable', () => {
    const same = [item('x', 'TODO'), item('y', 'TODO'), item('z', 'TODO')]
    expect(pages(sortItems(same, 'deadline asc'))).toEqual(['x', 'y', 'z'])
  })
})

describe('groupItems', () => {
  const list = [item('a', 'TODO'), item('b', 'DOING'), item('a', 'DOING', { deadline: '2026-10-05' })]
  it('is one group without a key', () => {
    expect(groupItems(list, '')).toHaveLength(1)
  })
  it('groups by page and by state in first-seen order', () => {
    expect(groupItems(list, 'page').map((g) => [g.label, g.items.length])).toEqual([
      ['a', 2],
      ['b', 1],
    ])
    expect(groupItems(list, 'task').map((g) => g.label)).toEqual(['TODO', 'DOING'])
    expect(groupItems(list, 'deadline').map((g) => g.label)).toEqual(['no deadline', '2026-10-05'])
  })
})

describe('sourceTitle', () => {
  it('is the page alone without parents', () => {
    expect(sourceTitle('RfP/Example', [])).toBe('RfP/Example')
  })
  it('lists the parents as plain words', () => {
    expect(sourceTitle('p', ['TODO **Plan** [[part. Capgemini/x]]', 'see #urgent'])).toBe(
      'p › Plan part. Capgemini/x › see urgent',
    )
  })
})

describe('sourceLabel', () => {
  it('shortens subpages, formats journal days, leaves other pages alone', () => {
    expect(sourceLabel('Conference/Example', 'Conference')).toBe('Example')
    expect(sourceLabel('Conference/Example')).toBe('Conference/Example')
    expect(sourceLabel('Other/Page', 'Conference')).toBe('Other/Page')
    expect(sourceLabel('2026-10-09', '', (d) => `<${d}>`)).toBe('<2026-10-09>')
  })
  it('writes parent blocks as plain words', () => {
    expect(crumbText(['Plan [[Conference]]', 'TODO see #urgent'])).toBe('Plan Conference › see urgent')
  })
})
