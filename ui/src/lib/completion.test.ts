import { describe, expect, it } from 'vitest'
import {
  propertyTrigger,
  queryPropValueTrigger,
  queryValueOptions,
  queryValueTrigger,
  QUERY_KEYS,
} from './completion'

describe('propertyTrigger', () => {
  it('knows the four stages of a property line', () => {
    expect(propertyTrigger('client:: Ac')).toEqual({ kind: 'value', key: 'client', typed: 'Ac' })
    expect(propertyTrigger('  client:: ')).toEqual({ kind: 'value', key: 'client', typed: '' })
    expect(propertyTrigger('ca::')).toEqual({ kind: 'colons', typed: 'ca' })
    expect(propertyTrigger('::')).toEqual({ kind: 'colons', typed: '' })
    expect(propertyTrigger('::ca')).toEqual({ kind: 'picked', typed: 'ca' })
    expect(propertyTrigger('cli')).toEqual({ kind: 'bare', typed: 'cli' })
  })
  it('ignores ordinary text', () => {
    expect(propertyTrigger('some words here')).toBeNull()
    expect(propertyTrigger('std::fmt')).toBeNull()
    expect(propertyTrigger('Status::')).toBeNull()
  })
})

describe('queryPropValueTrigger', () => {
  it('finds the value being written after prop.<key>:', () => {
    expect(queryPropValueTrigger('prop.partner: ')).toEqual({ key: 'partner', typed: '', bare: '' })
    expect(queryPropValueTrigger('prop.partner: [[part. Ca')).toEqual({
      key: 'partner',
      typed: '[[part. Ca',
      bare: 'part. Ca',
    })
    expect(queryPropValueTrigger('-prop.client: Ac')).toEqual({ key: 'client', typed: 'Ac', bare: 'Ac' })
  })
  it('skips operators and earlier alternatives', () => {
    expect(queryPropValueTrigger('prop.rating: >= 3')?.typed).toBe('3')
    expect(queryPropValueTrigger('prop.status: contains act')?.typed).toBe('act')
    expect(queryPropValueTrigger('prop.partner: [[A]], [[B')?.typed).toBe('[[B')
  })
  it('is only for prop clauses', () => {
    expect(queryPropValueTrigger('tag: ur')).toBeNull()
    expect(queryPropValueTrigger('prop.partner')).toBeNull()
  })
})

describe('QUERY_KEYS', () => {
  it('offers children among the clause hints', () => {
    expect(QUERY_KEYS).toContain('children')
  })
})

describe('query value hints', () => {
  it('finds the clause and the alternative being written', () => {
    expect(queryValueTrigger('task: ')).toEqual({ key: 'task', typed: '' })
    expect(queryValueTrigger('task: todo, do')).toEqual({ key: 'task', typed: 'do' })
    expect(queryValueTrigger('-task: wa')).toEqual({ key: 'task', typed: 'wa' })
    expect(queryValueTrigger('sort: deadline a')).toEqual({ key: 'sort', typed: 'deadline a' })
    expect(queryValueTrigger('children: ')).toEqual({ key: 'children', typed: '' })
  })
  it('does not trigger for other clauses', () => {
    expect(queryValueTrigger('tag: ur')).toBeNull()
    expect(queryValueTrigger('prop.status: ac')).toBeNull()
    expect(queryValueTrigger('task')).toBeNull()
  })
  const labels = (key: string, typed = '', keys: string[] = [], pr: string[] = []) =>
    queryValueOptions(key, typed, keys, pr).options.map((o) => o.label)
  it('offers what the parser accepts', () => {
    expect(labels('task')).toContain('canceled')
    expect(labels('type')).toEqual(['block', 'page'])
    expect(labels('view')).toEqual(['list', 'table', 'kanban', 'timeline'])
    expect(labels('children')).toEqual(['yes', 'no'])
    expect(labels('deadline')).toEqual(expect.arrayContaining(['none', 'today', 'this-week', '< +7d']))
    expect(labels('priority')).toEqual(['A', 'B', 'C'])
    expect(labels('priority', '', [], ['P1', 'P2'])).toEqual(['P1', 'P2'])
  })
  it('sort: fields first, then the direction after a field', () => {
    expect(labels('sort', 'dea', ['rating'])).toEqual(expect.arrayContaining(['deadline', 'prop.rating']))
    const dir = queryValueOptions('sort', 'deadline ', [], [])
    expect(dir.options.map((o) => o.label)).toEqual(['asc', 'desc'])
    expect(dir.replace).toBe(0)
    expect(queryValueOptions('sort', 'deadline de', [], []).replace).toBe(2)
  })
  it('group and columns add the property keys of the graph', () => {
    expect(labels('group', '', ['client'])).toEqual(['page', 'task', 'scheduled', 'deadline', 'prop.client'])
    expect(labels('columns', '', ['client'])).toEqual(['text', 'page', 'linked-from', 'prop.client'])
    expect(labels('inherit', '', ['client', 'type'])).toEqual(['client', 'type'])
    expect(labels('properties', '', ['status'])).toEqual(['none', 'all', 'status'])
    expect(queryValueTrigger('properties: no')).toEqual({ key: 'properties', typed: 'no' })
  })
})
