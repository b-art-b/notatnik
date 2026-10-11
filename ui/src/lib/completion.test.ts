import { describe, expect, it } from 'vitest'
import { propertyTrigger } from './completion'

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
