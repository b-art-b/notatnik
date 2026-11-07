import { describe, expect, it } from 'vitest'
import type { BlockView } from './api'
import { listMark, listStyleOf } from './listStyle'

const mark = (s: 'numbered' | 'letters' | 'roman', n: number) => listMark(s, n - 1)

describe('listMark', () => {
  it('has no mark for bullets', () => expect(listMark('bullets', 0)).toBeNull())
  it('numbers', () => {
    expect(mark('numbered', 1)).toBe('1.')
    expect(mark('numbered', 10)).toBe('10.')
    expect(mark('numbered', 1234)).toBe('1234.')
  })
  it('letters continue after z', () => {
    expect([1, 2, 26, 27, 28, 52, 53, 702, 703].map((n) => mark('letters', n))).toEqual([
      'a.',
      'b.',
      'z.',
      'aa.',
      'ab.',
      'az.',
      'ba.',
      'zz.',
      'aaa.',
    ])
  })
  it('roman numerals', () => {
    expect([1, 4, 9, 14, 40, 90, 400, 3999].map((n) => mark('roman', n))).toEqual([
      'i.',
      'iv.',
      'ix.',
      'xiv.',
      'xl.',
      'xc.',
      'cd.',
      'mmmcmxcix.',
    ])
  })
  it('shows a number roman numerals do not reach as it is', () => expect(mark('roman', 4000)).toBe('4000.'))
})

describe('listStyleOf', () => {
  const block = (raw?: string) =>
    ({ props: raw === undefined ? [] : [{ key: 'list', raw }] }) as unknown as BlockView
  it('reads the property', () => {
    expect(listStyleOf(block('numbered'))).toBe('numbered')
    expect(listStyleOf(block(' Letters '))).toBe('letters')
    expect(listStyleOf(block('roman'))).toBe('roman')
  })
  it('is bullets without it or with another value', () => {
    expect(listStyleOf(block())).toBe('bullets')
    expect(listStyleOf(block('stars'))).toBe('bullets')
  })
})
