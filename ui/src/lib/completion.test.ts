import { describe, expect, it } from 'vitest'
import { propertyTrigger, queryPropValueTrigger } from './completion'

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
