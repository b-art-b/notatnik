import { describe, expect, it } from 'vitest'
import { displayValue, linkList, linkedPages, yamlValue } from './props'

describe('page property values', () => {
  it('quotes page links so YAML keeps them as links', () => {
    expect(yamlValue('[[part. Capgemini]]')).toBe('"[[part. Capgemini]]"')
    expect(yamlValue('[[A]], [[B]]')).toBe('["[[A]]", "[[B]]"]')
  })
  it('keeps plain words and lists, quotes what YAML would misread', () => {
    expect(yamlValue('active')).toBe('active')
    expect(yamlValue('[red, blue]')).toBe('[red, blue]')
    expect(yamlValue('a: b')).toBe('"a: b"')
    expect(yamlValue('#x')).toBe('"#x"')
  })
  it('shows the value without the quoting', () => {
    expect(displayValue('"[[part. Capgemini]]"')).toBe('[[part. Capgemini]]')
    expect(displayValue('["[[A]]", "[[B]]"]')).toBe('[[A]], [[B]]')
    expect(displayValue('"a: b"')).toBe('a: b')
    expect(displayValue('[red, blue]')).toBe('[red, blue]')
    expect(displayValue('plain')).toBe('plain')
  })
  it('round-trips what it shows', () => {
    for (const v of ['[[A]]', '[[A]], [[B]]', 'a: b', 'plain']) {
      expect(displayValue(yamlValue(v))).toBe(v)
    }
  })
  it('finds page names', () => {
    expect(linkedPages('[[A]], [[b/c|label]]')).toEqual(['A', 'b/c'])
    expect(linkList('[[A]], x')).toBeNull()
  })
})
